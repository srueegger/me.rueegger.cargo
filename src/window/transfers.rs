use gtk::{glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;

use gettextrs::gettext;

use crate::transfer::item::*;
use crate::transfer::queue::TransferQueue;

use super::{CargoWindow, TransferDirection};

impl CargoWindow {
    pub(crate) fn setup_transfer_buttons(&self) {
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

    pub(crate) fn setup_transfer_queue_ui(&self) {
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

    pub(crate) fn enqueue_items(
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

    pub(crate) fn on_upload_clicked(&self) {
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

    pub(crate) fn on_download_clicked(&self) {
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
}
