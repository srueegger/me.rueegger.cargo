// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

mod sidebar;
mod connection;
mod transfers;
mod file_operations;
mod drag_drop;
mod context_menus;
mod sync_nav;

use std::rc::Rc;

use gtk::{gio, glib, prelude::*, subclass::prelude::*, CompositeTemplate};
use libadwaita as adw;
use adw::prelude::*;
use adw::subclass::prelude::*;

use crate::config::APP_ID;
use crate::transfer::item::TransferItem;
use crate::transfer::queue::TransferQueue;

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct DndPayload {
    pub source: String,
    pub items: Vec<(String, bool)>, // (name, is_dir)
}

pub(crate) enum TransferDirection {
    Upload,
    Download,
}

mod imp {
    use super::*;
    use std::cell::{Cell, OnceCell, RefCell};

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
        pub new_site_button: TemplateChild<gtk::Button>,
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
        #[template_child]
        pub refresh_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub split_view: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub sidebar_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub sidebar_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub sidebar_separator: TemplateChild<gtk::Separator>,

        pub transfer_queue: OnceCell<Rc<TransferQueue>>,
        pub syncing: Cell<bool>,
        pub connected_site_id: RefCell<Option<String>>,
        pub left_context_menu: RefCell<Option<gtk::PopoverMenu>>,
        pub right_context_menu: RefCell<Option<gtk::PopoverMenu>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CargoWindow {
        const NAME: &'static str = "CargoWindow";
        type Type = super::CargoWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            crate::file_item::FileItem::ensure_type();
            crate::file_panel::FilePanel::ensure_type();
            crate::dialogs::site_manager::CargoSiteManagerDialog::ensure_type();
            TransferItem::ensure_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for CargoWindow {
        fn dispose(&self) {
            if let Some(popover) = self.left_context_menu.take() {
                popover.unparent();
            }
            if let Some(popover) = self.right_context_menu.take() {
                popover.unparent();
            }
        }

        fn constructed(&self) {
            self.parent_constructed();
            self.transfer_queue
                .set(Rc::new(TransferQueue::new()))
                .unwrap();
            let obj = self.obj();
            obj.load_window_state();
            obj.setup_hidden_files_toggle();
            obj.setup_sidebar();
            obj.setup_paned_position();
            obj.setup_site_manager_action();
            obj.setup_transfer_buttons();
            obj.setup_transfer_queue_ui();
            obj.setup_double_click_transfer();
            obj.setup_sync_navigation();
            obj.setup_refresh_button();
            obj.setup_drag_and_drop();
            obj.setup_context_menus();
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

    pub(crate) fn set_transfer_buttons_sensitive(&self, sensitive: bool) {
        self.imp().upload_button.set_sensitive(sensitive);
        self.imp().download_button.set_sensitive(sensitive);
        self.imp().sync_nav_button.set_sensitive(sensitive);
    }

    fn open_site_manager(&self) {
        let dialog = crate::dialogs::site_manager::CargoSiteManagerDialog::new();
        dialog.connect_closed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |dialog| {
                // Refresh sidebar (sites may have been added/removed/edited)
                window.populate_sidebar();
                if let Some((profile, password)) = dialog.take_connect_request() {
                    window.initiate_connection(profile, password);
                }
            }
        ));
        dialog.present(Some(self));
    }

    fn setup_site_manager_action(&self) {
        // Menu action + keyboard shortcut (Ctrl+S)
        let sm_action = gio::SimpleAction::new("site-manager", None);
        sm_action.connect_activate(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                window.open_site_manager();
            }
        ));
        self.add_action(&sm_action);
    }
}
