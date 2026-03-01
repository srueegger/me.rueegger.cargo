// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

use std::rc::Rc;

use gtk::{gio, gdk, glib, prelude::*, subclass::prelude::*, CompositeTemplate};
use libadwaita as adw;
use adw::prelude::*;
use adw::subclass::prelude::*;

use gettextrs::gettext;

use crate::config::APP_ID;
use crate::connection::ConnectionHandle;
use crate::file_panel::{PanelMode, SyncEvent};
use crate::site_manager::SiteProfile;
use crate::transfer_item::*;
use crate::transfer_queue::TransferQueue;

#[derive(serde::Serialize, serde::Deserialize)]
struct DndPayload {
    source: String,
    items: Vec<(String, bool)>, // (name, is_dir)
}

enum TransferDirection {
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
            crate::dialogs::site_manager_dialog::CargoSiteManagerDialog::ensure_type();
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
            obj.setup_paned_position();
            obj.setup_site_manager_button();
            obj.setup_transfer_buttons();
            obj.setup_transfer_queue_ui();
            obj.setup_sync_navigation();
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

    fn set_transfer_buttons_sensitive(&self, sensitive: bool) {
        self.imp().upload_button.set_sensitive(sensitive);
        self.imp().download_button.set_sensitive(sensitive);
        self.imp().sync_nav_button.set_sensitive(sensitive);
    }

