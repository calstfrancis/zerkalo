use std::cell::RefCell;
use std::rc::Rc;

use gtk4::gdk::Rectangle;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Label, ListBox, ListBoxRow, Orientation, Popover, ScrolledWindow,
    SelectionMode,
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

    /// Filter entries by `query` and show at widget-relative position `(x, y)`.
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

        // Keys starting with query sort first
        matched.sort_by_key(|e| {
            if e.key.to_lowercase().starts_with(&q) {
                0u8
            } else {
                1u8
            }
        });

        let shown: Vec<&BibEntry> = matched.into_iter().take(15).collect();

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

        // Select the first row
        if let Some(row) = self.list_box.row_at_index(0) {
            self.list_box.select_row(Some(&row));
        }

        self.popover
            .set_pointing_to(Some(&Rectangle::new(x, y, 1, 1)));

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

    /// Move selection down (positive) or up (negative) by `delta` rows.
    pub fn move_selection(&self, delta: i32) {
        let current_idx = self
            .list_box
            .selected_row()
            .map(|r| r.index())
            .unwrap_or(0);
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
        row_box.append(&key_lbl);

        let detail = if !entry.author.is_empty() && !entry.year.is_empty() {
            format!("{} ({})", truncate(&entry.author, 40), entry.year)
        } else if !entry.title.is_empty() {
            truncate(&entry.title, 52)
        } else {
            entry.entry_type.clone()
        };

        if !detail.is_empty() {
            let detail_lbl = Label::new(Some(&detail));
            detail_lbl.set_halign(Align::Start);
            detail_lbl.set_xalign(0.0);
            detail_lbl.add_css_class("dim-label");
            row_box.append(&detail_lbl);
        }

        row.set_child(Some(&row_box));

        let on_complete = self.on_complete.clone();
        let key = entry.key.clone();
        row.connect_activate(move |_| {
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
