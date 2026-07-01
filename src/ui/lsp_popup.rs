use std::cell::RefCell;
use std::rc::Rc;

use gtk4::gdk::Rectangle;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Label, ListBox, ListBoxRow, Orientation, Popover, PositionType,
    ScrolledWindow, SelectionMode, Separator,
};

use crate::lsp::CompletionItem;

#[derive(Clone)]
pub struct LspPopup {
    popover: Popover,
    list_box: ListBox,
    items: Rc<RefCell<Vec<CompletionItem>>>,
    filter_prefix: Rc<RefCell<String>>,
    on_complete: Rc<RefCell<Option<Box<dyn Fn(CompletionItem)>>>>,
}

impl LspPopup {
    pub fn new(parent: &impl IsA<gtk4::Widget>) -> Self {
        let popover = Popover::new();
        popover.set_has_arrow(false);
        popover.set_autohide(false);
        popover.set_parent(parent);

        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::Browse);
        list_box.set_activate_on_single_click(false);

        let scroll = ScrolledWindow::new();
        scroll.set_child(Some(&list_box));
        scroll.set_min_content_width(480);
        scroll.set_min_content_height(60);
        scroll.set_max_content_height(380);
        scroll.set_propagate_natural_height(true);

        let hint = Label::new(Some("↑ ↓ navigate · double-click or ↵ insert · Esc dismiss"));
        hint.add_css_class("dim-label");
        hint.set_margin_top(4);
        hint.set_margin_bottom(4);
        hint.set_margin_start(10);
        hint.set_margin_end(10);
        hint.set_xalign(0.0);

        let outer = GtkBox::new(Orientation::Vertical, 0);
        outer.set_margin_top(2);
        outer.set_margin_bottom(4);
        outer.append(&scroll);
        outer.append(&Separator::new(Orientation::Horizontal));
        outer.append(&hint);
        popover.set_child(Some(&outer));

        let items: Rc<RefCell<Vec<CompletionItem>>> = Rc::new(RefCell::new(Vec::new()));
        let filter_prefix: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
        let on_complete: Rc<RefCell<Option<Box<dyn Fn(CompletionItem)>>>> =
            Rc::new(RefCell::new(None));

        // Client-side filter: hide rows whose label doesn't start with the current prefix.
        {
            let items_f = items.clone();
            let prefix_f = filter_prefix.clone();
            list_box.set_filter_func(move |row| {
                let prefix = prefix_f.borrow();
                if prefix.is_empty() {
                    return true;
                }
                let idx = row.index() as usize;
                items_f
                    .borrow()
                    .get(idx)
                    .map(|item| item.label.to_lowercase().starts_with(prefix.as_str()))
                    .unwrap_or(false)
            });
        }

        let p = Self { popover, list_box, items, filter_prefix, on_complete };

        // Double-click (or Enter key on the list) triggers completion
        {
            let items2 = p.items.clone();
            let cb2 = p.on_complete.clone();
            p.list_box.connect_row_activated(move |_, row| {
                let idx = row.index() as usize;
                if let Some(item) = items2.borrow().get(idx).cloned() {
                    if let Some(f) = cb2.borrow().as_ref() { f(item); }
                }
            });
        }

