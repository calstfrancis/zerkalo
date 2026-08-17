use std::rc::Rc;
use std::cell::RefCell;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Entry, Label, ListBox, ListBoxRow,
    Orientation, ScrolledWindow, SelectionMode, Separator,
};

#[derive(Clone)]
struct PackageEntry {
    namespace: String,
    name: String,
    version: String,
}

#[derive(Clone)]
pub struct PackageBrowser {
    #[allow(dead_code)]
    widget: GtkBox,
    list_box: ListBox,
    filter_entry: Entry,
    packages: Rc<RefCell<Vec<PackageEntry>>>,
    on_insert: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
}

impl PackageBrowser {
    pub fn new() -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 0);

        let header = GtkBox::new(Orientation::Horizontal, 0);
        header.set_margin_start(10);
        header.set_margin_end(10);
        header.set_margin_top(6);
        header.set_margin_bottom(6);
        let title = Label::new(Some("Packages"));
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.add_css_class("heading");
        header.append(&title);

        widget.append(&Separator::new(Orientation::Horizontal));
        widget.append(&header);
        widget.append(&Separator::new(Orientation::Horizontal));

        let filter_entry = Entry::new();
        filter_entry.set_placeholder_text(Some("Filter packages…"));
        filter_entry.set_margin_start(8);
        filter_entry.set_margin_end(8);
        filter_entry.set_margin_top(6);
        filter_entry.set_margin_bottom(4);
        widget.append(&filter_entry);

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::None);
        list_box.add_css_class("navigation-sidebar");
        scroll.set_child(Some(&list_box));
        widget.append(&scroll);

        let packages: Rc<RefCell<Vec<PackageEntry>>> = Rc::new(RefCell::new(Vec::new()));
        let on_insert: Rc<RefCell<Option<Box<dyn Fn(String)>>>> = Rc::new(RefCell::new(None));

        let pb = Self { widget, list_box, filter_entry, packages, on_insert };
        pb.scan_local_packages();
        pb.rebuild_list("");

        {
            let pb_c = pb.clone();
            pb.filter_entry.connect_changed(move |entry| {
                pb_c.rebuild_list(entry.text().as_ref());
            });
        }

        pb
    }

    pub fn set_on_insert(&self, f: impl Fn(String) + 'static) {
        *self.on_insert.borrow_mut() = Some(Box::new(f));
    }

    fn scan_local_packages(&self) {
        let mut pkgs = Vec::new();
        let base = glib::user_data_dir().join("typst/packages");

        if let Ok(ns_entries) = std::fs::read_dir(&base) {
            for ns_entry in ns_entries.flatten() {
                if !ns_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let ns = ns_entry.file_name().to_string_lossy().to_string();
                if let Ok(name_entries) = std::fs::read_dir(ns_entry.path()) {
                    for name_entry in name_entries.flatten() {
                        if !name_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            continue;
                        }
                        let name = name_entry.file_name().to_string_lossy().to_string();
                        if let Ok(ver_entries) = std::fs::read_dir(name_entry.path()) {
                            for ver_entry in ver_entries.flatten() {
                                if !ver_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                                    continue;
                                }
                                let version = ver_entry.file_name().to_string_lossy().to_string();
                                pkgs.push(PackageEntry {
                                    namespace: ns.clone(),
                                    name: name.clone(),
                                    version,
                                });
                            }
                        }
                    }
                }
            }
        }

        pkgs.sort_by(|a, b| a.name.cmp(&b.name).then(a.version.cmp(&b.version)));
        *self.packages.borrow_mut() = pkgs;
    }

    fn rebuild_list(&self, filter: &str) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        let filter_lower = filter.to_lowercase();
        let pkgs = self.packages.borrow();
        let filtered: Vec<&PackageEntry> = pkgs
            .iter()
            .filter(|p| {
                filter_lower.is_empty()
                    || p.name.to_lowercase().contains(&filter_lower)
                    || p.namespace.to_lowercase().contains(&filter_lower)
            })
            .collect();

        if filtered.is_empty() {
            let row = ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            let msg = if pkgs.is_empty() {
                "No packages downloaded yet.\n\nPackages install automatically the\nfirst time your document uses one —\nnothing to do here yet."
            } else {
                "No matches"
            };
            let lbl = Label::new(Some(msg));
            lbl.add_css_class("dim-label");
            lbl.set_justify(gtk4::Justification::Center);
            lbl.set_margin_top(16);
            lbl.set_margin_bottom(16);
            row.set_child(Some(&lbl));
            self.list_box.append(&row);
            return;
        }

        for pkg in filtered {
            let row = ListBoxRow::new();
            row.set_activatable(false);

            let row_box = GtkBox::new(Orientation::Horizontal, 6);
            row_box.set_margin_start(8);
            row_box.set_margin_end(6);
            row_box.set_margin_top(4);
            row_box.set_margin_bottom(4);

            let info_box = GtkBox::new(Orientation::Vertical, 2);
            info_box.set_hexpand(true);

            let name_lbl = Label::new(Some(&format!("@{}/{}", pkg.namespace, pkg.name)));
            name_lbl.set_xalign(0.0);
            name_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

            let ver_lbl = Label::new(Some(&pkg.version));
            ver_lbl.add_css_class("caption");
            ver_lbl.add_css_class("dim-label");
            ver_lbl.set_xalign(0.0);

            info_box.append(&name_lbl);
            info_box.append(&ver_lbl);

            let insert_btn = Button::from_icon_name("list-add-symbolic");
            insert_btn.add_css_class("flat");
            insert_btn.set_tooltip_text(Some(
                "Add this package at your cursor, so you can use what it provides in this document",
            ));
            insert_btn.set_valign(gtk4::Align::Center);

            let import_str = format!(
                "#import \"@{}/{}:{}\": *\n",
                pkg.namespace, pkg.name, pkg.version
            );
            let on_insert_c = self.on_insert.clone();
            insert_btn.connect_clicked(move |_| {
                if let Some(f) = on_insert_c.borrow().as_ref() {
                    f(import_str.clone());
                }
            });

            row_box.append(&info_box);
            row_box.append(&insert_btn);
            row.set_child(Some(&row_box));
            self.list_box.append(&row);
        }
    }
}
