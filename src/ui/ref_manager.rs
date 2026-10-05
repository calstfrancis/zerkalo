use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Entry, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow,
    SelectionMode, Separator,
};
use libadwaita as adw;

use crate::bibliography::BibEntry;

type InsertCb = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;
type JumpCb = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;
type RenameCb = Rc<RefCell<Option<Box<dyn Fn(String, String)>>>>;

#[derive(Clone)]
pub struct RefManager {
    widget: GtkBox,
    list_box: ListBox,
    filter_entry: Entry,
    entries: Rc<RefCell<Vec<BibEntry>>>,
    on_insert: InsertCb,
    on_jump_citation: JumpCb,
    on_rename: RenameCb,
    on_create_bib: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    used_keys: Rc<RefCell<HashSet<String>>>,
    /// `<label>`s defined in the project: `@fig-1` is a cross-reference to one of
    /// these, not a citation missing from the bibliography.
    labels: Rc<RefCell<HashSet<String>>>,
    bib_path: Rc<RefCell<Option<PathBuf>>>,
    problem: Rc<RefCell<Option<String>>>,
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

        let export_btn = Button::from_icon_name("document-save-symbolic");
        export_btn.set_tooltip_text(Some("Export cited-only bibliography"));
        export_btn.update_property(&[gtk4::accessible::Property::Label(
            "Export cited-only bibliography",
        )]);
        export_btn.add_css_class("flat");
        export_btn.set_valign(Align::Center);
        header.append(&export_btn);

