// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

use gtk::gio;
use libsecret::prelude::*;

const SCHEMA_NAME: &str = "me.rueegger.cargo.Password";
const ATTR_SITE_ID: &str = "site-id";

/// Build the libsecret schema for Cargo site passwords.
fn schema() -> libsecret::Schema {
    let attributes = std::collections::HashMap::from([(
        ATTR_SITE_ID,
        libsecret::SchemaAttributeType::String,
    )]);
    libsecret::Schema::new(
        SCHEMA_NAME,
        libsecret::SchemaFlags::NONE,
        attributes,
    )
}

/// Build the attribute map for a given site ID.
fn attributes(site_id: &str) -> std::collections::HashMap<&str, &str> {
    std::collections::HashMap::from([(ATTR_SITE_ID, site_id)])
}

/// Store a password in the system keyring for the given site.
pub fn store_password(site_id: &str, label: &str, password: &str) {
    let schema = schema();
    let attrs = attributes(site_id);

    libsecret::password_store_sync(
        Some(&schema),
        attrs,
        Some(libsecret::COLLECTION_DEFAULT),
        label,
        password,
        gio::Cancellable::NONE,
    )
    .unwrap_or_else(|e| {
        log::warn!("Failed to store password in keyring: {}", e);
    });
}

/// Look up a password from the system keyring for the given site.
/// Returns `None` if no password is stored or on error.
pub fn lookup_password(site_id: &str) -> Option<String> {
    let schema = schema();
    let attrs = attributes(site_id);

    match libsecret::password_lookup_sync(
        Some(&schema),
        attrs,
        gio::Cancellable::NONE,
    ) {
        Ok(Some(pw)) => Some(pw.to_string()),
        Ok(None) => None,
        Err(e) => {
            log::warn!("Failed to lookup password in keyring: {}", e);
            None
        }
    }
}

/// Remove a stored password from the system keyring for the given site.
pub fn clear_password(site_id: &str) {
    let schema = schema();
    let attrs = attributes(site_id);

    libsecret::password_clear_sync(
        Some(&schema),
        attrs,
        gio::Cancellable::NONE,
    )
    .unwrap_or_else(|e| {
        log::warn!("Failed to clear password from keyring: {}", e);
    });
}
