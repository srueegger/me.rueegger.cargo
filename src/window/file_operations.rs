use gtk::{glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;

use gettextrs::gettext;

use super::CargoWindow;

impl CargoWindow {
    pub(crate) fn on_rename_local(&self) {
        let imp = self.imp();
        let selected = imp.left_panel.selected_items();
        if selected.len() != 1 {
            return;
        }

        let old_name = selected[0].name();
        let base_path = imp.left_panel.current_path();

        let window_weak = self.downgrade();
        glib::spawn_future_local(async move {
            let Some(window) = window_weak.upgrade() else { return; };

            let Some(new_name) = crate::dialogs::show_rename_dialog(&window, &old_name).await else {
                return;
            };

            let old_path = base_path.join(&old_name);
            let new_path = base_path.join(&new_name);

            match std::fs::rename(&old_path, &new_path) {
                Ok(()) => {
                    window.imp().left_panel.reload();
                    let toast = adw::Toast::new(&gettext("Renamed successfully"));
                    window.imp().toast_overlay.add_toast(toast);
                }
                Err(e) => {
                    let toast = adw::Toast::new(
                        &gettext("Rename failed: %s").replace("%s", &e.to_string()),
                    );
                    window.imp().toast_overlay.add_toast(toast);
                }
            }
        });
    }

    pub(crate) fn on_rename_remote(&self) {
        let imp = self.imp();
        let selected = imp.right_panel.selected_items();
        if selected.len() != 1 {
            return;
        }

        let old_name = selected[0].name();
        let remote_dir = imp.right_panel.remote_path();

        let window_weak = self.downgrade();
        glib::spawn_future_local(async move {
            let Some(window) = window_weak.upgrade() else { return; };

            let Some(new_name) = crate::dialogs::show_rename_dialog(&window, &old_name).await else {
                return;
            };

            let Some(conn) = window.get_connection() else {
                let toast = adw::Toast::new(&gettext("Connect to a server first"));
                window.imp().toast_overlay.add_toast(toast);
                return;
            };

            let old_path = if remote_dir.ends_with('/') {
                format!("{}{}", remote_dir, old_name)
            } else {
                format!("{}/{}", remote_dir, old_name)
            };
            let new_path = if remote_dir.ends_with('/') {
                format!("{}{}", remote_dir, new_name)
            } else {
                format!("{}/{}", remote_dir, new_name)
            };

            match conn.rename(&old_path, &new_path).await {
                Ok(()) => {
                    window.imp().right_panel.reload();
                    let toast = adw::Toast::new(&gettext("Renamed successfully"));
                    window.imp().toast_overlay.add_toast(toast);
                }
                Err(e) => {
                    let toast = adw::Toast::new(
                        &gettext("Rename failed: %s").replace("%s", &e.to_string()),
                    );
                    window.imp().toast_overlay.add_toast(toast);
                }
            }
        });
    }

    pub(crate) fn on_chmod_remote(&self) {
        let imp = self.imp();
        let selected = imp.right_panel.selected_items();
        if selected.len() != 1 {
            return;
        }

        let name = selected[0].name();
        let current_perms = selected[0].permissions();
        let is_dir = selected[0].is_dir();
        let remote_dir = imp.right_panel.remote_path();

        let window_weak = self.downgrade();
        glib::spawn_future_local(async move {
            let Some(window) = window_weak.upgrade() else { return; };

            let Some(result) = crate::dialogs::show_chmod_dialog(
                &window,
                &name,
                &current_perms,
                is_dir,
            )
            .await
            else {
                return;
            };

            let Some(conn) = window.get_connection() else {
                let toast = adw::Toast::new(&gettext("Connect to a server first"));
                window.imp().toast_overlay.add_toast(toast);
                return;
            };

            let path = if remote_dir.ends_with('/') {
                format!("{}{}", remote_dir, name)
            } else {
                format!("{}/{}", remote_dir, name)
            };

            let res = if result.recursive {
                conn.chmod_recursive(&path, result.mode).await
            } else {
                conn.chmod(&path, result.mode).await
            };

            match res {
                Ok(()) => {
                    window.imp().right_panel.reload();
                    let toast = adw::Toast::new(&gettext("Permissions changed"));
                    window.imp().toast_overlay.add_toast(toast);
                }
                Err(e) => {
                    let toast = adw::Toast::new(
                        &gettext("Change permissions failed: %s")
                            .replace("%s", &e.to_string()),
                    );
                    window.imp().toast_overlay.add_toast(toast);
                }
            }
        });
    }

    pub(crate) fn on_delete_local(&self) {
        let imp = self.imp();
        let selected = imp.left_panel.selected_items();
        if selected.is_empty() {
            return;
        }

        let names: Vec<String> = selected.iter().map(|i| i.name()).collect();
        let items: Vec<(String, bool)> = selected
            .iter()
            .map(|i| (i.name(), i.is_dir()))
            .collect();
        let base_path = imp.left_panel.current_path();

        let window_weak = self.downgrade();
        glib::spawn_future_local(async move {
            let Some(window) = window_weak.upgrade() else { return; };

            if !crate::dialogs::show_delete_confirmation(&window, &names).await {
                return;
            }

            let mut errors: Vec<String> = Vec::new();
            for (name, is_dir) in &items {
                let path = base_path.join(name);
                let result = if *is_dir {
                    std::fs::remove_dir_all(&path)
                } else {
                    std::fs::remove_file(&path)
                };
                if let Err(e) = result {
                    errors.push(format!("{}: {}", name, e));
                }
            }

            window.imp().left_panel.reload();

            let toast = if errors.is_empty() {
                adw::Toast::new(&gettext("Deleted successfully"))
            } else {
                adw::Toast::new(
                    &gettext("Delete failed: %s").replace("%s", &errors.join(", ")),
                )
            };
            window.imp().toast_overlay.add_toast(toast);
        });
    }

    pub(crate) fn on_delete_remote(&self) {
        let imp = self.imp();
        let selected = imp.right_panel.selected_items();
        if selected.is_empty() {
            return;
        }

        let names: Vec<String> = selected.iter().map(|i| i.name()).collect();
        let items: Vec<(String, bool)> = selected
            .iter()
            .map(|i| (i.name(), i.is_dir()))
            .collect();
        let remote_dir = imp.right_panel.remote_path();

        let window_weak = self.downgrade();
        glib::spawn_future_local(async move {
            let Some(window) = window_weak.upgrade() else { return; };

            if !crate::dialogs::show_delete_confirmation(&window, &names).await {
                return;
            }

            let Some(conn) = window.get_connection() else {
                let toast = adw::Toast::new(&gettext("Connect to a server first"));
                window.imp().toast_overlay.add_toast(toast);
                return;
            };

            let mut errors: Vec<String> = Vec::new();
            for (name, is_dir) in &items {
                let remote_path = if remote_dir.ends_with('/') {
                    format!("{}{}", remote_dir, name)
                } else {
                    format!("{}/{}", remote_dir, name)
                };
                let result = if *is_dir {
                    conn.delete_recursive(&remote_path).await
                } else {
                    conn.delete(&remote_path).await
                };
                if let Err(e) = result {
                    errors.push(format!("{}: {}", name, e));
                }
            }

            window.imp().right_panel.reload();

            let toast = if errors.is_empty() {
                adw::Toast::new(&gettext("Deleted successfully"))
            } else {
                adw::Toast::new(
                    &gettext("Delete failed: %s").replace("%s", &errors.join(", ")),
                )
            };
            window.imp().toast_overlay.add_toast(toast);
        });
    }
}
