// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

use std::cell::Cell;
use std::rc::Rc;

use gettextrs::gettext;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

/// Result of the chmod dialog: (mode, recursive).
pub struct ChmodResult {
    pub mode: u32,
    pub recursive: bool,
}

/// Parse a permission string into an octal mode value.
/// Handles both octal strings ("755") and rwx strings ("rwxr-xr-x").
fn parse_permissions(perms: &str) -> u32 {
    if perms.is_empty() {
        return 0o644;
    }

    // Try octal number first — strip file type bits (e.g. 0o100644 → 0o644)
    if let Ok(mode) = u32::from_str_radix(perms, 8) {
        return mode & 0o7777;
    }

    // Try rwx string — FTP uses 10-char format ("drwxr-xr-x"), skip file type char
    let rwx = if perms.len() == 10 {
        &perms.as_bytes()[1..]
    } else if perms.len() >= 9 {
        &perms.as_bytes()[..9]
    } else {
        return 0o644;
    };

    if rwx.len() >= 9 {
        let mut mode: u32 = 0;

        // Owner
        if rwx[0] == b'r' { mode |= 0o400; }
        if rwx[1] == b'w' { mode |= 0o200; }
        if rwx[2] == b'x' || rwx[2] == b's' { mode |= 0o100; }

        // Group
        if rwx[3] == b'r' { mode |= 0o040; }
        if rwx[4] == b'w' { mode |= 0o020; }
        if rwx[5] == b'x' || rwx[5] == b's' { mode |= 0o010; }

        // Other
        if rwx[6] == b'r' { mode |= 0o004; }
        if rwx[7] == b'w' { mode |= 0o002; }
        if rwx[8] == b'x' || rwx[8] == b't' { mode |= 0o001; }

        return mode;
    }

    0o644
}

/// Show a file permissions dialog.
/// Returns `Some(ChmodResult)` if the user confirmed, `None` if cancelled.
pub async fn show_chmod_dialog(
    window: &impl IsA<gtk::Widget>,
    name: &str,
    current_perms: &str,
    is_dir: bool,
) -> Option<ChmodResult> {
    let (tx, rx) = async_channel::bounded::<Option<ChmodResult>>(1);

    let initial_mode = parse_permissions(current_perms);

    // Track whether we're currently updating programmatically to avoid loops
    let updating = Rc::new(Cell::new(false));

    // Build the checkbox grid
    let grid = gtk::Grid::builder()
        .row_spacing(8)
        .column_spacing(12)
        .margin_top(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    // Header row
    let read_header = gtk::Label::new(Some(&gettext("Read")));
    let write_header = gtk::Label::new(Some(&gettext("Write")));
    let exec_header = gtk::Label::new(Some(&gettext("Execute")));
    read_header.add_css_class("dim-label");
    write_header.add_css_class("dim-label");
    exec_header.add_css_class("dim-label");
    grid.attach(&read_header, 1, 0, 1, 1);
    grid.attach(&write_header, 2, 0, 1, 1);
    grid.attach(&exec_header, 3, 0, 1, 1);

    // Permission bits: (row_label, row_index, r_bit, w_bit, x_bit)
    let rows = [
        (gettext("Owner"), 1, 0o400u32, 0o200u32, 0o100u32),
        (gettext("Group"), 2, 0o040u32, 0o020u32, 0o010u32),
        (gettext("Public"), 3, 0o004u32, 0o002u32, 0o001u32),
    ];

    let mut checkboxes: Vec<(gtk::CheckButton, u32)> = Vec::with_capacity(9);

    for (label_text, row, r_bit, w_bit, x_bit) in &rows {
        let label = gtk::Label::new(Some(label_text));
        label.set_xalign(0.0);
        label.set_hexpand(true);
        grid.attach(&label, 0, *row, 1, 1);

        for (col, bit) in [(1, r_bit), (2, w_bit), (3, x_bit)] {
            let cb = gtk::CheckButton::new();
            cb.set_active(initial_mode & bit != 0);
            cb.set_halign(gtk::Align::Center);
            grid.attach(&cb, col, *row, 1, 1);
            checkboxes.push((cb, *bit));
        }
    }

    // Numeric entry
    let num_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    num_box.set_margin_top(8);
    num_box.set_margin_start(12);
    num_box.set_margin_end(12);

    let num_label = gtk::Label::new(Some(&gettext("Numeric value")));
    num_label.set_hexpand(true);
    num_label.set_xalign(0.0);

    let num_entry = gtk::Entry::new();
    num_entry.set_text(&format!("{:o}", initial_mode));
    num_entry.set_max_length(4);
    num_entry.set_width_chars(5);
    num_entry.set_input_purpose(gtk::InputPurpose::Digits);

    num_box.append(&num_label);
    num_box.append(&num_entry);

    // Recursive checkbox (only for directories)
    let recurse_check = gtk::CheckButton::with_label(&gettext("Recurse into subdirectories"));
    recurse_check.set_margin_top(8);
    recurse_check.set_margin_start(12);
    recurse_check.set_margin_end(12);

    // Container
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&grid);
    content.append(&num_box);
    if is_dir {
        content.append(&recurse_check);
    }

    // Sync: checkboxes → entry
    let checkboxes_rc = Rc::new(checkboxes);
    for (cb, _bit) in checkboxes_rc.iter() {
        let entry_clone = num_entry.clone();
        let cbs = checkboxes_rc.clone();
        let upd = updating.clone();
        cb.connect_toggled(move |_| {
            if upd.get() {
                return;
            }
            upd.set(true);
            let mut mode: u32 = 0;
            for (c, b) in cbs.iter() {
                if c.is_active() {
                    mode |= b;
                }
            }
            entry_clone.set_text(&format!("{:o}", mode));
            upd.set(false);
        });
    }

    // Sync: entry → checkboxes
    {
        let cbs = checkboxes_rc.clone();
        let upd = updating.clone();
        num_entry.connect_changed(move |entry| {
            if upd.get() {
                return;
            }
            let text = entry.text();
            if let Ok(mode) = u32::from_str_radix(text.as_str(), 8) {
                if mode <= 0o7777 {
                    upd.set(true);
                    for (cb, bit) in cbs.iter() {
                        cb.set_active(mode & bit != 0);
                    }
                    upd.set(false);
                }
            }
        });
    }

    // Build dialog
    let dialog = adw::AlertDialog::builder()
        .heading(gettext("Permissions"))
        .body(name)
        .build();

    dialog.set_extra_child(Some(&content));
    dialog.add_response("cancel", &gettext("Cancel"));
    dialog.add_response("apply", &gettext("Apply"));
    dialog.set_response_appearance("apply", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("apply"));
    dialog.set_close_response("cancel");

    let entry_clone = num_entry.clone();
    let recurse_clone = recurse_check.clone();
    dialog.connect_response(None, move |_dialog, response| {
        let result = if response == "apply" {
            let text = entry_clone.text();
            if let Ok(mode) = u32::from_str_radix(text.as_str(), 8) {
                Some(ChmodResult {
                    mode,
                    recursive: recurse_clone.is_active(),
                })
            } else {
                None
            }
        } else {
            None
        };
        let _ = tx.send_blocking(result);
    });

    dialog.present(Some(window));

    rx.recv().await.unwrap_or(None)
}
