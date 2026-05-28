use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, CheckButton, Entry, Label, ListBox, ListBoxRow, Orientation,
    Paned, ScrolledWindow, SelectionMode,
};

// ── Serialised item format ─────────────────────────────────────────────────────
// Lines: "- [ ] text" (open)  or  "- [x] text" (done)
// Other lines are preserved verbatim in the file but not shown as checkboxes.

#[derive(Clone)]
struct Item {
    text: String,
    done: bool,
}

fn parse_items(s: &str) -> Vec<Item> {
    s.lines()
        .filter_map(|l| {
            if let Some(rest) = l.strip_prefix("- [ ] ") {
                Some(Item { text: rest.to_string(), done: false })
            } else if let Some(rest) = l.strip_prefix("- [x] ") {
                Some(Item { text: rest.to_string(), done: true })
            } else if !l.trim().is_empty() && !l.starts_with("---") {
                // treat plain text lines as open items
                Some(Item { text: l.to_string(), done: false })
            } else {
                None
            }
        })
        .collect()
}

fn items_to_string(items: &[Item]) -> String {
    let mut out = String::new();
    let open: Vec<&Item> = items.iter().filter(|i| !i.done).collect();
    let done: Vec<&Item> = items.iter().filter(|i| i.done).collect();
    for i in &open {
        out.push_str(&format!("- [ ] {}\n", i.text));
    }
    if !done.is_empty() {
        out.push_str("--- Completed ---\n");
        for i in &done {
            out.push_str(&format!("- [x] {}\n", i.text));
        }
    }
    out
}

// ── Per-list component ─────────────────────────────────────────────────────────

#[derive(Clone)]
struct TodoList {
    widget: GtkBox,
    list_box: ListBox,
    items: Rc<RefCell<Vec<Item>>>,
    save_path: Rc<RefCell<Option<PathBuf>>>,
    is_loading: Rc<RefCell<bool>>,
}

impl TodoList {
    fn new() -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 0);

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_margin_start(4);
        scroll.set_margin_end(4);
        scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);

        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::None);
        list_box.add_css_class("boxed-list");
        list_box.set_margin_top(4);
        list_box.set_margin_bottom(4);
        scroll.set_child(Some(&list_box));
        widget.append(&scroll);

        // Add-item entry row
        let entry_row = GtkBox::new(Orientation::Horizontal, 4);
        entry_row.set_margin_start(8);
        entry_row.set_margin_end(8);
        entry_row.set_margin_top(4);
        entry_row.set_margin_bottom(6);

        let entry = Entry::new();
        entry.set_placeholder_text(Some("Add item…"));
        entry.set_hexpand(true);

        let add_btn = Button::from_icon_name("list-add-symbolic");
        add_btn.add_css_class("flat");
        add_btn.set_tooltip_text(Some("Add item (Enter)"));

        entry_row.append(&entry);
        entry_row.append(&add_btn);
        widget.append(&entry_row);

        let items: Rc<RefCell<Vec<Item>>> = Rc::new(RefCell::new(Vec::new()));
        let save_path: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
        let is_loading: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        let list = Self { widget, list_box, items, save_path, is_loading };

        // Wire add button + enter key
        {
            let list2 = list.clone();
            let e = entry.clone();
            add_btn.connect_clicked(move |_| {
                let text = e.text().trim().to_string();
                if !text.is_empty() {
                    list2.add_item(text);
                    e.set_text("");
                }
            });
        }
        {
            let list2 = list.clone();
            entry.connect_activate(move |e| {
                let text = e.text().trim().to_string();
                if !text.is_empty() {
                    list2.add_item(text);
                    e.set_text("");
                }
            });
        }

        list
    }

    fn widget(&self) -> &GtkBox { &self.widget }

    fn load(&self, path: &PathBuf) {
        *self.save_path.borrow_mut() = Some(path.clone());
        *self.is_loading.borrow_mut() = true;
        let text = std::fs::read_to_string(path).unwrap_or_default();
        *self.items.borrow_mut() = parse_items(&text);
        self.rebuild();
        *self.is_loading.borrow_mut() = false;
    }

    fn load_text(&self, text: &str) {
        *self.is_loading.borrow_mut() = true;
        *self.items.borrow_mut() = parse_items(text);
        self.rebuild();
        *self.is_loading.borrow_mut() = false;
    }

    fn add_item(&self, text: String) {
        self.items.borrow_mut().push(Item { text, done: false });
        self.rebuild();
        self.schedule_save();
    }

    fn toggle_item(&self, idx: usize) {
        let mut items = self.items.borrow_mut();
        if let Some(item) = items.get_mut(idx) {
            item.done = !item.done;
            // Sort: open items first, done items at end (stable)
            items.sort_by_key(|i| i.done as u8);
        }
        drop(items);
        self.rebuild();
        self.schedule_save();
    }

    fn delete_item(&self, idx: usize) {
        let mut items = self.items.borrow_mut();
        if idx < items.len() {
            items.remove(idx);
        }
        drop(items);
        self.rebuild();
        self.schedule_save();
    }

    fn rebuild(&self) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        let items = self.items.borrow();
        let open_count = items.iter().filter(|i| !i.done).count();

        for (idx, item) in items.iter().enumerate() {
            if idx == open_count && !items[idx..].is_empty() {
                // Completed separator
                let sep_row = ListBoxRow::new();
                sep_row.set_activatable(false);
                sep_row.set_selectable(false);
                let sep_lbl = Label::new(Some("Completed"));
                sep_lbl.add_css_class("caption");
                sep_lbl.add_css_class("dim-label");
                sep_lbl.set_margin_start(8);
                sep_lbl.set_margin_top(4);
                sep_lbl.set_margin_bottom(2);
                sep_lbl.set_xalign(0.0);
                sep_row.set_child(Some(&sep_lbl));
                self.list_box.append(&sep_row);
            }

            let row = ListBoxRow::new();
            row.set_activatable(false);
            row.set_selectable(false);

            let row_box = GtkBox::new(Orientation::Horizontal, 4);
            row_box.set_margin_start(4);
            row_box.set_margin_end(4);
            row_box.set_margin_top(2);
            row_box.set_margin_bottom(2);

            let check = CheckButton::new();
            check.set_active(item.done);
            check.set_valign(gtk4::Align::Center);

            let lbl = Label::new(Some(&item.text));
            lbl.set_hexpand(true);
            lbl.set_xalign(0.0);
            lbl.set_wrap(true);
            lbl.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
            if item.done {
                lbl.add_css_class("dim-label");
                // Strikethrough via pango attributes
                let attrs = gtk4::pango::AttrList::new();
                attrs.insert(gtk4::pango::AttrInt::new_strikethrough(true));
                lbl.set_attributes(Some(&attrs));
            }

            let del_btn = Button::from_icon_name("edit-delete-symbolic");
            del_btn.add_css_class("flat");
            del_btn.add_css_class("circular");
            del_btn.set_valign(gtk4::Align::Center);
            del_btn.set_tooltip_text(Some("Remove"));

            row_box.append(&check);
            row_box.append(&lbl);
            row_box.append(&del_btn);
            row.set_child(Some(&row_box));
            self.list_box.append(&row);

            // Wire checkbox
            {
                let list2 = self.clone();
                check.connect_toggled(move |_| {
                    if !*list2.is_loading.borrow() {
                        list2.toggle_item(idx);
                    }
                });
            }
            // Wire delete
            {
                let list2 = self.clone();
                del_btn.connect_clicked(move |_| {
                    list2.delete_item(idx);
                });
            }
        }
    }

    fn schedule_save(&self) {
        if *self.is_loading.borrow() { return; }
        let items = self.items.borrow().clone();
        let path_opt = self.save_path.borrow().clone();
        let text = items_to_string(&items);
        if let Some(path) = path_opt {
            glib::timeout_add_local_once(Duration::from_millis(200), move || {
                let _ = std::fs::write(&path, &text);
            });
        }
    }
}

