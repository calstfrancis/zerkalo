use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow,
    SearchEntry, SelectionMode, Separator,
};

use crate::bibliography::BibEntry;

type InsertCb = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;
type ChooseBibCb = Rc<RefCell<Option<Box<dyn Fn()>>>>;

#[derive(Clone)]
pub struct CitationPanel {
    widget: GtkBox,
    list: ListBox,
    search: SearchEntry,
    entries: Rc<RefCell<Vec<BibEntry>>>,
    on_insert: InsertCb,
    on_choose_bib: ChooseBibCb,
    bib_name_label: Label,
}

impl CitationPanel {
    pub fn new() -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 0);

        let header_box = GtkBox::new(Orientation::Horizontal, 6);
        header_box.set_margin_start(10);
        header_box.set_margin_end(6);
        header_box.set_margin_top(6);
        header_box.set_margin_bottom(6);

        let title = Label::new(Some("Citations"));
        title.set_xalign(0.0);
        title.add_css_class("heading");
        header_box.append(&title);

        let bib_name_label = Label::new(None);
        bib_name_label.add_css_class("dim-label");
        bib_name_label.add_css_class("caption");
        bib_name_label.set_hexpand(true);
        bib_name_label.set_halign(Align::Start);
        bib_name_label.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
        bib_name_label.set_visible(false);
        header_box.append(&bib_name_label);

        let choose_btn = Button::from_icon_name("document-open-symbolic");
        choose_btn.add_css_class("flat");
        choose_btn.add_css_class("circular");
        choose_btn.set_tooltip_text(Some("Choose bibliography file (.bib)"));
        choose_btn.update_property(&[gtk4::accessible::Property::Label("Choose bibliography file")]);
        header_box.append(&choose_btn);

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
        list.set_activate_on_single_click(true);
        list.add_css_class("navigation-sidebar");
        scroll.set_child(Some(&list));
        widget.append(&scroll);

        let on_insert: InsertCb = Rc::new(RefCell::new(None));
        let on_choose_bib: ChooseBibCb = Rc::new(RefCell::new(None));
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

        {
            let cb = on_choose_bib.clone();
            choose_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() { f(); }
            });
        }

        let panel = Self { widget, list, search, entries, on_insert, on_choose_bib, bib_name_label };

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

    pub fn set_on_choose_bib(&self, f: impl Fn() + 'static) {
        *self.on_choose_bib.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_bib_filename(&self, name: Option<&str>) {
        match name {
            Some(n) => {
                self.bib_name_label.set_text(n);
                self.bib_name_label.set_visible(true);
            }
            None => {
                self.bib_name_label.set_visible(false);
            }
        }
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
