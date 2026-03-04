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
use russh::keys::*;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use super::{
    AuthMethod, ConnectionConfig, ProgressSender, Protocol, ProtocolError, ProtocolType,
    RemoteEntry, Result, TransferProgress,
};

struct SshHandler;

impl russh::client::Handler for SshHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &ssh_key::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        // TODO: Implement known_hosts verification
        Ok(true)
    }
}

/// SFTP protocol implementation using russh and russh-sftp.
pub struct SftpProtocol {
    session: Option<Arc<Mutex<russh::client::Handle<SshHandler>>>>,
    sftp: Option<Arc<Mutex<SftpSession>>>,
}

impl SftpProtocol {
    pub fn new() -> Self {
        Self {
            session: None,
            sftp: None,
        }
    }

    fn sftp(&self) -> Result<Arc<Mutex<SftpSession>>> {
        self.sftp
            .clone()
            .ok_or(ProtocolError::NotConnected)
    }
}

#[async_trait]
impl Protocol for SftpProtocol {
    async fn connect(&mut self, config: &ConnectionConfig) -> Result<()> {
        let ssh_config = Arc::new(russh::client::Config::default());
        let handler = SshHandler;

        let mut session = russh::client::connect(ssh_config, config.address(), handler)
            .await
            .map_err(|e| ProtocolError::ConnectionFailed(e.to_string()))?;

        // Authenticate based on method
        let authenticated = match &config.auth {
            AuthMethod::Password(password) => session
                .authenticate_password(&config.username, password)
                .await
                .map_err(|e| ProtocolError::AuthenticationFailed(e.to_string()))?,

            AuthMethod::KeyFile { path, passphrase } => {
                let key_pair = load_secret_key(path, passphrase.as_deref())
                    .map_err(|e| ProtocolError::AuthenticationFailed(e.to_string()))?;

                let best_hash = session
                    .best_supported_rsa_hash()
                    .await
                    .map_err(|e| ProtocolError::AuthenticationFailed(e.to_string()))?
                    .flatten();

                session
                    .authenticate_publickey(
                        &config.username,
                        PrivateKeyWithHashAlg::new(Arc::new(key_pair), best_hash),
                    )
                    .await
                    .map_err(|e| ProtocolError::AuthenticationFailed(e.to_string()))?
            }

            AuthMethod::Agent => {
                let mut agent =
                    russh::keys::agent::client::AgentClient::connect_env()
                        .await
                        .map_err(|e| {
                            ProtocolError::AuthenticationFailed(format!(
                                "SSH agent not available: {}",
                                e
                            ))
                        })?;

                let identities = agent.request_identities().await.map_err(|e| {
                    ProtocolError::AuthenticationFailed(format!(
                        "Failed to get agent identities: {}",
                        e
                    ))
                })?;

                let mut auth_success = false;
                for identity in identities {
                    let hash_alg = session
                        .best_supported_rsa_hash()
                        .await
                        .map_err(|e| ProtocolError::AuthenticationFailed(e.to_string()))?
                        .flatten();

                    match session
                        .authenticate_publickey_with(
                            &config.username,
                            identity,
                            hash_alg,
                            &mut agent,
                        )
                        .await
                    {
                        Ok(result) => {
                            if result.success() {
                                auth_success = true;
                                break;
                            }
                        }
                        Err(_) => {
                            continue;
                        }
                    }
                }

                if !auth_success {
                    return Err(ProtocolError::AuthenticationFailed(
                        "No agent key accepted".to_string(),
                    ));
                }

                russh::client::AuthResult::Success
            }
        };

        if !matches!(&config.auth, AuthMethod::Agent) && !authenticated.success() {
            return Err(ProtocolError::AuthenticationFailed(
                "Authentication rejected by server".to_string(),
            ));
        }

        // Open SFTP subsystem
        let channel = session
            .channel_open_session()
            .await
            .map_err(|e| ProtocolError::ConnectionFailed(e.to_string()))?;

        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|e| ProtocolError::ConnectionFailed(e.to_string()))?;

