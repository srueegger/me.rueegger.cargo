// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

use gtk::{glib, prelude::*, subclass::prelude::*};
use std::cell::{Cell, RefCell};

/// Transfer direction constants.
pub const DIRECTION_UPLOAD: u32 = 0;
pub const DIRECTION_DOWNLOAD: u32 = 1;

/// Transfer status constants.
pub const STATUS_QUEUED: u32 = 0;
pub const STATUS_ACTIVE: u32 = 1;
pub const STATUS_COMPLETED: u32 = 2;
pub const STATUS_FAILED: u32 = 3;

mod imp {
    use super::*;
    use glib::Properties;

    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::TransferItem)]
    pub struct TransferItem {
        #[property(get, set)]
        filename: RefCell<String>,

        #[property(get, set)]
        local_path: RefCell<String>,

        #[property(get, set)]
        remote_path: RefCell<String>,

        #[property(get, set)]
        direction: Cell<u32>,

        #[property(get, set)]
        status: Cell<u32>,

        #[property(get, set)]
        progress: Cell<f64>,

        #[property(get, set)]
        bytes_transferred: Cell<u64>,

        #[property(get, set)]
        total_bytes: Cell<u64>,

        #[property(get, set)]
        error_message: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TransferItem {
        const NAME: &'static str = "CargoTransferItem";
        type Type = super::TransferItem;
        type ParentType = glib::Object;
    }

    #[glib::derived_properties]
    impl ObjectImpl for TransferItem {}
}

glib::wrapper! {
    pub struct TransferItem(ObjectSubclass<imp::TransferItem>);
}

impl TransferItem {
    pub fn new_upload(filename: &str, local_path: &str, remote_path: &str) -> Self {
        glib::Object::builder()
            .property("filename", filename)
            .property("local-path", local_path)
            .property("remote-path", remote_path)
            .property("direction", DIRECTION_UPLOAD)
            .property("status", STATUS_QUEUED)
            .property("progress", 0.0f64)
            .property("bytes-transferred", 0u64)
            .property("total-bytes", 0u64)
            .build()
    }

    pub fn new_download(filename: &str, local_path: &str, remote_path: &str) -> Self {
        glib::Object::builder()
            .property("filename", filename)
            .property("local-path", local_path)
            .property("remote-path", remote_path)
            .property("direction", DIRECTION_DOWNLOAD)
            .property("status", STATUS_QUEUED)
            .property("progress", 0.0f64)
            .property("bytes-transferred", 0u64)
            .property("total-bytes", 0u64)
            .build()
    }

    pub fn direction_icon(&self) -> &'static str {
        if self.direction() == DIRECTION_UPLOAD {
            "go-up-symbolic"
        } else {
            "go-down-symbolic"
        }
    }

    pub fn status_label(&self) -> String {
        match self.status() {
            STATUS_QUEUED => "Queued".to_string(),
            STATUS_ACTIVE => {
                let pct = (self.progress() * 100.0) as u32;
                let transferred = format_size(self.bytes_transferred());
                let total = self.total_bytes();
                if total > 0 {
                    format!("{}% ({} / {})", pct, transferred, format_size(total))
                } else {
                    format!("{}% ({})", pct, transferred)
                }
            }
            STATUS_COMPLETED => "Completed".to_string(),
            STATUS_FAILED => {
                let msg = self.error_message();
                if msg.is_empty() {
                    "Failed".to_string()
                } else {
                    format!("Failed: {}", msg)
                }
            }
            _ => String::new(),
        }
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
