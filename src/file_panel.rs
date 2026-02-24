// Cargo - A dual-pane file transfer application for GNOME
// Copyright (C) 2026 Samuel Rüegger
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gettextrs::{gettext, ngettext};
use gtk::{gio, glib, prelude::*, subclass::prelude::*, CompositeTemplate};

use crate::config::APP_ID;
use crate::connection::ConnectionHandle;
use crate::file_item::FileItem;

/// Panel operating mode.
pub(crate) enum PanelMode {
    Local,
    Remote(Rc<ConnectionHandle>),
}

/// Events emitted when a panel navigates, used for synchronized browsing.
pub enum SyncEvent {
    /// User navigated into a subdirectory by name.
    EnterDir(String),
    /// User navigated to the parent directory.
    Up,
}

/// Callback type for sync navigation events.
pub type SyncCallback = Box<dyn Fn(SyncEvent)>;

impl Default for PanelMode {
    fn default() -> Self {
        Self::Local
    }
}

mod imp {
    use super::*;
    use std::cell::OnceCell;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/me/rueegger/cargo/ui/file_panel.ui")]
    pub struct FilePanel {
        #[template_child]
        pub path_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub up_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub column_view: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub name_column: TemplateChild<gtk::ColumnViewColumn>,
        #[template_child]
        pub size_column: TemplateChild<gtk::ColumnViewColumn>,
        #[template_child]
        pub modified_column: TemplateChild<gtk::ColumnViewColumn>,
        #[template_child]
        pub permissions_column: TemplateChild<gtk::ColumnViewColumn>,
        #[template_child]
        pub status_label: TemplateChild<gtk::Label>,

        pub current_path: RefCell<PathBuf>,
        pub list_store: OnceCell<gio::ListStore>,
        pub settings: OnceCell<gio::Settings>,

        // Remote mode state
        pub mode: RefCell<PanelMode>,
        pub remote_path: RefCell<String>,

        // Sync navigation callback
        pub sync_callback: RefCell<Option<super::SyncCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FilePanel {
        const NAME: &'static str = "CargoFilePanel";
        type Type = super::FilePanel;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            FileItem::ensure_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for FilePanel {
        fn constructed(&self) {
            self.parent_constructed();

            self.list_store
                .set(gio::ListStore::new::<FileItem>())
                .unwrap();
            self.settings.set(gio::Settings::new(APP_ID)).unwrap();
            *self.current_path.borrow_mut() = glib::home_dir();

            let obj = self.obj();
            obj.setup_column_factories();
            obj.setup_sorting();
            obj.setup_selection_model();
            obj.setup_signals();
            obj.navigate_to(glib::home_dir());
        }
    }

    impl WidgetImpl for FilePanel {}
    impl BoxImpl for FilePanel {}
}

glib::wrapper! {
    pub struct FilePanel(ObjectSubclass<imp::FilePanel>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Orientable;
}

impl FilePanel {
    // --- Setup ---

    fn setup_column_factories(&self) {
        let imp = self.imp();

        // Name column: icon + label
        let name_factory = gtk::SignalListItemFactory::new();
        name_factory.connect_setup(|_, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            let bx = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            bx.set_margin_start(4);
            let icon = gtk::Image::new();
            let label = gtk::Label::new(None);
            label.set_xalign(0.0);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_hexpand(true);
            bx.append(&icon);
            bx.append(&label);
            list_item.set_child(Some(&bx));
        });
        name_factory.connect_bind(|_, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            let item: FileItem = list_item.item().and_downcast().unwrap();
            let bx: gtk::Box = list_item.child().and_downcast().unwrap();
            let icon = bx.first_child().and_downcast::<gtk::Image>().unwrap();
            let label = icon
                .next_sibling()
                .and_downcast::<gtk::Label>()
                .unwrap();
            icon.set_icon_name(Some(&item.icon_name()));
            label.set_label(&item.name());
            if item.is_dir() {
                label.add_css_class("accent");
            } else {
                label.remove_css_class("accent");
            }
        });
        name_factory.connect_unbind(|_, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            if let Some(bx) = list_item.child().and_downcast::<gtk::Box>() {
                if let Some(label) = bx
                    .first_child()
                    .and_then(|c| c.next_sibling())
                    .and_downcast::<gtk::Label>()
                {
                    label.remove_css_class("accent");
                }
            }
        });
        imp.name_column.set_factory(Some(&name_factory));