// ── Public panel ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct TodoPanel {
    widget: GtkBox,
    #[allow(dead_code)] global_list: TodoList,
    file_list: TodoList,
    file_header_label: Label,
    current_file: Rc<RefCell<Option<PathBuf>>>,
}

impl TodoPanel {
    pub fn new() -> Self {
        let root = GtkBox::new(Orientation::Vertical, 0);

        // ── Global TODO section ─────────────────────────────────────────────
        let global_section = GtkBox::new(Orientation::Vertical, 0);

        let global_header = GtkBox::new(Orientation::Horizontal, 4);
        global_header.set_margin_start(8);
        global_header.set_margin_end(8);
        global_header.set_margin_top(6);
        global_header.set_margin_bottom(2);

        let global_title = Label::new(Some("Global TODO"));
        global_title.set_hexpand(true);
        global_title.set_xalign(0.0);
        global_title.add_css_class("heading");
        global_header.append(&global_title);
        global_section.append(&global_header);

        let global_list = TodoList::new();
        global_section.append(global_list.widget());

        // Load global list from disk
        let global_path = global_todo_path();
        if global_path.exists() {
            global_list.load(&global_path);
        } else {
            // Initialise save path even if file doesn't exist yet
            *global_list.save_path.borrow_mut() = Some(global_path);
        }

        // ── File TODO section ───────────────────────────────────────────────
        let file_section = GtkBox::new(Orientation::Vertical, 0);

        let file_header = GtkBox::new(Orientation::Horizontal, 4);
        file_header.set_margin_start(8);
        file_header.set_margin_end(8);
        file_header.set_margin_top(6);
        file_header.set_margin_bottom(2);

        let file_header_label = Label::new(Some("File TODO"));
        file_header_label.set_hexpand(true);
        file_header_label.set_xalign(0.0);
        file_header_label.add_css_class("heading");
        file_header_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        file_header.append(&file_header_label);
        file_section.append(&file_header);

        let file_list = TodoList::new();
        file_section.append(file_list.widget());

        // ── Paned divider ───────────────────────────────────────────────────
        let paned = Paned::new(Orientation::Vertical);
        paned.set_vexpand(true);
        paned.set_resize_start_child(true);
        paned.set_resize_end_child(true);
        paned.set_shrink_start_child(true);
        paned.set_shrink_end_child(true);
        paned.set_start_child(Some(&global_section));
        paned.set_end_child(Some(&file_section));
        root.append(&paned);

        let current_file = Rc::new(RefCell::new(None));

        Self { widget: root, global_list, file_list, file_header_label, current_file }
    }

    pub fn widget(&self) -> &GtkBox { &self.widget }

    pub fn set_current_file(&self, path: Option<&PathBuf>) {
        *self.current_file.borrow_mut() = path.cloned();
        match path {
            None => {
                self.file_header_label.set_text("File TODO");
                self.file_list.load_text("");
                *self.file_list.save_path.borrow_mut() = None;
            }
            Some(p) => {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                self.file_header_label.set_text(name);
                let todo_path = todo_path_for(p);
                self.file_list.load(&todo_path);
                *self.file_list.save_path.borrow_mut() = Some(todo_path);
            }
        }
    }
}

fn global_todo_path() -> PathBuf {
    let base = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(base).join(".local/share/zerkalo/global-todo.md")
}

fn todo_path_for(file: &PathBuf) -> PathBuf {
    let name = format!(
        "{}.todo",
        file.file_name().and_then(|n| n.to_str()).unwrap_or("_")
    );
    file.with_file_name(name)
}
