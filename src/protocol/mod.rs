// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

pub mod error;
pub mod ftp;
pub mod sftp;

use std::path::Path;

use async_trait::async_trait;

pub use error::{ProtocolError, Result};

/// Supported protocol types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolType {
    Ftp,
    Ftps,
    Sftp,
}

impl std::fmt::Display for ProtocolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ftp => write!(f, "FTP"),
            Self::Ftps => write!(f, "FTPS"),
            Self::Sftp => write!(f, "SFTP"),
        }
    }
}

/// SSH authentication method.
#[derive(Debug, Clone)]
pub enum AuthMethod {
    Password(String),
    KeyFile {
        path: String,
        passphrase: Option<String>,
    },
    Agent,
}

/// Connection configuration for a remote server.
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub protocol: ProtocolType,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthMethod,
}

impl ConnectionConfig {
    pub fn default_port(protocol: ProtocolType) -> u16 {
        match protocol {
            ProtocolType::Ftp | ProtocolType::Ftps => 21,
            ProtocolType::Sftp => 22,
        }
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// A single entry in a remote directory listing.
#[derive(Debug, Clone)]
pub struct RemoteEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<i64>,
    pub permissions: Option<String>,
    pub owner: Option<String>,
}

/// Progress information for a file transfer.
#[derive(Debug, Clone)]
pub struct TransferProgress {
    pub bytes_transferred: u64,
    pub total_bytes: Option<u64>,
}

/// Sender type for progress updates during transfers.
pub type ProgressSender = async_channel::Sender<TransferProgress>;

/// The core protocol trait. All file transfer protocols implement this.
///
/// To add a new protocol, implement this trait for your protocol struct.
/// The trait is object-safe and can be used as `Box<dyn Protocol>`.
#[async_trait]
pub trait Protocol: Send + Sync {
    /// Connect to the remote server.
    async fn connect(&mut self, config: &ConnectionConfig) -> Result<()>;

    /// Disconnect from the remote server.
    async fn disconnect(&mut self) -> Result<()>;

    /// Check if currently connected.
    fn is_connected(&self) -> bool;

    /// List directory contents at the given path.
    async fn list_dir(&self, path: &str) -> Result<Vec<RemoteEntry>>;

    /// Get the current working directory.
    async fn get_current_dir(&self) -> Result<String>;

    /// Change the current working directory.
    async fn change_dir(&mut self, path: &str) -> Result<()>;

    /// Download a file from the remote server to a local path.
    async fn download(
        &self,
        remote_path: &str,
        local_path: &Path,
        progress: Option<ProgressSender>,
    ) -> Result<()>;

    /// Upload a file from a local path to the remote server.
    async fn upload(
        &self,
        local_path: &Path,
        remote_path: &str,
        progress: Option<ProgressSender>,
    ) -> Result<()>;

    /// Create a directory on the remote server.
    async fn mkdir(&self, path: &str) -> Result<()>;

    /// Rename a file or directory on the remote server.
    async fn rename(&self, from: &str, to: &str) -> Result<()>;

    /// Delete a file on the remote server.
    async fn delete(&self, path: &str) -> Result<()>;

    /// Delete a directory on the remote server.
    async fn delete_dir(&self, path: &str) -> Result<()>;

    /// Return the protocol type.
    fn protocol_type(&self) -> ProtocolType;
}

/// Create a protocol instance for the given type.
pub fn create_protocol(protocol_type: ProtocolType) -> Box<dyn Protocol> {
    match protocol_type {
        ProtocolType::Ftp | ProtocolType::Ftps => Box::new(ftp::FtpProtocol::new()),
        ProtocolType::Sftp => Box::new(sftp::SftpProtocol::new()),
    }
}
