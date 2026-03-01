use gtk::{glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use adw::prelude::*;

use gettextrs::gettext;

use crate::site_manager::{AuthMethodType, SiteStore};

use super::CargoWindow;

impl CargoWindow {
    pub(crate) fn setup_sidebar(&self) {
        // Add breakpoint: collapse sidebar when window width < 650px
        let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
            adw::BreakpointConditionLengthType::MaxWidth,
            650.0,
            adw::LengthUnit::Px,
        ));
        breakpoint.add_setter(&*self.imp().split_view, "collapsed", Some(&true.to_value()));
        self.add_breakpoint(breakpoint);

        // Sidebar button + separator only visible when collapsed (narrow window)
        let collapsed = self.imp().split_view.is_collapsed();
        self.imp().sidebar_button.set_visible(collapsed);
        self.imp().sidebar_separator.set_visible(collapsed);

        // Toggle button controls overlay sidebar when collapsed
        self.imp()
            .sidebar_button
            .bind_property("active", &*self.imp().split_view, "show-sidebar")
            .bidirectional()
            .sync_create()
            .build();

        // When collapsed state changes, show/hide the toggle button
        // and auto-hide the sidebar overlay
        self.imp().split_view.connect_collapsed_notify(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |split_view| {
                let collapsed = split_view.is_collapsed();
                window.imp().sidebar_button.set_visible(collapsed);
                window.imp().sidebar_separator.set_visible(collapsed);
                if collapsed {
                    split_view.set_show_sidebar(false);
                }
            }
        ));

        // New Site button → open Site Manager
        self.imp().new_site_button.connect_clicked(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| {
                window.open_site_manager();
            }
        ));

        // Populate sidebar with saved sites
        self.populate_sidebar();

        // Connect row activation → connect or disconnect
        self.imp().sidebar_list.connect_row_activated(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, row| {
                let site_id = row.widget_name().to_string();
                let connected_id = window.imp().connected_site_id.borrow().clone();
                if connected_id.as_deref() == Some(site_id.as_str()) {
                    // Already connected → disconnect
                    window.disconnect();
                } else {
                    window.initiate_sidebar_connect(&site_id);
                }
            }
        ));
    }

    pub(crate) fn populate_sidebar(&self) {
        let sidebar = &*self.imp().sidebar_list;

        // Remove all existing rows
        while let Some(row) = sidebar.row_at_index(0) {
            sidebar.remove(&row);
        }

        let store = SiteStore::load();
        let connected_id = self.imp().connected_site_id.borrow().clone();

        for site in &store.sites {
            let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row_box.set_margin_start(4);
            row_box.set_margin_end(4);
            row_box.set_margin_top(2);
            row_box.set_margin_bottom(2);

            let icon_name = match site.protocol {
                crate::site_manager::SerializableProtocol::Ftp => "cargo-ftp-symbolic",
                crate::site_manager::SerializableProtocol::Ftps => "cargo-ftps-symbolic",
                crate::site_manager::SerializableProtocol::Sftp => "cargo-sftp-symbolic",
            };
            let icon = gtk::Image::from_icon_name(icon_name);
            row_box.append(&icon);

            let label = gtk::Label::new(Some(&site.name));
            label.set_xalign(0.0);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_hexpand(true);
            row_box.append(&label);

            // Show connected indicator
            if connected_id.as_deref() == Some(&site.id) {
                let dot = gtk::Label::new(Some("\u{25CF}"));
                dot.add_css_class("success");
                row_box.append(&dot);
            }

            let row = gtk::ListBoxRow::new();
            row.set_child(Some(&row_box));
            row.set_widget_name(&site.id);
            sidebar.append(&row);
        }
    }

    pub(crate) fn initiate_sidebar_connect(&self, site_id: &str) {
        let store = SiteStore::load();
        let Some(profile) = store.find(site_id) else {
            return;
        };
        let profile = profile.clone();

        match &profile.auth_method {
            AuthMethodType::Agent => {
                self.initiate_connection(profile, None);
            }
            AuthMethodType::Password | AuthMethodType::KeyFile { .. } => {
                let heading = gettext("Connect to %s").replace("%s", &profile.name);
                let body = match &profile.auth_method {
                    AuthMethodType::KeyFile { .. } => gettext("Enter passphrase for key"),
                    _ => gettext("Enter password"),
                };

                let dialog = adw::AlertDialog::builder()
                    .heading(heading)
                    .body(body)
                    .build();

                let entry = gtk::PasswordEntry::builder()
                    .show_peek_icon(true)
                    .build();
                entry.set_margin_start(12);
                entry.set_margin_end(12);
                dialog.set_extra_child(Some(&entry));

                dialog.add_response("cancel", &gettext("Cancel"));
                dialog.add_response("connect", &gettext("Connect"));
                dialog.set_response_appearance(
                    "connect",
                    adw::ResponseAppearance::Suggested,
                );
                dialog.set_default_response(Some("connect"));
                dialog.set_close_response("cancel");

                let window_weak = self.downgrade();
                let entry_clone = entry.clone();
                dialog.connect_response(None, move |_dialog, response| {
                    if response == "connect" {
                        let Some(window) = window_weak.upgrade() else {
                            return;
                        };
                        let password = entry_clone.text().to_string();
                        let pw = if password.is_empty() {
                            None
                        } else {
                            Some(password)
                        };
                        window.initiate_connection(profile.clone(), pw);
                    }
                });

                dialog.present(Some(self));
            }
        }
    }
}
