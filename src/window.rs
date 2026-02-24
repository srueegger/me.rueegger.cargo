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
use crate::file_panel::{PanelMode, SyncEvent};
use crate::site_manager::SiteProfile;
use crate::transfer_item::*;
use crate::transfer_queue::TransferQueue;

mod imp {
    use super::*;
    use std::cell::{Cell, OnceCell};

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
        #[template_child]
        pub upload_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub download_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub transfer_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub transfer_list_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub clear_transfers_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub sync_nav_button: TemplateChild<gtk::ToggleButton>,

        pub transfer_queue: OnceCell<Rc<TransferQueue>>,
        pub syncing: Cell<bool>,
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
            TransferItem::ensure_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for CargoWindow {
        fn constructed(&self) {
            self.parent_constructed();
            self.transfer_queue
                .set(Rc::new(TransferQueue::new()))
                .unwrap();
            let obj = self.obj();
            obj.load_window_state();
            obj.setup_hidden_files_toggle();
            obj.setup_paned_position();
            obj.setup_site_manager_button();
            obj.setup_transfer_buttons();
            obj.setup_transfer_queue_ui();
            obj.setup_sync_navigation();
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

    fn set_transfer_buttons_sensitive(&self, sensitive: bool) {
        self.imp().upload_button.set_sensitive(sensitive);
        self.imp().download_button.set_sensitive(sensitive);
        self.imp().sync_nav_button.set_sensitive(sensitive);
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
                    window.set_transfer_buttons_sensitive(false);
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

    fn setup_transfer_buttons(&self) {
        // Upload: selected files from left (local) → right (remote)
        self.imp().upload_button.connect_clicked(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| {
                window.on_upload_clicked();
            }
        ));

        // Download: selected files from right (remote) → left (local)
        self.imp().download_button.connect_clicked(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| {
                window.on_download_clicked();
            }
        ));

        // Clear completed transfers
        self.imp().clear_transfers_button.connect_clicked(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| {
                window.imp().transfer_queue.get().unwrap().clear_completed();
                if !window.imp().transfer_queue.get().unwrap().has_items() {
                    window.imp().transfer_revealer.set_reveal_child(false);
                }
            }
        ));
    }

