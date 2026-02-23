// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

use std::cell::RefCell;
use std::sync::{Arc, OnceLock};

use tokio::runtime::Runtime;
use tokio::sync::Mutex as TokioMutex;

use crate::protocol::{self, ConnectionConfig, Protocol, ProtocolError, RemoteEntry};

/// Global tokio runtime shared across all connections.
fn tokio_runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| Runtime::new().expect("Failed to create tokio runtime"))
}

/// Bridges the async protocol layer (tokio) with the GTK main loop.
///
/// All protocol operations are dispatched to the tokio runtime via `handle.spawn()`
/// and results are returned via `async_channel`, which is executor-agnostic and
/// works inside `glib::spawn_future_local()`.
///
/// This type is NOT Send/Sync (contains RefCell). Hold it as `Rc<ConnectionHandle>`.
pub struct ConnectionHandle {
    protocol: Arc<TokioMutex<Box<dyn Protocol>>>,
    rt_handle: tokio::runtime::Handle,
    host_label: String,
    current_dir: RefCell<String>,
}

impl ConnectionHandle {
    /// Connect to a remote server. Call from within `glib::spawn_future_local`.
    pub async fn connect(config: ConnectionConfig) -> Result<Self, ProtocolError> {
        let rt = tokio_runtime();
        let handle = rt.handle().clone();

        let host_label = format!("{}@{}:{}", config.username, config.host, config.port);

        // Spawn the connect operation on the tokio runtime
        let (tx, rx) =
            async_channel::bounded::<Result<Box<dyn Protocol>, ProtocolError>>(1);

        let config_clone = config.clone();
        handle.spawn(async move {
            let mut proto = protocol::create_protocol(config_clone.protocol);
            let result = proto.connect(&config_clone).await.map(|()| proto);
            let _ = tx.send(result).await;
        });

        let proto = rx
            .recv()
            .await
            .map_err(|_| ProtocolError::ConnectionFailed("Channel closed".into()))??;

        // Get initial working directory
        let proto_arc = Arc::new(TokioMutex::new(proto));
        let (dir_tx, dir_rx) =
            async_channel::bounded::<Result<String, ProtocolError>>(1);
        let proto_clone = proto_arc.clone();
        handle.spawn(async move {
            let p = proto_clone.lock().await;
            let _ = dir_tx.send(p.get_current_dir().await).await;
        });

        let initial_dir = dir_rx
            .recv()
            .await
            .map_err(|_| ProtocolError::ConnectionFailed("Channel closed".into()))??;

        Ok(Self {
            protocol: proto_arc,
            rt_handle: handle,
            host_label,
            current_dir: RefCell::new(initial_dir),
        })
    }

    /// List a remote directory. Call from within `glib::spawn_future_local`.
    pub async fn list_dir(&self, path: &str) -> Result<Vec<RemoteEntry>, ProtocolError> {
        let (tx, rx) = async_channel::bounded(1);
        let proto = self.protocol.clone();
        let path = path.to_string();

        self.rt_handle.spawn(async move {
            let p = proto.lock().await;
            let _ = tx.send(p.list_dir(&path).await).await;
        });

        rx.recv()
            .await
            .map_err(|_| ProtocolError::ConnectionFailed("Channel closed".into()))?
    }

    /// Disconnect from the remote server.
    pub async fn disconnect(&self) -> Result<(), ProtocolError> {
        let (tx, rx) = async_channel::bounded(1);
        let proto = self.protocol.clone();

        self.rt_handle.spawn(async move {
            let mut p = proto.lock().await;
            let _ = tx.send(p.disconnect().await).await;
        });

        rx.recv()
            .await
            .map_err(|_| ProtocolError::ConnectionFailed("Channel closed".into()))?
    }

    pub fn host_label(&self) -> &str {
        &self.host_label
    }

    pub fn current_dir(&self) -> String {
        self.current_dir.borrow().clone()
    }

    pub fn set_current_dir(&self, dir: String) {
        *self.current_dir.borrow_mut() = dir;
    }
}
