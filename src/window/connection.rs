use std::rc::Rc;

use gtk::{glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;

use gettextrs::gettext;

use crate::connection::ConnectionHandle;
use crate::file_panel::PanelMode;
use crate::site_manager::SiteProfile;

use super::CargoWindow;

impl CargoWindow {
    pub(crate) fn get_connection(&self) -> Option<Rc<ConnectionHandle>> {
        match &*self.imp().right_panel.imp().mode.borrow() {
            PanelMode::Remote(conn) => Some(conn.clone()),
            PanelMode::Local => None,
        }
    }

    pub(crate) fn disconnect(&self) {
        self.imp().right_panel.set_local_mode();
        self.set_transfer_buttons_sensitive(false);
        *self.imp().connected_site_id.borrow_mut() = None;
        self.populate_sidebar();
        let toast = adw::Toast::new(&gettext("Disconnected"));
        self.imp().toast_overlay.add_toast(toast);
    }

    pub(crate) fn setup_connection_error_handler(&self) {
        self.imp().right_panel.connect_closure(
            "connection-error",
            false,
            glib::closure_local!(
                #[weak(rename_to = window)]
                self,
                move |_panel: crate::file_panel::FilePanel, _msg: String| {
                    if window.get_connection().is_some() {
                        window.disconnect();
                    }
                }
            ),
        );
    }

    pub(crate) fn initiate_connection(&self, profile: SiteProfile, password: Option<String>) {
        let config = profile.to_connection_config(password);
        let site_id = profile.id.clone();
        let local_dir = profile.local_dir.clone();
        let remote_dir = profile.remote_dir.clone();
        let sync_browsing = profile.sync_browsing;
        let imp = self.imp();

        // Show connecting state
        if let Some(store) = imp.right_panel.imp().list_store.get() {
            store.remove_all();
        }
        imp.right_panel.imp().status_label.set_label(&gettext("Connecting…"));

        let window_weak = self.downgrade();

        glib::spawn_future_local(async move {
            match ConnectionHandle::connect(config).await {
                Ok(handle) => {
                    let Some(window) = window_weak.upgrade() else {
                        return;
                    };
                    let conn = Rc::new(handle);
                    window.imp().right_panel.set_remote_mode(
                        conn,
                        remote_dir.as_deref(),
                    );
                    window.set_transfer_buttons_sensitive(true);

                    // Track connected site and update sidebar
                    *window.imp().connected_site_id.borrow_mut() = Some(site_id);
                    window.populate_sidebar();

                    // Apply profile directory settings
                    if let Some(ref dir) = local_dir {
                        let path = std::path::PathBuf::from(dir);
                        if path.is_dir() {
                            window.imp().left_panel.navigate_to(path);
                        }
                    }
                    if sync_browsing {
                        window.imp().sync_nav_button.set_active(true);
                    }

                    let toast = adw::Toast::new(&gettext("Connected"));
                    window.imp().toast_overlay.add_toast(toast);
                }
                Err(e) => {
                    let Some(window) = window_weak.upgrade() else {
                        return;
                    };
                    let err_msg = gettext("Connection failed: %s")
                        .replace("%s", &e.to_string());
                    window
                        .imp()
                        .right_panel
                        .imp()
                        .status_label
                        .set_label(&err_msg);
                    let toast = adw::Toast::builder()
                        .title(err_msg.clone())
                        .timeout(5)
                        .build();
                    window.imp().toast_overlay.add_toast(toast);
                }
            }
        });
    }
}
