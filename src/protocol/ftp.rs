// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;

use super::{
    AuthMethod, ConnectionConfig, ProgressSender, Protocol, ProtocolError, ProtocolType,
    RemoteEntry, Result, TransferProgress,
};

/// Wrapper to handle both plain FTP and FTPS (TLS) connections.
/// Both variants share the same suppaftp API but differ in the
/// underlying stream type.
enum FtpStream {
    Plain(suppaftp::tokio::AsyncFtpStream),
    Secure(suppaftp::tokio::AsyncRustlsFtpStream),
}

/// Dispatch an async method call on both FTP stream variants.
/// Works for methods that return the same concrete type.
macro_rules! ftp_call {
    ($stream:expr, $method:ident ( $($arg:expr),* $(,)? )) => {
        match $stream {
            FtpStream::Plain(s) => s.$method($($arg),*).await,
            FtpStream::Secure(s) => s.$method($($arg),*).await,
        }
    };
}

/// Download implementation macro. Used in both Plain and Secure match arms
/// because `retr_as_stream` returns a type-parameterized `DataStream<T>`.
macro_rules! ftp_download_impl {
    ($ftp:expr, $remote_path:expr, $local_path:expr, $progress:expr) => {{
        let total_bytes = $ftp.size($remote_path).await.ok();

        let mut reader = $ftp
            .retr_as_stream($remote_path)
            .await
            .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;

        let mut file = tokio::fs::File::create($local_path)
            .await
            .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;

        let mut bytes_transferred: u64 = 0;
        let mut buf = vec![0u8; 32768];

        loop {
            let n = reader
                .read(&mut buf)
                .await
                .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;
            if n == 0 {
                break;
            }

            tokio::io::AsyncWriteExt::write_all(&mut file, &buf[..n])
                .await
                .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;

            bytes_transferred += n as u64;

            if let Some(ref sender) = $progress {
                let _ = sender
                    .send(TransferProgress {
                        bytes_transferred,
                        total_bytes: total_bytes.map(|s| s as u64),
                    })
                    .await;
            }
        }

        $ftp.finalize_retr_stream(reader)
            .await
            .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;

        Ok(())
    }};
}

/// Upload implementation macro. Used in both Plain and Secure match arms
/// because `put_with_stream` returns a type-parameterized `DataStream<T>`.
macro_rules! ftp_upload_impl {
    ($ftp:expr, $local_path:expr, $remote_path:expr, $progress:expr) => {{
        let metadata = tokio::fs::metadata($local_path)
            .await
            .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;
        let total_bytes = metadata.len();

        let mut file = tokio::fs::File::open($local_path)
            .await
            .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;

        let mut writer = $ftp
            .put_with_stream($remote_path)
            .await
            .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;

        let mut bytes_transferred: u64 = 0;
        let mut buf = vec![0u8; 32768];

        loop {
            let n = file
                .read(&mut buf)
                .await
                .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;
            if n == 0 {
                break;
            }

            tokio::io::AsyncWriteExt::write_all(&mut writer, &buf[..n])
                .await
                .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;

            bytes_transferred += n as u64;

            if let Some(ref sender) = $progress {
                let _ = sender
                    .send(TransferProgress {
                        bytes_transferred,
                        total_bytes: Some(total_bytes),
                    })
                    .await;
            }
        }

        $ftp.finalize_put_stream(writer)
            .await
            .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;

        Ok(())
    }};
}

/// FTP/FTPS protocol implementation using suppaftp.
pub struct FtpProtocol {
    stream: Option<Arc<Mutex<FtpStream>>>,
    protocol_type: ProtocolType,
}

impl FtpProtocol {
    pub fn new() -> Self {
        Self {
            stream: None,
            protocol_type: ProtocolType::Ftp,
        }
    }

    fn stream(&self) -> Result<Arc<Mutex<FtpStream>>> {
        self.stream
            .clone()
            .ok_or(ProtocolError::NotConnected)
    }
}