        let sftp = SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| ProtocolError::ConnectionFailed(e.to_string()))?;

        self.session = Some(Arc::new(Mutex::new(session)));
        self.sftp = Some(Arc::new(Mutex::new(sftp)));

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(sftp) = self.sftp.take() {
            let sftp = sftp.lock().await;
            let _ = sftp.close().await;
        }

        if let Some(session) = self.session.take() {
            let session = session.lock().await;
            let _ = session
                .disconnect(russh::Disconnect::ByApplication, "", "")
                .await;
        }

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.sftp.is_some()
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<RemoteEntry>> {
        let sftp = self.sftp()?;
        let sftp = sftp.lock().await;

        let entries = sftp
            .read_dir(path)
            .await
            .map_err(|e| ProtocolError::ListFailed(e.to_string()))?;

        let result = entries
            .into_iter()
            .filter(|e| {
                let name = e.file_name();
                name != "." && name != ".."
            })
            .map(|entry| {
                let metadata = entry.metadata();
                RemoteEntry {
                    name: entry.file_name(),
                    is_dir: metadata.is_dir(),
                    size: metadata.size.unwrap_or(0),
                    modified: metadata.mtime.map(|t| t as i64),
                    permissions: metadata.permissions.map(|p| format!("{:o}", p & 0o7777)),
                    owner: metadata.uid.map(|u| u.to_string()),
                }
            })
            .collect();

        Ok(result)
    }

    async fn get_current_dir(&self) -> Result<String> {
        let sftp = self.sftp()?;
        let sftp = sftp.lock().await;

        sftp.canonicalize(".")
            .await
            .map_err(|e| ProtocolError::FileOperation(e.to_string()))
    }

    async fn change_dir(&mut self, _path: &str) -> Result<()> {
        // SFTP doesn't have a native cwd concept like FTP.
        // We track it logically in the file panel.
        Ok(())
    }

    async fn download(
        &self,
        remote_path: &str,
        local_path: &Path,
        progress: Option<ProgressSender>,
    ) -> Result<()> {
        let sftp = self.sftp()?;
        let sftp = sftp.lock().await;

        // Get file size for progress
        let total_bytes = sftp
            .metadata(remote_path)
            .await
            .ok()
            .and_then(|m| m.size);

        let mut remote_file = sftp
            .open(remote_path)
            .await
            .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;

        let mut local_file = tokio::fs::File::create(local_path)
            .await
            .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;

        let mut bytes_transferred: u64 = 0;
        let mut last_reported: u64 = 0;
        let mut buf = vec![0u8; 32768];

        loop {
            let n = remote_file
                .read(&mut buf)
                .await
                .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;
            if n == 0 {
                break;
            }

            local_file
                .write_all(&buf[..n])
                .await
                .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;

            bytes_transferred += n as u64;

            if let Some(ref sender) = progress {
                if bytes_transferred - last_reported >= 262144 {
                    last_reported = bytes_transferred;
                    let _ = sender.try_send(TransferProgress {
                        bytes_transferred,
                        total_bytes,
                    });
                }
            }
        }

        // Send final progress
        if let Some(ref sender) = progress {
            let _ = sender.try_send(TransferProgress {
                bytes_transferred,
                total_bytes,
            });
        }

        remote_file
            .shutdown()
            .await
            .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;

        Ok(())
    }

    async fn upload(
        &self,
        local_path: &Path,
        remote_path: &str,
        progress: Option<ProgressSender>,
    ) -> Result<()> {
        let sftp = self.sftp()?;
        let sftp = sftp.lock().await;

        let metadata = tokio::fs::metadata(local_path)
            .await
            .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;
        let total_bytes = metadata.len();

        let mut local_file = tokio::fs::File::open(local_path)
            .await
            .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;

        let mut remote_file = sftp
            .open_with_flags(
                remote_path,
                OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
            )
            .await
            .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;

        let mut bytes_transferred: u64 = 0;
        let mut last_reported: u64 = 0;
        let mut buf = vec![0u8; 32768];

        loop {
            let n = local_file
                .read(&mut buf)
                .await
                .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;
            if n == 0 {
                break;
            }

            remote_file
                .write_all(&buf[..n])
                .await
                .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;

            bytes_transferred += n as u64;

            if let Some(ref sender) = progress {
                if bytes_transferred - last_reported >= 262144 {
                    last_reported = bytes_transferred;
                    let _ = sender.try_send(TransferProgress {
                        bytes_transferred,
                        total_bytes: Some(total_bytes),
                    });
                }
            }
        }

        // Send final progress
        if let Some(ref sender) = progress {
            let _ = sender.try_send(TransferProgress {
                bytes_transferred,
                total_bytes: Some(total_bytes),
            });
        }

        remote_file
            .flush()
            .await
            .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;
        remote_file
            .shutdown()
            .await
            .map_err(|e| ProtocolError::TransferFailed(e.to_string()))?;

        Ok(())
    }

    async fn mkdir(&self, path: &str) -> Result<()> {
        let sftp = self.sftp()?;
        let sftp = sftp.lock().await;

        sftp.create_dir(path)
            .await
            .map_err(|e| ProtocolError::FileOperation(e.to_string()))
    }

    async fn rename(&self, from: &str, to: &str) -> Result<()> {
        let sftp = self.sftp()?;
        let sftp = sftp.lock().await;

        sftp.rename(from, to)
            .await
            .map_err(|e| ProtocolError::FileOperation(e.to_string()))
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let sftp = self.sftp()?;
        let sftp = sftp.lock().await;

        sftp.remove_file(path)
            .await
            .map_err(|e| ProtocolError::FileOperation(e.to_string()))
    }

    async fn delete_dir(&self, path: &str) -> Result<()> {
        let sftp = self.sftp()?;
        let sftp = sftp.lock().await;

        sftp.remove_dir(path)
            .await
            .map_err(|e| ProtocolError::FileOperation(e.to_string()))
    }

    async fn chmod(&self, path: &str, mode: u32) -> Result<()> {
        let sftp = self.sftp()?;
        let sftp = sftp.lock().await;

        let mut attrs = sftp
            .metadata(path)
            .await
            .map_err(|e| ProtocolError::FileOperation(e.to_string()))?;
        attrs.permissions = Some(mode);
        sftp.set_metadata(path, attrs)
            .await
            .map_err(|e| ProtocolError::FileOperation(e.to_string()))
    }

    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::Sftp
    }
}
