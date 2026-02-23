// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Not connected")]
    NotConnected,

    #[error("Directory listing failed: {0}")]
    ListFailed(String),

    #[error("Transfer failed: {0}")]
    TransferFailed(String),

    #[error("File operation failed: {0}")]
    FileOperation(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Path not found: {0}")]
    NotFound(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, ProtocolError>;