        // Size column
        Self::setup_label_column(&imp.size_column, |item: &FileItem| {
            item.display_size()
        });

        // Modified column
        Self::setup_label_column(&imp.modified_column, |item: &FileItem| {
            item.display_modified()
        });

        // Permissions column
        Self::setup_label_column(&imp.permissions_column, |item: &FileItem| {
            item.permissions()
        });
    }

    fn setup_label_column(
        column: &gtk::ColumnViewColumn,
        get_text: fn(&FileItem) -> String,
    ) {
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            let label = gtk::Label::new(None);
            label.set_xalign(0.0);
            label.set_margin_start(4);
            label.set_margin_end(4);
            list_item.set_child(Some(&label));
        });
        let get_text_fn = get_text;
        factory.connect_bind(move |_, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            let item: FileItem = list_item.item().and_downcast().unwrap();
            let label: gtk::Label = list_item.child().and_downcast().unwrap();
            label.set_label(&get_text_fn(&item));
        });
        column.set_factory(Some(&factory));
    }

    fn setup_sorting(&self) {
        let imp = self.imp();

        // Name: directories first, then case-insensitive alphabetical
        let name_sorter = gtk::CustomSorter::new(move |a, b| {
            let a = a.downcast_ref::<FileItem>().unwrap();
            let b = b.downcast_ref::<FileItem>().unwrap();
            match (a.is_dir(), b.is_dir()) {
                (true, false) => gtk::Ordering::Smaller,
                (false, true) => gtk::Ordering::Larger,
                _ => a
                    .name()
                    .to_lowercase()
                    .cmp(&b.name().to_lowercase())
                    .into(),
            }
        });
        imp.name_column.set_sorter(Some(&name_sorter));

        // Size: numeric
        let size_sorter = gtk::CustomSorter::new(move |a, b| {
            let a = a.downcast_ref::<FileItem>().unwrap();
            let b = b.downcast_ref::<FileItem>().unwrap();
            a.size().cmp(&b.size()).into()
        });
        imp.size_column.set_sorter(Some(&size_sorter));

        // Modified: numeric timestamp
        let modified_sorter = gtk::CustomSorter::new(move |a, b| {
            let a = a.downcast_ref::<FileItem>().unwrap();
            let b = b.downcast_ref::<FileItem>().unwrap();
            a.modified().cmp(&b.modified()).into()
        });
        imp.modified_column.set_sorter(Some(&modified_sorter));

        // Permissions: string
        let perms_sorter = gtk::CustomSorter::new(move |a, b| {
            let a = a.downcast_ref::<FileItem>().unwrap();
            let b = b.downcast_ref::<FileItem>().unwrap();
            a.permissions().cmp(&b.permissions()).into()
        });
        imp.permissions_column.set_sorter(Some(&perms_sorter));
    }

    fn setup_selection_model(&self) {
        let imp = self.imp();
        let store = imp.list_store.get().unwrap().clone();

        let sorter = imp.column_view.sorter();
        let sort_model = gtk::SortListModel::new(Some(store), sorter);
        let selection = gtk::MultiSelection::new(Some(sort_model));
        imp.column_view.set_model(Some(&selection));

        // Default sort: Name ascending
        imp.column_view
            .sort_by_column(Some(&*imp.name_column), gtk::SortType::Ascending);
    }

    fn setup_signals(&self) {
        let imp = self.imp();

        // Double-click / Enter on row → navigate into directory
        imp.column_view.connect_activate(glib::clone!(
            #[weak(rename_to = panel)]
            self,
            move |_view, position| {
                panel.activate_row(position);
            }
        ));

        // Path entry Enter → navigate to typed path
        imp.path_entry.connect_activate(glib::clone!(
            #[weak(rename_to = panel)]
            self,
            move |entry| {
                let text = entry.text();
                if panel.is_remote() {
                    panel.navigate_to_remote(text.as_str());
                } else {
                    let path = PathBuf::from(text.as_str());
                    if path.is_dir() {
                        panel.navigate_to(path);
                    }
                }
            }
        ));

        // Up button
        imp.up_button.connect_clicked(glib::clone!(
            #[weak(rename_to = panel)]
            self,
            move |_| {
                panel.navigate_up();
            }
        ));

        // Keyboard: Backspace → go up
        let key_controller = gtk::EventControllerKey::new();
        key_controller.connect_key_pressed(glib::clone!(
            #[weak(rename_to = panel)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, _| {
                if key == gtk::gdk::Key::BackSpace {
                    panel.navigate_up();
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            }
        ));
        self.add_controller(key_controller);

        // GSettings: show-hidden-files changed → reload
        let settings = imp.settings.get().unwrap();
        settings.connect_changed(
            Some("show-hidden-files"),
            glib::clone!(
                #[weak(rename_to = panel)]
                self,
                move |_, _| {
                    panel.reload();
                }
            ),
        );
    }

    // --- Sync navigation ---

    pub fn set_sync_callback(&self, cb: SyncCallback) {
        *self.imp().sync_callback.borrow_mut() = Some(cb);
    }

    fn emit_sync(&self, event: SyncEvent) {
        if let Some(cb) = self.imp().sync_callback.borrow().as_ref() {
            cb(event);
        }
    }

    // --- Mode switching ---

    /// Switch this panel to remote mode with the given connection.
    /// If `initial_path` is provided, navigate there instead of the connection default.
    pub fn set_remote_mode(&self, connection: Rc<ConnectionHandle>, initial_path: Option<&str>) {
        let dir = initial_path
            .map(|s| s.to_string())
            .unwrap_or_else(|| connection.current_dir());
        *self.imp().mode.borrow_mut() = PanelMode::Remote(connection);
        self.navigate_to_remote(&dir);
    }

    /// Switch this panel back to local mode and disconnect.
    pub fn set_local_mode(&self) {
        let mode = self.imp().mode.replace(PanelMode::Local);
        if let PanelMode::Remote(conn) = mode {
            glib::spawn_future_local(async move {
                let _ = conn.disconnect().await;
            });
        }
        self.navigate_to(glib::home_dir());
    }

    /// Check if panel is in remote mode.
    pub fn is_remote(&self) -> bool {
        matches!(&*self.imp().mode.borrow(), PanelMode::Remote(_))
    }

    // --- Navigation ---

    pub fn navigate_to(&self, path: PathBuf) {
        *self.imp().current_path.borrow_mut() = path.clone();
        self.imp()
            .path_entry
            .set_text(&path.display().to_string());
        self.load_directory(&path);
    }

    pub fn navigate_to_remote(&self, path: &str) {
        *self.imp().remote_path.borrow_mut() = path.to_string();
        self.imp().path_entry.set_text(path);
        self.load_remote_directory(path);
    }

    pub fn navigate_up(&self) {
        let navigated = match &*self.imp().mode.borrow() {
            PanelMode::Local => {
                let current = self.imp().current_path.borrow().clone();
                if let Some(parent) = current.parent() {
                    self.navigate_to(parent.to_path_buf());
                    true
                } else {
                    false
                }
            }
            PanelMode::Remote(_) => {
                let current = self.imp().remote_path.borrow().clone();
                if current != "/" {
                    let parent = current
                        .trim_end_matches('/')
                        .rsplit_once('/')
                        .map(|(p, _)| if p.is_empty() { "/" } else { p })
                        .unwrap_or("/");
                    self.navigate_to_remote(parent);
                    true
                } else {
                    false
                }
            }
        };
        if navigated {
            self.emit_sync(SyncEvent::Up);
        }
    }

    pub fn reload(&self) {
        match &*self.imp().mode.borrow() {
            PanelMode::Local => {
                let current = self.imp().current_path.borrow().clone();
                self.load_directory(&current);
            }
            PanelMode::Remote(_) => {
                let current = self.imp().remote_path.borrow().clone();
                self.load_remote_directory(&current);
            }
        }
    }

    pub fn current_path(&self) -> PathBuf {
        self.imp().current_path.borrow().clone()
    }

    pub fn remote_path(&self) -> String {
        self.imp().remote_path.borrow().clone()
    }

    /// Returns all currently selected FileItems.
    pub fn selected_items(&self) -> Vec<FileItem> {
        let model = self.imp().column_view.model().unwrap();
        let selection = model.downcast_ref::<gtk::MultiSelection>().unwrap();
        let mut items = Vec::new();
        let n = selection.n_items();
        for i in 0..n {
            if selection.is_selected(i) {
                if let Some(obj) = selection.item(i) {
                    if let Ok(item) = obj.downcast::<FileItem>() {
                        items.push(item);
                    }
                }
            }
        }
        items
    }

    fn activate_row(&self, position: u32) {
        let model = self.imp().column_view.model().unwrap();
        if let Some(obj) = model.item(position) {
            let item: FileItem = obj.downcast().unwrap();
            if item.is_dir() {
                let dir_name = item.name();
                match &*self.imp().mode.borrow() {
                    PanelMode::Local => {
                        let new_path = self.imp().current_path.borrow().join(&dir_name);
                        self.navigate_to(new_path);
                    }
                    PanelMode::Remote(_) => {
                        let current = self.imp().remote_path.borrow().clone();
                        let new_path = if current.ends_with('/') {
                            format!("{}{}", current, dir_name)
                        } else {
                            format!("{}/{}", current, dir_name)
                        };
                        self.navigate_to_remote(&new_path);
                    }
                }
                self.emit_sync(SyncEvent::EnterDir(dir_name));
            }
        }
    }

    // --- Directory reading (local) ---

    fn load_directory(&self, path: &std::path::Path) {
        let imp = self.imp();
        let store = imp.list_store.get().unwrap();
        store.remove_all();

        let show_hidden = imp.settings.get().unwrap().boolean("show-hidden-files");

        match std::fs::read_dir(path) {
            Ok(entries) => {
                let mut items: Vec<FileItem> = Vec::new();

                for entry in entries.flatten() {
                    let file_name = entry.file_name().to_string_lossy().to_string();

                    if !show_hidden && file_name.starts_with('.') {
                        continue;
                    }

                    let metadata = entry.metadata().ok();
                    let is_dir = metadata.as_ref().map_or(false, |m| m.is_dir());
                    let size = metadata.as_ref().map_or(0, |m| m.len());
                    let modified = metadata
                        .as_ref()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map_or(0, |d| d.as_secs() as i64);
                    let permissions = Self::format_permissions(&metadata);

                    items.push(FileItem::new(
                        &file_name,
                        is_dir,
                        size,
                        modified,
                        &permissions,
                    ));
                }

                let objects: Vec<glib::Object> = items
                    .into_iter()
                    .map(|i| i.upcast::<glib::Object>())
                    .collect();
                store.splice(0, 0, &objects);

                let n = store.n_items();
                imp.status_label.set_label(
                    &ngettext("%u item", "%u items", n)
                        .replace("%u", &n.to_string()),
                );
            }
            Err(e) => {
                imp.status_label.set_label(
                    &gettext("Error: %s").replace("%s", &e.to_string()),
                );
            }
        }
    }

    // --- Directory reading (remote) ---

    fn load_remote_directory(&self, path: &str) {
        let imp = self.imp();
        let store = imp.list_store.get().unwrap();
        store.remove_all();
        imp.status_label.set_label(&gettext("Loading…"));

        let connection = match &*imp.mode.borrow() {
            PanelMode::Remote(conn) => conn.clone(),
            PanelMode::Local => return,
        };

        let show_hidden = imp.settings.get().unwrap().boolean("show-hidden-files");
        let path = path.to_string();
        let panel_weak = self.downgrade();

        glib::spawn_future_local(async move {
            let result = connection.list_dir(&path).await;

            let Some(panel) = panel_weak.upgrade() else {
                return;
            };
            let store = panel.imp().list_store.get().unwrap();

            match result {
                Ok(entries) => {
                    let items: Vec<FileItem> = entries
                        .into_iter()
                        .filter(|e| show_hidden || !e.name.starts_with('.'))
                        .map(|e| {
                            FileItem::new(
                                &e.name,
                                e.is_dir,
                                e.size,
                                e.modified.unwrap_or(0),
                                e.permissions.as_deref().unwrap_or(""),
                            )
                        })
                        .collect();

                    let objects: Vec<glib::Object> = items
                        .into_iter()
                        .map(|i| i.upcast::<glib::Object>())
                        .collect();
                    store.splice(0, 0, &objects);

                    let n = store.n_items();
                    panel.imp().status_label.set_label(
                        &ngettext("%u item", "%u items", n)
                            .replace("%u", &n.to_string()),
                    );
                }
                Err(e) => {
                    panel.imp().status_label.set_label(
                        &gettext("Error: %s").replace("%s", &e.to_string()),
                    );
                }
            }
        });
    }

    fn format_permissions(metadata: &Option<std::fs::Metadata>) -> String {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            metadata
                .as_ref()
                .map(|m| format!("{:o}", m.permissions().mode() & 0o7777))
                .unwrap_or_default()
        }
        #[cfg(not(unix))]
        {
            let _ = metadata;
            String::new()
        }
    }
}
