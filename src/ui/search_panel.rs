use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::pango::EllipsizeMode;
use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Entry, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow,
    SelectionMode, Separator,
};

#[derive(Clone)]
pub struct SearchPanel {
    widget: GtkBox,
    entry: Entry,
    results: ListBox,
    work_dir: Rc<RefCell<PathBuf>>,
    on_result: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32)>>>>,
}

impl SearchPanel {
    pub fn new(work_dir: PathBuf) -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 0);
        widget.set_vexpand(false);
        widget.set_visible(false);

        let bar = GtkBox::new(Orientation::Horizontal, 8);
        bar.set_margin_start(8);
        bar.set_margin_end(8);
        bar.set_margin_top(5);
        bar.set_margin_bottom(5);

        let entry = Entry::new();
        entry.set_placeholder_text(Some("Search in project (.typ files)…"));
        entry.set_hexpand(true);

        let count_lbl = Label::new(None);
        count_lbl.add_css_class("dim-label");
        count_lbl.add_css_class("caption");

        let close_btn = Button::from_icon_name("window-close-symbolic");
        close_btn.add_css_class("flat");

        bar.append(&entry);
        bar.append(&count_lbl);
        bar.append(&close_btn);

        let scroll = ScrolledWindow::new();
        scroll.set_max_content_height(220);
        scroll.set_propagate_natural_height(true);
        scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);

        let results = ListBox::new();
        results.set_selection_mode(SelectionMode::Single);
        results.add_css_class("boxed-list-separate");
        scroll.set_child(Some(&results));

        widget.append(&Separator::new(Orientation::Horizontal));
        widget.append(&bar);
        widget.append(&scroll);

        let panel = Self {
            widget,
            entry: entry.clone(),
            results: results.clone(),
            work_dir: Rc::new(RefCell::new(work_dir)),
            on_result: Rc::new(RefCell::new(None)),
        };

        // Search on Enter
        let p = panel.clone();
        let count_c = count_lbl.clone();
        entry.connect_activate(move |e| {
            p.run_search(e.text().as_str(), &count_c);
        });

        // Live search on text change (debounced via glib idle)
        let p2 = panel.clone();
        let count_c2 = count_lbl.clone();
        entry.connect_changed(move |e| {
            let p3 = p2.clone();
            let text = e.text().to_string();
            let count_c3 = count_c2.clone();
            glib::idle_add_local_once(move || {
                p3.run_search(&text, &count_c3);
            });
        });

        // Close button
        let w = panel.widget.clone();
        close_btn.connect_clicked(move |_| w.set_visible(false));

        // Activate result row
        let p4 = panel.clone();
        results.connect_row_activated(move |_, row| {
            let name = row.widget_name().to_string();
            if let Some((file, line)) = decode_row_name(&name) {
                if let Some(f) = p4.on_result.borrow().as_ref() {
                    f(file, line);
                }
            }
        });

        panel
    }

    pub fn widget(&self) -> &GtkBox {
        &self.widget
    }

    pub fn toggle(&self) {
        let visible = self.widget.is_visible();
        self.widget.set_visible(!visible);
        if !visible {
            self.entry.grab_focus();
        }
    }

    pub fn set_on_result(&self, f: impl Fn(PathBuf, u32) + 'static) {
        *self.on_result.borrow_mut() = Some(Box::new(f));
    }

    fn run_search(&self, query: &str, count_lbl: &Label) {
        while let Some(child) = self.results.first_child() {
            self.results.remove(&child);
        }
        if query.is_empty() {
            count_lbl.set_text("");
            return;
        }
        let work_dir = self.work_dir.borrow().clone();
        let query_lower = query.to_lowercase();
        let matches = search_typ_files(&work_dir, &query_lower, 200);

        if matches.is_empty() {
            count_lbl.set_text("No results");
        } else {
            let file_count = {
                let mut seen = std::collections::HashSet::new();
                for m in &matches { seen.insert(m.file.clone()); }
                seen.len()
            };
            count_lbl.set_text(&format!("{} matches in {} files", matches.len(), file_count));
        }

        if matches.is_empty() {
            let row = ListBoxRow::new();
            row.set_activatable(false);
            let lbl = Label::new(Some("No results"));
            lbl.add_css_class("dim-label");
            lbl.set_margin_top(10);
            lbl.set_margin_bottom(10);
            row.set_child(Some(&lbl));
            self.results.append(&row);
            return;
        }

        for m in matches {
            let row = ListBoxRow::new();
            row.set_widget_name(&encode_row_name(&m.file, m.line));

            let rb = GtkBox::new(Orientation::Horizontal, 8);
            rb.set_margin_start(8);
            rb.set_margin_end(8);
            rb.set_margin_top(4);
            rb.set_margin_bottom(4);

            let file_name = m.file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            let loc = Label::new(Some(&format!("{}:{}", file_name, m.line)));
            loc.add_css_class("monospace");
            loc.add_css_class("caption");
            loc.set_width_chars(22);
            loc.set_xalign(0.0);

            let preview = Label::new(Some(&m.preview));
            preview.set_hexpand(true);
            preview.set_xalign(0.0);
            preview.set_ellipsize(EllipsizeMode::End);
            preview.add_css_class("dim-label");

            rb.append(&loc);
            rb.append(&preview);
            row.set_child(Some(&rb));
            self.results.append(&row);
        }
    }
}

struct Match {
    file: PathBuf,
    line: u32,
    preview: String,
}

fn search_typ_files(work_dir: &PathBuf, query: &str, limit: usize) -> Vec<Match> {
    let mut out = Vec::new();
    visit_dir(work_dir, query, &mut out, limit);
    out
}

fn visit_dir(dir: &PathBuf, query: &str, out: &mut Vec<Match>, limit: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if out.len() >= limit {
            return;
        }
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') || name == "target" {
            continue;
        }
        if path.is_dir() {
            visit_dir(&path, query, out, limit);
        } else if path.extension().and_then(|e| e.to_str()) == Some("typ") {
            let Ok(content) = std::fs::read_to_string(&path) else { continue };
            for (i, line) in content.lines().enumerate() {
                if out.len() >= limit {
                    return;
                }
                if line.to_lowercase().contains(query) {
                    out.push(Match {
                        file: path.clone(),
                        line: (i + 1) as u32,
                        preview: line.trim().to_string(),
                    });
                }
            }
        }
    }
}

fn encode_row_name(file: &PathBuf, line: u32) -> String {
    format!("{}||{}", file.display(), line)
}

fn decode_row_name(name: &str) -> Option<(PathBuf, u32)> {
    let mut parts = name.rsplitn(2, "||");
    let line: u32 = parts.next()?.parse().ok()?;
    let file = PathBuf::from(parts.next()?);
    Some((file, line))
}
