use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow, SelectionMode,
    Separator,
};

fn collect_typ_files(root: &Path) -> Vec<PathBuf> {
    let repo = git2::Repository::open(root).ok();
    let mut files = Vec::new();
    collect_recursive(root, root, &repo, &mut files);
    files.sort();
    files
}

fn collect_recursive(
    root: &Path,
    dir: &Path,
    repo: &Option<git2::Repository>,
    out: &mut Vec<PathBuf>,
) {
    let mut entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.flatten().collect(),
        Err(_) => return,
    };
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if let Some(repo) = repo {
            if repo.is_path_ignored(&path).unwrap_or(false) {
                continue;
            }
        }
        if path.is_dir() {
            collect_recursive(root, &path, repo, out);
        } else if path.extension().map(|e| e == "typ").unwrap_or(false) {
            out.push(path);
        }
    }
}

#[derive(Clone)]
pub struct FileTree {
    root_widget: GtkBox,
    list_box: ListBox,
    project_root: Rc<PathBuf>,
    on_open: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>,
}

impl FileTree {
    pub fn new(project_root: PathBuf) -> Self {
        let root_widget = GtkBox::new(Orientation::Vertical, 0);
        root_widget.set_hexpand(false);
        root_widget.set_vexpand(true);

        let header = Label::new(Some("Files"));
        header.add_css_class("dim-label");
        header.set_halign(Align::Start);
        header.set_margin_start(10);
        header.set_margin_top(10);
        header.set_margin_bottom(6);
        root_widget.append(&header);
        root_widget.append(&Separator::new(Orientation::Horizontal));

        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::Single);
        list_box.add_css_class("navigation-sidebar");

        let scrolled = ScrolledWindow::new();
        scrolled.set_child(Some(&list_box));
        scrolled.set_vexpand(true);
        scrolled.set_min_content_width(200);
        root_widget.append(&scrolled);

        let on_open: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>> = Rc::new(RefCell::new(None));
        let project_root = Rc::new(project_root);

        let ft = Self {
            root_widget,
            list_box,
            project_root,
            on_open,
        };
        ft.refresh();
        ft
    }

    pub fn widget(&self) -> &GtkBox {
        &self.root_widget
    }

    pub fn set_on_open(&self, f: impl Fn(PathBuf) + 'static) {
        *self.on_open.borrow_mut() = Some(Box::new(f));
    }

    pub fn refresh(&self) {
        while let Some(row) = self.list_box.row_at_index(0) {
            self.list_box.remove(&row);
        }

        let files = collect_typ_files(&self.project_root);

        if files.is_empty() {
            let row = ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            let lbl = Label::new(Some("No .typ files found"));
            lbl.add_css_class("dim-label");
            lbl.set_margin_top(16);
            lbl.set_margin_bottom(16);
            row.set_child(Some(&lbl));
            self.list_box.append(&row);
            return;
        }

        // Group by relative directory (BTreeMap keeps empty "" before named dirs)
        let mut by_dir: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
        for file in &files {
            let rel = file.strip_prefix(self.project_root.as_ref()).unwrap_or(file);
            let dir = rel.parent().unwrap_or(Path::new("")).to_path_buf();
            by_dir.entry(dir).or_default().push(file.clone());
        }

        for (dir, dir_files) in &by_dir {
            // Subdirectory header row
            if dir != Path::new("") {
                let row = ListBoxRow::new();
                row.set_selectable(false);
                row.set_activatable(false);
                let lbl = Label::new(Some(&format!("{}/", dir.to_string_lossy())));
                lbl.add_css_class("dim-label");
                lbl.set_halign(Align::Start);
                lbl.set_margin_start(8);
                lbl.set_margin_top(8);
                lbl.set_margin_bottom(2);
                row.set_child(Some(&lbl));
                self.list_box.append(&row);
            }

            let indent = if dir == Path::new("") { 10 } else { 22 };

            for file_path in dir_files {
                let filename = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();

                let row = ListBoxRow::new();
                let lbl = Label::new(Some(&filename));
                lbl.set_halign(Align::Start);
                lbl.set_margin_start(indent);
                lbl.set_margin_top(5);
                lbl.set_margin_bottom(5);
                row.set_child(Some(&lbl));

                let path_capture = file_path.clone();
                let cb = self.on_open.clone();
                row.connect_activate(move |_| {
                    if let Some(f) = cb.borrow().as_ref() {
                        f(path_capture.clone());
                    }
                });

                self.list_box.append(&row);
            }
        }
    }
}
