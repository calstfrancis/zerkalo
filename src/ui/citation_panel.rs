use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow,
    SearchEntry, SelectionMode, Separator,
};

use crate::bibliography::BibEntry;

type InsertCb = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;

#[derive(Clone)]
pub struct CitationPanel {
    widget: GtkBox,
    list: ListBox,
    search: SearchEntry,
    entries: Rc<RefCell<Vec<BibEntry>>>,
    on_insert: InsertCb,
}

impl CitationPanel {
    pub fn new() -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 0);

        let header_box = GtkBox::new(Orientation::Horizontal, 0);
        header_box.set_margin_start(10);
        header_box.set_margin_end(10);
        header_box.set_margin_top(6);
        header_box.set_margin_bottom(6);
        let title = Label::new(Some("Citations"));
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.add_css_class("heading");
        header_box.append(&title);
        widget.append(&Separator::new(Orientation::Horizontal));
        widget.append(&header_box);
        widget.append(&Separator::new(Orientation::Horizontal));

        let search = SearchEntry::new();
        search.set_placeholder_text(Some("Search by key, author, title…"));
        search.set_margin_start(8);
        search.set_margin_end(8);
        search.set_margin_top(6);
        search.set_margin_bottom(6);
        search.set_size_request(0, -1);
        widget.append(&search);
        widget.append(&Separator::new(Orientation::Horizontal));

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        let list = ListBox::new();
        list.set_selection_mode(SelectionMode::Single);
        list.set_activate_on_single_click(false);
        list.add_css_class("navigation-sidebar");
        scroll.set_child(Some(&list));
        widget.append(&scroll);

        let on_insert: InsertCb = Rc::new(RefCell::new(None));
        let entries: Rc<RefCell<Vec<BibEntry>>> = Rc::new(RefCell::new(Vec::new()));

        // Wire activation once on the list — fires on double-click and Enter.
        // Row's widget_name holds the citation key set during rebuild_list.
        {
            let cb = on_insert.clone();
            list.connect_row_activated(move |_, row| {
                let key = row.widget_name().to_string();
                if !key.is_empty() {
                    if let Some(f) = cb.borrow().as_ref() {
                        f(key);
                    }
                }
            });
        }

        let panel = Self { widget, list, search, entries, on_insert };

        {
            let p = panel.clone();
            panel.search.connect_search_changed(move |e| {
                p.rebuild_list(e.text().as_str());
            });
        }

        panel
    }

    pub fn widget(&self) -> &GtkBox {
        &self.widget
    }

    pub fn load_bib(&self, entries: Vec<BibEntry>) {
        *self.entries.borrow_mut() = entries;
        let query = self.search.text();
        self.rebuild_list(query.as_str());
    }

    pub fn set_on_insert(&self, f: impl Fn(String) + 'static) {
        *self.on_insert.borrow_mut() = Some(Box::new(f));
    }

    fn rebuild_list(&self, filter: &str) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
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
            self.list.append(&row);
            return;
        }

        let mut shown = 0usize;
        for entry in entries.iter() {
            if !filter_lower.is_empty() {
                let haystack = format!("{} {} {}", entry.key, entry.author, entry.title)
                    .to_lowercase();
                if !haystack.contains(&filter_lower) {
                    continue;
                }
            }

            let row = ListBoxRow::new();
            row.set_activatable(true);
            // Store the key as widget name so connect_row_activated can retrieve it
            row.set_widget_name(&entry.key);
            row.set_tooltip_text(Some(&format!("Double-click or Enter to insert @{}", entry.key)));

            let box_ = GtkBox::new(Orientation::Vertical, 2);
            box_.set_margin_start(8);
            box_.set_margin_end(8);
            box_.set_margin_top(5);
            box_.set_margin_bottom(5);

            // First line: bold key + dim "· author (year)"
            let top = GtkBox::new(Orientation::Horizontal, 4);
            let key_lbl = Label::new(None);
            key_lbl.set_markup(&format!(
                "<b>{}</b>",
                glib::markup_escape_text(&entry.key)
            ));
            key_lbl.set_xalign(0.0);
            key_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

            let meta_str = match (entry.author.is_empty(), entry.year.is_empty()) {
                (false, false) => format!(" · {} ({})", entry.author, entry.year),
                (false, true)  => format!(" · {}", entry.author),
                (true,  false) => format!(" · ({})", entry.year),
                (true,  true)  => String::new(),
            };
            let meta_lbl = Label::new(Some(&meta_str));
            meta_lbl.add_css_class("dim-label");
            meta_lbl.add_css_class("caption");
            meta_lbl.set_xalign(0.0);
            meta_lbl.set_hexpand(true);
            meta_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            top.append(&key_lbl);
            top.append(&meta_lbl);
            box_.append(&top);

            // Second line: italic title
            if !entry.title.is_empty() {
                let title_lbl = Label::new(None);
                title_lbl.set_markup(&format!(
                    "<i>{}</i>",
                    glib::markup_escape_text(&entry.title)
                ));
                title_lbl.set_xalign(0.0);
                title_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                title_lbl.add_css_class("caption");
                box_.append(&title_lbl);
            }

            row.set_child(Some(&box_));
            self.list.append(&row);
            shown += 1;
        }

        if shown == 0 {
            let row = ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            let lbl = Label::new(Some("No matching entries"));
            lbl.add_css_class("dim-label");
            lbl.set_margin_top(16);
            lbl.set_margin_bottom(16);
            row.set_child(Some(&lbl));
            self.list.append(&row);
        }
    }
}
