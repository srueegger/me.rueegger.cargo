use std::cell::Cell;
use std::rc::Rc;

use gettextrs::gettext;
use gtk::{prelude::*, subclass::prelude::*};

use crate::transfer::item::*;

use super::CargoWindow;

impl CargoWindow {
    pub(crate) fn setup_progress_indicator(&self) {
        let imp = self.imp();
        let store = imp.transfer_queue.get().unwrap().store().clone();

        // Shared state for the drawing function
        let progress_value = Rc::new(Cell::new(0.0f64));

        // --- Circular progress DrawingArea ---
        let drawing_area = gtk::DrawingArea::new();
        drawing_area.set_content_width(16);
        drawing_area.set_content_height(16);

        let pv = progress_value.clone();
        drawing_area.set_draw_func(move |area, cr, width, height| {
            let progress = pv.get();
            let color = area.color();

            let cx = width as f64 / 2.0;
            let cy = height as f64 / 2.0;
            let radius = width.min(height) as f64 / 2.0 - 0.5;

            // Background circle at 25% opacity
            cr.set_source_rgba(
                color.red() as f64,
                color.green() as f64,
                color.blue() as f64,
                color.alpha() as f64 * 0.25,
            );
            let _ = cr.arc(cx, cy, radius, 0.0, 2.0 * std::f64::consts::PI);
            let _ = cr.stroke();

            // Foreground arc from 12 o'clock (top), clockwise
            if progress > 0.0 {
                cr.set_source_rgba(
                    color.red() as f64,
                    color.green() as f64,
                    color.blue() as f64,
                    color.alpha() as f64,
                );
                let start = -std::f64::consts::FRAC_PI_2;
                let end = progress * 2.0 * std::f64::consts::PI + start;
                let _ = cr.arc(cx, cy, radius, start, end);
                let _ = cr.stroke();
            }
        });

        // --- Status label ---
        let progress_label = gtk::Label::new(None);
        progress_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        progress_label.add_css_class("caption");

        // --- Button content ---
        let content_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        content_box.append(&drawing_area);
        content_box.append(&progress_label);

        // --- Popover with transfer list ---
        let popover = gtk::Popover::new();
        let popover_scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .max_content_height(200)
            .propagate_natural_height(true)
            .build();

        let popover_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .margin_start(6)
            .margin_end(6)
            .margin_top(6)
            .margin_bottom(6)
            .build();
        popover_list.add_css_class("boxed-list");

        popover_scrolled.set_child(Some(&popover_list));
        popover.set_child(Some(&popover_scrolled));

        // --- MenuButton ---
        let menu_button = gtk::MenuButton::builder()
            .tooltip_text(gettext("Show File Operations"))
            .popover(&popover)
            .child(&content_box)
            .build();
        menu_button.add_css_class("flat");
        menu_button.set_hexpand(true);

        imp.progress_box.add_css_class("progress-indicator");
        imp.progress_box.append(&menu_button);

        // --- Monitor transfer queue ---
        let revealer = imp.progress_revealer.clone();
        let da = drawing_area.clone();
        let pl = progress_label.clone();
        let pv = progress_value.clone();
        let pop_list = popover_list.clone();

        // Helper: recompute overall progress and update UI
        let update_fn = Rc::new({
            let da = da.clone();
            let pl = pl.clone();
            let pv = pv.clone();
            let store = store.clone();
            let revealer = revealer.clone();
            move || {
                let n = store.n_items();
                if n == 0 {
                    revealer.set_reveal_child(false);
                    return;
                }

                let mut completed_weight = 0.0f64;
                let mut has_active_or_queued = false;

                for i in 0..n {
                    let Some(obj) = store.item(i) else { continue };
                    let item: TransferItem = obj.downcast().unwrap();
                    match item.status() {
                        STATUS_COMPLETED | STATUS_SKIPPED => completed_weight += 1.0,
                        STATUS_ACTIVE => {
                            completed_weight += item.progress();
                            has_active_or_queued = true;
                        }
                        STATUS_QUEUED => {
                            has_active_or_queued = true;
                        }
                        _ => {} // failed
                    }
                }

                let overall = completed_weight / n as f64;
                pv.set(overall);
                da.queue_draw();

                // Label: "done/total"
                let done = completed_weight.floor() as u32;
                pl.set_label(&format!("{}/{}", done, n));

                revealer.set_reveal_child(has_active_or_queued);
            }
        });

        // Rebuild popover rows
        let rebuild_popover = Rc::new({
            let store = store.clone();
            let pop_list = pop_list.clone();
            move || {
                // Remove existing rows
                while let Some(row) = pop_list.row_at_index(0) {
                    pop_list.remove(&row);
                }

                for i in 0..store.n_items() {
                    let Some(obj) = store.item(i) else { continue };
                    let item: TransferItem = obj.downcast().unwrap();

                    let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    row_box.set_margin_start(6);
                    row_box.set_margin_end(6);
                    row_box.set_margin_top(4);
                    row_box.set_margin_bottom(4);

                    let icon = gtk::Image::from_icon_name(item.direction_icon());
                    row_box.append(&icon);

                    let name = gtk::Label::new(Some(&item.filename()));
                    name.set_xalign(0.0);
                    name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                    name.set_hexpand(true);
                    row_box.append(&name);

                    let bar = gtk::ProgressBar::new();
                    bar.set_fraction(item.progress());
                    bar.set_valign(gtk::Align::Center);
                    bar.set_width_request(60);
                    row_box.append(&bar);

                    // Live-update the progress bar
                    let bar_clone = bar.clone();
                    item.connect_notify_local(Some("progress"), move |item, _| {
                        bar_clone.set_fraction(item.progress());
                    });

                    let row = gtk::ListBoxRow::builder()
                        .activatable(false)
                        .selectable(false)
                        .child(&row_box)
                        .build();
                    pop_list.append(&row);
                }
            }
        });

        // Connect to items-changed on the store
        let update_fn_changed = update_fn.clone();
        let rebuild_popover_changed = rebuild_popover.clone();
        store.connect_items_changed(move |_store, _pos, _removed, added| {
            // Connect progress/status notify on new items
            if added > 0 {
                for i in _pos..(_pos + added) {
                    let Some(obj) = _store.item(i) else { continue };
                    let item: TransferItem = obj.downcast().unwrap();

                    let uf = update_fn_changed.clone();
                    item.connect_notify_local(Some("progress"), move |_, _| {
                        uf();
                    });

                    let uf2 = update_fn_changed.clone();
                    item.connect_notify_local(Some("status"), move |_, _| {
                        uf2();
                    });
                }
            }

            update_fn_changed();
            rebuild_popover_changed();
        });
    }
}
