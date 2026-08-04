use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow,
    SearchEntry, SelectionMode, Separator,
};

use crate::bibliography::BibEntry;

type InsertCb = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;
type ChooseCb = Rc<RefCell<Option<Box<dyn Fn()>>>>;

/// The panel connects to either a bibliography or a Skrizhal CV-element
/// database, never both at once — `cv_mode` mirrors the active document's
/// CV mode (`EditorPane::set_cv_mode`) and swaps which list is shown.
#[derive(Clone)]
pub struct CitationPanel {
    widget: GtkBox,
    list: ListBox,
    search: SearchEntry,
    title_label: Label,
    bib_entries: Rc<RefCell<Vec<BibEntry>>>,
    cv_entries: Rc<RefCell<Vec<skrizhal_core::CvEntry>>>,
    cv_mode: Rc<Cell<bool>>,
    on_insert: InsertCb,
    on_choose_bib: ChooseCb,
    on_choose_cv: ChooseCb,
    on_open_skrizhal: ChooseCb,
    choose_btn: Button,
    bib_name_label: Label,
    skrizhal_btn: Button,
    bib_filename: Rc<RefCell<Option<String>>>,
    cv_filename: Rc<RefCell<Option<String>>>,
}

impl CitationPanel {
    pub fn new() -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 0);
        widget.add_css_class("fond-sidebar");

        let header_box = GtkBox::new(Orientation::Horizontal, 6);
        header_box.set_margin_start(10);
        header_box.set_margin_end(6);
        header_box.set_margin_top(6);
        header_box.set_margin_bottom(6);

        let dot = Label::new(Some("\u{25cf}"));
        dot.add_css_class("fond-section-dot");
        dot.add_css_class("fond-accent-citations");
        dot.set_valign(Align::Center);
        header_box.append(&dot);

        let title_label = Label::new(Some("Citations"));
        title_label.set_xalign(0.0);
        title_label.add_css_class("fond-section-title");
        header_box.append(&title_label);

        let bib_name_label = Label::new(None);
        bib_name_label.add_css_class("dim-label");
        bib_name_label.add_css_class("caption");
        bib_name_label.set_hexpand(true);
        bib_name_label.set_halign(Align::Start);
        bib_name_label.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
        bib_name_label.set_visible(false);
        header_box.append(&bib_name_label);

        // In CV mode, replaces bib_name_label — opens the actual Skrizhal
        // app to edit the YAML database, rather than just naming the file.
        let skrizhal_btn = Button::with_label("Skrizhal");
        skrizhal_btn.add_css_class("flat");
        skrizhal_btn.set_hexpand(true);
        skrizhal_btn.set_halign(Align::Start);
        skrizhal_btn.set_tooltip_text(Some("Open Skrizhal to edit CV elements"));
        skrizhal_btn.set_visible(false);
        header_box.append(&skrizhal_btn);

        let choose_btn = Button::from_icon_name("document-open-symbolic");
        choose_btn.add_css_class("flat");
        choose_btn.add_css_class("circular");
        choose_btn.set_tooltip_text(Some("Choose bibliography file (.bib, .yaml)"));
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
        let on_choose_bib: ChooseCb = Rc::new(RefCell::new(None));
        let on_choose_cv: ChooseCb = Rc::new(RefCell::new(None));
        let on_open_skrizhal: ChooseCb = Rc::new(RefCell::new(None));
        let bib_entries: Rc<RefCell<Vec<BibEntry>>> = Rc::new(RefCell::new(Vec::new()));
        let cv_entries: Rc<RefCell<Vec<skrizhal_core::CvEntry>>> = Rc::new(RefCell::new(Vec::new()));
        let cv_mode: Rc<Cell<bool>> = Rc::new(Cell::new(false));

        // Wire activation once on the list — fires on double-click and Enter.
        // Row's widget_name holds the citation/CV key set during rebuild_list;
        // the insert text format depends on which mode was active at build time.
        {
            let cb = on_insert.clone();
            let cv_mode_ra = cv_mode.clone();
            list.connect_row_activated(move |_, row| {
                let key = row.widget_name().to_string();
                if !key.is_empty() {
                    let text = if cv_mode_ra.get() {
                        format!("#cv-entry(\"{key}\")")
                    } else {
                        format!("@{key}")
                    };
                    if let Some(f) = cb.borrow().as_ref() {
                        f(text);
                    }
                }
            });
        }

        {
            let cb_bib = on_choose_bib.clone();
            let cb_cv = on_choose_cv.clone();
            let cv_mode_cb = cv_mode.clone();
            choose_btn.connect_clicked(move |_| {
                if cv_mode_cb.get() {
                    if let Some(f) = cb_cv.borrow().as_ref() { f(); }
                } else if let Some(f) = cb_bib.borrow().as_ref() { f(); }
            });
        }

        {
            let cb = on_open_skrizhal.clone();
            skrizhal_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() { f(); }
            });
        }

        let panel = Self {
            widget,
            list,
            search,
            title_label,
            bib_entries,
            cv_entries,
            cv_mode,
            on_insert,
            on_choose_bib,
            on_choose_cv,
            on_open_skrizhal,
            choose_btn,
            bib_name_label,
            skrizhal_btn,
            bib_filename: Rc::new(RefCell::new(None)),
            cv_filename: Rc::new(RefCell::new(None)),
        };

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
        *self.bib_entries.borrow_mut() = entries;
        if !self.cv_mode.get() {
            let query = self.search.text();
            self.rebuild_list(query.as_str());
        }
    }

    pub fn load_cv_entries(&self, entries: Vec<skrizhal_core::CvEntry>) {
        *self.cv_entries.borrow_mut() = entries;
        if self.cv_mode.get() {
            let query = self.search.text();
            self.rebuild_list(query.as_str());
        }
    }

    /// Swaps the panel between citation mode and CV-element mode — the
    /// active document's `#doc-kind: cv` front matter drives this, not a
    /// user toggle (see `EditorPane::set_cv_mode`).
    pub fn set_cv_mode(&self, active: bool) {
        if self.cv_mode.get() == active {
            return;
        }
        self.cv_mode.set(active);
        if active {
            self.title_label.set_text("CV Elements");
            self.search.set_placeholder_text(Some("Search by key, title, tag…"));
            self.choose_btn.set_tooltip_text(Some("Choose CV element file (.yaml)"));
            self.choose_btn.update_property(&[gtk4::accessible::Property::Label(
                "Choose CV element file",
            )]);
            self.bib_name_label.set_visible(false);
            self.skrizhal_btn.set_visible(true);
        } else {
            self.title_label.set_text("Citations");
            self.search.set_placeholder_text(Some("Search by key, author, title…"));
            self.choose_btn.set_tooltip_text(Some("Choose bibliography file (.bib, .yaml)"));
            self.choose_btn.update_property(&[gtk4::accessible::Property::Label(
                "Choose bibliography file",
            )]);
            self.skrizhal_btn.set_visible(false);
            self.refresh_filename_label(self.bib_filename.borrow().as_deref());
        }
        let query = self.search.text();
        self.rebuild_list(query.as_str());
    }

    pub fn set_on_insert(&self, f: impl Fn(String) + 'static) {
        *self.on_insert.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_choose_bib(&self, f: impl Fn() + 'static) {
        *self.on_choose_bib.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_choose_cv(&self, f: impl Fn() + 'static) {
        *self.on_choose_cv.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_open_skrizhal(&self, f: impl Fn() + 'static) {
        *self.on_open_skrizhal.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_bib_filename(&self, name: Option<&str>) {
        *self.bib_filename.borrow_mut() = name.map(str::to_string);
        if !self.cv_mode.get() {
            self.refresh_filename_label(name);
        }
    }

    pub fn set_cv_filename(&self, name: Option<&str>) {
        *self.cv_filename.borrow_mut() = name.map(str::to_string);
        self.skrizhal_btn.set_tooltip_text(Some(&match name {
            Some(n) => format!("Open Skrizhal to edit CV elements ({n})"),
            None => "Open Skrizhal to edit CV elements".to_string(),
        }));
    }

    fn refresh_filename_label(&self, name: Option<&str>) {
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

        if self.cv_mode.get() {
            self.rebuild_cv_list(filter);
        } else {
            self.rebuild_bib_list(filter);
        }
    }

    fn rebuild_bib_list(&self, filter: &str) {
        let filter_lower = filter.to_lowercase();
        let entries = self.bib_entries.borrow();

        if entries.is_empty() {
            self.append_placeholder("No bibliography loaded.\nSet a .bib file in Settings.");
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
            row.set_widget_name(&entry.key);
            row.set_tooltip_text(Some(&format!("Double-click or Enter to insert @{}", entry.key)));

            let box_ = GtkBox::new(Orientation::Vertical, 2);
            box_.set_margin_start(8);
            box_.set_margin_end(8);
            box_.set_margin_top(5);
            box_.set_margin_bottom(5);

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
            self.append_placeholder("No matching entries");
        }
    }

    fn rebuild_cv_list(&self, filter: &str) {
        let filter_lower = filter.to_lowercase();
        let entries = self.cv_entries.borrow();

        if entries.is_empty() {
            self.append_placeholder("No CV elements loaded.\nSet a Skrizhal file in Settings.");
            return;
        }

        let mut shown = 0usize;
        for entry in entries.iter() {
            if !filter_lower.is_empty() {
                let haystack = format!(
                    "{} {} {} {}",
                    entry.key,
                    entry.title,
                    entry.organization.as_deref().unwrap_or(""),
                    entry.tags.join(" ")
                )
                .to_lowercase();
                if !haystack.contains(&filter_lower) {
                    continue;
                }
            }

            let row = ListBoxRow::new();
            row.set_activatable(true);
            row.set_widget_name(&entry.key);
            row.set_tooltip_text(Some(&format!(
                "Double-click or Enter to insert #cv-entry(\"{}\")",
                entry.key
            )));

            let box_ = GtkBox::new(Orientation::Vertical, 2);
            box_.set_margin_start(8);
            box_.set_margin_end(8);
            box_.set_margin_top(5);
            box_.set_margin_bottom(5);

            let top = GtkBox::new(Orientation::Horizontal, 4);
            let title_lbl = Label::new(None);
            let title_text = if entry.title.is_empty() { entry.key.as_str() } else { &entry.title };
            title_lbl.set_markup(&format!(
                "<b>{}</b>",
                glib::markup_escape_text(title_text)
            ));
            title_lbl.set_xalign(0.0);
            title_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

            let meta_str = match (&entry.organization, &entry.date) {
                (Some(org), Some(date)) => format!(" · {org} ({date})"),
                (Some(org), None) => format!(" · {org}"),
                (None, Some(date)) => format!(" · ({date})"),
                (None, None) => String::new(),
            };
            let meta_lbl = Label::new(Some(&meta_str));
            meta_lbl.add_css_class("dim-label");
            meta_lbl.add_css_class("caption");
            meta_lbl.set_xalign(0.0);
            meta_lbl.set_hexpand(true);
            meta_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            top.append(&title_lbl);
            top.append(&meta_lbl);
            box_.append(&top);

            let sub_str = format!("{} · {}", entry.category, entry.key);
            let sub_lbl = Label::new(Some(&sub_str));
            sub_lbl.set_xalign(0.0);
            sub_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            sub_lbl.add_css_class("caption");
            sub_lbl.add_css_class("dim-label");
            box_.append(&sub_lbl);

            row.set_child(Some(&box_));
            self.list.append(&row);
            shown += 1;
        }

        if shown == 0 {
            self.append_placeholder("No matching entries");
        }
    }

    fn append_placeholder(&self, text: &str) {
        let row = ListBoxRow::new();
        row.set_selectable(false);
        row.set_activatable(false);
        let lbl = Label::new(Some(text));
        lbl.add_css_class("dim-label");
        lbl.set_justify(gtk4::Justification::Center);
        lbl.set_margin_top(16);
        lbl.set_margin_bottom(16);
        row.set_child(Some(&lbl));
        self.list.append(&row);
    }
}
