// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::{gio, glib, prelude::*};

use crate::connection::ConnectionHandle;
use crate::file_panel::FilePanel;
use crate::protocol::TransferProgress;
use crate::transfer_item::*;

/// Manages a queue of file transfers processed sequentially.
#[derive(Debug)]
pub struct TransferQueue {
    store: gio::ListStore,
    is_processing: Cell<bool>,
}

impl TransferQueue {
    pub fn new() -> Self {
        Self {
            store: gio::ListStore::new::<TransferItem>(),
            is_processing: Cell::new(false),
        }
    }

    pub fn store(&self) -> &gio::ListStore {
        &self.store
    }

    pub fn enqueue(&self, item: TransferItem) {
        self.store.append(&item);
    }

    pub fn has_items(&self) -> bool {
        self.store.n_items() > 0
    }

    pub fn clear_completed(&self) {
        let mut i = 0;
        while i < self.store.n_items() {
            if let Some(obj) = self.store.item(i) {
                let item: TransferItem = obj.downcast().unwrap();
                let status = item.status();
                if status == STATUS_COMPLETED || status == STATUS_FAILED {
                    self.store.remove(i);
                    continue;
                }
            }
            i += 1;
        }
    }

    /// Start processing the queue if not already running.
    pub fn start_processing(
        queue: &Rc<TransferQueue>,
        connection: &Rc<ConnectionHandle>,
        left_panel: &FilePanel,
        right_panel: &FilePanel,
    ) {
        if queue.is_processing.get() {
            return;
        }
        Self::process_next(queue, connection, left_panel, right_panel);
    }

    fn process_next(
        queue: &Rc<TransferQueue>,
        connection: &Rc<ConnectionHandle>,
        left_panel: &FilePanel,
        right_panel: &FilePanel,
    ) {
        // Find next queued item
        let mut next_item: Option<TransferItem> = None;
        for i in 0..queue.store.n_items() {
            if let Some(obj) = queue.store.item(i) {
                let item: TransferItem = obj.downcast().unwrap();
                if item.status() == STATUS_QUEUED {
                    next_item = Some(item);
                    break;
                }
            }
        }

        let Some(item) = next_item else {
            queue.is_processing.set(false);
            return;
        };

        queue.is_processing.set(true);
        item.set_status(STATUS_ACTIVE);

        let (progress_tx, progress_rx) = async_channel::unbounded::<TransferProgress>();

        let item_ref = item.clone();
        // Progress monitor: update UI properties from progress channel
        glib::spawn_future_local(async move {
            while let Ok(p) = progress_rx.recv().await {
                item_ref.set_bytes_transferred(p.bytes_transferred);
                if let Some(total) = p.total_bytes {
                    item_ref.set_total_bytes(total);
                    if total > 0 {
                        item_ref
                            .set_progress(p.bytes_transferred as f64 / total as f64);
                    }
                }
            }
        });

        // Execute the transfer
        let conn = connection.clone();
        let queue_clone = queue.clone();
        let left_weak = left_panel.downgrade();
        let right_weak = right_panel.downgrade();
        let direction = item.direction();
        let local_path = PathBuf::from(item.local_path());
        let remote_path = item.remote_path();

        glib::spawn_future_local(async move {
            let result = if direction == DIRECTION_UPLOAD {
                conn.upload(&local_path, &remote_path, Some(progress_tx))
                    .await
            } else {
                conn.download(&remote_path, &local_path, Some(progress_tx))
                    .await
            };

            match result {
                Ok(()) => {
                    item.set_status(STATUS_COMPLETED);
                    item.set_progress(1.0);
                }
                Err(e) => {
                    item.set_status(STATUS_FAILED);
                    item.set_error_message(e.to_string());
                }
            }

            // Reload destination panel
            if direction == DIRECTION_UPLOAD {
                if let Some(panel) = right_weak.upgrade() {
                    panel.reload();
                }
            } else if let Some(panel) = left_weak.upgrade() {
                panel.reload();
            }

            // Process next item in queue
            if let (Some(left), Some(right)) =
                (left_weak.upgrade(), right_weak.upgrade())
            {
                TransferQueue::process_next(&queue_clone, &conn, &left, &right);
            } else {
                queue_clone.is_processing.set(false);
            }
        });
    }
}
