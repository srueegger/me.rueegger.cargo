use gtk::{gdk, gio, glib, prelude::*, subclass::prelude::*};

use gettextrs::gettext;

use super::CargoWindow;

impl CargoWindow {
    pub(crate) fn setup_context_menus(&self) {
        // Actions
        let upload_action = gio::SimpleAction::new("upload-selected", None);
        upload_action.connect_activate(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                window.on_upload_clicked();
            }
        ));

        let download_action = gio::SimpleAction::new("download-selected", None);
        download_action.connect_activate(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                window.on_download_clicked();
            }
        ));

        let rename_local_action = gio::SimpleAction::new("rename-local", None);
        rename_local_action.connect_activate(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                window.on_rename_local();
            }
        ));

        let rename_remote_action = gio::SimpleAction::new("rename-remote", None);
        rename_remote_action.connect_activate(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                window.on_rename_remote();
            }
        ));

        let delete_local_action = gio::SimpleAction::new("delete-local", None);
        delete_local_action.connect_activate(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                window.on_delete_local();
            }
        ));

        let delete_remote_action = gio::SimpleAction::new("delete-remote", None);
        delete_remote_action.connect_activate(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                window.on_delete_remote();
            }
        ));

        let chmod_remote_action = gio::SimpleAction::new("chmod-remote", None);
        chmod_remote_action.connect_activate(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                window.on_chmod_remote();
            }
        ));

        self.add_action(&upload_action);
        self.add_action(&download_action);
        self.add_action(&rename_local_action);
        self.add_action(&rename_remote_action);
        self.add_action(&delete_local_action);
        self.add_action(&delete_remote_action);
        self.add_action(&chmod_remote_action);

        // Left panel: Upload | Rename + Delete
        let left_menu = gio::Menu::new();
        left_menu.append(Some(&gettext("Upload")), Some("win.upload-selected"));
        let left_section = gio::Menu::new();
        left_section.append(Some(&gettext("Rename")), Some("win.rename-local"));
        left_section.append(Some(&gettext("Delete")), Some("win.delete-local"));
        left_menu.append_section(None, &left_section);
        let left_popover = gtk::PopoverMenu::from_model(Some(&left_menu));
        left_popover.set_parent(self.imp().left_panel.column_view().upcast_ref::<gtk::Widget>());
        left_popover.set_has_arrow(false);

        let left_gesture = gtk::GestureClick::new();
        left_gesture.set_button(3);
        left_gesture.connect_pressed(glib::clone!(
            #[weak]
            left_popover,
            #[weak(rename_to = panel)]
            self.imp().left_panel,
            #[weak(rename_to = window)]
            self,
            move |gesture, _, x, y| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                panel.select_at_coords(x, y);
                // Rename only enabled for single selection
                if let Some(action) = window.lookup_action("rename-local") {
                    if let Some(action) = action.downcast_ref::<gio::SimpleAction>() {
                        action.set_enabled(panel.selected_items().len() == 1);
                    }
                }
                left_popover.set_pointing_to(Some(&gdk::Rectangle::new(
                    x as i32, y as i32, 1, 1,
                )));
                left_popover.popup();
            }
        ));
        self.imp().left_panel.column_view().add_controller(left_gesture);
        self.imp().left_context_menu.replace(Some(left_popover));

        // Right panel: Download | Rename + Delete
        let right_menu = gio::Menu::new();
        right_menu.append(Some(&gettext("Download")), Some("win.download-selected"));
        let right_section = gio::Menu::new();
        right_section.append(Some(&gettext("Rename")), Some("win.rename-remote"));
        right_section.append(Some(&gettext("Delete")), Some("win.delete-remote"));
        right_menu.append_section(None, &right_section);
        let perm_section = gio::Menu::new();
        perm_section.append(Some(&gettext("Permissions")), Some("win.chmod-remote"));
        right_menu.append_section(None, &perm_section);
        let right_popover = gtk::PopoverMenu::from_model(Some(&right_menu));
        right_popover.set_parent(self.imp().right_panel.column_view().upcast_ref::<gtk::Widget>());
        right_popover.set_has_arrow(false);

        let right_gesture = gtk::GestureClick::new();
        right_gesture.set_button(3);
        right_gesture.connect_pressed(glib::clone!(
            #[weak]
            right_popover,
            #[weak(rename_to = panel)]
            self.imp().right_panel,
            #[weak(rename_to = window)]
            self,
            move |gesture, _, x, y| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                panel.select_at_coords(x, y);
                // Rename and Permissions only enabled for single selection
                let single = panel.selected_items().len() == 1;
                for name in &["rename-remote", "chmod-remote"] {
                    if let Some(action) = window.lookup_action(name) {
                        if let Some(action) = action.downcast_ref::<gio::SimpleAction>() {
                            action.set_enabled(single);
                        }
                    }
                }
                right_popover.set_pointing_to(Some(&gdk::Rectangle::new(
                    x as i32, y as i32, 1, 1,
                )));
                right_popover.popup();
            }
        ));
        self.imp().right_panel.column_view().add_controller(right_gesture);
        self.imp().right_context_menu.replace(Some(right_popover));
    }
}
