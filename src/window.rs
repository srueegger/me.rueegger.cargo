// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

use gtk::{gio, glib, prelude::*, subclass::prelude::*, CompositeTemplate};
use libadwaita as adw;
use adw::subclass::prelude::*;

use crate::config::APP_ID;

mod imp {
    use super::*;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/me/rueegger/cargo/ui/window.ui")]
    pub struct CargoWindow {
        #[template_child]
        pub header_bar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub status_page: TemplateChild<adw::StatusPage>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CargoWindow {
        const NAME: &'static str = "CargoWindow";
        type Type = super::CargoWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
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
        }
    }

    impl WidgetImpl for CargoWindow {}
    impl WindowImpl for CargoWindow {
        fn close_request(&self) -> glib::Propagation {
            self.obj().save_window_state();
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
}
