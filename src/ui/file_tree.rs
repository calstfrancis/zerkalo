use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4::gdk::Rectangle;
use gtk4::prelude::*;
use gtk4::{
    Align, AlertDialog, Box as GtkBox, Button, Entry, GestureClick, Label, ListBox, ListBoxRow,
    Orientation, Popover, ScrolledWindow, SelectionMode, Separator,
};

type Callback<T> = Rc<RefCell<Option<Box<dyn Fn(T)>>>>;

#[derive(Clone)]
pub struct FileTree {
    root_widget: GtkBox,
    list_box: ListBox,
    project_root: Rc<PathBuf>,
    on_open: Callback<PathBuf>,
    on_new_file: Callback<String>,
    on_delete: Callback<PathBuf>,
}

impl FileTree {
    pub fn new(project_root: PathBuf) -> Self {
        let root_widget = GtkBox::new(Orientation::Vertical, 0);
        root_widget.set_hexpand(false);
        root_widget.set_vexpand(true);

        // ── Header row ─────────────────────────────────────────────────────
        let header_row = GtkBox::new(Orientation::Horizontal, 0);
        header_row.set_margin_top(8);
        header_row.set_margin_bottom(6);
        header_row.set_margin_start(10);
        header_row.set_margin_end(6);

        let header_lbl = Label::new(Some("Files"));
        header_lbl.add_css_class("heading");
        header_lbl.set_hexpand(true);
        header_lbl.set_halign(Align::Start);
        header_row.append(&header_lbl);

        let new_btn = Button::from_icon_name("list-add-symbolic");
        new_btn.add_css_class("flat");
        new_btn.set_tooltip_text(Some("New file"));
        header_row.append(&new_btn);

        root_widget.append(&header_row);
        root_widget.append(&Separator::new(Orientation::Horizontal));

        // ── New-file popover ────────────────────────────────────────────────
        let nf_popover = Popover::new();
        let nf_box = GtkBox::new(Orientation::Vertical, 6);
        nf_box.set_margin_top(10);
        nf_box.set_margin_bottom(10);
        nf_box.set_margin_start(10);
        nf_box.set_margin_end(10);
        let nf_lbl = Label::new(Some("New file name"));
        nf_lbl.set_halign(Align::Start);
        let nf_entry = Entry::new();
        nf_entry.set_placeholder_text(Some("filename.typ"));
        nf_entry.set_width_chars(18);
        nf_box.append(&nf_lbl);
        nf_box.append(&nf_entry);
        nf_popover.set_child(Some(&nf_box));
        nf_popover.set_parent(&new_btn);

        let pop_for_btn = nf_popover.clone();
        let entry_for_btn = nf_entry.clone();
        new_btn.connect_clicked(move |_| {
            entry_for_btn.set_text("");
            pop_for_btn.popup();
        });

        // ── File list ───────────────────────────────────────────────────────
        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::Single);
        list_box.add_css_class("navigation-sidebar");

        let scrolled = ScrolledWindow::new();
        scrolled.set_child(Some(&list_box));
        scrolled.set_vexpand(true);
        scrolled.set_min_content_width(200);
        root_widget.append(&scrolled);

        let on_open: Callback<PathBuf> = Rc::new(RefCell::new(None));
        let on_new_file: Callback<String> = Rc::new(RefCell::new(None));
        let on_delete: Callback<PathBuf> = Rc::new(RefCell::new(None));

        // Wire new-file entry: Enter creates the file
        let pop_for_entry = nf_popover.clone();
        let cb_new = on_new_file.clone();
        nf_entry.connect_activate(move |entry| {
            let name = entry.text().trim().to_string();
            if !name.is_empty() {
                if let Some(f) = cb_new.borrow().as_ref() {
                    f(name);
                }
                pop_for_entry.popdown();
            }
        });

        let ft = Self {
            root_widget,
            list_box,
            project_root: Rc::new(project_root),
            on_open,
            on_new_file,
            on_delete,
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

    pub fn set_on_new_file(&self, f: impl Fn(String) + 'static) {
        *self.on_new_file.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_delete(&self, f: impl Fn(PathBuf) + 'static) {
        *self.on_delete.borrow_mut() = Some(Box::new(f));
    }

    pub fn refresh(&self) {
        while let Some(row) = self.list_box.row_at_index(0) {
            self.list_box.remove(&row);
        }

        let files = crate::project::collect_typ_files(&self.project_root);

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

        // Group files by relative directory
        let mut by_dir: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
        for file in &files {
            let rel = file.strip_prefix(self.project_root.as_ref()).unwrap_or(file);
            let dir = rel.parent().unwrap_or(Path::new("")).to_path_buf();
            by_dir.entry(dir).or_default().push(file.clone());
        }

        for (dir, dir_files) in &by_dir {
            // Dim subdirectory header
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

                // Left-click: open the file
                let open_cb = self.on_open.clone();
                let path_open = file_path.clone();
                row.connect_activate(move |_| {
                    if let Some(f) = open_cb.borrow().as_ref() {
                        f(path_open.clone());
                    }
                });

                // Right-click: delete context popover
                self.attach_delete_gesture(&row, file_path, &filename);

                self.list_box.append(&row);
            }
        }
    }

    /// Attach a right-click gesture to `row` that shows a delete popover.
    fn attach_delete_gesture(&self, row: &ListBoxRow, file_path: &PathBuf, filename: &str) {
        let del_popover = Popover::new();
        del_popover.set_has_arrow(false);

        let del_btn = Button::with_label("Delete");
        del_btn.add_css_class("destructive-action");
        let btn_box = GtkBox::new(Orientation::Vertical, 0);
        btn_box.set_margin_top(4);
        btn_box.set_margin_bottom(4);
        btn_box.set_margin_start(4);
        btn_box.set_margin_end(4);
        btn_box.append(&del_btn);
        del_popover.set_child(Some(&btn_box));
        del_popover.set_parent(row);

        // Show popover on right-click
        let pop_for_gesture = del_popover.clone();
        let gesture = GestureClick::new();
        gesture.set_button(3);
        gesture.connect_pressed(move |_, _, x, y| {
            pop_for_gesture.set_pointing_to(Some(&Rectangle::new(x as i32, y as i32, 1, 1)));
            pop_for_gesture.popup();
        });
        row.add_controller(gesture);

        // Delete button: confirm then fire callback
        let pop_for_del = del_popover.clone();
        let path_del = file_path.clone();
        let name_del = filename.to_owned();
        let del_cb = self.on_delete.clone();
        del_btn.connect_clicked(move |_| {
            pop_for_del.popdown();

            let alert = AlertDialog::builder()
                .modal(true)
                .message("Delete this file?")
                .detail(&format!("'{}' will be permanently deleted.", name_del))
                .buttons(["Cancel", "Delete"])
                .cancel_button(0)
                .default_button(0)
                .build();

            let path_cb = path_del.clone();
            let cb = del_cb.clone();
            alert.choose(
                None::<&gtk4::Window>,
                None::<&gtk4::gio::Cancellable>,
                move |result| {
                    if result == Ok(1) {
                        if let Some(f) = cb.borrow().as_ref() {
                            f(path_cb.clone());
                        }
                    }
                },
            );
        });
    }
}
