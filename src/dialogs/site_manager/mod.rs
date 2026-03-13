// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

mod actions;
mod form;

use gtk::{glib, prelude::*, subclass::prelude::*, CompositeTemplate};
use libadwaita as adw;
use adw::prelude::*;

use crate::site_manager::SiteProfile;

mod imp {
    use super::*;
    use crate::site_manager::SiteStore;
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

        // Connect button
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

        self.imp().password_entry.connect_changed(glib::clone!(
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

    /// Take the connect request (profile + password), consumed by the window.
    pub fn take_connect_request(&self) -> Option<(SiteProfile, Option<String>)> {
        self.imp().connect_request.borrow_mut().take()
    }

    /// Select a site by its unique ID so the form opens on that entry.
    pub fn select_site_by_id(&self, id: &str) {
        let store = self.imp().store.borrow();
        let index = store.sites.iter().position(|s| s.id == id);
        drop(store);
        if let Some(idx) = index {
            if let Some(row) = self.imp().sites_list.row_at_index(idx as i32) {
                self.imp().sites_list.select_row(Some(&row));
            }
        }
    }
}
