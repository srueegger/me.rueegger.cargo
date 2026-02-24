// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

use gtk::{gio, glib, prelude::*, subclass::prelude::*, CompositeTemplate};
use libadwaita as adw;
use adw::prelude::*;

use crate::site_manager::{AuthMethodType, SerializableProtocol, SiteProfile, SiteStore};

mod imp {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/me/rueegger/cargo/ui/site_manager_dialog.ui")]
    pub struct CargoSiteManagerDialog {
        #[template_child]
        pub sites_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub new_site_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub connect_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub delete_button: TemplateChild<gtk::Button>,

        // Form fields
        #[template_child]
        pub name_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub protocol_combo: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub host_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub port_spin: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub username_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub auth_method_combo: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub key_file_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub key_file_browse_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub password_entry: TemplateChild<adw::PasswordEntryRow>,

        // Directories
        #[template_child]
        pub local_dir_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub local_dir_browse_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub remote_dir_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub sync_browsing_switch: TemplateChild<adw::SwitchRow>,

        pub key_file_path: RefCell<String>,
        pub local_dir_path: RefCell<String>,

        // State containers
        #[template_child]
        pub empty_status: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub edit_form: TemplateChild<gtk::Box>,

        // Internal state
        pub store: RefCell<SiteStore>,
        pub selected_id: RefCell<Option<String>>,
        pub updating_form: Cell<bool>,
        pub connect_request: RefCell<Option<(SiteProfile, Option<String>)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CargoSiteManagerDialog {
        const NAME: &'static str = "CargoSiteManagerDialog";
        type Type = super::CargoSiteManagerDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for CargoSiteManagerDialog {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.load_sites();
            obj.setup_signals();
        }
    }

    impl WidgetImpl for CargoSiteManagerDialog {}
    impl adw::subclass::prelude::AdwDialogImpl for CargoSiteManagerDialog {}
}

glib::wrapper! {
    pub struct CargoSiteManagerDialog(ObjectSubclass<imp::CargoSiteManagerDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl CargoSiteManagerDialog {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    fn load_sites(&self) {
        let store = SiteStore::load();
        self.populate_list(&store);
        *self.imp().store.borrow_mut() = store;
    }

    fn populate_list(&self, store: &SiteStore) {
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

    fn setup_signals(&self) {
        // List selection changed
        self.imp().sites_list.connect_row_selected(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_, row| {
                dialog.on_site_selected(row);
            }
        ));

        // New Site button
        self.imp().new_site_button.connect_clicked(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| {
                dialog.on_new_site();
            }
        ));

        // Delete button
        self.imp().delete_button.connect_clicked(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| {
                dialog.on_delete_site();
            }
        ));

        // Connect button (placeholder)
        self.imp().connect_button.connect_clicked(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| {
                dialog.on_connect();
            }
        ));

        // Form field change handlers (auto-save)
        self.imp().name_entry.connect_changed(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| {
                dialog.save_current_form();
            }
        ));

        self.imp().host_entry.connect_changed(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| {
                dialog.save_current_form();
            }
        ));

        self.imp().username_entry.connect_changed(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| {
                dialog.save_current_form();
            }
        ));

        self.imp().key_file_browse_button.connect_clicked(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| {
                dialog.on_browse_key_file();
            }
        ));

        self.imp().protocol_combo.connect_selected_notify(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| {
                dialog.on_protocol_changed();
                dialog.save_current_form();
            }
        ));

        self.imp().auth_method_combo.connect_selected_notify(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| {
                dialog.on_auth_method_changed();
                dialog.save_current_form();
            }
        ));

        self.imp().port_spin.connect_value_notify(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| {
                dialog.save_current_form();
            }
        ));

        self.imp().local_dir_browse_button.connect_clicked(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| {
                dialog.on_browse_local_dir();
            }
        ));

        self.imp().remote_dir_entry.connect_changed(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| {
                dialog.save_current_form();
            }
        ));

        self.imp().sync_browsing_switch.connect_active_notify(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| {
                dialog.save_current_form();
            }
        ));
    }

    fn on_site_selected(&self, row: Option<&gtk::ListBoxRow>) {
        let imp = self.imp();
        if let Some(row) = row {
            let index = row.index() as usize;
            let store = imp.store.borrow();
            if let Some(site) = store.sites.get(index) {
                *imp.selected_id.borrow_mut() = Some(site.id.clone());
                self.populate_form(site);
                imp.empty_status.set_visible(false);
                imp.edit_form.set_visible(true);
                imp.connect_button.set_sensitive(true);
            }
        } else {
            *imp.selected_id.borrow_mut() = None;
            imp.empty_status.set_visible(true);
            imp.edit_form.set_visible(false);
            imp.connect_button.set_sensitive(false);
        }
    }

    fn populate_form(&self, site: &SiteProfile) {
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
            imp.key_file_row.set_subtitle("No file selected");
        } else {
            // Show just the filename
            let display = std::path::Path::new(&key_path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or(key_path);
            imp.key_file_row.set_subtitle(&display);
        }
        imp.key_file_row.set_visible(auth_index == 1);
        // Password: visible for Password (0) and KeyFile passphrase (1), clear on site switch
        imp.password_entry.set_text("");
        imp.password_entry.set_visible(auth_index != 2);

        // Directories
        let local_dir = site.local_dir.clone().unwrap_or_default();
        *imp.local_dir_path.borrow_mut() = local_dir.clone();
        if local_dir.is_empty() {
            imp.local_dir_row.set_subtitle("Default");
        } else {
            imp.local_dir_row.set_subtitle(&local_dir);
        }
        imp.remote_dir_entry.set_text(site.remote_dir.as_deref().unwrap_or(""));
        imp.sync_browsing_switch.set_active(site.sync_browsing);

        imp.updating_form.set(false);
    }

    fn save_current_form(&self) {
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
        drop(store);

        self.update_selected_row_label(&site_name, &site_host, site_port);

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

    fn on_new_site(&self) {
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

    fn on_delete_site(&self) {
        let selected_id = self.imp().selected_id.borrow().clone();
        if let Some(id) = selected_id {
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

    fn on_connect(&self) {
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

    /// Take the connect request (profile + password), consumed by the window.
    pub fn take_connect_request(&self) -> Option<(SiteProfile, Option<String>)> {
        self.imp().connect_request.borrow_mut().take()
    }

    fn on_protocol_changed(&self) {
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

    fn on_auth_method_changed(&self) {
        let selected = self.imp().auth_method_combo.selected();
        // Key file row: visible only for KeyFile (1)
        self.imp().key_file_row.set_visible(selected == 1);
        // Password: visible for Password (0) and KeyFile passphrase (1)
        self.imp().password_entry.set_visible(selected != 2);
    }

    fn on_browse_local_dir(&self) {
        let dialog = gtk::FileDialog::builder()
            .title("Select Local Directory")
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

    fn on_browse_key_file(&self) {
        let dialog = gtk::FileDialog::builder()
            .title("Select SSH Key File")
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
