use gtk::{glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;

use gettextrs::gettext;

use crate::file_panel::SyncEvent;

use super::CargoWindow;

impl CargoWindow {
    pub(crate) fn setup_sync_navigation(&self) {
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
                            let toast = adw::Toast::new(
                                &gettext("Directory '%s' not found locally")
                                    .replace("%s", &name),
                            );
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

    pub(crate) fn setup_refresh_button(&self) {
        self.imp().refresh_button.connect_clicked(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| {
                window.imp().left_panel.reload();
                window.imp().right_panel.reload();
            }
        ));
    }
}
