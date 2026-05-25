use std::cell::RefCell;
use std::rc::Rc;

use gtk4::gdk::Rectangle;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Label, ListBox, ListBoxRow, Orientation, Popover, ScrolledWindow,
    SelectionMode,
};

use crate::lsp::CompletionItem;

#[derive(Clone)]
pub struct LspPopup {
    popover: Popover,
    list_box: ListBox,
    items: Rc<RefCell<Vec<CompletionItem>>>,
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

        let scroll = ScrolledWindow::new();
        scroll.set_child(Some(&list_box));
        scroll.set_min_content_width(320);
        scroll.set_min_content_height(60);
        scroll.set_max_content_height(280);
        scroll.set_propagate_natural_height(true);

        let outer = GtkBox::new(Orientation::Vertical, 0);
        outer.set_margin_top(2);
        outer.set_margin_bottom(2);
        outer.append(&scroll);
        popover.set_child(Some(&outer));

        let items: Rc<RefCell<Vec<CompletionItem>>> = Rc::new(RefCell::new(Vec::new()));
        let on_complete: Rc<RefCell<Option<Box<dyn Fn(CompletionItem)>>>> =
            Rc::new(RefCell::new(None));

        Self { popover, list_box, items, on_complete }
    }

    pub fn set_on_complete(&self, f: impl Fn(CompletionItem) + 'static) {
        *self.on_complete.borrow_mut() = Some(Box::new(f));
    }

    /// Replace the popup contents with new items and show at position (x, y)
    /// relative to the parent widget.
    pub fn show_items(&self, new_items: Vec<CompletionItem>, x: i32, y: i32) {
        self.clear_rows();

        if new_items.is_empty() {
            if self.popover.is_visible() {
                self.popover.popdown();
            }
            *self.items.borrow_mut() = Vec::new();
            return;
        }

        for item in &new_items {
            self.append_row(item);
        }
        *self.items.borrow_mut() = new_items;

        // Select the first row
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

    pub fn selected_item(&self) -> Option<CompletionItem> {
        let row = self.list_box.selected_row()?;
        let idx = row.index() as usize;
        self.items.borrow().get(idx).cloned()
    }

    pub fn first_item(&self) -> Option<CompletionItem> {
        self.items.borrow().first().cloned()
    }

    pub fn move_selection(&self, delta: i32) {
        let current = self
            .list_box
            .selected_row()
            .map(|r| r.index())
            .unwrap_or(0);
        let next = (current + delta).max(0);
        if let Some(row) = self.list_box.row_at_index(next) {
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
        kind_lbl.set_width_chars(5);
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
            detail_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            text_col.append(&detail_lbl);
        }

        row_box.append(&text_col);
        row.set_child(Some(&row_box));

        let on_complete = self.on_complete.clone();
        let item_clone = item.clone();
        row.connect_activate(move |_| {
            if let Some(f) = on_complete.borrow().as_ref() {
                f(item_clone.clone());
            }
        });

        self.list_box.append(&row);
    }
}

fn kind_label(kind: u8) -> &'static str {
    match kind {
        2 => "mthd",
        3 => "fn",
        4 => "ctor",
        5 | 10 => "prop",
        6 => "var",
        7 | 8 => "type",
        9 => "mod",
        12 => "val",
        13 => "enum",
        14 => "kw",
        15 => "snip",
        _ => "·",
    }
}
