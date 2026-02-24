// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

use std::path::PathBuf;

use gtk::glib;
use serde::{Deserialize, Serialize};

use crate::protocol::{AuthMethod, ConnectionConfig, ProtocolType};

/// Protocol type that can be serialized to/from JSON.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SerializableProtocol {
    Ftp,
    Ftps,
    Sftp,
}

impl From<SerializableProtocol> for ProtocolType {
    fn from(p: SerializableProtocol) -> Self {
        match p {
            SerializableProtocol::Ftp => ProtocolType::Ftp,
            SerializableProtocol::Ftps => ProtocolType::Ftps,
            SerializableProtocol::Sftp => ProtocolType::Sftp,
        }
    }
}

impl From<ProtocolType> for SerializableProtocol {
    fn from(p: ProtocolType) -> Self {
        match p {
            ProtocolType::Ftp => SerializableProtocol::Ftp,
            ProtocolType::Ftps => SerializableProtocol::Ftps,
            ProtocolType::Sftp => SerializableProtocol::Sftp,
        }
    }
}

/// Authentication method type for serialization. Passwords are never stored.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthMethodType {
    Password,
    KeyFile { path: String },
    Agent,
}

/// A saved server connection profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteProfile {
    pub id: String,
    pub name: String,
    pub protocol: SerializableProtocol,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethodType,
    #[serde(default)]
    pub local_dir: Option<String>,
    #[serde(default)]
    pub remote_dir: Option<String>,
    #[serde(default)]
    pub sync_browsing: bool,
}

impl SiteProfile {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: String::from("New Site"),
            protocol: SerializableProtocol::Sftp,
            host: String::new(),
            port: 22,
            username: String::new(),
            auth_method: AuthMethodType::Password,
            local_dir: None,
            remote_dir: None,
            sync_browsing: false,
        }
    }

    /// Convert to a ConnectionConfig for the protocol layer.
    /// The password/passphrase must be supplied separately (never stored on disk).
    pub fn to_connection_config(&self, password: Option<String>) -> ConnectionConfig {
        let auth = match &self.auth_method {
            AuthMethodType::Password => AuthMethod::Password(password.unwrap_or_default()),
            AuthMethodType::KeyFile { path } => AuthMethod::KeyFile {
                path: path.clone(),
                passphrase: password,
            },
            AuthMethodType::Agent => AuthMethod::Agent,
        };
        ConnectionConfig {
            protocol: self.protocol.into(),
            host: self.host.clone(),
            port: self.port,
            username: self.username.clone(),
            auth,
        }
    }
}

/// Persistent storage for all saved site profiles.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SiteStore {
    pub sites: Vec<SiteProfile>,
}

impl SiteStore {
    /// Path to the sites.json file in the XDG data directory.
    fn storage_path() -> PathBuf {
        let mut path = glib::user_data_dir();
        path.push("cargo");
        path.push("sites.json");
        path
    }

    /// Load profiles from disk. Returns an empty store if the file doesn't exist.
    pub fn load() -> Self {
        let path = Self::storage_path();
        if !path.exists() {
            return Self::default();
        }
        match std::fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(e) => {
                log::warn!("Failed to load sites.json: {}", e);
                Self::default()
            }
        }
    }

    /// Save profiles to disk.
    pub fn save(&self) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let path = Self::storage_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    pub fn add(&mut self, profile: SiteProfile) {
        self.sites.push(profile);
    }

    pub fn remove(&mut self, id: &str) {
        self.sites.retain(|s| s.id != id);
    }

    pub fn find(&self, id: &str) -> Option<&SiteProfile> {
        self.sites.iter().find(|s| s.id == id)
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut SiteProfile> {
        self.sites.iter_mut().find(|s| s.id == id)
    }
}
