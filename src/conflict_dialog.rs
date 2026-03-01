// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

use gettextrs::{gettext, ngettext};
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

use crate::protocol::RemoteEntry;

/// What to do when a destination file already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictAction {
    Overwrite,
    OverwriteIfSizeDiffers,
    OverwriteIfNewer,
    Skip,
    Rename,
}

/// The result returned by the overwrite confirmation dialog.
#[derive(Debug, Clone)]
pub struct ConflictResolution {
    pub action: ConflictAction,
    pub apply_to_queue: bool,
}

/// Show an overwrite confirmation dialog for the given filename.
/// Returns `None` if the dialog was dismissed (treated as Skip).
pub async fn show_overwrite_dialog(
    window: &impl IsA<gtk::Widget>,
    filename: &str,
) -> Option<ConflictResolution> {
    let (tx, rx) = async_channel::bounded::<Option<ConflictResolution>>(1);

    let radio_overwrite = gtk::CheckButton::with_label(&gettext("Overwrite"));
    let radio_size = gtk::CheckButton::with_label(&gettext("Overwrite if size differs"));
    radio_size.set_group(Some(&radio_overwrite));
    let radio_newer = gtk::CheckButton::with_label(&gettext("Overwrite if source is newer"));
    radio_newer.set_group(Some(&radio_overwrite));
    let radio_skip = gtk::CheckButton::with_label(&gettext("Skip"));
    radio_skip.set_group(Some(&radio_overwrite));
    let radio_rename = gtk::CheckButton::with_label(&gettext("Rename (append number)"));
    radio_rename.set_group(Some(&radio_overwrite));

    radio_overwrite.set_active(true);

    let apply_check = gtk::CheckButton::with_label(&gettext("Apply to current queue"));

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(4);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);
    vbox.append(&radio_overwrite);
    vbox.append(&radio_size);
    vbox.append(&radio_newer);
    vbox.append(&radio_skip);
    vbox.append(&radio_rename);

    let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    separator.set_margin_top(4);
    separator.set_margin_bottom(4);
    vbox.append(&separator);
    vbox.append(&apply_check);

    let dialog = adw::AlertDialog::builder()
        .heading(gettext("File already exists"))
        .body(
            gettext("The file '%s' already exists at the destination. What would you like to do?")
                .replace("%s", filename),
        )
        .build();

    dialog.set_extra_child(Some(&vbox));
    dialog.add_response("cancel", &gettext("Cancel"));
    dialog.add_response("ok", &gettext("OK"));
    dialog.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("ok"));
    dialog.set_close_response("cancel");

    dialog.connect_response(None, move |_dialog, response| {
        let result = if response == "ok" {
            let action = if radio_size.is_active() {
                ConflictAction::OverwriteIfSizeDiffers
            } else if radio_newer.is_active() {
                ConflictAction::OverwriteIfNewer
            } else if radio_skip.is_active() {
                ConflictAction::Skip
            } else if radio_rename.is_active() {
                ConflictAction::Rename
            } else {
                ConflictAction::Overwrite
            };
            Some(ConflictResolution {
                action,
                apply_to_queue: apply_check.is_active(),
            })
        } else {
            None
        };
        let _ = tx.send_blocking(result);
    });

    dialog.present(Some(window));

    rx.recv().await.unwrap_or(None)
}

/// Show a delete confirmation dialog.
/// Returns `true` if the user confirmed the deletion.
pub async fn show_delete_confirmation(
    window: &impl IsA<gtk::Widget>,
    names: &[String],
) -> bool {
    let (tx, rx) = async_channel::bounded::<bool>(1);

    let heading = if names.len() == 1 {
        gettext("Delete '%s'?").replace("%s", &names[0])
    } else {
        ngettext("Delete %u item?", "Delete %u items?", names.len() as u32)
            .replace("%u", &names.len().to_string())
    };

    let dialog = adw::AlertDialog::builder()
        .heading(heading)
        .body(gettext("This cannot be undone."))
        .build();

    dialog.add_response("cancel", &gettext("Cancel"));
    dialog.add_response("delete", &gettext("Delete"));
    dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    dialog.connect_response(None, move |_dialog, response| {
        let _ = tx.send_blocking(response == "delete");
    });

    dialog.present(Some(window));

    rx.recv().await.unwrap_or(false)
}

/// Get local file metadata (size, modified timestamp as unix epoch).
pub fn local_file_info(path: &std::path::Path) -> Option<(u64, Option<i64>)> {
    let meta = std::fs::metadata(path).ok()?;
    let size = meta.len();
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64);
    Some((size, modified))
}

/// Find a file by name in a remote directory listing.
pub fn remote_file_exists<'a>(
    entries: &'a [RemoteEntry],
    filename: &str,
) -> Option<&'a RemoteEntry> {
    entries.iter().find(|e| e.name == filename && !e.is_dir)
}

/// Determine whether a transfer should proceed based on the conflict action
/// and source/destination metadata.
///
/// Returns `(should_transfer, destination_path)` — the path may be renamed.
pub fn should_transfer(
    action: ConflictAction,
    source_size: u64,
    source_modified: Option<i64>,
    dest_size: u64,
    dest_modified: Option<i64>,
    dest_path: &str,
) -> (bool, String) {
    match action {
        ConflictAction::Overwrite => (true, dest_path.to_string()),
        ConflictAction::OverwriteIfSizeDiffers => {
            (source_size != dest_size, dest_path.to_string())
        }
        ConflictAction::OverwriteIfNewer => match (source_modified, dest_modified) {
            (Some(src), Some(dst)) => (src > dst, dest_path.to_string()),
            _ => (true, dest_path.to_string()),
        },
        ConflictAction::Skip => (false, dest_path.to_string()),
        ConflictAction::Rename => {
            let renamed = generate_rename(dest_path);
            (true, renamed)
        }
    }
}

/// Generate a renamed path: `file.txt` → `file (1).txt`
fn generate_rename(path: &str) -> String {
    let (dir, filename) = match path.rfind('/') {
        Some(i) => (&path[..=i], &path[i + 1..]),
        None => ("", path),
    };
    let (stem, ext) = match filename.rfind('.') {
        Some(i) => (&filename[..i], &filename[i..]),
        None => (filename, ""),
    };
    format!("{}{} (1){}", dir, stem, ext)
}
