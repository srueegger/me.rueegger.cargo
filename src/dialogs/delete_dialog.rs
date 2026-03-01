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