    fn setup_transfer_queue_ui(&self) {
        let imp = self.imp();
        let queue = imp.transfer_queue.get().unwrap();

        // Set up the ListView model
        let selection = gtk::NoSelection::new(Some(queue.store().clone()));
        imp.transfer_list_view.set_model(Some(&selection));

        // Set up the factory for transfer rows
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();

            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.set_margin_start(8);
            row.set_margin_end(8);
            row.set_margin_top(4);
            row.set_margin_bottom(4);

            let icon = gtk::Image::new();
            row.append(&icon);

            let filename_label = gtk::Label::new(None);
            filename_label.set_xalign(0.0);
            filename_label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            filename_label.set_width_chars(20);
            row.append(&filename_label);

            let progress_bar = gtk::ProgressBar::new();
            progress_bar.set_hexpand(true);
            progress_bar.set_valign(gtk::Align::Center);
            row.append(&progress_bar);

            let status_label = gtk::Label::new(None);
            status_label.set_width_chars(25);
            status_label.set_xalign(1.0);
            status_label.add_css_class("caption");
            status_label.add_css_class("dim-label");
            row.append(&status_label);

            list_item.set_child(Some(&row));
        });

        factory.connect_bind(|_, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            let item: TransferItem = list_item.item().and_downcast().unwrap();
            let row: gtk::Box = list_item.child().and_downcast().unwrap();

            let icon = row.first_child().and_downcast::<gtk::Image>().unwrap();
            let filename_label = icon
                .next_sibling()
                .and_downcast::<gtk::Label>()
                .unwrap();
            let progress_bar = filename_label
                .next_sibling()
                .and_downcast::<gtk::ProgressBar>()
                .unwrap();
            let status_label = progress_bar
                .next_sibling()
                .and_downcast::<gtk::Label>()
                .unwrap();

            icon.set_icon_name(Some(item.direction_icon()));
            filename_label.set_label(&item.filename());
            progress_bar.set_fraction(item.progress());
            status_label.set_label(&item.status_label());

            // Bind to property changes for live updates
            let pb = progress_bar.clone();
            let sl = status_label.clone();
            let item_clone = item.clone();
            item.connect_notify_local(Some("progress"), move |item, _| {
                pb.set_fraction(item.progress());
                sl.set_label(&item_clone.status_label());
            });

            let sl2 = status_label.clone();
            item.connect_notify_local(Some("status"), move |item, _| {
                sl2.set_label(&item.status_label());
            });
        });

        imp.transfer_list_view.set_factory(Some(&factory));
    }

    fn get_connection(&self) -> Option<Rc<ConnectionHandle>> {
        match &*self.imp().right_panel.imp().mode.borrow() {
            PanelMode::Remote(conn) => Some(conn.clone()),
            PanelMode::Local => None,
        }
    }

    fn on_upload_clicked(&self) {
        let Some(conn) = self.get_connection() else {
            return;
        };

        let imp = self.imp();
        let selected = imp.left_panel.selected_items();
        if selected.is_empty() {
            let toast = adw::Toast::new("No files selected for upload");
            imp.toast_overlay.add_toast(toast);
            return;
        }

        let local_dir = imp.left_panel.current_path();
        let remote_dir = imp.right_panel.remote_path();
        let mut queued = 0;

        for item in &selected {
            if item.is_dir() {
                continue;
            }
            let local_path = local_dir.join(item.name());
            let remote_path = if remote_dir.ends_with('/') {
                format!("{}{}", remote_dir, item.name())
            } else {
                format!("{}/{}", remote_dir, item.name())
            };

            let transfer = TransferItem::new_upload(
                &item.name(),
                &local_path.display().to_string(),
                &remote_path,
            );
            imp.transfer_queue.get().unwrap().enqueue(transfer);
            queued += 1;
        }

        if queued == 0 {
            let toast = adw::Toast::new("No files to upload (directories are skipped)");
            imp.toast_overlay.add_toast(toast);
            return;
        }

        imp.transfer_revealer.set_reveal_child(true);
        TransferQueue::start_processing(
            imp.transfer_queue.get().unwrap(),
            &conn,
            &imp.left_panel,
            &imp.right_panel,
        );
    }

    fn on_download_clicked(&self) {
        let Some(conn) = self.get_connection() else {
            return;
        };

        let imp = self.imp();
        let selected = imp.right_panel.selected_items();
        if selected.is_empty() {
            let toast = adw::Toast::new("No files selected for download");
            imp.toast_overlay.add_toast(toast);
            return;
        }

        let local_dir = imp.left_panel.current_path();
        let remote_dir = imp.right_panel.remote_path();
        let mut queued = 0;

        for item in &selected {
            if item.is_dir() {
                continue;
            }
            let local_path = local_dir.join(item.name());
            let remote_path = if remote_dir.ends_with('/') {
                format!("{}{}", remote_dir, item.name())
            } else {
                format!("{}/{}", remote_dir, item.name())
            };

            let transfer = TransferItem::new_download(
                &item.name(),
                &local_path.display().to_string(),
                &remote_path,
            );
            imp.transfer_queue.get().unwrap().enqueue(transfer);
            queued += 1;
        }

        if queued == 0 {
            let toast = adw::Toast::new("No files to download (directories are skipped)");
            imp.toast_overlay.add_toast(toast);
            return;
        }

        imp.transfer_revealer.set_reveal_child(true);
        TransferQueue::start_processing(
            imp.transfer_queue.get().unwrap(),
            &conn,
            &imp.left_panel,
            &imp.right_panel,
        );
    }

    fn setup_sync_navigation(&self) {
        // Bind GSettings to toggle button
        let settings = self.settings();
        settings
            .bind("sync-navigation", &*self.imp().sync_nav_button, "active")
            .build();

        // Left panel navigated → sync right panel
        let window_weak = self.downgrade();
        self.imp()
            .left_panel
            .set_sync_callback(Box::new(move |event| {
                let Some(window) = window_weak.upgrade() else {
                    return;
                };
                let imp = window.imp();
                if imp.syncing.get() || !imp.sync_nav_button.is_active() {
                    return;
                }
                if !imp.right_panel.is_remote() {
                    return;
                }

                imp.syncing.set(true);
                match event {
                    SyncEvent::EnterDir(name) => {
                        let current = imp.right_panel.remote_path();
                        let new_path = if current.ends_with('/') {
                            format!("{}{}", current, name)
                        } else {
                            format!("{}/{}", current, name)
                        };
                        imp.right_panel.navigate_to_remote(&new_path);
                    }
                    SyncEvent::Up => {
                        imp.right_panel.navigate_up();
                    }
                }
                imp.syncing.set(false);
            }));

        // Right panel navigated → sync left panel
        let window_weak = self.downgrade();
        self.imp()
            .right_panel
            .set_sync_callback(Box::new(move |event| {
                let Some(window) = window_weak.upgrade() else {
                    return;
                };
                let imp = window.imp();
                if imp.syncing.get() || !imp.sync_nav_button.is_active() {
                    return;
                }

                imp.syncing.set(true);
                match event {
                    SyncEvent::EnterDir(name) => {
                        let new_path = imp.left_panel.current_path().join(&name);
                        if new_path.is_dir() {
                            imp.left_panel.navigate_to(new_path);
                        } else {
                            let toast = adw::Toast::new(&format!(
                                "Directory '{}' not found locally",
                                name
                            ));
                            imp.toast_overlay.add_toast(toast);
                        }
                    }
                    SyncEvent::Up => {
                        imp.left_panel.navigate_up();
                    }
                }
                imp.syncing.set(false);
            }));
    }

    fn initiate_connection(&self, profile: SiteProfile, password: Option<String>) {
        let config = profile.to_connection_config(password);
        let local_dir = profile.local_dir.clone();
        let remote_dir = profile.remote_dir.clone();
        let sync_browsing = profile.sync_browsing;
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
                    window.set_transfer_buttons_sensitive(true);

                    // Apply profile directory settings
                    if let Some(ref dir) = local_dir {
                        let path = std::path::PathBuf::from(dir);
                        if path.is_dir() {
                            window.imp().left_panel.navigate_to(path);
                        }
                    }
                    if let Some(ref dir) = remote_dir {
                        window.imp().right_panel.navigate_to_remote(dir);
                    }
                    if sync_browsing {
                        window.imp().sync_nav_button.set_active(true);
                    }

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
