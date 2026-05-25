use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::SystemTime;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Entry, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow, SelectionMode,
    Separator,
};
use libadwaita as adw;
use adw::prelude::*;

type OpenCb = Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>;

pub struct DocsBrowser {
    window: adw::Window,
    on_open: OpenCb,
}

impl DocsBrowser {
    pub fn new(parent: &impl IsA<gtk4::Window>, work_dir: PathBuf) -> Self {
        let window = adw::Window::builder()
            .title("My Documents")
            .transient_for(parent)
            .modal(true)
            .default_width(420)
            .default_height(500)
            .build();

        let on_open: OpenCb = Rc::new(RefCell::new(None));

        let header = adw::HeaderBar::new();

        let search_entry = Entry::new();
        search_entry.set_placeholder_text(Some("Search documents…"));
        search_entry.set_hexpand(true);
        header.set_title_widget(Some(&search_entry));

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_hexpand(true);

        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::None);
        list_box.add_css_class("navigation-sidebar");
        scroll.set_child(Some(&list_box));

        // Collect .typ files from work_dir (3 levels deep) sorted by last-modified (newest first)
        let mut files: Vec<(PathBuf, SystemTime)> = scan_typ_files(&work_dir, 3);
        files.sort_by(|a, b| b.1.cmp(&a.1));

        let file_paths: Rc<RefCell<Vec<PathBuf>>> = Rc::new(RefCell::new(
            files.iter().map(|(p, _)| p.clone()).collect(),
        ));

        for (path, mtime) in &files {
            append_row(&list_box, path, *mtime, &on_open, &window);
        }

        // Filter on search
        let list_box_c = list_box.clone();
        let file_paths_c = file_paths.clone();
        let on_open_c = on_open.clone();
        let window_c = window.clone();
        search_entry.connect_changed(move |entry| {
            let query = entry.text().to_lowercase();
            while let Some(child) = list_box_c.first_child() {
                list_box_c.remove(&child);
            }
            let paths = file_paths_c.borrow().clone();
            for path in &paths {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                if query.is_empty() || name.contains(&query) {
                    let mtime = std::fs::metadata(path)
                        .and_then(|m| m.modified())
                        .unwrap_or(SystemTime::UNIX_EPOCH);
                    append_row(&list_box_c, path, mtime, &on_open_c, &window_c);
                }
            }
        });

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);

        let content = GtkBox::new(Orientation::Vertical, 0);
        content.append(&Separator::new(Orientation::Horizontal));
        content.append(&scroll);
        toolbar_view.set_content(Some(&content));
        window.set_content(Some(&toolbar_view));

        Self { window, on_open }
    }

    pub fn set_on_open(&self, f: impl Fn(PathBuf) + 'static) {
        *self.on_open.borrow_mut() = Some(Box::new(f));
    }

    pub fn present(&self) {
        self.window.present();
    }
}

fn append_row(
    list_box: &ListBox,
    path: &PathBuf,
    mtime: SystemTime,
    on_open: &OpenCb,
    window: &adw::Window,
) {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
    let date_str = format_mtime_local(mtime);

    let row = ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);

    let btn = gtk4::Button::new();
    btn.add_css_class("flat");
    btn.set_hexpand(true);

    let row_box = GtkBox::new(Orientation::Vertical, 2);
    row_box.set_margin_start(6);
    row_box.set_margin_end(6);
    row_box.set_margin_top(4);
    row_box.set_margin_bottom(4);

    let name_lbl = Label::new(Some(&name));
    name_lbl.set_xalign(0.0);
    name_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    name_lbl.set_halign(gtk4::Align::Start);

    let date_lbl = Label::new(Some(&date_str));
    date_lbl.set_xalign(0.0);
    date_lbl.add_css_class("caption");
    date_lbl.add_css_class("dim-label");
    date_lbl.set_halign(gtk4::Align::Start);

    row_box.append(&name_lbl);
    row_box.append(&date_lbl);
    btn.set_child(Some(&row_box));

    let cb = on_open.clone();
    let p = path.clone();
    let win = window.clone();
    btn.connect_clicked(move |_| {
        if let Some(f) = cb.borrow().as_ref() {
            f(p.clone());
        }
        win.close();
    });

    row.set_child(Some(&btn));
    list_box.append(&row);
}

pub fn scan_typ_files(dir: &PathBuf, depth: usize) -> Vec<(PathBuf, SystemTime)> {
    let mut files = Vec::new();
    if depth == 0 {
        return files;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return files;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !name.starts_with('.') {
                files.extend(scan_typ_files(&p, depth - 1));
            }
        } else if p.extension().and_then(|e| e.to_str()) == Some("typ") {
            let mtime = std::fs::metadata(&p)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            files.push((p, mtime));
        }
    }
    files
}

pub fn format_mtime_local(mtime: SystemTime) -> String {
    let Ok(dur) = SystemTime::now().duration_since(mtime) else {
        return "unknown".to_string();
    };
    let secs = dur.as_secs();
    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        format!("{} min ago", secs / 60)
    } else if secs < 86400 {
        format!("{} h ago", secs / 3600)
    } else if secs < 86400 * 30 {
        format!("{} days ago", secs / 86400)
    } else {
        format!("{} months ago", secs / (86400 * 30))
    }
}
