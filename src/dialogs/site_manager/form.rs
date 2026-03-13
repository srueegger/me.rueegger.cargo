use gtk::{prelude::*, subclass::prelude::*};
use libadwaita as adw;
use adw::prelude::*;

use gettextrs::gettext;

use crate::keyring;
use crate::site_manager::{AuthMethodType, SerializableProtocol, SiteProfile};

use super::CargoSiteManagerDialog;

impl CargoSiteManagerDialog {
    pub(super) fn populate_form(&self, site: &SiteProfile) {
        let imp = self.imp();
        imp.updating_form.set(true);

        imp.name_entry.set_text(&site.name);
        imp.host_entry.set_text(&site.host);
        imp.port_spin.set_value(site.port as f64);
        imp.username_entry.set_text(&site.username);

        // Protocol: SFTP=0, FTP=1, FTPS=2
        let proto_index = match site.protocol {
            SerializableProtocol::Sftp => 0,
            SerializableProtocol::Ftp => 1,
            SerializableProtocol::Ftps => 2,
        };
        imp.protocol_combo.set_selected(proto_index);

        // Auth method: Password=0, KeyFile=1, Agent=2
        let (auth_index, key_path) = match &site.auth_method {
            AuthMethodType::Password => (0, String::new()),
            AuthMethodType::KeyFile { path } => (1, path.clone()),
            AuthMethodType::Agent => (2, String::new()),
        };
        imp.auth_method_combo.set_selected(auth_index);
        *imp.key_file_path.borrow_mut() = key_path.clone();
        if key_path.is_empty() {
            imp.key_file_row.set_subtitle(&gettext("No file selected"));
        } else {
            // Show just the filename
            let display = std::path::Path::new(&key_path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or(key_path);
            imp.key_file_row.set_subtitle(&display);
        }
        imp.key_file_row.set_visible(auth_index == 1);
        // Password: visible for Password (0) and KeyFile passphrase (1)
        // Load saved password from keyring
        let stored_pw = keyring::lookup_password(&site.id);
        imp.password_entry.set_text(stored_pw.as_deref().unwrap_or(""));
        imp.password_entry.set_visible(auth_index != 2);

        // Directories
        let local_dir = site.local_dir.clone().unwrap_or_default();
        *imp.local_dir_path.borrow_mut() = local_dir.clone();
        if local_dir.is_empty() {
            imp.local_dir_row.set_subtitle(&gettext("Default"));
        } else {
            imp.local_dir_row.set_subtitle(&local_dir);
        }
        imp.remote_dir_entry.set_text(site.remote_dir.as_deref().unwrap_or(""));
        imp.sync_browsing_switch.set_active(site.sync_browsing);

        imp.updating_form.set(false);
    }

    pub(crate) fn save_current_form(&self) {
        let imp = self.imp();
        if imp.updating_form.get() {
            return;
        }

        let selected_id = imp.selected_id.borrow().clone();
        let Some(ref id) = selected_id else {
            return;
        };

        let mut store = imp.store.borrow_mut();
        let Some(site) = store.find_mut(id) else {
            return;
        };

        site.name = imp.name_entry.text().to_string();
        site.host = imp.host_entry.text().to_string();
        site.port = imp.port_spin.value() as u16;
        site.username = imp.username_entry.text().to_string();

        site.protocol = match imp.protocol_combo.selected() {
            0 => SerializableProtocol::Sftp,
            1 => SerializableProtocol::Ftp,
            2 => SerializableProtocol::Ftps,
            _ => SerializableProtocol::Sftp,
        };

        site.auth_method = match imp.auth_method_combo.selected() {
            0 => AuthMethodType::Password,
            1 => AuthMethodType::KeyFile {
                path: imp.key_file_path.borrow().clone(),
            },
            2 => AuthMethodType::Agent,
            _ => AuthMethodType::Password,
        };

        let local_dir = imp.local_dir_path.borrow().clone();
        site.local_dir = if local_dir.is_empty() { None } else { Some(local_dir) };
        let remote_dir = imp.remote_dir_entry.text().to_string();
        site.remote_dir = if remote_dir.is_empty() { None } else { Some(remote_dir) };
        site.sync_browsing = imp.sync_browsing_switch.is_active();

        let site_name = site.name.clone();
        let site_host = site.host.clone();
        let site_port = site.port;
        let site_id = site.id.clone();
        drop(store);

        self.update_selected_row_label(&site_name, &site_host, site_port);

        // Store password in system keyring
        let password_text = imp.password_entry.text().to_string();
        if password_text.is_empty() {
            keyring::clear_password(&site_id);
        } else {
            let label = format!("Cargo: {}", site_name);
            keyring::store_password(&site_id, &label, &password_text);
        }

        let store = self.imp().store.borrow();
        if let Err(e) = store.save() {
            log::error!("Failed to save sites: {}", e);
        }
    }

    fn update_selected_row_label(&self, name: &str, host: &str, port: u16) {
        if let Some(row) = self.imp().sites_list.selected_row() {
            if let Some(action_row) = row.downcast_ref::<adw::ActionRow>() {
                action_row.set_title(name);
                let subtitle = if host.is_empty() {
                    String::new()
                } else {
                    format!("{}:{}", host, port)
                };
                action_row.set_subtitle(&subtitle);
            }
        }
    }

    pub(crate) fn on_protocol_changed(&self) {
        if self.imp().updating_form.get() {
            return;
        }
        let port = match self.imp().protocol_combo.selected() {
            0 => 22,  // SFTP
            1 => 21,  // FTP
            2 => 21,  // FTPS
            _ => 22,
        };
        self.imp().port_spin.set_value(port as f64);
    }

    pub(crate) fn on_auth_method_changed(&self) {
        let selected = self.imp().auth_method_combo.selected();
        // Key file row: visible only for KeyFile (1)
        self.imp().key_file_row.set_visible(selected == 1);
        // Password: visible for Password (0) and KeyFile passphrase (1)
        self.imp().password_entry.set_visible(selected != 2);
    }
}
