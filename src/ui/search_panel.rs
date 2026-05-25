use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Entry, Label, ListBox, ListBoxRow, Orientation,
    ScrolledWindow, SelectionMode, Separator,
};

type JumpCb = Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32)>>>>;

#[derive(Clone)]
pub struct SearchPanel {
    widget: GtkBox,
    list_box: ListBox,
    project_root: Rc<PathBuf>,
    on_jump: JumpCb,
}

impl SearchPanel {
    pub fn new(project_root: PathBuf) -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 0);

        let header = GtkBox::new(Orientation::Horizontal, 0);
        header.set_margin_start(10);
        header.set_margin_end(10);
        header.set_margin_top(6);
        header.set_margin_bottom(6);
        let title = Label::new(Some("Search"));
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.add_css_class("heading");
        header.append(&title);

        widget.append(&Separator::new(Orientation::Horizontal));
        widget.append(&header);
        widget.append(&Separator::new(Orientation::Horizontal));

        let entry = Entry::new();
        entry.set_placeholder_text(Some("Search in project… (Enter)"));
        entry.set_margin_start(8);
        entry.set_margin_end(8);
        entry.set_margin_top(6);
        entry.set_margin_bottom(6);
        widget.append(&entry);
        widget.append(&Separator::new(Orientation::Horizontal));

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::None);
        list_box.add_css_class("navigation-sidebar");
        scroll.set_child(Some(&list_box));
        widget.append(&scroll);

        let on_jump: JumpCb = Rc::new(RefCell::new(None));

        let panel = Self {
            widget,
            list_box,
            project_root: Rc::new(project_root),
            on_jump,
        };

        let p = panel.clone();
        entry.connect_activate(move |e| p.run_search(e.text().as_str()));

        panel
    }

    pub fn widget(&self) -> &GtkBox {
        &self.widget
    }

    pub fn set_on_jump(&self, f: impl Fn(PathBuf, u32) + 'static) {
        *self.on_jump.borrow_mut() = Some(Box::new(f));
    }

    fn run_search(&self, query: &str) {
        let query = query.trim();

        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        if query.is_empty() {
            return;
        }

        let query_lower = query.to_lowercase();
        let files = crate::project::collect_typ_files(&self.project_root);
        let mut result_count = 0usize;
        let mut truncated = false;

        'outer: for file_path in &files {
            let content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let rel = file_path
                .strip_prefix(self.project_root.as_ref())
                .unwrap_or(file_path)
                .to_string_lossy()
                .to_string();

            for (idx, line) in content.lines().enumerate() {
                if !line.to_lowercase().contains(&query_lower) {
                    continue;
                }
                let line_no = (idx + 1) as u32;
                let snippet = line.trim().to_string();

                let row = ListBoxRow::new();
                row.set_activatable(true);

                let row_box = GtkBox::new(Orientation::Vertical, 2);
                row_box.set_margin_start(8);
                row_box.set_margin_end(8);
                row_box.set_margin_top(3);
                row_box.set_margin_bottom(3);

                let loc_lbl = Label::new(Some(&format!("{}:{}", rel, line_no)));
                loc_lbl.add_css_class("caption");
                loc_lbl.add_css_class("dim-label");
                loc_lbl.set_xalign(0.0);

                let snippet_lbl = Label::new(Some(&snippet));
                snippet_lbl.set_xalign(0.0);
                snippet_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

                row_box.append(&loc_lbl);
                row_box.append(&snippet_lbl);
                row.set_child(Some(&row_box));

                let cb = self.on_jump.clone();
                let fp = file_path.clone();
                row.connect_activate(move |_| {
                    if let Some(f) = cb.borrow().as_ref() {
                        f(fp.clone(), line_no);
                    }
                });

                self.list_box.append(&row);
                result_count += 1;
                if result_count >= 300 {
                    truncated = true;
                    break 'outer;
                }
            }
        }

        if result_count == 0 {
            let row = ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            let lbl = Label::new(Some("No results"));
            lbl.add_css_class("dim-label");
            lbl.set_margin_top(16);
            lbl.set_margin_bottom(16);
            row.set_child(Some(&lbl));
            self.list_box.append(&row);
        } else if truncated {
            let row = ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            let lbl = Label::new(Some("… results truncated — refine your query"));
            lbl.add_css_class("dim-label");
            lbl.add_css_class("caption");
            lbl.set_margin_top(6);
            lbl.set_margin_bottom(6);
            row.set_child(Some(&lbl));
            self.list_box.append(&row);
        }
    }
}