        let new_entry_btn = Button::from_icon_name("list-add-symbolic");
        new_entry_btn.set_tooltip_text(Some("Add new bibliography entry"));
        new_entry_btn.update_property(&[gtk4::accessible::Property::Label(
            "Add new bibliography entry",
        )]);
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
        let on_rename: RenameCb = Rc::new(RefCell::new(None));
        let on_create_bib: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let entries: Rc<RefCell<Vec<BibEntry>>> = Rc::new(RefCell::new(Vec::new()));
        let used_keys: Rc<RefCell<HashSet<String>>> = Rc::new(RefCell::new(HashSet::new()));
        let labels: Rc<RefCell<HashSet<String>>> = Rc::new(RefCell::new(HashSet::new()));
        let problem: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let bib_path: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));

        let panel = Self {
            widget,
            list_box,
            filter_entry,
            entries,
            on_insert,
            on_jump_citation,
            on_rename,
            on_create_bib,
            used_keys,
            labels,
            bib_path,
            problem,
        };

        // Filter entry → rebuild list
        {
            let p = panel.clone();
            panel.filter_entry.connect_changed(move |e| {
                p.rebuild_list(e.text().as_str());
            });
        }

        // Export cited-only bibliography button
        {
            let p = panel.clone();
            export_btn.connect_clicked(move |btn| {
                let root = btn.root().and_then(|r| r.downcast::<gtk4::Window>().ok());
                let bib = p.bib_path.borrow().clone();
                let source = bib.as_deref().map(crate::bibliography::bib_target_path);
                let (title, body): (&str, &str) = if source.is_none() {
                    (
                        "No bibliography configured",
                        "Set a bibliography in Settings before exporting.",
                    )
                } else if p.used_keys.borrow().is_empty() {
                    (
                        "No citations found",
                        "This document doesn't cite any keys from the bibliography yet.",
                    )
                } else {
                    ("", "")
                };
                if !title.is_empty() {
                    let dlg = adw::MessageDialog::new(root.as_ref(), Some(title), Some(body));
                    dlg.add_response("ok", "OK");
                    dlg.present();
                    return;
                }
                let Some(source) = source else { return };
                let used_keys: std::collections::BTreeSet<String> =
                    p.used_keys.borrow().iter().cloned().collect();

                let dialog = gtk4::FileDialog::new();
                dialog.set_title("Export Cited-Only Bibliography (.bib or .yaml)");
                dialog.set_initial_name(Some("cited.bib"));
                let root_c = root.clone();
                dialog.save(
                    root.as_ref(),
                    None::<&gtk4::gio::Cancellable>,
                    move |result| {
                        let Some(path) = result.ok().and_then(|f| f.path()) else {
                            return;
                        };
                        let format = match path.extension().and_then(|e| e.to_str()) {
                            Some(e)
                                if e.eq_ignore_ascii_case("yaml")
                                    || e.eq_ignore_ascii_case("yml") =>
                            {
                                crate::cited_refs::RefFormat::Yaml
                            }
                            _ => crate::cited_refs::RefFormat::Bib,
                        };
                        let result = crate::cited_refs::export_cited(&source, &used_keys, format)
                            .and_then(|r| {
                                std::fs::write(&path, r.text)
                                    .map_err(|e| format!("Write error: {e}"))
                            });
                        if let Err(e) = result {
                            let dlg = adw::MessageDialog::new(
                                root_c.as_ref(),
                                Some("Couldn't make the export"),
                                Some(&e),
                            );
                            dlg.add_response("ok", "OK");
                            dlg.present();
                        }
                    },
                );
            });
        }

        // New entry button → open dialog
        {
            let p = panel.clone();
            new_entry_btn.connect_clicked(move |btn| {
                let bib = p.bib_path.borrow().clone();
                let is_yaml = bib.as_ref().is_some_and(|path| {
                    path.extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
                });
                let is_vault = bib.as_ref().is_some_and(|path| crate::bibliography::is_vault_dir(path));
                if bib.is_none() || is_yaml || is_vault {
                    let root = btn.root().and_then(|r| r.downcast::<gtk4::Window>().ok());
                    if bib.is_none() {
                        let dlg = adw::MessageDialog::new(
                            root.as_ref(),
                            Some("No bibliography yet"),
                            Some("Start a new one, or pick an existing .bib file in Settings."),
                        );
                        dlg.add_response("cancel", "Cancel");
                        dlg.add_response("create", "Start a New Bibliography");
                        dlg.set_response_appearance("create", adw::ResponseAppearance::Suggested);
                        dlg.set_default_response(Some("create"));
                        let create_cb = p.on_create_bib.clone();
                        dlg.connect_response(None, move |_, response| {
                            if response == "create" {
                                if let Some(f) = create_cb.borrow().as_ref() {
                                    f();
                                }
                            }
                        });
                        dlg.present();
                        return;
                    }
                    let (title, body) = if is_vault {
                        ("Kartoteka vault is read-only here",
                         "Add or edit entries in Kartoteka — Zerkalo just reads the vault live.")
                    } else {
                        ("YAML bibliography is read-only",
                         "Adding new entries is only supported for .bib files. Edit the .yaml file directly, or switch to a .bib bibliography in Settings.")
                    };
                    let dlg = adw::MessageDialog::new(root.as_ref(), Some(title), Some(body));
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

    pub fn widget(&self) -> &GtkBox {
        &self.widget
    }

    pub fn set_on_insert(&self, f: impl Fn(String) + 'static) {
        *self.on_insert.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_jump_citation(&self, f: impl Fn(String) + 'static) {
        *self.on_jump_citation.borrow_mut() = Some(Box::new(f));
    }

    /// Called with `(old_key, new_key)` when the user confirms a rename via
    /// the per-entry rename popover.
    pub fn set_on_rename(&self, f: impl Fn(String, String) + 'static) {
        *self.on_rename.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_create_bib(&self, f: impl Fn() + 'static) {
        *self.on_create_bib.borrow_mut() = Some(Box::new(f));
    }

    /// The keys the open document cites.
    pub fn used(&self) -> HashSet<String> {
        self.used_keys.borrow().clone()
    }

    /// Re-reads `path` and shows it.
    pub fn load_bib(&self, path: &Path) {
        self.set_source(path, crate::bibliography::try_load(path));
    }

    /// Shows the outcome of reading `path`: its entries, or why it couldn't be
    /// read (instead of an empty list that invites starting a new file).
    pub fn set_source(&self, path: &Path, loaded: Result<Vec<BibEntry>, String>) {
        *self.bib_path.borrow_mut() = Some(path.to_path_buf());
        match loaded {
            Ok(entries) => {
                *self.entries.borrow_mut() = entries;
                *self.problem.borrow_mut() = None;
            }
            Err(why) => {
                self.entries.borrow_mut().clear();
                *self.problem.borrow_mut() = Some(why);
            }
        }
        self.rebuild_list("");
    }

    /// No bibliography at all.
    pub fn clear(&self) {
        *self.bib_path.borrow_mut() = None;
        self.entries.borrow_mut().clear();
        *self.problem.borrow_mut() = None;
        self.rebuild_list("");
    }

    /// Refreshes which keys the document cites. `text` is the open file as
    /// typed (possibly unsaved); chapters it `#include`s are read from disk.
    pub fn update_used_keys(&self, text: &str, path: &Path) {
        let (keys, labels) = crate::cited_refs::collect_citations(path, Some(text));
        let keys: HashSet<String> = keys.into_iter().collect();
        let labels: HashSet<String> = labels.into_iter().collect();
        if *self.used_keys.borrow() == keys && *self.labels.borrow() == labels {
            return;
        }
        *self.used_keys.borrow_mut() = keys;
        *self.labels.borrow_mut() = labels;
        let filter = self.filter_entry.text();
        self.rebuild_list(filter.as_str());
    }

    fn rebuild_list(&self, filter: &str) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        let entries = self.entries.borrow();
        let used = self.used_keys.borrow();
        let labels = self.labels.borrow();
        let has_used_data = !used.is_empty();

        // Broken citations section — keys used in doc but not found in bib
        if has_used_data && !entries.is_empty() {
            let entry_keys: HashSet<&str> = entries.iter().map(|e| e.key.as_str()).collect();
            let mut broken: Vec<&str> = used
                .iter()
                .filter(|k| !entry_keys.contains(k.as_str()) && !labels.contains(k.as_str()))
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

                self.list_box
                    .append(&Separator::new(Orientation::Horizontal));
            }
        }

        if entries.is_empty() {
            let row = ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            let message = match self.problem.borrow().as_deref() {
                Some(problem) => format!("{problem}\nYour file hasn't been changed."),
                None if self.bib_path.borrow().is_some() => {
                    "This bibliography has no entries yet.".to_string()
                }
                None => "No bibliography loaded yet.\nClick + above to start one.".to_string(),
            };
            let lbl = Label::new(Some(&message));
            lbl.add_css_class("dim-label");
            lbl.set_justify(gtk4::Justification::Center);
            lbl.set_margin_top(16);
            lbl.set_margin_bottom(16);
            row.set_child(Some(&lbl));
            self.list_box.append(&row);
            return;
        }

        let mut shown = 0usize;
        let order = crate::cite_search::search(&entries, filter, &used);
        for entry in order.iter().map(|&i| &entries[i]) {
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
            top.append(&key_lbl);

            let rename_btn = Button::from_icon_name("document-edit-symbolic");
            rename_btn.add_css_class("flat");
            rename_btn.add_css_class("circular");
            rename_btn.set_tooltip_text(Some("Rename citation key"));
            rename_btn.update_property(&[gtk4::accessible::Property::Label("Rename citation key")]);
            {
                let key = entry.key.clone();
                let cb = self.on_rename.clone();
                rename_btn.connect_clicked(move |btn| {
                    open_rename_popover(btn, key.clone(), cb.clone());
                });
            }
            top.append(&rename_btn);

            let year_lbl = Label::new(Some(&entry.year));
            year_lbl.add_css_class("dim-label");
            year_lbl.add_css_class("caption");
            year_lbl.set_hexpand(true);
            year_lbl.set_xalign(1.0);
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

// ── Rename popover ──────────────────────────────────────────────────────────

fn open_rename_popover(anchor: &Button, old_key: String, on_rename: RenameCb) {
    let popover = gtk4::Popover::new();
    popover.set_parent(anchor);

    let vbox = GtkBox::new(Orientation::Vertical, 6);
    vbox.set_margin_top(10);
    vbox.set_margin_bottom(10);
    vbox.set_margin_start(10);
    vbox.set_margin_end(10);

    let label = Label::new(Some("New citation key"));
    label.set_xalign(0.0);
    label.add_css_class("caption");
    vbox.append(&label);

    let entry = Entry::new();
    entry.set_text(&old_key);
    entry.set_width_chars(24);
    vbox.append(&entry);

    let confirm_btn = Button::with_label("Rename");
    confirm_btn.add_css_class("suggested-action");
    vbox.append(&confirm_btn);

    popover.set_child(Some(&vbox));

    let do_rename: Rc<dyn Fn()> = {
        let entry = entry.clone();
        let popover = popover.clone();
        Rc::new(move || {
            let new_key = entry.text().trim().to_string();
            if !new_key.is_empty() && new_key != old_key {
                if let Some(f) = on_rename.borrow().as_ref() {
                    f(old_key.clone(), new_key);
                }
            }
            popover.popdown();
        })
    };
    {
        let f = do_rename.clone();
        confirm_btn.connect_clicked(move |_| f());
    }
    {
        let f = do_rename.clone();
        entry.connect_activate(move |_| f());
    }

    popover.popup();
}

// ── New entry dialog ──────────────────────────────────────────────────────────

/// (BibTeX type, plain-language label). BibTeX's own type names
/// ("inproceedings", "incollection") mean nothing to someone outside CS
/// academia — the label is what's shown; the BibTeX type is what's stored.
const ENTRY_TYPES: &[(&str, &str)] = &[
    ("article", "Journal Article"),
    ("book", "Book"),
    ("inproceedings", "Conference Paper"),
    ("incollection", "Book Chapter"),
    ("phdthesis", "PhD Thesis"),
    ("mastersthesis", "Master's Thesis"),
    ("techreport", "Technical Report"),
    ("misc", "Other"),
    ("unpublished", "Unpublished"),
];

fn open_new_entry_dialog(parent: Option<&gtk4::Window>, panel: RefManager) {
    let dialog = adw::Window::builder()
        .title("New Bibliography Entry")
        .default_width(440)
        .default_height(500)
        .modal(true)
        .resizable(false)
        .build();
    if let Some(p) = parent {
        dialog.set_transient_for(Some(p));
    }

    let header = adw::HeaderBar::new();
    header.add_css_class("fond-chrome");
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
    let type_labels: Vec<&str> = ENTRY_TYPES.iter().map(|(_, label)| *label).collect();
    let type_model = gtk4::StringList::new(&type_labels);
    type_row.set_model(Some(&type_model));
    let key_row = adw::EntryRow::new();
    key_row.set_title("Cite key");
    key_row.set_tooltip_text(Some(
        "A short nickname, e.g. smith2019 — you'll type @smith2019 to cite it.",
    ));
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
    toolbar.set_top_bar_style(adw::ToolbarStyle::RaisedBorder);
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));
    dialog.set_content(Some(&toolbar));

    let dlg_cancel = dialog.clone();
    cancel_btn.connect_clicked(move |_| dlg_cancel.close());

    let problem_label = Label::new(None);
    problem_label.add_css_class("error");
    problem_label.set_wrap(true);
    problem_label.set_xalign(0.0);
    problem_label.set_visible(false);
    type_group.set_description(Some(
        "Fill in what you know; only the nickname is required.",
    ));
    page.add(&{
        let g = adw::PreferencesGroup::new();
        g.add(&problem_label);
        g
    });

    let dlg_add = dialog.clone();
    add_btn.connect_clicked(move |_| {
        let key = key_row.text().trim().to_string();
        let show = |msg: &str| {
            problem_label.set_text(msg);
            problem_label.set_visible(true);
        };
        if let Some(msg) = crate::citation_keys::key_problem(&key) {
            show(msg);
            return;
        }
        let existing_text = panel
            .bib_path
            .borrow()
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        if crate::bibliography::bib_text_has_key(&existing_text, &key) {
            show("Another entry already uses that nickname. Pick a different one.");
            return;
        }
        for (label, field) in [
            ("Author(s)", author_row.text()),
            ("Title", title_row.text()),
            ("Journal / Publisher", venue_row.text()),
        ] {
            if !braces_balanced(field.as_str()) {
                show(&format!(
                    "{label} has a {{ or }} without its partner. Remove it or add the other one."
                ));
                return;
            }
        }
        let year_text = year_row.text();
        if !year_text.trim().is_empty() && !is_plausible_year(year_text.trim()) {
            show("Year should be four digits, like 1990 — or leave it blank.");
            return;
        }

        let entry_type = ENTRY_TYPES
            .get(type_row.selected() as usize)
            .map(|(t, _)| *t)
            .unwrap_or("misc");

        let author = author_row.text().trim().to_string();
        let title = title_row.text().trim().to_string();
        let year = year_row.text().trim().to_string();
        let venue = venue_row.text().trim().to_string();

        let venue_field = match entry_type {
            "article" => "journal",
            "book" => "publisher",
            "inproceedings" => "booktitle",
            "incollection" => "booktitle",
            _ => "publisher",
        };

        let mut bibtex = format!("@{entry_type}{{{key},\n");
        if !author.is_empty() {
            bibtex.push_str(&format!("  author = {{{author}}},\n"));
        }
        if !title.is_empty() {
            bibtex.push_str(&format!("  title = {{{title}}},\n"));
        }
        if !year.is_empty() {
            bibtex.push_str(&format!("  year = {{{year}}},\n"));
        }
        if !venue.is_empty() {
            bibtex.push_str(&format!("  {venue_field} = {{{venue}}},\n"));
        }
        bibtex.push_str("}\n");

        if let Some(ref p) = *panel.bib_path.borrow() {
            let existing = existing_text.as_str();
            let updated = if existing.is_empty() {
                bibtex.clone()
            } else if existing.ends_with('\n') {
                format!("{existing}\n{bibtex}")
            } else {
                format!("{existing}\n\n{bibtex}")
            };
            if crate::bibliography::write_atomic(p, &updated).is_ok() {
                panel.load_bib(p);
            } else {
                show("Couldn't save the bibliography file. Nothing was added.");
                return;
            }
        }

        dlg_add.close();
    });

    dialog.present();
}

/// `{` and `}` pair up, in order — an unpaired one in a field would swallow the
/// rest of the `.bib` file.
fn braces_balanced(text: &str) -> bool {
    let mut depth = 0i32;
    for c in text.chars() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

fn is_plausible_year(text: &str) -> bool {
    text.len() == 4 && text.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braces_must_pair_up() {
        assert!(braces_balanced("The {Bible} and {{God}}"));
        assert!(!braces_balanced("open { only"));
        assert!(!braces_balanced("close } first {"));
    }

    #[test]
    fn year_is_four_digits() {
        assert!(is_plausible_year("1990"));
        assert!(!is_plausible_year("Winter 2001"));
        assert!(!is_plausible_year("90"));
    }
}