#[async_trait]
impl Protocol for FtpProtocol {
    async fn connect(&mut self, config: &ConnectionConfig) -> Result<()> {
        let address = config.address();

        let mut stream = if config.protocol == ProtocolType::Ftps {
            // For FTPS: create a rustls-typed stream and upgrade to TLS
            let ftp = suppaftp::tokio::AsyncRustlsFtpStream::connect(&address)
                .await
                .map_err(|e| ProtocolError::ConnectionFailed(e.to_string()))?;

            let tls_config = suppaftp::tokio_rustls::rustls::ClientConfig::builder()
                .with_root_certificates(
                    suppaftp::tokio_rustls::rustls::RootCertStore::empty(),
                )
                .with_no_client_auth();
            let tls_connector =
                suppaftp::tokio_rustls::TlsConnector::from(Arc::new(tls_config));
            let connector =
                suppaftp::tokio::AsyncRustlsConnector::from(tls_connector);

            let secure = ftp
                .into_secure(connector, &config.host)
                .await
                .map_err(|e| ProtocolError::ConnectionFailed(e.to_string()))?;

            FtpStream::Secure(secure)
        } else {
            // Plain FTP
            let ftp = suppaftp::tokio::AsyncFtpStream::connect(&address)
                .await
                .map_err(|e| ProtocolError::ConnectionFailed(e.to_string()))?;
            FtpStream::Plain(ftp)
        };

        // Authenticate
        let password = match &config.auth {
            AuthMethod::Password(pw) => pw.clone(),
            _ => String::new(),
        };

        ftp_call!(&mut stream, login(&config.username, &password))
            .map_err(|e| ProtocolError::AuthenticationFailed(e.to_string()))?;

        // Set binary transfer mode
        ftp_call!(&mut stream, transfer_type(suppaftp::types::FileType::Binary))
            .map_err(|e| ProtocolError::ConnectionFailed(e.to_string()))?;

        self.protocol_type = config.protocol;
        self.stream = Some(Arc::new(Mutex::new(stream)));
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(stream) = self.stream.take() {
            let mut ftp = stream.lock().await;
            let _ = ftp_call!(&mut *ftp, quit());
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<RemoteEntry>> {
        let stream = self.stream()?;
        let mut ftp = stream.lock().await;

        let path_arg = if path.is_empty() { None } else { Some(path) };

        let listing = ftp_call!(&mut *ftp, list(path_arg))
            .map_err(|e| ProtocolError::ListFailed(e.to_string()))?;

        let entries = listing
            .iter()
            .filter_map(|line| parse_ftp_list_entry(line))
            .collect();

        Ok(entries)
    }

    async fn get_current_dir(&self) -> Result<String> {
        let stream = self.stream()?;
        let mut ftp = stream.lock().await;

        ftp_call!(&mut *ftp, pwd())
            .map_err(|e| ProtocolError::FileOperation(e.to_string()))
    }

    async fn change_dir(&mut self, path: &str) -> Result<()> {
        let stream = self.stream()?;
        let mut ftp = stream.lock().await;

        ftp_call!(&mut *ftp, cwd(path))
            .map_err(|e| ProtocolError::FileOperation(e.to_string()))
    }

    async fn download(
        &self,
        remote_path: &str,
        local_path: &Path,
        progress: Option<ProgressSender>,
    ) -> Result<()> {
        let stream = self.stream()?;
        let mut ftp = stream.lock().await;

        match &mut *ftp {
            FtpStream::Plain(s) => ftp_download_impl!(s, remote_path, local_path, progress),
            FtpStream::Secure(s) => ftp_download_impl!(s, remote_path, local_path, progress),
        }
    }

    async fn upload(
        &self,
        local_path: &Path,
        remote_path: &str,
        progress: Option<ProgressSender>,
    ) -> Result<()> {
        let stream = self.stream()?;
        let mut ftp = stream.lock().await;

        match &mut *ftp {
            FtpStream::Plain(s) => ftp_upload_impl!(s, local_path, remote_path, progress),
            FtpStream::Secure(s) => ftp_upload_impl!(s, local_path, remote_path, progress),
        }
    }

    async fn mkdir(&self, path: &str) -> Result<()> {
        let stream = self.stream()?;
        let mut ftp = stream.lock().await;

        ftp_call!(&mut *ftp, mkdir(path))
            .map_err(|e| ProtocolError::FileOperation(e.to_string()))?;
        Ok(())
    }

    async fn rename(&self, from: &str, to: &str) -> Result<()> {
        let stream = self.stream()?;
        let mut ftp = stream.lock().await;

        ftp_call!(&mut *ftp, rename(from, to))
            .map_err(|e| ProtocolError::FileOperation(e.to_string()))
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let stream = self.stream()?;
        let mut ftp = stream.lock().await;

        ftp_call!(&mut *ftp, rm(path))
            .map_err(|e| ProtocolError::FileOperation(e.to_string()))
    }

    async fn delete_dir(&self, path: &str) -> Result<()> {
        let stream = self.stream()?;
        let mut ftp = stream.lock().await;

        ftp_call!(&mut *ftp, rmdir(path))
            .map_err(|e| ProtocolError::FileOperation(e.to_string()))?;
        Ok(())
    }

    fn protocol_type(&self) -> ProtocolType {
        self.protocol_type
    }
}

/// Parse a single line from FTP LIST output into a RemoteEntry.
///
/// Handles standard Unix-style listings like:
/// `drwxr-xr-x  2 user group  4096 Feb 23 10:30 dirname`
/// `-rw-r--r--  1 user group 12345 Feb 23 10:30 filename.txt`
fn parse_ftp_list_entry(line: &str) -> Option<RemoteEntry> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    // Skip total line
    if line.starts_with("total ") {
        return None;
    }

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 9 {
        // Try simple name-only fallback
        return Some(RemoteEntry {
            name: line.to_string(),
            is_dir: false,
            size: 0,
            modified: None,
            permissions: None,
            owner: None,
        });
    }

    let permissions = parts[0];
    let is_dir = permissions.starts_with('d');
    let owner = Some(parts[2].to_string());
    let size = parts[4].parse::<u64>().unwrap_or(0);
    let name = parts[8..].join(" ");

    // Skip . and .. entries
    if name == "." || name == ".." {
        return None;
    }

    Some(RemoteEntry {
        name,
        is_dir,
        size,
        modified: None,
        permissions: Some(permissions.to_string()),
        owner,
    })
}
