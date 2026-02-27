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
    let ext = name
        .rfind('.')
        .map(|i| name[i + 1..].to_ascii_lowercase())
        .unwrap_or_default();

    // Icons verified against Adwaita icon theme (mimetypes/symbolic)
    match ext.as_str() {
        // Images
        "jpg" | "jpeg" | "png" | "gif" | "svg" | "webp" | "bmp" | "ico" | "tiff" | "tif"
        | "heic" | "heif" | "avif" | "raw" | "cr2" | "nef" | "psd" | "xcf" => {
            "image-x-generic-symbolic"
        }

        // Audio
        "mp3" | "flac" | "ogg" | "oga" | "wav" | "aac" | "wma" | "m4a" | "opus" | "aiff"
        | "mid" | "midi" | "ac3" => "audio-x-generic-symbolic",

        // Video
        "mp4" | "mkv" | "avi" | "webm" | "mov" | "wmv" | "flv" | "m4v" | "mpg" | "mpeg"
        | "3gp" | "ogv" => "video-x-generic-symbolic",

        // Documents
        "pdf" | "doc" | "docx" | "odt" | "rtf" | "epub" | "djvu" | "pages" | "md"
        | "markdown" | "rst" | "adoc" | "tex" | "latex" => "x-office-document-symbolic",

        // Spreadsheets
        "xls" | "xlsx" | "ods" | "csv" | "tsv" | "numbers" => "x-office-spreadsheet-symbolic",

        // Presentations
        "ppt" | "pptx" | "odp" | "key" => "x-office-presentation-symbolic",

        // Drawings / vector
        "odg" | "vsd" | "vsdx" | "dia" => "x-office-drawing-symbolic",

        // Archives / packages
        "zip" | "tar" | "gz" | "bz2" | "xz" | "zst" | "lz4" | "lzma" | "7z" | "rar"
        | "deb" | "rpm" | "flatpak" | "snap" | "cab" | "cpio" | "ar" | "tgz" | "tbz2"
        | "txz" => "package-x-generic-symbolic",

        // Fonts
        "ttf" | "otf" | "woff" | "woff2" | "eot" => "font-x-generic-symbolic",

        // Executables / binaries
        "exe" | "msi" | "bin" | "appimage" | "run" | "dmg" | "pkg" => {
            "application-x-executable-symbolic"
        }

        // Shared libraries
        "so" | "dll" | "dylib" => "application-x-sharedlib-symbolic",

        // Disk images
        "iso" | "img" => "media-optical-symbolic",

        // Certificates / keys
        "pem" | "crt" | "cer" | "p12" | "pfx" => "application-certificate-symbolic",

        // Add-ons / plugins / extensions
        "xpi" | "crx" => "application-x-addon-symbolic",

        // Firmware
        "fw" | "rom" | "bios" => "application-x-firmware-symbolic",

        // RSS / Atom feeds
        "rss" | "atom" => "application-rss+xml-symbolic",

        // vCard / contacts
        "vcf" | "vcard" => "x-office-address-book-symbolic",

        // Calendar
        "ics" | "ical" => "x-office-calendar-symbolic",

        // Code, scripts, web, config — all use text-x-generic
        // (no script/html/css-specific icons in Adwaita)
        "sh" | "bash" | "zsh" | "fish" | "py" | "rb" | "pl" | "pm" | "php" | "js" | "mjs"
        | "cjs" | "jsx" | "tsx" | "lua" | "go" | "rs" | "c" | "cpp" | "cc" | "cxx" | "h"
        | "hpp" | "hxx" | "java" | "kt" | "kts" | "swift" | "dart" | "cs" | "vb" | "fs"
        | "r" | "scala" | "groovy" | "clj" | "ex" | "exs" | "erl" | "hs" | "ml" | "mli"
        | "ps1" | "bat" | "cmd" | "vbs" | "awk" | "sed" | "m" | "mm" | "v" | "sv"
        | "vhdl" | "zig" | "nim" | "cr" | "jl" | "tcl" | "lisp" | "el" | "vim" | "html"
        | "htm" | "xhtml" | "vue" | "svelte" | "css" | "scss" | "sass" | "less" | "styl"
        | "json" | "yaml" | "yml" | "xml" | "toml" | "ini" | "cfg" | "conf" | "env"
        | "properties" | "plist" | "reg" | "sql" | "graphql" | "proto" | "ts" => {
            "text-x-generic-symbolic"
        }

        // Fallback
        _ => "text-x-generic-symbolic",
    }
}
