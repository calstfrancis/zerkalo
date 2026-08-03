use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Label, ListBox, ListBoxRow, Orientation, Paned,
    ScrolledWindow, SelectionMode, Separator, TextView, WrapMode,
};

#[derive(Clone)]
pub struct NotesPanel {
    widget: GtkBox,
    list_box: ListBox,
    text_view: TextView,
    notes: Rc<RefCell<HashMap<String, String>>>,
    headings: Rc<RefCell<Vec<String>>>,
    selected_key: Rc<RefCell<Option<String>>>,
    save_path: Rc<RefCell<Option<PathBuf>>>,
    current_file: Rc<RefCell<Option<PathBuf>>>,
    is_loading: Rc<RefCell<bool>>,
    save_gen: Rc<RefCell<u64>>,
    #[allow(dead_code)]
    placeholder: Label,
}

impl NotesPanel {
    pub fn new() -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 0);
        widget.set_vexpand(true);

        let paned = Paned::new(Orientation::Vertical);
        paned.set_vexpand(true);
        paned.set_position(180);

        // ── Top: heading list ─────────────────────────────────────────────────

        let list_scroll = ScrolledWindow::new();
        list_scroll.set_vexpand(true);
        list_scroll.set_hexpand(true);
        list_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        list_scroll.set_min_content_height(80);

        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::Single);
        list_box.add_css_class("navigation-sidebar");
        list_scroll.set_child(Some(&list_box));

        let placeholder = Label::new(Some("No headings"));
        placeholder.add_css_class("dim-label");
        placeholder.add_css_class("caption");
        placeholder.set_margin_top(12);
        placeholder.set_margin_bottom(12);
        list_box.set_placeholder(Some(&placeholder));

        paned.set_start_child(Some(&list_scroll));

        // ── Bottom: note text area ────────────────────────────────────────────

        let text_scroll = ScrolledWindow::new();
        text_scroll.set_vexpand(true);
        text_scroll.set_hexpand(true);
        text_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        text_scroll.set_min_content_height(80);

        let text_view = TextView::new();
        text_view.set_vexpand(true);
        text_view.set_wrap_mode(WrapMode::Word);
        text_view.set_left_margin(8);
        text_view.set_right_margin(8);
        text_view.set_top_margin(6);
        text_view.set_bottom_margin(6);
        text_view.set_sensitive(false);
        let buf = text_view.buffer();
        buf.set_text("Select a heading to add notes");
        text_scroll.set_child(Some(&text_view));

        paned.set_end_child(Some(&text_scroll));

        widget.append(&Separator::new(Orientation::Horizontal));
        widget.append(&paned);

        let notes: Rc<RefCell<HashMap<String, String>>> = Rc::new(RefCell::new(HashMap::new()));
        let headings: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let selected_key: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let save_path: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
        let current_file: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
        let is_loading: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let save_gen: Rc<RefCell<u64>> = Rc::new(RefCell::new(0));

        let panel = Self {
            widget,
            list_box,
            text_view,
            notes,
            headings,
            selected_key,
            save_path,
            current_file,
            is_loading,
            placeholder,
            save_gen,
        };

        // Row click: flush current note, load selected note
        {
            let p = panel.clone();
            panel.list_box.connect_row_activated(move |_, row| {
                let idx = row.index() as usize;
                let key = p.headings.borrow().get(idx).cloned();
                let Some(key) = key else { return };

                // Flush the outgoing note before switching
                let outgoing = p.selected_key.borrow().clone();
                if let Some(old_key) = outgoing {
                    let buf = p.text_view.buffer();
                    let text = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
                    p.notes.borrow_mut().insert(old_key, text);
                }

                *p.selected_key.borrow_mut() = Some(key.clone());
                let note = p.notes.borrow().get(&key).cloned().unwrap_or_default();
                *p.is_loading.borrow_mut() = true;
                p.text_view.set_sensitive(true);
                let buf = p.text_view.buffer();
                buf.set_text(&note);
                *p.is_loading.borrow_mut() = false;
            });
        }

        // Text change: debounced save
        {
            let p = panel.clone();
            panel.text_view.buffer().connect_changed(move |buf| {
                if *p.is_loading.borrow() { return; }
                let Some(key) = p.selected_key.borrow().clone() else { return };
                let text = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
                p.notes.borrow_mut().insert(key, text);

                *p.save_gen.borrow_mut() += 1;
                let my_gen = *p.save_gen.borrow();
                let gen2 = p.save_gen.clone();
                let notes2 = p.notes.clone();
                let path2 = p.save_path.clone();

                glib::timeout_add_local_once(Duration::from_millis(500), move || {
                    if *gen2.borrow() != my_gen { return; }
                    if let Some(path) = path2.borrow().clone() {
                        let notes = notes2.borrow().clone();
                        if let Ok(json) = serde_json::to_string_pretty(&notes) {
                            let _ = std::fs::write(&path, json);
                        }
                    }
                });
            });
        }

        panel
    }

    pub fn widget(&self) -> &GtkBox {
        &self.widget
    }

    pub fn update(&self, content: &str, path: &PathBuf) {
        let new_path = notes_path_for(path);
        let file_changed = self.current_file.borrow().as_ref() != Some(path);

        if file_changed {
            self.flush_current_note();
            *self.current_file.borrow_mut() = Some(path.clone());
            let loaded: HashMap<String, String> = std::fs::read_to_string(&new_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            *self.notes.borrow_mut() = loaded;
            *self.save_path.borrow_mut() = Some(new_path);
            *self.selected_key.borrow_mut() = None;
        }

        let new_headings = parse_headings(content);

        // GC: drop notes whose key is no longer a heading
        {
            let mut notes = self.notes.borrow_mut();
            notes.retain(|k, _| new_headings.contains(k));
        }

        // Preserve selection if heading still exists
        let current_sel = self.selected_key.borrow().clone();
        let new_sel = current_sel
            .filter(|k| new_headings.contains(k))
            .or_else(|| new_headings.first().cloned());

        *self.headings.borrow_mut() = new_headings;
        self.repopulate_list();

        // Re-select
        if let Some(ref key) = new_sel {
            let headings = self.headings.borrow();
            if let Some(idx) = headings.iter().position(|h| h == key) {
                let row = self.list_box.row_at_index(idx as i32);
                self.list_box.select_row(row.as_ref());
                if self.selected_key.borrow().as_ref() != Some(key) {
                    *self.selected_key.borrow_mut() = Some(key.clone());
                    let note = self.notes.borrow().get(key).cloned().unwrap_or_default();
                    *self.is_loading.borrow_mut() = true;
                    self.text_view.set_sensitive(true);
                    self.text_view.buffer().set_text(&note);
                    *self.is_loading.borrow_mut() = false;
                }
            }
        } else {
            self.list_box.unselect_all();
            *self.selected_key.borrow_mut() = None;
            *self.is_loading.borrow_mut() = true;
            self.text_view.set_sensitive(false);
            self.text_view.buffer().set_text("Select a heading to add notes");
            *self.is_loading.borrow_mut() = false;
        }
    }

    fn repopulate_list(&self) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }
        let headings = self.headings.borrow();
        for heading in headings.iter() {
            let row = ListBoxRow::new();
            row.set_activatable(true);
            let label = Label::new(Some(heading));
            label.set_xalign(0.0);
            label.set_hexpand(true);
            label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            label.set_margin_start(8);
            label.set_margin_end(8);
            label.set_margin_top(4);
            label.set_margin_bottom(4);
            row.set_child(Some(&label));
            self.list_box.append(&row);
        }
    }

    fn flush_current_note(&self) {
        let key = self.selected_key.borrow().clone();
        if let Some(key) = key {
            let buf = self.text_view.buffer();
            let text = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
            self.notes.borrow_mut().insert(key, text);
            if let Some(path) = self.save_path.borrow().clone() {
                let notes = self.notes.borrow().clone();
                if let Ok(json) = serde_json::to_string_pretty(&notes) {
                    let _ = std::fs::write(&path, json);
                }
            }
        }
    }
}

fn notes_path_for(file: &Path) -> PathBuf {
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("_");
    file.with_file_name(format!("{stem}.notes.json"))
}

fn parse_headings(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            if !line.starts_with('=') { return None; }
            let stripped = line.trim_start_matches('=');
            let level = line.len() - stripped.len();
            if level == 0 || !stripped.starts_with(' ') { return None; }
            let text = stripped.trim_start().to_string();
            if text.is_empty() { return None; }
            Some(text)
        })
        .collect()
}