    fn open_site_manager(&self) {
        let dialog = crate::dialogs::site_manager_dialog::CargoSiteManagerDialog::new();
        dialog.connect_closed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |dialog| {
                if let Some((profile, password)) = dialog.take_connect_request() {
                    window.initiate_connection(profile, password);
                }
            }
        ));
        dialog.present(Some(self));
    }

    fn setup_site_manager_button(&self) {
        // Toolbar button: toggles between Site Manager / Disconnect
        self.imp().connect_button.connect_clicked(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| {
                if window.imp().right_panel.is_remote() {
                    // Disconnect
                    window.imp().right_panel.set_local_mode();
                    window.imp().connect_button.set_icon_name("network-server-symbolic");
                    window.imp().connect_button.set_tooltip_text(Some(&gettext("Site Manager")));
                    window.set_transfer_buttons_sensitive(false);
                    let toast = adw::Toast::new(&gettext("Disconnected"));
                    window.imp().toast_overlay.add_toast(toast);
                } else {
                    window.open_site_manager();
                }
            }
        ));

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

    fn enqueue_items(
        &self,
        items: Vec<(String, bool)>,
        local_dir: std::path::PathBuf,
        remote_dir: String,
        direction: TransferDirection,
    ) {
        let window_weak = self.downgrade();
        glib::spawn_future_local(async move {
            let Some(window) = window_weak.upgrade() else {
                return;
            };
            let Some(conn) = window.get_connection() else {
                return;
            };
            let imp = window.imp();

            let mut transfers: Vec<(String, String, String)> = Vec::new();

            for (name, is_dir) in &items {
                let local_path = local_dir.join(name);
                let remote_path = if remote_dir.ends_with('/') {
                    format!("{}{}", remote_dir, name)
                } else {
                    format!("{}/{}", remote_dir, name)
                };

                if !is_dir {
                    transfers.push((
                        name.clone(),
                        local_path.display().to_string(),
                        remote_path,
                    ));
                } else {
                    match direction {
                        TransferDirection::Upload => {
                            let mut dirs_to_walk =
                                vec![(local_path.clone(), remote_path.clone())];
                            while let Some((ldir, rdir)) = dirs_to_walk.pop() {
                                let _ = conn.mkdir(&rdir).await;
                                let Ok(entries) = std::fs::read_dir(&ldir) else {
                                    continue;
                                };
                                for entry in entries.flatten() {
                                    let ename =
                                        entry.file_name().to_string_lossy().to_string();
                                    let epath = entry.path();
                                    let rpath = format!("{}/{}", rdir, ename);
                                    if entry
                                        .file_type()
                                        .map(|t| t.is_dir())
                                        .unwrap_or(false)
                                    {
                                        dirs_to_walk.push((epath, rpath));
                                    } else {
                                        transfers.push((
                                            ename,
                                            epath.display().to_string(),
                                            rpath,
                                        ));
                                    }
                                }
                            }
                        }
                        TransferDirection::Download => {
                            let mut dirs_to_walk =
                                vec![(local_path.clone(), remote_path.clone())];
                            while let Some((ldir, rdir)) = dirs_to_walk.pop() {
                                let _ = std::fs::create_dir_all(&ldir);
                                let Ok(entries) = conn.list_dir(&rdir).await else {
                                    continue;
                                };
                                for entry in entries {
                                    let lpath = ldir.join(&entry.name);
                                    let rpath = format!("{}/{}", rdir, entry.name);
                                    if entry.is_dir {
                                        dirs_to_walk.push((lpath, rpath));
                                    } else {
                                        transfers.push((
                                            entry.name.clone(),
                                            lpath.display().to_string(),
                                            rpath,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            for (filename, local_path, remote_path) in &transfers {
                let transfer = match direction {
                    TransferDirection::Upload => {
                        TransferItem::new_upload(filename, local_path, remote_path)
                    }
                    TransferDirection::Download => {
                        TransferItem::new_download(filename, local_path, remote_path)
                    }
                };
                imp.transfer_queue.get().unwrap().enqueue(transfer);
            }

            if !transfers.is_empty() {
                imp.transfer_revealer.set_reveal_child(true);
                TransferQueue::start_processing(
                    imp.transfer_queue.get().unwrap(),
                    &conn,
                    &imp.left_panel,
                    &imp.right_panel,
                    &window,
                );
            }
        });
    }

    fn on_upload_clicked(&self) {
        if self.get_connection().is_none() {
            return;
        }

        let imp = self.imp();
        let selected = imp.left_panel.selected_items();
        if selected.is_empty() {
            let toast = adw::Toast::new(&gettext("No files selected for upload"));
            imp.toast_overlay.add_toast(toast);
            return;
        }

        let items: Vec<(String, bool)> = selected
            .iter()
            .map(|i| (i.name(), i.is_dir()))
            .collect();

        self.enqueue_items(
            items,
            imp.left_panel.current_path(),
            imp.right_panel.remote_path(),
            TransferDirection::Upload,
        );
    }

    fn on_download_clicked(&self) {
        if self.get_connection().is_none() {
            return;
        }

        let imp = self.imp();
        let selected = imp.right_panel.selected_items();
        if selected.is_empty() {
            let toast = adw::Toast::new(&gettext("No files selected for download"));
            imp.toast_overlay.add_toast(toast);
            return;
        }

        let items: Vec<(String, bool)> = selected
            .iter()
            .map(|i| (i.name(), i.is_dir()))
            .collect();

        self.enqueue_items(
            items,
            imp.left_panel.current_path(),
            imp.right_panel.remote_path(),
            TransferDirection::Download,
        );
    }

    fn on_rename_local(&self) {
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

    fn on_rename_remote(&self) {
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

    fn on_delete_local(&self) {
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

    fn on_delete_remote(&self) {
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

    fn setup_drag_and_drop(&self) {
        let imp = self.imp();
        Self::attach_drag_source(&imp.left_panel, "left");
        Self::attach_drag_source(&imp.right_panel, "right");
        self.attach_drop_target(&imp.left_panel, "left");
        self.attach_drop_target(&imp.right_panel, "right");
    }

    fn attach_drag_source(panel: &crate::file_panel::FilePanel, panel_id: &str) {
        let drag_source = gtk::DragSource::new();
        drag_source.set_actions(gdk::DragAction::COPY);
        drag_source.set_propagation_phase(gtk::PropagationPhase::Capture);

        let panel_id = panel_id.to_string();
        let panel_weak = panel.downgrade();
        drag_source.connect_prepare(move |_source, _x, _y| {
            let panel = panel_weak.upgrade()?;
            let items: Vec<(String, bool)> = panel
                .selected_items()
                .iter()
                .map(|i| (i.name(), i.is_dir()))
                .collect();
            if items.is_empty() {
                return None;
            }
            let payload = DndPayload {
                source: panel_id.clone(),
                items,
            };
            let json = serde_json::to_string(&payload).ok()?;
            Some(gdk::ContentProvider::for_value(&json.to_value()))
        });

        panel.column_view().add_controller(drag_source);
    }

    fn attach_drop_target(&self, panel: &crate::file_panel::FilePanel, panel_id: &str) {
        let drop_target = gtk::DropTarget::new(String::static_type(), gdk::DragAction::COPY);

        let panel_weak = panel.downgrade();
        drop_target.connect_enter(move |_target, _x, _y| {
            if let Some(p) = panel_weak.upgrade() {
                p.column_view().add_css_class("drop-target-highlight");
            }
            gdk::DragAction::COPY
        });

        let panel_weak = panel.downgrade();
        drop_target.connect_leave(move |_target| {
            if let Some(p) = panel_weak.upgrade() {
                p.column_view().remove_css_class("drop-target-highlight");
            }
        });

        let this_panel = panel_id.to_string();
        let window_weak = self.downgrade();
        let panel_weak = panel.downgrade();
        drop_target.connect_drop(move |_target, value, _x, _y| {
            if let Some(p) = panel_weak.upgrade() {
                p.column_view().remove_css_class("drop-target-highlight");
            }
            let json = match value.get::<String>() {
                Ok(s) => s,
                Err(_) => return false,
            };
            let payload: DndPayload = match serde_json::from_str(&json) {
                Ok(p) => p,
                Err(_) => return false,
            };
            if payload.source == this_panel {
                return false;
            }
            let Some(window) = window_weak.upgrade() else {
                return false;
            };
            if window.get_connection().is_none() {
                let toast = adw::Toast::new(&gettext("Connect to a server first"));
                window.imp().toast_overlay.add_toast(toast);
                return false;
            }
            let direction = if payload.source == "left" {
                TransferDirection::Upload
            } else {
                TransferDirection::Download
            };
            window.enqueue_items(
                payload.items,
                window.imp().left_panel.current_path(),
                window.imp().right_panel.remote_path(),
                direction,
            );
            true
        });

        panel.column_view().add_controller(drop_target);
    }

    fn setup_context_menus(&self) {
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

        self.add_action(&upload_action);
        self.add_action(&download_action);
        self.add_action(&rename_local_action);
        self.add_action(&rename_remote_action);
        self.add_action(&delete_local_action);
        self.add_action(&delete_remote_action);

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
                // Rename only enabled for single selection
                if let Some(action) = window.lookup_action("rename-remote") {
                    if let Some(action) = action.downcast_ref::<gio::SimpleAction>() {
                        action.set_enabled(panel.selected_items().len() == 1);
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
                    window
                        .imp()
                        .connect_button
                        .set_icon_name("network-offline-symbolic");
                    window
                        .imp()
                        .connect_button
                        .set_tooltip_text(Some(&gettext("Disconnect")));
                    window.set_transfer_buttons_sensitive(true);

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
