use gtk::{gio, glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use adw::prelude::*;

use gettextrs::gettext;

use crate::keyring;
use crate::site_manager::{SiteProfile, SiteStore};

use super::CargoSiteManagerDialog;

impl CargoSiteManagerDialog {
    pub(super) fn load_sites(&self) {
        let store = SiteStore::load();
        self.populate_list(&store);
        *self.imp().store.borrow_mut() = store;
    }

    pub(super) fn populate_list(&self, store: &SiteStore) {
        let list = &*self.imp().sites_list;
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }
        for site in &store.sites {
            let subtitle = if site.host.is_empty() {
                String::new()
            } else {
                format!("{}:{}", site.host, site.port)
            };
            let row = adw::ActionRow::builder()
                .title(&site.name)
                .subtitle(&subtitle)
                .activatable(true)
                .build();
            list.append(&row);
        }
    }

    pub(crate) fn on_new_site(&self) {
        let new_site = SiteProfile::new();
        let id = new_site.id.clone();
        let _ = id;

        {
            let mut store = self.imp().store.borrow_mut();
            store.add(new_site);
            if let Err(e) = store.save() {
                log::error!("Failed to save sites: {}", e);
            }
        }

        let store = self.imp().store.borrow();
        self.populate_list(&store);
        let index = store.sites.len() as i32 - 1;
        drop(store);

        if let Some(row) = self.imp().sites_list.row_at_index(index) {
            self.imp().sites_list.select_row(Some(&row));
        }
    }

    pub(crate) fn on_delete_site(&self) {
        let selected_id = self.imp().selected_id.borrow().clone();
        if let Some(id) = selected_id {
            // Remove password from keyring
            keyring::clear_password(&id);

            {
                let mut store = self.imp().store.borrow_mut();
                store.remove(&id);
                if let Err(e) = store.save() {
                    log::error!("Failed to save sites: {}", e);
                }
            }

            *self.imp().selected_id.borrow_mut() = None;
            let store = self.imp().store.borrow();
            self.populate_list(&store);
            drop(store);

            self.imp().empty_status.set_visible(true);
            self.imp().edit_form.set_visible(false);
            self.imp().connect_button.set_sensitive(false);
        }
    }

    pub(crate) fn on_connect(&self) {
        let imp = self.imp();
        let selected_id = imp.selected_id.borrow().clone();
        let Some(id) = selected_id else { return };

        let store = imp.store.borrow();
        let Some(site) = store.find(&id) else { return };

        let profile = site.clone();
        let password = {
            let text = imp.password_entry.text().to_string();
            if text.is_empty() { None } else { Some(text) }
        };

        drop(store);
        *imp.connect_request.borrow_mut() = Some((profile, password));
        self.close();
    }

    pub(crate) fn on_browse_local_dir(&self) {
        let dialog = gtk::FileDialog::builder()
            .title(gettext("Select Local Directory"))
            .modal(true)
            .build();

        // Start in the currently configured path, or home
        let current = self.imp().local_dir_path.borrow().clone();
        let start_dir = if current.is_empty() {
            glib::home_dir()
        } else {
            std::path::PathBuf::from(&current)
        };
        if start_dir.is_dir() {
            dialog.set_initial_folder(Some(&gio::File::for_path(&start_dir)));
        }

        let widget_weak = self.downgrade();
        dialog.select_folder(
            gtk::Window::NONE,
            gio::Cancellable::NONE,
            move |result| {
                let Some(widget) = widget_weak.upgrade() else {
                    return;
                };
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        let path_str = path.display().to_string();
                        *widget.imp().local_dir_path.borrow_mut() = path_str.clone();
                        widget.imp().local_dir_row.set_subtitle(&path_str);
                        widget.save_current_form();
                    }
                }
            },
        );
    }

    pub(crate) fn on_browse_key_file(&self) {
        let dialog = gtk::FileDialog::builder()
            .title(gettext("Select SSH Key File"))
            .modal(true)
            .build();

        // Start in ~/.ssh if it exists
        let ssh_dir = glib::home_dir().join(".ssh");
        if ssh_dir.is_dir() {
            dialog.set_initial_folder(Some(&gio::File::for_path(&ssh_dir)));
        }

        let widget_weak = self.downgrade();
        dialog.open(
            gtk::Window::NONE,
            gio::Cancellable::NONE,
            move |result| {
                let Some(widget) = widget_weak.upgrade() else {
                    return;
                };
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        let path_str = path.display().to_string();
                        let filename = path
                            .file_name()
                            .map(|f| f.to_string_lossy().to_string())
                            .unwrap_or_else(|| path_str.clone());
                        *widget.imp().key_file_path.borrow_mut() = path_str;
                        widget.imp().key_file_row.set_subtitle(&filename);
                        widget.save_current_form();
                    }
                }
            },
        );
    }
}