        p
    }

    pub fn set_on_complete(&self, f: impl Fn(CompletionItem) + 'static) {
        *self.on_complete.borrow_mut() = Some(Box::new(f));
    }

    /// Replace the popup contents with a new master item list and show at (x, y).
    /// `above`: true = popup sits above the cursor (PositionType::Top), false = below.
    /// Resets any active filter. Call `apply_filter` afterwards to filter the new list.
    pub fn show_items(&self, mut new_items: Vec<CompletionItem>, x: i32, y: i32, above: bool) {
        self.clear_rows();
        *self.filter_prefix.borrow_mut() = String::new();

        if new_items.is_empty() {
            if self.popover.is_visible() {
                self.popover.popdown();
            }
            *self.items.borrow_mut() = Vec::new();
            return;
        }

        new_items.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));

        for item in &new_items {
            self.append_row(item);
        }
        *self.items.borrow_mut() = new_items;

        self.list_box.invalidate_filter();

        if let Some(row) = self.list_box.row_at_index(0) {
            self.list_box.select_row(Some(&row));
        }

        self.popover.set_position(if above { PositionType::Top } else { PositionType::Bottom });
        self.popover.set_pointing_to(Some(&Rectangle::new(x, y, 1, 1)));
        if !self.popover.is_visible() {
            self.popover.popup();
        }
    }

    /// Update the client-side filter prefix and scroll to the first matching row.
    /// Safe to call while the popup is visible; does not rebuild the row list.
    pub fn apply_filter(&self, prefix: &str) {
        let lprefix = prefix.to_lowercase();
        *self.filter_prefix.borrow_mut() = lprefix.clone();
        self.list_box.invalidate_filter();

        // Select the first item that passes the filter
        let first_idx = {
            let items = self.items.borrow();
            if lprefix.is_empty() {
                Some(0usize)
            } else {
                items
                    .iter()
                    .position(|item| item.label.to_lowercase().starts_with(&lprefix))
            }
        };
        if let Some(idx) = first_idx {
            if let Some(row) = self.list_box.row_at_index(idx as i32) {
                self.list_box.select_row(Some(&row));
            }
        }
    }

    /// Merge additional items into the existing master list (dedup by label, re-sort, re-filter).
    /// Used when LSP results arrive after the popup was already shown with local snippets.
    pub fn merge_items(&self, new_items: Vec<CompletionItem>) {
        let any_new = {
            let existing = self.items.borrow();
            new_items.iter().any(|ni| !existing.iter().any(|ei| ei.label == ni.label))
        };
        if !any_new { return; }

        let mut all = self.items.borrow().clone();
        for item in new_items {
            if !all.iter().any(|ei| ei.label == item.label) {
                all.push(item);
            }
        }
        all.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));

        self.clear_rows();
        for item in &all {
            self.append_row(item);
        }
        *self.items.borrow_mut() = all;

        let prefix = self.filter_prefix.borrow().clone();
        self.list_box.invalidate_filter();
        let first_idx = {
            let items = self.items.borrow();
            if prefix.is_empty() { Some(0usize) }
            else { items.iter().position(|item| item.label.to_lowercase().starts_with(&prefix)) }
        };
        if let Some(idx) = first_idx {
            if let Some(row) = self.list_box.row_at_index(idx as i32) {
                self.list_box.select_row(Some(&row));
            }
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

    pub fn selected_item(&self) -> Option<CompletionItem> {
        let row = self.list_box.selected_row()?;
        let idx = row.index() as usize;
        self.items.borrow().get(idx).cloned()
    }

    pub fn first_item(&self) -> Option<CompletionItem> {
        self.items.borrow().first().cloned()
    }

    pub fn move_selection(&self, delta: i32) {
        // Collect only the visible row indices (filter may hide some)
        let mut visible: Vec<i32> = Vec::new();
        let mut i = 0i32;
        while let Some(row) = self.list_box.row_at_index(i) {
            if row.is_visible() {
                visible.push(i);
            }
            i += 1;
        }
        if visible.is_empty() { return; }
        let current_idx = self.list_box.selected_row().map(|r| r.index()).unwrap_or(-1);
        let pos = visible.iter().position(|&idx| idx == current_idx).unwrap_or(0) as i32;
        let next_pos = (pos + delta).clamp(0, visible.len() as i32 - 1) as usize;
        if let Some(row) = self.list_box.row_at_index(visible[next_pos]) {
            self.list_box.select_row(Some(&row));
        }
    }

    fn clear_rows(&self) {
        while let Some(row) = self.list_box.row_at_index(0) {
            self.list_box.remove(&row);
        }
    }

    fn append_row(&self, item: &CompletionItem) {
        let row = ListBoxRow::new();
        row.set_activatable(true);

        let row_box = GtkBox::new(Orientation::Horizontal, 8);
        row_box.set_margin_top(4);
        row_box.set_margin_bottom(4);
        row_box.set_margin_start(10);
        row_box.set_margin_end(10);

        // Kind badge
        let kind_str = kind_label(item.kind);
        let kind_lbl = Label::new(Some(kind_str));
        kind_lbl.add_css_class("dim-label");
        kind_lbl.add_css_class("caption");
        kind_lbl.set_width_chars(12);
        kind_lbl.set_xalign(0.0);
        kind_lbl.set_valign(Align::Center);
        row_box.append(&kind_lbl);

        // Label + detail
        let text_col = GtkBox::new(Orientation::Vertical, 1);

        let label_lbl = Label::new(Some(&item.label));
        label_lbl.set_halign(Align::Start);
        label_lbl.set_xalign(0.0);
        label_lbl.set_hexpand(true);
        text_col.append(&label_lbl);

        if let Some(ref detail) = item.detail {
            let detail_lbl = Label::new(Some(detail));
            detail_lbl.set_halign(Align::Start);
            detail_lbl.set_xalign(0.0);
            detail_lbl.add_css_class("dim-label");
            detail_lbl.set_wrap(true);
            detail_lbl.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
            detail_lbl.set_max_width_chars(60);
            text_col.append(&detail_lbl);
        }

        row_box.append(&text_col);
        row.set_child(Some(&row_box));
        self.list_box.append(&row);
    }
}

fn kind_label(kind: u8) -> &'static str {
    match kind {
        2  => "Method",
        3  => "Function",
        4  => "Constructor",
        5  => "Field",
        6  => "Variable",
        7  => "Class",
        8  => "Interface",
        9  => "Module",
        10 => "Property",
        12 => "Value",
        13 => "Enum",
        14 => "Keyword",
        15 => "Snippet",
        _  => "",
    }
}
