// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

use gettextrs::gettext;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

/// Show a rename dialog with the current name pre-filled.
/// Returns `Some(new_name)` if the user confirmed, `None` if cancelled.
pub async fn show_rename_dialog(
    window: &impl IsA<gtk::Widget>,
    current_name: &str,
) -> Option<String> {
    let (tx, rx) = async_channel::bounded::<Option<String>>(1);

    let entry = gtk::Entry::new();
    entry.set_text(current_name);
    entry.set_margin_top(12);
    entry.set_margin_start(12);
    entry.set_margin_end(12);
    entry.set_activates_default(true);

    // Select filename without extension
    let select_end = current_name.rfind('.').unwrap_or(current_name.len());
    entry.select_region(0, select_end as i32);

    let dialog = adw::AlertDialog::builder()
        .heading(gettext("Rename"))
        .build();

    dialog.set_extra_child(Some(&entry));
    dialog.add_response("cancel", &gettext("Cancel"));
    dialog.add_response("rename", &gettext("Rename"));
    dialog.set_response_appearance("rename", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("rename"));
    dialog.set_close_response("cancel");

    // Disable rename button when name is empty or unchanged
    let current = current_name.to_string();
    dialog.set_response_enabled("rename", false);
    let dialog_weak = dialog.downgrade();
    let current_clone = current.clone();
    entry.connect_changed(move |entry| {
        let text = entry.text();
        let enabled = !text.is_empty() && text.as_str() != current_clone;
        if let Some(d) = dialog_weak.upgrade() {
            d.set_response_enabled("rename", enabled);
        }
    });

    let entry_clone = entry.clone();
    dialog.connect_response(None, move |_dialog, response| {
        let result = if response == "rename" {
            let new_name = entry_clone.text().to_string();
            if new_name.is_empty() || new_name == current {
                None
            } else {
                Some(new_name)
            }
        } else {
            None
        };
        let _ = tx.send_blocking(result);
    });

    dialog.present(Some(window));

    rx.recv().await.unwrap_or(None)
}
