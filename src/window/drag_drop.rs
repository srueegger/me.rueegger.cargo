use gtk::{gdk, prelude::*, subclass::prelude::*};
use libadwaita as adw;

use gettextrs::gettext;

use super::{CargoWindow, DndPayload, TransferDirection};

impl CargoWindow {
    pub(crate) fn setup_drag_and_drop(&self) {
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
}
