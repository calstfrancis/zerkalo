use std::cell::RefCell;
use std::rc::Rc;

use gtk4::gdk::Rectangle;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, EventControllerKey, Label, ListBox, ListBoxRow, Orientation, Popover,
    ScrolledWindow, SelectionMode,
};

use crate::bibliography::BibEntry;

#[derive(Clone)]
pub struct BibPopup {
    popover: Popover,
    list_box: ListBox,
    entries: Rc<RefCell<Vec<BibEntry>>>,
    on_complete: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    filtered_keys: Rc<RefCell<Vec<String>>>,
}

impl BibPopup {
    pub fn new(parent: &impl IsA<gtk4::Widget>, entries: Rc<RefCell<Vec<BibEntry>>>) -> Self {
        let popover = Popover::new();
        popover.set_has_arrow(false);
        popover.set_autohide(false);
        popover.set_parent(parent);

        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::Browse);
        list_box.set_focusable(true);

        let scroll = ScrolledWindow::new();
        scroll.set_child(Some(&list_box));
        scroll.set_min_content_width(300);
        scroll.set_min_content_height(60);
        scroll.set_max_content_height(280);
        scroll.set_propagate_natural_height(true);

        let outer = GtkBox::new(Orientation::Vertical, 0);
        outer.set_margin_top(2);
        outer.set_margin_bottom(2);
        outer.append(&scroll);
        popover.set_child(Some(&outer));

        let on_complete: Rc<RefCell<Option<Box<dyn Fn(String)>>>> =
            Rc::new(RefCell::new(None));
        let filtered_keys: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

        // Key controller on the list_box so Tab/Return work when popup has focus
        {
            let on_complete_kc = on_complete.clone();
            let filtered_kc = filtered_keys.clone();
            let list_kc = list_box.clone();
            let popover_kc = popover.clone();

            let kc = EventControllerKey::new();
            kc.connect_key_pressed(move |_, key, _, _mods| {
                use gtk4::gdk::Key;
                match key {
                    Key::Tab | Key::Return | Key::KP_Enter => {
                        let idx = list_kc.selected_row()
                            .map(|r| r.index() as usize)
                            .unwrap_or(0);
                        let k = filtered_kc.borrow().get(idx).cloned()
                            .or_else(|| filtered_kc.borrow().first().cloned());
                        if let Some(k) = k {
                            popover_kc.popdown();
                            if let Some(f) = on_complete_kc.borrow().as_ref() {
                                f(k);
                            }
                        }
                        glib::Propagation::Stop
                    }
                    Key::Escape => {
                        popover_kc.popdown();
                        glib::Propagation::Stop
                    }
                    Key::Down => {
                        let cur = list_kc.selected_row().map(|r| r.index()).unwrap_or(-1);
                        if let Some(row) = list_kc.row_at_index(cur + 1) {
                            list_kc.select_row(Some(&row));
                        }
                        glib::Propagation::Stop
                    }
                    Key::Up => {
                        let cur = list_kc.selected_row().map(|r| r.index()).unwrap_or(1);
                        if let Some(row) = list_kc.row_at_index((cur - 1).max(0)) {
                            list_kc.select_row(Some(&row));
                        }
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                }
            });
            list_box.add_controller(kc);
        }

        Self {
            popover,
            list_box,
            entries,
            on_complete,
            filtered_keys,
        }
    }

    pub fn set_on_complete(&self, f: impl Fn(String) + 'static) {
        *self.on_complete.borrow_mut() = Some(Box::new(f));
    }

    pub fn show_filtered(&self, query: &str, x: i32, y: i32) {
        self.clear_rows();
        self.filtered_keys.borrow_mut().clear();

        let entries = self.entries.borrow();
        let q = query.to_lowercase();

        let mut matched: Vec<&BibEntry> = entries
            .iter()
            .filter(|e| {
                let k = e.key.to_lowercase();
                q.is_empty() || k.contains(&q)
            })
            .collect();

        matched.sort_by_key(|e| {
            if e.key.to_lowercase().starts_with(&q) { 0u8 } else { 1u8 }
        });

        let shown: Vec<&BibEntry> = matched;

        if shown.is_empty() {
            if self.popover.is_visible() {
                self.popover.popdown();
            }
            return;
        }

        for entry in &shown {
            self.filtered_keys.borrow_mut().push(entry.key.clone());
            self.append_row(entry);
        }

        if let Some(row) = self.list_box.row_at_index(0) {
            self.list_box.select_row(Some(&row));
        }

        self.popover.set_pointing_to(Some(&Rectangle::new(x, y, 1, 1)));

        if !self.popover.is_visible() {
            self.popover.popup();
        }
    }

    pub fn hide(&self) {
        if self.popover.is_visible() {
            self.popover.popdown();
        }
    }

    pub fn is_visible(&self) -> bool {
        self.popover.is_visible()
    }

    pub fn first_filtered_key(&self) -> Option<String> {
        self.filtered_keys.borrow().first().cloned()
    }

    pub fn move_selection(&self, delta: i32) {
        let current_idx = self.list_box.selected_row().map(|r| r.index()).unwrap_or(0);
        let next_idx = (current_idx + delta).max(0);
        if let Some(row) = self.list_box.row_at_index(next_idx) {
            self.list_box.select_row(Some(&row));
        }
    }

    pub fn selected_key(&self) -> Option<String> {
        let row = self.list_box.selected_row()?;
        let idx = row.index() as usize;
        self.filtered_keys.borrow().get(idx).cloned()
    }

    fn clear_rows(&self) {
        while let Some(row) = self.list_box.row_at_index(0) {
            self.list_box.remove(&row);
        }
    }

    fn append_row(&self, entry: &BibEntry) {
        let row = ListBoxRow::new();
        row.set_activatable(true);

        let row_box = GtkBox::new(Orientation::Vertical, 2);
        row_box.set_margin_top(5);
        row_box.set_margin_bottom(5);
        row_box.set_margin_start(10);
        row_box.set_margin_end(10);

        let key_lbl = Label::new(Some(&format!("@{}", entry.key)));
        key_lbl.set_halign(Align::Start);
        key_lbl.set_xalign(0.0);
        key_lbl.add_css_class("caption");
        key_lbl.add_css_class("dim-label");
        row_box.append(&key_lbl);

        if !entry.title.is_empty() {
            let title_lbl = Label::new(Some(&truncate(&entry.title, 50)));
            title_lbl.set_halign(Align::Start);
            title_lbl.set_xalign(0.0);
            title_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            row_box.append(&title_lbl);
        }

        let author_year = match (entry.author.is_empty(), entry.year.is_empty()) {
            (false, false) => format!("{} ({})", truncate(&entry.author, 30), entry.year),
            (false, true) => truncate(&entry.author, 40),
            (true, false) => entry.year.clone(),
            (true, true) if entry.title.is_empty() => entry.entry_type.clone(),
            _ => String::new(),
        };

        if !author_year.is_empty() {
            let detail_lbl = Label::new(Some(&author_year));
            detail_lbl.set_halign(Align::Start);
            detail_lbl.set_xalign(0.0);
            detail_lbl.add_css_class("dim-label");
            detail_lbl.add_css_class("caption");
            row_box.append(&detail_lbl);
        }

        row.set_child(Some(&row_box));

        let on_complete = self.on_complete.clone();
        let key = entry.key.clone();
        let popover = self.popover.clone();
        row.connect_activate(move |_| {
            popover.popdown();
            if let Some(f) = on_complete.borrow().as_ref() {
                f(key.clone());
            }
        });

        self.list_box.append(&row);
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max - 1).collect();
        format!("{t}\u{2026}")
    }
}
