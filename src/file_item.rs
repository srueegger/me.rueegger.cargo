// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

use gtk::{glib, prelude::*, subclass::prelude::*};
use std::cell::{Cell, RefCell};

mod imp {
    use super::*;
    use glib::Properties;

    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::FileItem)]
    pub struct FileItem {
        #[property(get, set)]
        name: RefCell<String>,

        #[property(get, set)]
        is_dir: Cell<bool>,

        #[property(get, set)]
        size: Cell<u64>,

        #[property(get, set)]
        modified: Cell<i64>,

        #[property(get, set)]
        permissions: RefCell<String>,

        #[property(get, set)]
        display_size: RefCell<String>,

        #[property(get, set)]
        display_modified: RefCell<String>,

        #[property(get, set)]
        icon_name: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FileItem {
        const NAME: &'static str = "CargoFileItem";
        type Type = super::FileItem;
        type ParentType = glib::Object;
    }

    #[glib::derived_properties]
    impl ObjectImpl for FileItem {}
}

glib::wrapper! {
    pub struct FileItem(ObjectSubclass<imp::FileItem>);
}

impl FileItem {
    pub fn new(
        name: &str,
        is_dir: bool,
        size: u64,
        modified: i64,
        permissions: &str,
    ) -> Self {
        let icon_name = if is_dir {
            "folder-symbolic"
        } else {
            icon_for_filename(name)
        };

        let display_size = if is_dir {
            String::new()
        } else {
            format_size(size)
        };

        let display_modified = format_timestamp(modified);

        glib::Object::builder()
            .property("name", name)
            .property("is-dir", is_dir)
            .property("size", size)
            .property("modified", modified)
            .property("permissions", permissions)
            .property("display-size", &display_size)
            .property("display-modified", &display_modified)
            .property("icon-name", icon_name)
            .build()
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

fn format_timestamp(ts: i64) -> String {
    if ts == 0 {
        return String::new();
    }
    let dt = glib::DateTime::from_unix_local(ts);
    match dt {
        Ok(dt) => dt
            .format("%Y-%m-%d %H:%M")
            .map(|s| s.to_string())
            .unwrap_or_default(),
        Err(_) => String::new(),
    }
}

fn icon_for_filename(name: &str) -> &'static str {
    let lower = name.to_lowercase();
    if lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".png")
        || lower.ends_with(".gif")
        || lower.ends_with(".svg")
        || lower.ends_with(".webp")
    {
        "image-x-generic-symbolic"
    } else if lower.ends_with(".mp3")
        || lower.ends_with(".flac")
        || lower.ends_with(".ogg")
        || lower.ends_with(".wav")
    {
        "audio-x-generic-symbolic"
    } else if lower.ends_with(".mp4")
        || lower.ends_with(".mkv")
        || lower.ends_with(".avi")
        || lower.ends_with(".webm")
    {
        "video-x-generic-symbolic"
    } else {
        "text-x-generic-symbolic"
    }
}
