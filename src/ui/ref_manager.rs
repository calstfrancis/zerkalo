use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Entry, Label, ListBox, ListBoxRow, Orientation,
    ScrolledWindow, SelectionMode, Separator,
};

use crate::bibliography::BibEntry;

type InsertCb = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;

#[derive(Clone)]
pub struct RefManager {
    widget: GtkBox,
    list_box: ListBox,
    filter_entry: Entry,
    entries: Rc<RefCell<Vec<BibEntry>>>,
    on_insert: InsertCb,
}

impl RefManager {
    pub fn new() -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 0);

        let header = GtkBox::new(Orientation::Horizontal, 0);
        header.set_margin_start(10);
        header.set_margin_end(10);
        header.set_margin_top(6);
        header.set_margin_bottom(6);
        let title = Label::new(Some("References"));
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.add_css_class("heading");
        header.append(&title);

        widget.append(&Separator::new(Orientation::Horizontal));
        widget.append(&header);
        widget.append(&Separator::new(Orientation::Horizontal));

        let filter_entry = Entry::new();
        filter_entry.set_placeholder_text(Some("Filter references…"));
        filter_entry.set_margin_start(8);
        filter_entry.set_margin_end(8);
        filter_entry.set_margin_top(6);
        filter_entry.set_margin_bottom(6);
        widget.append(&filter_entry);
        widget.append(&Separator::new(Orientation::Horizontal));

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::None);
        list_box.add_css_class("navigation-sidebar");
        scroll.set_child(Some(&list_box));
        widget.append(&scroll);

        let on_insert: InsertCb = Rc::new(RefCell::new(None));
        let entries: Rc<RefCell<Vec<BibEntry>>> = Rc::new(RefCell::new(Vec::new()));

        let panel = Self { widget, list_box, filter_entry, entries, on_insert };

        let p = panel.clone();
        panel.filter_entry.connect_changed(move |e| p.rebuild_list(e.text().as_str()));

        panel
    }

    pub fn widget(&self) -> &GtkBox {
        &self.widget
    }

    pub fn set_on_insert(&self, f: impl Fn(String) + 'static) {
        *self.on_insert.borrow_mut() = Some(Box::new(f));
    }

    pub fn load_bib(&self, path: &Path) {
        let entries = crate::bibliography::load_bib(path);
        *self.entries.borrow_mut() = entries;
        self.rebuild_list("");
    }

    pub fn clear_entries(&self) {
        self.entries.borrow_mut().clear();
        self.rebuild_list("");
    }

    fn rebuild_list(&self, filter: &str) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        let filter_lower = filter.to_lowercase();
        let entries = self.entries.borrow();

        if entries.is_empty() {
            let row = ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            let lbl = Label::new(Some("No bibliography loaded.\nSet a .bib file in Settings."));
            lbl.add_css_class("dim-label");
            lbl.set_justify(gtk4::Justification::Center);
            lbl.set_margin_top(16);
            lbl.set_margin_bottom(16);
            row.set_child(Some(&lbl));
            self.list_box.append(&row);
            return;
        }

        let mut shown = 0usize;
        for entry in entries.iter() {
            if !filter_lower.is_empty() {
                let haystack = format!(
                    "{} {} {} {}",
                    entry.key, entry.author, entry.title, entry.year
                )
                .to_lowercase();
                if !haystack.contains(&filter_lower) {
                    continue;
                }
            }

            let row = ListBoxRow::new();
            row.set_activatable(true);
            row.set_tooltip_text(Some(&format!("Click to insert @{}", entry.key)));

            let box_ = GtkBox::new(Orientation::Vertical, 2);
            box_.set_margin_start(8);
            box_.set_margin_end(8);
            box_.set_margin_top(5);
            box_.set_margin_bottom(5);

            let top = GtkBox::new(Orientation::Horizontal, 6);
            let key_lbl = Label::new(Some(&format!("@{}", entry.key)));
            key_lbl.add_css_class("caption");
            key_lbl.set_xalign(0.0);
            let year_lbl = Label::new(Some(&entry.year));
            year_lbl.add_css_class("dim-label");
            year_lbl.add_css_class("caption");
            year_lbl.set_hexpand(true);
            year_lbl.set_xalign(1.0);
            top.append(&key_lbl);
            top.append(&year_lbl);

            let title_lbl = Label::new(Some(if entry.title.is_empty() {
                "(no title)"
            } else {
                &entry.title
            }));
            title_lbl.set_xalign(0.0);
            title_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

            box_.append(&top);
            box_.append(&title_lbl);

            if !entry.author.is_empty() {
                let author_lbl = Label::new(Some(&entry.author));
                author_lbl.add_css_class("dim-label");
                author_lbl.add_css_class("caption");
                author_lbl.set_xalign(0.0);
                author_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                box_.append(&author_lbl);
            }

            row.set_child(Some(&box_));

            let cb = self.on_insert.clone();
            let key = entry.key.clone();
            row.connect_activate(move |_| {
                if let Some(f) = cb.borrow().as_ref() {
                    f(format!("@{}", key));
                }
            });

            self.list_box.append(&row);
            shown += 1;
        }

        if shown == 0 && !entries.is_empty() {
            let row = ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            let lbl = Label::new(Some("No matching references"));
            lbl.add_css_class("dim-label");
            lbl.set_margin_top(16);
            lbl.set_margin_bottom(16);
            row.set_child(Some(&lbl));
            self.list_box.append(&row);
        }
    }
}
