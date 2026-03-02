use gtk::{gdk, gio, glib, prelude::*, subclass::prelude::*};
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

    pub(crate) fn setup_sidebar_context_menu(&self) {
        // Actions
        let disconnect_action = gio::SimpleAction::new("sidebar-disconnect", None);
        disconnect_action.connect_activate(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                window.disconnect();
            }
        ));

        let edit_action = gio::SimpleAction::new("sidebar-edit", None);
        edit_action.connect_activate(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                let site_id = window.imp().sidebar_context_site_id.borrow().clone();
                if let Some(id) = site_id {
                    let dialog =
                        crate::dialogs::site_manager::CargoSiteManagerDialog::new();
                    dialog.select_site_by_id(&id);
                    dialog.connect_closed(glib::clone!(
                        #[weak]
                        window,
                        move |dialog| {
                            window.populate_sidebar();
                            if let Some((profile, password)) =
                                dialog.take_connect_request()
                            {
                                window.initiate_connection(profile, password);
                            }
                        }
                    ));
                    dialog.present(Some(&window));
                }
            }
        ));

        let delete_action = gio::SimpleAction::new("sidebar-delete", None);
        delete_action.connect_activate(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                let site_id = window.imp().sidebar_context_site_id.borrow().clone();
                if let Some(id) = site_id {
                    let store = SiteStore::load();
                    let name = store
                        .find(&id)
                        .map(|s| s.name.clone())
                        .unwrap_or_default();

                    let dialog = adw::AlertDialog::builder()
                        .heading(gettext("Delete Connection"))
                        .body(
                            gettext("Are you sure you want to delete '%s'?")
                                .replace("%s", &name),
                        )
                        .build();
                    dialog.add_response("cancel", &gettext("Cancel"));
                    dialog.add_response("delete", &gettext("Delete"));
                    dialog.set_response_appearance(
                        "delete",
                        adw::ResponseAppearance::Destructive,
                    );
                    dialog.set_default_response(Some("cancel"));
                    dialog.set_close_response("cancel");

                    let window_weak = window.downgrade();
                    let id_clone = id.clone();
                    dialog.connect_response(None, move |_dialog, response| {
                        if response == "delete" {
                            let Some(window) = window_weak.upgrade() else {
                                return;
                            };
                            let mut store = SiteStore::load();
                            store.remove(&id_clone);
                            if let Err(e) = store.save() {
                                log::error!("Failed to save sites: {}", e);
                            }
                            window.populate_sidebar();
                        }
                    });

                    dialog.present(Some(&window));
                }
            }
        ));

        self.add_action(&disconnect_action);
        self.add_action(&edit_action);
        self.add_action(&delete_action);

        // Menu model
        let menu = gio::Menu::new();
        let conn_section = gio::Menu::new();
        conn_section.append(
            Some(&gettext("Disconnect")),
            Some("win.sidebar-disconnect"),
        );
        menu.append_section(None, &conn_section);

        let edit_section = gio::Menu::new();
        edit_section.append(Some(&gettext("Edit")), Some("win.sidebar-edit"));
        edit_section.append(Some(&gettext("Delete")), Some("win.sidebar-delete"));
        menu.append_section(None, &edit_section);

        let popover = gtk::PopoverMenu::from_model(Some(&menu));
        popover.set_parent(self.imp().sidebar_list.upcast_ref::<gtk::Widget>());
        popover.set_has_arrow(false);

        // Right-click gesture
        let gesture = gtk::GestureClick::new();
        gesture.set_button(3);
        gesture.connect_pressed(glib::clone!(
            #[weak]
            popover,
            #[weak(rename_to = window)]
            self,
            move |gesture, _, x, y| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                let sidebar = &window.imp().sidebar_list;
                let Some(row) = sidebar.row_at_y(y as i32) else {
                    return;
                };
                let site_id = row.widget_name().to_string();
                let connected_id =
                    window.imp().connected_site_id.borrow().clone();
                let is_connected =
                    connected_id.as_deref() == Some(site_id.as_str());

                *window.imp().sidebar_context_site_id.borrow_mut() =
                    Some(site_id);

                // Disconnect only for active connection
                if let Some(a) = window.lookup_action("sidebar-disconnect") {
                    if let Some(a) = a.downcast_ref::<gio::SimpleAction>() {
                        a.set_enabled(is_connected);
                    }
                }
                // Edit and Delete disabled for active connection
                for name in &["sidebar-edit", "sidebar-delete"] {
                    if let Some(a) = window.lookup_action(name) {
                        if let Some(a) = a.downcast_ref::<gio::SimpleAction>() {
                            a.set_enabled(!is_connected);
                        }
                    }
                }

                popover.set_pointing_to(Some(&gdk::Rectangle::new(
                    x as i32, y as i32, 1, 1,
                )));
                popover.popup();
            }
        ));

        self.imp().sidebar_list.add_controller(gesture);
        self.imp().sidebar_context_menu.replace(Some(popover));
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
