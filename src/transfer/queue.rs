// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use gettextrs::gettext;
use gtk::{gio, glib, prelude::*};

use crate::dialogs::{self, ConflictAction};
use crate::connection::ConnectionHandle;
use crate::file_panel::FilePanel;
use crate::protocol::TransferProgress;
use crate::transfer::item::*;

/// Manages a queue of file transfers processed sequentially.
#[derive(Debug)]
pub struct TransferQueue {
    store: gio::ListStore,
    is_processing: Cell<bool>,
    conflict_policy: RefCell<Option<ConflictAction>>,
}

impl TransferQueue {
    pub fn new() -> Self {
        Self {
            store: gio::ListStore::new::<TransferItem>(),
            is_processing: Cell::new(false),
            conflict_policy: RefCell::new(None),
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
                if status == STATUS_COMPLETED
                    || status == STATUS_FAILED
                    || status == STATUS_SKIPPED
                {
                    self.store.remove(i);
                    continue;
                }
            }
            i += 1;
        }
    }

    fn reset_policy(&self) {
        *self.conflict_policy.borrow_mut() = None;
    }

    /// Start processing the queue if not already running.
    pub fn start_processing(
        queue: &Rc<TransferQueue>,
        connection: &Rc<ConnectionHandle>,
        left_panel: &FilePanel,
        right_panel: &FilePanel,
        window: &crate::window::CargoWindow,
    ) {
        if queue.is_processing.get() {
            return;
        }
        Self::process_next(queue, connection, left_panel, right_panel, window);
    }

    fn process_next(
        queue: &Rc<TransferQueue>,
        connection: &Rc<ConnectionHandle>,
        left_panel: &FilePanel,
        right_panel: &FilePanel,
        window: &crate::window::CargoWindow,
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
            queue.reset_policy();
            Self::send_completion_notification(queue);
            queue.clear_completed();
            return;
        };

        queue.is_processing.set(true);
        item.set_status(STATUS_ACTIVE);

        let (progress_tx, progress_rx) = async_channel::bounded::<TransferProgress>(64);

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
        let window_weak = window.downgrade();
        let direction = item.direction();
        let local_path = PathBuf::from(item.local_path());
        let remote_path = item.remote_path();
        let filename = item.filename();

        glib::spawn_future_local(async move {
            let mut final_local_path = local_path.clone();
            let mut final_remote_path = remote_path.clone();

            // --- Conflict check ---
            let conflict_info = if direction == DIRECTION_UPLOAD {
                let remote_dir = remote_path
                    .rsplit_once('/')
                    .map(|(d, _)| if d.is_empty() { "/" } else { d })
                    .unwrap_or("/");
                match conn.list_dir(remote_dir).await {
                    Ok(entries) => dialogs::remote_file_exists(&entries, &filename)
                        .map(|e| (e.size, e.modified)),
                    Err(_) => None,
                }
            } else {
                if final_local_path.exists() && final_local_path.is_file() {
                    dialogs::local_file_info(&final_local_path)
                } else {
                    None
                }
            };

            if let Some((dest_size, dest_modified)) = conflict_info {
                let remembered = *queue_clone.conflict_policy.borrow();

                let action = if let Some(policy) = remembered {
                    policy
                } else {
                    let Some(window) = window_weak.upgrade() else {
                        queue_clone.is_processing.set(false);
                        return;
                    };
                    match dialogs::show_overwrite_dialog(&window, &filename).await {
                        Some(resolution) => {
                            if resolution.apply_to_queue {
                                *queue_clone.conflict_policy.borrow_mut() =
                                    Some(resolution.action);
                            }
                            resolution.action
                        }
                        None => ConflictAction::Skip,
                    }
                };

                // Get source metadata
                let (source_size, source_modified) = if direction == DIRECTION_UPLOAD {
                    dialogs::local_file_info(&local_path).unwrap_or((0, None))
                } else {
                    let remote_dir = remote_path
                        .rsplit_once('/')
                        .map(|(d, _)| if d.is_empty() { "/" } else { d })
                        .unwrap_or("/");
                    match conn.list_dir(remote_dir).await {
                        Ok(entries) => {
                            dialogs::remote_file_exists(&entries, &filename)
                                .map(|e| (e.size, e.modified))
                                .unwrap_or((0, None))
                        }
                        Err(_) => (0, None),
                    }
                };

                let dest_path_str = if direction == DIRECTION_UPLOAD {
                    remote_path.clone()
                } else {
                    local_path.display().to_string()
                };

                let (should_proceed, new_dest) = dialogs::should_transfer(
                    action,
                    source_size,
                    source_modified,
                    dest_size,
                    dest_modified,
                    &dest_path_str,
                );

                if !should_proceed {
                    item.set_status(STATUS_SKIPPED);

                    if let (Some(left), Some(right), Some(win)) = (
                        left_weak.upgrade(),
                        right_weak.upgrade(),
                        window_weak.upgrade(),
                    ) {
                        TransferQueue::process_next(
                            &queue_clone, &conn, &left, &right, &win,
                        );
                    } else {
                        queue_clone.is_processing.set(false);
                    }
                    return;
                }

                if direction == DIRECTION_UPLOAD {
                    final_remote_path = new_dest;
                } else {
                    final_local_path = PathBuf::from(&new_dest);
                }
            }

            // --- Execute transfer ---
            let result = if direction == DIRECTION_UPLOAD {
                conn.upload(&final_local_path, &final_remote_path, Some(progress_tx))
                    .await
            } else {
                conn.download(&final_remote_path, &final_local_path, Some(progress_tx))
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
            if let (Some(left), Some(right), Some(win)) = (
                left_weak.upgrade(),
                right_weak.upgrade(),
                window_weak.upgrade(),
            ) {
                TransferQueue::process_next(&queue_clone, &conn, &left, &right, &win);
            } else {
                queue_clone.is_processing.set(false);
            }
        });
    }

    fn send_completion_notification(queue: &Rc<TransferQueue>) {
        let Some(app) = gio::Application::default() else {
            return;
        };

        let mut completed_uploads = 0u32;
        let mut completed_downloads = 0u32;
        let mut failed = 0u32;
        let mut last_filename = String::new();
        let mut last_direction = DIRECTION_UPLOAD;

        for i in 0..queue.store.n_items() {
            let Some(obj) = queue.store.item(i) else {
                continue;
            };
            let item: TransferItem = obj.downcast().unwrap();
            match item.status() {
                STATUS_COMPLETED => {
                    last_filename = item.filename();
                    last_direction = item.direction();
                    if item.direction() == DIRECTION_UPLOAD {
                        completed_uploads += 1;
                    } else {
                        completed_downloads += 1;
                    }
                }
                STATUS_FAILED => {
                    failed += 1;
                }
                _ => {}
            }
        }

        let total_completed = completed_uploads + completed_downloads;
        if total_completed == 0 && failed == 0 {
            return;
        }

        let body = if failed > 0 && total_completed == 0 {
            gettext("Transfer failed")
        } else if total_completed == 1 && failed == 0 {
            if last_direction == DIRECTION_UPLOAD {
                gettext("'%s' successfully uploaded").replace("%s", &last_filename)
            } else {
                gettext("'%s' successfully downloaded").replace("%s", &last_filename)
            }
        } else if failed > 0 {
            gettext("Transfer completed with %d errors")
                .replace("%d", &failed.to_string())
        } else if completed_downloads == 0 {
            gettext("All files successfully uploaded")
        } else if completed_uploads == 0 {
            gettext("All files successfully downloaded")
        } else {
            gettext("All files successfully transferred")
        };

        let notification = gio::Notification::new(&gettext("Transfer Complete"));
        notification.set_body(Some(&body));
        app.send_notification(Some("transfer-complete"), &notification);
    }
}
