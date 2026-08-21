use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Entry, Label, ListBox, ListBoxRow, Orientation, Revealer,
    RevealerTransitionType, ScrolledWindow, SelectionMode, Separator, Spinner,
};

use crate::typst_universe::UniversePackage;

#[derive(Clone)]
struct LocalPackage {
    namespace: String,
    name: String,
    version: String,
}

/// One row in the merged list: a package name (currently only `@preview` is
/// ever both locally installed and in the Typst Universe index — other
/// namespaces like `@local` have no remote entry).
#[derive(Clone)]
struct Row {
    namespace: String,
    name: String,
    installed_version: Option<String>,
    remote_version: Option<String>,
    description: Option<String>,
}

#[derive(Clone)]
pub struct PackageBrowser {
    widget: GtkBox,
    list_box: ListBox,
    filter_entry: Entry,
    status_label: Label,
    collapse_btn: Button,
    revealer: Revealer,
    local: Rc<RefCell<Vec<LocalPackage>>>,
    remote: Rc<RefCell<Vec<UniversePackage>>>,
    installing: Rc<RefCell<std::collections::HashSet<String>>>,
    on_insert: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    on_collapse_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
}

impl PackageBrowser {
    pub fn new() -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 0);

        let header = GtkBox::new(Orientation::Horizontal, 6);
        header.set_margin_start(10);
        header.set_margin_end(10);
        header.set_margin_top(6);
        header.set_margin_bottom(6);
        let title = Label::new(Some("Packages"));
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.add_css_class("heading");
        header.append(&title);

        let refresh_btn = Button::from_icon_name("view-refresh-symbolic");
        refresh_btn.add_css_class("flat");
        refresh_btn.set_tooltip_text(Some("Refresh the Typst Universe package list"));
        header.append(&refresh_btn);

        // Furthest right on the bar, matching Comments' and Citations'
        // collapse toggle placement.
        let collapse_btn = Button::from_icon_name("pan-down-symbolic");
        collapse_btn.add_css_class("flat");
        collapse_btn.set_tooltip_text(Some("Hide Packages"));
        collapse_btn.update_property(&[gtk4::accessible::Property::Label("Hide Packages")]);
        header.append(&collapse_btn);

        widget.append(&Separator::new(Orientation::Horizontal));
        widget.append(&header);
        widget.append(&Separator::new(Orientation::Horizontal));

        let body = GtkBox::new(Orientation::Vertical, 0);
        body.set_vexpand(true);

        let filter_entry = Entry::new();
        filter_entry.set_placeholder_text(Some("Search installed and Typst Universe packages…"));
        filter_entry.set_margin_start(8);
        filter_entry.set_margin_end(8);
        filter_entry.set_margin_top(6);
        filter_entry.set_margin_bottom(4);
        body.append(&filter_entry);

        let status_label = Label::new(None);
        status_label.add_css_class("caption");
        status_label.add_css_class("dim-label");
        status_label.set_xalign(0.0);
        status_label.set_margin_start(8);
        status_label.set_margin_bottom(4);
        status_label.set_visible(false);
        // Without wrap/a width cap, a long network-error message (e.g. a raw
        // reqwest error appended to "Couldn't reach Typst Universe…") makes
        // this label request a wide natural size. The sidebar's outer Paned
        // has shrink_start_child(false), so that natural size becomes a
        // floor the user can't drag the sidebar narrower than — a long error
        // could shove the sidebar wide and lock it there.
        status_label.set_wrap(true);
        status_label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        status_label.set_max_width_chars(24);
        body.append(&status_label);

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::None);
        list_box.add_css_class("navigation-sidebar");
        scroll.set_child(Some(&list_box));
        body.append(&scroll);

        let revealer = Revealer::new();
        revealer.set_transition_type(RevealerTransitionType::SlideDown);
        revealer.set_reveal_child(true);
        revealer.set_vexpand(true);
        revealer.set_child(Some(&body));
        widget.append(&revealer);

        let local: Rc<RefCell<Vec<LocalPackage>>> = Rc::new(RefCell::new(Vec::new()));
        let remote: Rc<RefCell<Vec<UniversePackage>>> = Rc::new(RefCell::new(Vec::new()));
        let installing: Rc<RefCell<std::collections::HashSet<String>>> =
            Rc::new(RefCell::new(std::collections::HashSet::new()));
        let on_insert: Rc<RefCell<Option<Box<dyn Fn(String)>>>> = Rc::new(RefCell::new(None));
        let on_collapse_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>> =
            Rc::new(RefCell::new(None));

        {
            let revealer = revealer.clone();
            let collapse_btn_c = collapse_btn.clone();
            let on_collapse_toggle_c = on_collapse_toggle.clone();
            collapse_btn.connect_clicked(move |_| {
                let now_collapsed = revealer.reveals_child();
                revealer.set_reveal_child(!now_collapsed);
                collapse_btn_c.set_icon_name(if now_collapsed {
                    "pan-end-symbolic"
                } else {
                    "pan-down-symbolic"
                });
                collapse_btn_c.set_tooltip_text(Some(if now_collapsed {
                    "Show Packages"
                } else {
                    "Hide Packages"
                }));
                if let Some(f) = on_collapse_toggle_c.borrow().as_ref() {
                    f(now_collapsed);
                }
            });
        }

        let pb = Self {
            widget,
            list_box,
            filter_entry,
            status_label,
            collapse_btn,
            revealer,
            local,
            remote,
            installing,
            on_insert,
            on_collapse_toggle,
        };

        pb.scan_local_packages();

        // Instant first paint from whatever's on disk (no network wait), then
        // a background refresh if the cache is missing or stale — repeated
        // panel opens within a day cost nothing extra.
        if let Some(cached) = crate::typst_universe::load_cached_only() {
            *pb.remote.borrow_mut() = cached;
        }
        pb.rebuild_list("");
        if !crate::typst_universe::cache_is_fresh() {
            pb.refresh_universe_index();
        }

        {
            let pb_c = pb.clone();
            pb.filter_entry.connect_changed(move |entry| {
                pb_c.rebuild_list(entry.text().as_ref());
            });
        }
        {
            let pb_c = pb.clone();
            refresh_btn.connect_clicked(move |_| pb_c.refresh_universe_index());
        }

        pb
    }

    pub fn set_on_insert(&self, f: impl Fn(String) + 'static) {
        *self.on_insert.borrow_mut() = Some(Box::new(f));
    }

    pub fn widget(&self) -> &GtkBox {
        &self.widget
    }

    /// Restores a persisted collapsed/expanded state — called once at
    /// startup, before the user has clicked anything.
    pub fn set_collapsed(&self, collapsed: bool) {
        self.revealer.set_reveal_child(!collapsed);
        self.collapse_btn.set_icon_name(if collapsed {
            "pan-end-symbolic"
        } else {
            "pan-down-symbolic"
        });
        self.collapse_btn.set_tooltip_text(Some(if collapsed {
            "Show Packages"
        } else {
            "Hide Packages"
        }));
    }

    pub fn is_collapsed(&self) -> bool {
        !self.revealer.reveals_child()
    }

    /// Fires with the new collapsed state whenever the user clicks the
    /// header's collapse toggle, so the caller can persist it.
    pub fn set_on_collapse_toggle(&self, f: impl Fn(bool) + 'static) {
        *self.on_collapse_toggle.borrow_mut() = Some(Box::new(f));
    }

    fn scan_local_packages(&self) {
        let mut pkgs = Vec::new();
        // Must match compiler::package_cache_root() — that's where packages
        // actually land (XDG cache dir, matching typst-cli), not the XDG
        // data dir this used to scan, which meant a package downloaded
        // implicitly on first use, or now explicitly via Install below,
        // never showed up here as installed.
        let base = crate::compiler::package_cache_root();

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
                                pkgs.push(LocalPackage {
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
        *self.local.borrow_mut() = pkgs;
    }

    /// Fetches the Typst Universe index on a background thread and rebuilds
    /// the list on completion. Safe to call repeatedly (e.g. the refresh
    /// button) — a failed fetch just leaves the previous list in place, with
    /// a status message explaining nothing changed.
    fn refresh_universe_index(&self) {
        self.status_label
            .set_text("Checking Typst Universe for new packages…");
        self.status_label.set_visible(true);

        let (tx, rx) = mpsc::sync_channel::<Result<Vec<UniversePackage>, String>>(1);
        std::thread::spawn(move || {
            tx.send(crate::typst_universe::fetch_index()).ok();
        });

        let pb = self.clone();
        glib::timeout_add_local(Duration::from_millis(150), move || match rx.try_recv() {
            Ok(Ok(pkgs)) => {
                *pb.remote.borrow_mut() = pkgs;
                pb.status_label.set_visible(false);
                pb.rebuild_list(pb.filter_entry.text().as_ref());
                glib::ControlFlow::Break
            }
            Ok(Err(e)) => {
                pb.status_label.set_text(&format!(
                    "Couldn't reach Typst Universe — showing what's cached. ({e})"
                ));
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        });
    }

    fn install(&self, namespace: &str, name: &str, version: &str) {
        let key = format!("@{namespace}/{name}:{version}");
        if self.installing.borrow().contains(&key) {
            return;
        }
        self.installing.borrow_mut().insert(key.clone());
        self.rebuild_list(self.filter_entry.text().as_ref());

        let (tx, rx) = mpsc::sync_channel::<Result<(), String>>(1);
        let spec = key.clone();
        std::thread::spawn(move || {
            tx.send(crate::compiler::install_package(&spec)).ok();
        });

        let pb = self.clone();
        let key_for_poll = key.clone();
        glib::timeout_add_local(Duration::from_millis(150), move || match rx.try_recv() {
            Ok(result) => {
                pb.installing.borrow_mut().remove(&key_for_poll);
                match result {
                    Ok(()) => {
                        pb.scan_local_packages();
                        pb.status_label.set_visible(false);
                    }
                    Err(e) => {
                        pb.status_label
                            .set_text(&format!("Couldn't install {key_for_poll}: {e}"));
                        pb.status_label.set_visible(true);
                    }
                }
                pb.rebuild_list(pb.filter_entry.text().as_ref());
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        });
    }

    /// Merges installed packages with the Typst Universe index into one
    /// name-keyed list. Non-`preview` namespaces (only `@local` in practice)
    /// have no remote counterpart and just pass through as installed-only.
    fn merged_rows(&self) -> Vec<Row> {
        let mut by_key: std::collections::BTreeMap<(String, String), Row> =
            std::collections::BTreeMap::new();

        for pkg in self.local.borrow().iter() {
            let key = (pkg.namespace.clone(), pkg.name.clone());
            let row = by_key.entry(key).or_insert_with(|| Row {
                namespace: pkg.namespace.clone(),
                name: pkg.name.clone(),
                installed_version: None,
                remote_version: None,
                description: None,
            });
            // Prefer the newest installed version if multiple are present.
            if row
                .installed_version
                .as_ref()
                .map(|v| v.as_str() < pkg.version.as_str())
                .unwrap_or(true)
            {
                row.installed_version = Some(pkg.version.clone());
            }
        }

        for pkg in self.remote.borrow().iter() {
            let key = ("preview".to_string(), pkg.name.clone());
            let row = by_key.entry(key).or_insert_with(|| Row {
                namespace: "preview".to_string(),
                name: pkg.name.clone(),
                installed_version: None,
                remote_version: None,
                description: None,
            });
            row.remote_version = Some(pkg.version.clone());
            row.description = pkg.description.clone();
        }

        by_key.into_values().collect()
    }

    fn rebuild_list(&self, filter: &str) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        let filter_lower = filter.to_lowercase();
        let rows = self.merged_rows();
        let filtered: Vec<&Row> = rows
            .iter()
            .filter(|r| {
                filter_lower.is_empty()
                    || r.name.to_lowercase().contains(&filter_lower)
                    || r.namespace.to_lowercase().contains(&filter_lower)
                    || r.description
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(&filter_lower))
                        .unwrap_or(false)
            })
            .collect();

        if filtered.is_empty() {
            let row = ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            let msg = if rows.is_empty() {
                "No packages yet.\n\nSearch above to browse Typst Universe,\nor use a package in your document —\nit installs automatically either way."
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
            info_box.append(&name_lbl);

            let version_text = match (&pkg.installed_version, &pkg.remote_version) {
                (Some(inst), Some(remote)) if inst == remote => format!("v{inst} installed"),
                (Some(inst), Some(remote)) => {
                    format!("v{inst} installed · v{remote} available")
                }
                (Some(inst), None) => format!("v{inst} installed"),
                (None, Some(remote)) => format!("v{remote} available"),
                (None, None) => String::new(),
            };
            if !version_text.is_empty() {
                let ver_lbl = Label::new(Some(&version_text));
                ver_lbl.add_css_class("caption");
                ver_lbl.add_css_class("dim-label");
                ver_lbl.set_xalign(0.0);
                info_box.append(&ver_lbl);
            }

            if let Some(desc) = &pkg.description {
                let desc_lbl = Label::new(Some(desc));
                desc_lbl.add_css_class("caption");
                desc_lbl.add_css_class("dim-label");
                desc_lbl.set_xalign(0.0);
                desc_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                info_box.append(&desc_lbl);
            }

            row_box.append(&info_box);

            let install_key = pkg
                .remote_version
                .as_ref()
                .map(|v| format!("@{}/{}:{}", pkg.namespace, pkg.name, v));
            let is_installing = install_key
                .as_ref()
                .map(|k| self.installing.borrow().contains(k))
                .unwrap_or(false);

            if is_installing {
                let spinner = Spinner::new();
                spinner.set_spinning(true);
                spinner.set_valign(gtk4::Align::Center);
                row_box.append(&spinner);
            } else if pkg.installed_version.is_none() {
                if let Some(remote_version) = &pkg.remote_version {
                    let install_btn = Button::from_icon_name("folder-download-symbolic");
                    install_btn.add_css_class("flat");
                    install_btn.set_tooltip_text(Some(
                        "Download this package from Typst Universe so it's ready to use",
                    ));
                    install_btn.set_valign(gtk4::Align::Center);
                    let pb_c = self.clone();
                    let ns = pkg.namespace.clone();
                    let name = pkg.name.clone();
                    let version = remote_version.clone();
                    install_btn.connect_clicked(move |_| pb_c.install(&ns, &name, &version));
                    row_box.append(&install_btn);
                }
            } else {
                let insert_btn = Button::from_icon_name("list-add-symbolic");
                insert_btn.add_css_class("flat");
                insert_btn.set_tooltip_text(Some(
                    "Add this package at your cursor, so you can use what it provides in this document",
                ));
                insert_btn.set_valign(gtk4::Align::Center);

                let version = pkg.installed_version.clone().unwrap_or_default();
                let import_str = format!(
                    "#import \"@{}/{}:{}\": *\n",
                    pkg.namespace, pkg.name, version
                );
                let on_insert_c = self.on_insert.clone();
                insert_btn.connect_clicked(move |_| {
                    if let Some(f) = on_insert_c.borrow().as_ref() {
                        f(import_str.clone());
                    }
                });
                row_box.append(&insert_btn);
            }

            row.set_child(Some(&row_box));
            self.list_box.append(&row);
        }
    }
}
