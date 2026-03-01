// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

use gettextrs::gettext;
use gtk::{gio, gdk, glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use adw::prelude::*;
use adw::subclass::prelude::*;

use crate::config::{APP_ID, VERSION};
use crate::window::CargoWindow;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct CargoApplication {}

    #[glib::object_subclass]
    impl ObjectSubclass for CargoApplication {
        const NAME: &'static str = "CargoApplication";
        type Type = super::CargoApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for CargoApplication {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.setup_gactions();
        }
    }

    impl ApplicationImpl for CargoApplication {
        fn startup(&self) {
            self.parent_startup();

            let display = gdk::Display::default().unwrap();

            let provider = gtk::CssProvider::new();
            provider.load_from_resource("/me/rueegger/cargo/style.css");
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );

            // Register app-bundled icons (protocol icons etc.)
            let icon_theme = gtk::IconTheme::for_display(&display);
            icon_theme.add_resource_path("/me/rueegger/cargo/icons");
        }

        fn activate(&self) {
            let app = self.obj();
            let window = if let Some(window) = app.active_window() {
                window
            } else {
                let window = CargoWindow::new(app.upcast_ref::<adw::Application>());
                window.upcast()
            };
            window.present();
        }
    }

    impl GtkApplicationImpl for CargoApplication {}
    impl AdwApplicationImpl for CargoApplication {}
}

glib::wrapper! {
    pub struct CargoApplication(ObjectSubclass<imp::CargoApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl CargoApplication {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", APP_ID)
            .property("flags", gio::ApplicationFlags::FLAGS_NONE)
            .build()
    }

    fn setup_gactions(&self) {
        let quit_action = gio::ActionEntry::builder("quit")
            .activate(move |app: &Self, _, _| app.quit())
            .build();
        let about_action = gio::ActionEntry::builder("about")
            .activate(move |app: &Self, _, _| app.show_about())
            .build();
        self.add_action_entries([quit_action, about_action]);

        self.set_accels_for_action("app.quit", &["<primary>q"]);
        self.set_accels_for_action("win.site-manager", &["<primary>s"]);
    }

    fn show_about(&self) {
        let window = self.active_window().unwrap();

        let about = adw::AboutDialog::builder()
            .application_name("Cargo")
            .application_icon(APP_ID)
            .developer_name("Samuel Rüegger")
            .version(VERSION)
            .developers(vec!["Samuel Rüegger"])
            .copyright("© 2026 Samuel Rüegger")
            .license_type(gtk::License::Gpl20Only)
            .website("https://github.com/srueegger/me.rueegger.cargo")
            .issue_url("https://github.com/srueegger/me.rueegger.cargo/issues")
            .translator_credits(gettext("translator-credits"))
            .build();

        about.add_credit_section(
            Some(&gettext("Libraries")),
            &[
                "GTK 4 https://gtk.org",
                "Libadwaita https://gnome.pages.gitlab.gnome.org/libadwaita",
                "russh https://github.com/Eugeny/russh",
                "suppaftp https://github.com/veeso/suppaftp",
                "Tokio https://tokio.rs",
            ],
        );

        about.present(Some(&window));
    }
}
