use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Entry, Label, ListBox, ListBoxRow, Orientation,
    ScrolledWindow, SelectionMode, Separator,
};
use libadwaita as adw;
use adw::prelude::*;
use regex::Regex;

use crate::bibliography::BibEntry;

type InsertCb = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;
type JumpCb = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;

static CITE_RE: OnceLock<Regex> = OnceLock::new();

fn cite_re() -> &'static Regex {
    CITE_RE.get_or_init(|| Regex::new(r"@([A-Za-z][A-Za-z0-9_:]*)").unwrap())
}

#[derive(Clone)]
pub struct RefManager {
    #[allow(dead_code)]
    widget: GtkBox,
    list_box: ListBox,
    filter_entry: Entry,
    entries: Rc<RefCell<Vec<BibEntry>>>,
    on_insert: InsertCb,
    on_jump_citation: JumpCb,
    used_keys: Rc<RefCell<HashSet<String>>>,
    bib_path: Rc<RefCell<Option<PathBuf>>>,
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

        let new_entry_btn = Button::from_icon_name("list-add-symbolic");
        new_entry_btn.set_tooltip_text(Some("Add new bibliography entry"));
        new_entry_btn.add_css_class("flat");
        new_entry_btn.set_valign(Align::Center);
        header.append(&new_entry_btn);

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
        scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::None);
        list_box.add_css_class("navigation-sidebar");
        scroll.set_child(Some(&list_box));
        widget.append(&scroll);

        let on_insert: InsertCb = Rc::new(RefCell::new(None));
        let on_jump_citation: JumpCb = Rc::new(RefCell::new(None));
        let entries: Rc<RefCell<Vec<BibEntry>>> = Rc::new(RefCell::new(Vec::new()));
        let used_keys: Rc<RefCell<HashSet<String>>> = Rc::new(RefCell::new(HashSet::new()));
        let bib_path: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));

        let panel = Self { widget, list_box, filter_entry, entries, on_insert, on_jump_citation, used_keys, bib_path };

        // Filter entry → rebuild list
        {
            let p = panel.clone();
            panel.filter_entry.connect_changed(move |e| {
                p.rebuild_list(e.text().as_str());
            });
        }

        // New entry button → open dialog
        {
            let p = panel.clone();
            new_entry_btn.connect_clicked(move |btn| {
                let bib = p.bib_path.borrow().clone();
                if bib.is_none() {
                    let root = btn.root().and_then(|r| r.downcast::<gtk4::Window>().ok());
                    let dlg = adw::MessageDialog::new(
                        root.as_ref(),
                        Some("No bibliography configured"),
                        Some("Set a .bib file in Settings before adding entries."),
                    );
                    dlg.add_response("ok", "OK");
                    dlg.present();
                    return;
                }
                let root_win = btn.root().and_then(|r| r.downcast::<gtk4::Window>().ok());
                open_new_entry_dialog(root_win.as_ref(), p.clone());
            });
        }

        panel
    }

    #[allow(dead_code)]
    pub fn widget(&self) -> &GtkBox {
        &self.widget
    }

    pub fn set_on_insert(&self, f: impl Fn(String) + 'static) {
        *self.on_insert.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_jump_citation(&self, f: impl Fn(String) + 'static) {
        *self.on_jump_citation.borrow_mut() = Some(Box::new(f));
    }

    pub fn load_bib(&self, path: &Path) {
        *self.bib_path.borrow_mut() = Some(path.to_path_buf());
        let entries = crate::bibliography::load_bib(path);
        *self.entries.borrow_mut() = entries;
        self.rebuild_list("");
    }

    #[allow(dead_code)]
    pub fn clear_entries(&self) {
        self.entries.borrow_mut().clear();
        self.rebuild_list("");
    }

    pub fn update_used_keys(&self, text: &str) {
        let mut keys = HashSet::new();
        for cap in cite_re().captures_iter(text) {
            keys.insert(cap[1].to_string());
        }
        *self.used_keys.borrow_mut() = keys;
        let filter = self.filter_entry.text();
        self.rebuild_list(filter.as_str());
    }

    fn rebuild_list(&self, filter: &str) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        let filter_lower = filter.to_lowercase();
        let entries = self.entries.borrow();
        let used = self.used_keys.borrow();
        let has_used_data = !used.is_empty();

        // Broken citations section — keys used in doc but not found in bib
        if has_used_data && !entries.is_empty() {
            let entry_keys: HashSet<&str> = entries.iter().map(|e| e.key.as_str()).collect();
            let mut broken: Vec<&str> = used.iter()
                .filter(|k| !entry_keys.contains(k.as_str()))
                .map(|k| k.as_str())
                .collect();
            broken.sort_unstable();

            if !broken.is_empty() {
                let hdr = ListBoxRow::new();
                hdr.set_selectable(false);
                hdr.set_activatable(false);
                let hdr_lbl = Label::new(Some("Broken citations"));
                hdr_lbl.add_css_class("caption");
                hdr_lbl.set_xalign(0.0);
                hdr_lbl.set_margin_start(8);
                hdr_lbl.set_margin_top(8);
                hdr_lbl.set_margin_bottom(2);
                hdr.set_child(Some(&hdr_lbl));
                self.list_box.append(&hdr);

                for key in &broken {
                    let row = ListBoxRow::new();
                    row.set_selectable(false);
                    row.set_activatable(true);
                    row.set_tooltip_text(Some("Key used in document but not in bibliography"));
                    let lbl = Label::new(Some(&format!("⚠ @{key}")));
                    lbl.add_css_class("caption");
                    lbl.set_xalign(0.0);
                    lbl.set_margin_start(16);
                    lbl.set_margin_top(3);
                    lbl.set_margin_bottom(3);
                    row.set_child(Some(&lbl));
                    let cb = self.on_jump_citation.clone();
                    let k = key.to_string();
                    row.connect_activate(move |_| {
                        if let Some(f) = cb.borrow().as_ref() {
                            f(k.clone());
                        }
                    });
                    self.list_box.append(&row);
                }

                self.list_box.append(&Separator::new(Orientation::Horizontal));
            }
        }

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

            let is_used = has_used_data && used.contains(&entry.key);

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

            // Citation status indicator
            if has_used_data {
                let status_lbl = Label::new(Some(if is_used { "●" } else { "○" }));
                status_lbl.add_css_class("caption");
                if is_used {
                    status_lbl.add_css_class("success");
                } else {
                    status_lbl.add_css_class("dim-label");
                }
                top.append(&status_lbl);
            }

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

// ── New entry dialog ──────────────────────────────────────────────────────────

fn open_new_entry_dialog(parent: Option<&gtk4::Window>, panel: RefManager) {
    let dialog = adw::Window::builder()
        .title("New Bibliography Entry")
        .default_width(440)
        .modal(true)
        .resizable(false)
        .build();
    if let Some(p) = parent {
        dialog.set_transient_for(Some(p));
    }

    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);

    let cancel_btn = Button::with_label("Cancel");
    header.pack_start(&cancel_btn);
    let add_btn = Button::with_label("Add");
    add_btn.add_css_class("suggested-action");
    header.pack_end(&add_btn);

    let page = adw::PreferencesPage::new();

    let type_group = adw::PreferencesGroup::new();
    type_group.set_title("Entry");
    let type_row = adw::ComboRow::new();
    type_row.set_title("Entry type");
    let type_model = gtk4::StringList::new(&[
        "article", "book", "inproceedings", "incollection", "phdthesis",
        "mastersthesis", "techreport", "misc", "unpublished",
    ]);
    type_row.set_model(Some(&type_model));
    let key_row = adw::EntryRow::new();
    key_row.set_title("Cite key");
    type_group.add(&type_row);
    type_group.add(&key_row);

    let meta_group = adw::PreferencesGroup::new();
    meta_group.set_title("Metadata");
    let author_row = adw::EntryRow::new();
    author_row.set_title("Author(s)");
    let title_row = adw::EntryRow::new();
    title_row.set_title("Title");
    let year_row = adw::EntryRow::new();
    year_row.set_title("Year");
    let venue_row = adw::EntryRow::new();
    venue_row.set_title("Journal / Publisher");
    meta_group.add(&author_row);
    meta_group.add(&title_row);
    meta_group.add(&year_row);
    meta_group.add(&venue_row);

    page.add(&type_group);
    page.add(&meta_group);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));
    dialog.set_content(Some(&toolbar));

    let dlg_cancel = dialog.clone();
    cancel_btn.connect_clicked(move |_| dlg_cancel.close());

    let dlg_add = dialog.clone();
    add_btn.connect_clicked(move |_| {
        let key = key_row.text().trim().to_string();
        if key.is_empty() {
            return;
        }

        let type_names = ["article", "book", "inproceedings", "incollection", "phdthesis",
                          "mastersthesis", "techreport", "misc", "unpublished"];
        let entry_type = type_names.get(type_row.selected() as usize).unwrap_or(&"misc");

        let author = author_row.text().trim().to_string();
        let title = title_row.text().trim().to_string();
        let year = year_row.text().trim().to_string();
        let venue = venue_row.text().trim().to_string();

        let venue_field = match *entry_type {
            "article" => "journal",
            "book" => "publisher",
            "inproceedings" => "booktitle",
            "incollection" => "booktitle",
            _ => "publisher",
        };

        let mut bibtex = format!("@{entry_type}{{{key},\n");
        if !author.is_empty() { bibtex.push_str(&format!("  author = {{{author}}},\n")); }
        if !title.is_empty()  { bibtex.push_str(&format!("  title = {{{title}}},\n")); }
        if !year.is_empty()   { bibtex.push_str(&format!("  year = {{{year}}},\n")); }
        if !venue.is_empty()  { bibtex.push_str(&format!("  {venue_field} = {{{venue}}},\n")); }
        bibtex.push_str("}\n");

        if let Some(ref p) = *panel.bib_path.borrow() {
            let existing = std::fs::read_to_string(p).unwrap_or_default();
            let updated = if existing.ends_with('\n') || existing.is_empty() {
                format!("{existing}\n{bibtex}")
            } else {
                format!("{existing}\n\n{bibtex}")
            };
            if std::fs::write(p, &updated).is_ok() {
                panel.load_bib(p);
            }
        }

        dlg_add.close();
    });

    dialog.present();
}
