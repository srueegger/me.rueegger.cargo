// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

use std::rc::Rc;

use gtk::{gio, glib, prelude::*, subclass::prelude::*, CompositeTemplate};
use libadwaita as adw;
use adw::prelude::*;
use adw::subclass::prelude::*;

use crate::config::APP_ID;
use crate::connection::ConnectionHandle;
use crate::site_manager::SiteProfile;

mod imp {
    use super::*;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/me/rueegger/cargo/ui/window.ui")]
    pub struct CargoWindow {
        #[template_child]
        pub header_bar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub paned: TemplateChild<gtk::Paned>,
        #[template_child]
        pub left_panel: TemplateChild<crate::file_panel::FilePanel>,
        #[template_child]
        pub right_panel: TemplateChild<crate::file_panel::FilePanel>,
        #[template_child]
        pub hidden_files_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub connect_button: TemplateChild<gtk::Button>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CargoWindow {
        const NAME: &'static str = "CargoWindow";
        type Type = super::CargoWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            crate::file_item::FileItem::ensure_type();
            crate::file_panel::FilePanel::ensure_type();
            crate::site_manager_dialog::CargoSiteManagerDialog::ensure_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for CargoWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.load_window_state();
            obj.setup_hidden_files_toggle();
            obj.setup_paned_position();
            obj.setup_site_manager_button();
        }
    }

    impl WidgetImpl for CargoWindow {}
    impl WindowImpl for CargoWindow {
        fn close_request(&self) -> glib::Propagation {
            let obj = self.obj();
            obj.save_window_state();
            // Disconnect remote panel if connected
            if obj.imp().right_panel.is_remote() {
                obj.imp().right_panel.set_local_mode();
            }
            glib::Propagation::Proceed
        }
    }
    impl ApplicationWindowImpl for CargoWindow {}
    impl AdwApplicationWindowImpl for CargoWindow {}
}

glib::wrapper! {
    pub struct CargoWindow(ObjectSubclass<imp::CargoWindow>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow,
                 gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap,
                    gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native,
                    gtk::Root, gtk::ShortcutManager;
}

impl CargoWindow {
    pub fn new(app: &adw::Application) -> Self {
        glib::Object::builder()
            .property("application", app)
            .build()
    }

    fn settings(&self) -> gio::Settings {
        gio::Settings::new(APP_ID)
    }

    fn save_window_state(&self) {
        let settings = self.settings();
        let (width, height) = self.default_size();
        settings.set_int("window-width", width).unwrap();
        settings.set_int("window-height", height).unwrap();
        settings
            .set_boolean("is-maximized", self.is_maximized())
            .unwrap();
    }

    fn load_window_state(&self) {
        let settings = self.settings();
        let width = settings.int("window-width");
        let height = settings.int("window-height");
        let is_maximized = settings.boolean("is-maximized");

        self.set_default_size(width, height);
        if is_maximized {
            self.maximize();
        }
    }

    fn setup_hidden_files_toggle(&self) {
        let settings = self.settings();
        settings
            .bind("show-hidden-files", &*self.imp().hidden_files_button, "active")
            .build();
    }

    fn setup_paned_position(&self) {
        self.imp().paned.set_position(600);
    }

    fn setup_site_manager_button(&self) {
        self.imp().connect_button.connect_clicked(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| {
                if window.imp().right_panel.is_remote() {
                    // Disconnect
                    window.imp().right_panel.set_local_mode();
                    window.imp().connect_button.set_icon_name("network-server-symbolic");
                    window.imp().connect_button.set_tooltip_text(Some("Site Manager"));
                    let toast = adw::Toast::new("Disconnected");
                    window.imp().toast_overlay.add_toast(toast);
                } else {
                    // Open Site Manager
                    let dialog = crate::site_manager_dialog::CargoSiteManagerDialog::new();
                    dialog.connect_closed(glib::clone!(
                        #[weak]
                        window,
                        move |dialog| {
                            if let Some((profile, password)) = dialog.take_connect_request() {
                                window.initiate_connection(profile, password);
                            }
                        }
                    ));
                    dialog.present(Some(&window));
                }
            }
        ));
    }

    fn initiate_connection(&self, profile: SiteProfile, password: Option<String>) {
        let config = profile.to_connection_config(password);
        let imp = self.imp();

        // Show connecting state
        if let Some(store) = imp.right_panel.imp().list_store.get() {
            store.remove_all();
        }
        imp.right_panel.imp().status_label.set_label("Connecting...");

        let window_weak = self.downgrade();

        glib::spawn_future_local(async move {
            match ConnectionHandle::connect(config).await {
                Ok(handle) => {
                    let Some(window) = window_weak.upgrade() else {
                        return;
                    };
                    let conn = Rc::new(handle);
                    window.imp().right_panel.set_remote_mode(conn);
                    window
                        .imp()
                        .connect_button
                        .set_icon_name("network-offline-symbolic");
                    window
                        .imp()
                        .connect_button
                        .set_tooltip_text(Some("Disconnect"));
                    let toast = adw::Toast::new("Connected");
                    window.imp().toast_overlay.add_toast(toast);
                }
                Err(e) => {
                    let Some(window) = window_weak.upgrade() else {
                        return;
                    };
                    window
                        .imp()
                        .right_panel
                        .imp()
                        .status_label
                        .set_label(&format!("Connection failed: {}", e));
                    let toast = adw::Toast::builder()
                        .title(format!("Connection failed: {}", e))
                        .timeout(5)
                        .build();
                    window.imp().toast_overlay.add_toast(toast);
                }
            }
        });
    }
}
