// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

mod application;
mod config;
mod connection;
mod file_item;
mod file_panel;
mod protocol;
mod site_manager;
mod site_manager_dialog;
mod transfer_item;
mod transfer_queue;
mod window;

use gettextrs::LocaleCategory;
use gtk::{gio, glib, prelude::*};

use self::application::CargoApplication;
use self::config::{GETTEXT_PACKAGE, LOCALEDIR, PKGDATADIR};

fn main() -> glib::ExitCode {
    // Set up gettext translations
    gettextrs::setlocale(LocaleCategory::LcAll, "");

    // Try multiple locale directories (Flatpak and system installs)
    let locale_dirs = [
        LOCALEDIR.to_string(),
        format!("{}/locale", PKGDATADIR),
        "/app/share/locale".to_string(),
    ];

    for dir in &locale_dirs {
        gettextrs::bindtextdomain(GETTEXT_PACKAGE, dir).expect("Unable to bind the text domain");
    }

    gettextrs::textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    // Register and include resources
    gio::resources_register_include!("cargo-app.gresource")
        .expect("Failed to register resources");

    let app = CargoApplication::new();
    app.run()
}
