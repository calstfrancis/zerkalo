use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4::gdk::Rectangle;
use gtk4::prelude::*;
use gtk4::{
    Align, AlertDialog, Box as GtkBox, Button, DragSource, DropTarget, Entry, GestureClick,
    Label, ListBox, ListBoxRow, Orientation, Popover, ScrolledWindow, SelectionMode, Separator,
};

type Callback<T> = Rc<RefCell<Option<Box<dyn Fn(T)>>>>;

#[derive(Clone)]
pub struct FileTree {
    #[allow(dead_code)]
    root_widget: GtkBox,
    list_box: ListBox,
    project_root: Rc<PathBuf>,
    on_open: Callback<PathBuf>,
    on_new_file: Callback<String>,
    on_delete: Callback<PathBuf>,
    on_set_root: Callback<PathBuf>,
    on_new_folder: Callback<String>,
    file_errors: Rc<RefCell<HashSet<PathBuf>>>,
    modified_files: Rc<RefCell<HashSet<PathBuf>>>,
    /// Current compilation root, used to render the ★ indicator.
    root_file: Rc<RefCell<Option<PathBuf>>>,
    /// Directories (relative to project_root) that are currently collapsed.
    collapsed_dirs: Rc<RefCell<HashSet<PathBuf>>>,
    /// User-defined display order: full paths in preferred display order.
    custom_order: Rc<RefCell<Vec<PathBuf>>>,
    /// Path of the row being dragged (set on drag-begin).
    drag_source_path: Rc<RefCell<Option<PathBuf>>>,
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

        let new_folder_btn = Button::from_icon_name("folder-new-symbolic");
        new_folder_btn.add_css_class("flat");
        new_folder_btn.set_tooltip_text(Some("New folder"));
        header_row.append(&new_folder_btn);

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

        // ── New-folder popover ──────────────────────────────────────────────
        let nd_popover = Popover::new();
        let nd_box = GtkBox::new(Orientation::Vertical, 6);
        nd_box.set_margin_top(10);
        nd_box.set_margin_bottom(10);
        nd_box.set_margin_start(10);
        nd_box.set_margin_end(10);
        let nd_lbl = Label::new(Some("New folder name"));
        nd_lbl.set_halign(Align::Start);
        let nd_entry = Entry::new();
        nd_entry.set_placeholder_text(Some("folder-name"));
        nd_entry.set_width_chars(18);
        nd_box.append(&nd_lbl);
        nd_box.append(&nd_entry);
        nd_popover.set_child(Some(&nd_box));
        nd_popover.set_parent(&new_folder_btn);

        let pop_for_folder_btn = nd_popover.clone();
        let entry_for_folder_btn = nd_entry.clone();
        new_folder_btn.connect_clicked(move |_| {
            entry_for_folder_btn.set_text("");
            pop_for_folder_btn.popup();
        });

        // ── File list ───────────────────────────────────────────────────────
        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::Single);
        list_box.add_css_class("navigation-sidebar");

        let scrolled = ScrolledWindow::new();
        scrolled.set_child(Some(&list_box));
        scrolled.set_vexpand(true);
        scrolled.set_min_content_width(200);
        scrolled.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        root_widget.append(&scrolled);

        let on_open: Callback<PathBuf> = Rc::new(RefCell::new(None));
        let on_new_file: Callback<String> = Rc::new(RefCell::new(None));
        let on_delete: Callback<PathBuf> = Rc::new(RefCell::new(None));
        let on_set_root: Callback<PathBuf> = Rc::new(RefCell::new(None));
        let on_new_folder: Callback<String> = Rc::new(RefCell::new(None));

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

        // Wire new-folder entry: Enter creates the folder
        let pop_for_folder_entry = nd_popover.clone();
        let cb_new_folder = on_new_folder.clone();
        nd_entry.connect_activate(move |entry| {
            let name = entry.text().trim().to_string();
            if !name.is_empty() {
                if let Some(f) = cb_new_folder.borrow().as_ref() {
                    f(name);
                }
                pop_for_folder_entry.popdown();
            }
        });

        // Load any saved custom order from project config
        let saved_order: Vec<PathBuf> = crate::config::ProjectConfig::load(&project_root)
            .map(|pc| pc.file_order.iter().map(|s| project_root.join(s)).collect())
            .unwrap_or_default();

        let ft = Self {
            root_widget,
            list_box,
            project_root: Rc::new(project_root),
            on_open,
            on_new_file,
            on_delete,
            on_set_root,
            on_new_folder,
            file_errors: Rc::new(RefCell::new(HashSet::new())),
            modified_files: Rc::new(RefCell::new(HashSet::new())),
            root_file: Rc::new(RefCell::new(None)),
            collapsed_dirs: Rc::new(RefCell::new(HashSet::new())),
            custom_order: Rc::new(RefCell::new(saved_order)),
            drag_source_path: Rc::new(RefCell::new(None)),
        };
        ft.refresh();
        ft
    }

    #[allow(dead_code)]
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

    pub fn set_on_set_root(&self, f: impl Fn(PathBuf) + 'static) {
        *self.on_set_root.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_new_folder(&self, f: impl Fn(String) + 'static) {
        *self.on_new_folder.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_root_file(&self, path: Option<PathBuf>) {
        *self.root_file.borrow_mut() = path;
        self.refresh();
    }

    pub fn set_file_error(&self, path: &Path, has_error: bool) {
        let mut errors = self.file_errors.borrow_mut();
        if has_error {
            errors.insert(path.to_path_buf());
        } else {
            errors.remove(path);
        }
        drop(errors);
        self.refresh();
    }

    pub fn set_file_modified(&self, path: &Path, modified: bool) {
        let mut set = self.modified_files.borrow_mut();
        if modified { set.insert(path.to_path_buf()); } else { set.remove(path); }
        drop(set);
        self.refresh();
    }

    pub fn grab_focus(&self) {
        self.list_box.grab_focus();
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

        // Sort files by custom order first, then alphabetically
        let custom = self.custom_order.borrow();
        let mut ordered: Vec<PathBuf> = files.clone();
        ordered.sort_by_key(|p| {
            let pos = custom.iter().position(|q| q == p);
            (pos.is_none() as usize, pos.unwrap_or(usize::MAX), p.to_string_lossy().to_string())
        });
        drop(custom);

        // Group by directory (preserve custom-sorted order within each group)
        let mut by_dir: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
        for file in &ordered {
            let rel = file.strip_prefix(self.project_root.as_ref()).unwrap_or(file);
            let dir = rel.parent().unwrap_or(Path::new("")).to_path_buf();
            by_dir.entry(dir).or_default().push(file.clone());
        }

        for (dir, dir_files) in &by_dir {
            // Collapsible subdirectory header
            if dir != Path::new("") {
                let is_collapsed = self.collapsed_dirs.borrow().contains(dir);
                let row = ListBoxRow::new();
                row.set_selectable(false);
                row.set_activatable(true);
                let hbox = GtkBox::new(Orientation::Horizontal, 4);
                let arrow = gtk4::Image::from_icon_name(
                    if is_collapsed { "pan-end-symbolic" } else { "pan-down-symbolic" },
                );
                arrow.set_pixel_size(12);
                arrow.add_css_class("dim-label");
                arrow.set_margin_start(8);
                hbox.append(&arrow);
                let lbl = Label::new(Some(&format!("{}/", dir.to_string_lossy())));
                lbl.add_css_class("dim-label");
                lbl.set_halign(Align::Start);
                lbl.set_margin_top(6);
                lbl.set_margin_bottom(2);
                hbox.append(&lbl);
                row.set_child(Some(&hbox));

                let collapsed = self.collapsed_dirs.clone();
                let dir_owned = dir.clone();
                let ft = self.clone();
                row.connect_activate(move |_| {
                    let mut dirs = collapsed.borrow_mut();
                    if dirs.contains(&dir_owned) {
                        dirs.remove(&dir_owned);
                    } else {
                        dirs.insert(dir_owned.clone());
                    }
                    drop(dirs);
                    ft.refresh();
                });
                self.list_box.append(&row);

                if is_collapsed {
                    continue;
                }
            }

            let indent: i32 = if dir == Path::new("") { 10 } else { 22 };

            for file_path in dir_files {
                let filename = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();
                let has_error = self.file_errors.borrow().contains(file_path.as_path());
                let is_modified = self.modified_files.borrow().contains(file_path.as_path());
                let is_root = self.root_file.borrow().as_ref() == Some(file_path);

                let row = ListBoxRow::new();
                // Drag handle icon to signal reorderability
                let row_box = GtkBox::new(Orientation::Horizontal, 4);
                row_box.set_hexpand(true);
                let drag_icon = gtk4::Image::from_icon_name("list-drag-handle-symbolic");
                drag_icon.set_pixel_size(12);
                drag_icon.add_css_class("dim-label");
                drag_icon.set_margin_start(4);
                row_box.append(&drag_icon);
                let lbl = Label::new(Some(&filename));
                lbl.set_halign(Align::Start);
                lbl.set_hexpand(true);
                lbl.set_margin_start(indent.saturating_sub(4));
                lbl.set_margin_top(5);
                lbl.set_margin_bottom(5);
                row_box.append(&lbl);
                if is_modified {
                    let dot = Label::new(Some("●"));
                    dot.add_css_class("accent");
                    dot.set_margin_end(4);
                    dot.set_tooltip_text(Some("Unsaved changes"));
                    row_box.append(&dot);
                }
                if has_error {
                    let err_icon = gtk4::Image::from_icon_name("dialog-error-symbolic");
                    err_icon.set_pixel_size(12);
                    err_icon.add_css_class("error");
                    err_icon.set_margin_end(6);
                    row_box.append(&err_icon);
                }
                if is_root {
                    let star = gtk4::Image::from_icon_name("starred-symbolic");
                    star.set_pixel_size(14);
                    star.add_css_class("accent");
                    star.set_margin_end(6);
                    star.set_tooltip_text(Some("Compilation root"));
                    row_box.append(&star);
                }
                row.set_child(Some(&row_box));

                // Left-click: open the file
                let open_cb = self.on_open.clone();
                let path_open = file_path.clone();
                row.connect_activate(move |_| {
                    if let Some(f) = open_cb.borrow().as_ref() {
                        f(path_open.clone());
                    }
                });

                // Right-click: context menu (set-as-root + delete)
                self.attach_context_menu(&row, file_path, &filename);

                // Drag-and-drop: reorder within the list
                self.attach_dnd(&row, file_path);

                self.list_box.append(&row);
            }
        }
    }

    /// Attach drag-source and drop-target controllers to a file row.
    fn attach_dnd(&self, row: &ListBoxRow, file_path: &PathBuf) {
        // Drag source: record which path is being dragged
        let drag_src = DragSource::new();
        drag_src.set_actions(gtk4::gdk::DragAction::MOVE);
        let src_path = file_path.clone();
        let drag_holder = self.drag_source_path.clone();
        drag_src.connect_drag_begin(move |src, _drag| {
            *drag_holder.borrow_mut() = Some(src_path.clone());
            // Provide a plain-text payload (the path string)
            let path_str = src_path.to_string_lossy().to_string();
            src.set_content(Some(&gtk4::gdk::ContentProvider::for_value(
                &path_str.to_value(),
            )));
        });
        row.add_controller(drag_src);

        // Drop target: accept a path string and reorder
        let drop_tgt = DropTarget::new(glib::Type::STRING, gtk4::gdk::DragAction::MOVE);
        let target_path = file_path.clone();
        let order = self.custom_order.clone();
        let project_root = self.project_root.clone();
        let drag_holder2 = self.drag_source_path.clone();
        let all_files_snapshot = crate::project::collect_typ_files(&self.project_root);
        drop_tgt.connect_drop(move |_, _value, _, _| {
            let src_path = drag_holder2.borrow().clone();
            let Some(src) = src_path else { return false; };
            if src == target_path { return false; }

            // Build the new order from current custom_order, inserting src before target
            let mut current: Vec<PathBuf> = {
                let ord = order.borrow();
                if ord.is_empty() {
                    all_files_snapshot.clone()
                } else {
                    // Merge: listed + unlisted (in filesystem order)
                    let mut v: Vec<PathBuf> = ord.iter().filter(|p| all_files_snapshot.contains(p)).cloned().collect();
                    for f in &all_files_snapshot { if !v.contains(f) { v.push(f.clone()); } }
                    v
                }
            };

            // Remove src from current position
            current.retain(|p| p != &src);
            // Insert before target
            if let Some(idx) = current.iter().position(|p| p == &target_path) {
                current.insert(idx, src);
            } else {
                current.push(src);
            }

            *order.borrow_mut() = current.clone();

            // Persist to .zerkalo/config.toml
            let rel_order: Vec<String> = current.iter()
                .filter_map(|p| p.strip_prefix(project_root.as_ref()).ok())
                .map(|r| r.to_string_lossy().to_string())
                .collect();
            let mut proj_cfg = crate::config::ProjectConfig::load(&project_root).unwrap_or_default();
            proj_cfg.file_order = rel_order;
            let _ = proj_cfg.save(&project_root);

            true
        });
        // Refresh display after drop completes
        let ft_for_dnd = self.clone();
        drop_tgt.connect_drop(move |_, _, _, _| {
            let ft = ft_for_dnd.clone();
            glib::idle_add_local_once(move || ft.refresh());
            false
        });
        row.add_controller(drop_tgt);
    }

    /// Attach a right-click gesture that shows a context menu with "Set as Compilation Root"
    /// and "Delete".
    fn attach_context_menu(&self, row: &ListBoxRow, file_path: &PathBuf, filename: &str) {
        let popover = Popover::new();
        popover.set_has_arrow(false);

        let btn_box = GtkBox::new(Orientation::Vertical, 2);
        btn_box.set_margin_top(4);
        btn_box.set_margin_bottom(4);
        btn_box.set_margin_start(4);
        btn_box.set_margin_end(4);

        let root_btn = Button::with_label("Set as Compilation Root");
        root_btn.add_css_class("flat");

        let del_btn = Button::with_label("Delete");
        del_btn.add_css_class("destructive-action");

        btn_box.append(&root_btn);
        btn_box.append(&del_btn);
        popover.set_child(Some(&btn_box));
        popover.set_parent(row);

        // Show popover on right-click
        let pop_for_gesture = popover.clone();
        let gesture = GestureClick::new();
        gesture.set_button(3);
        gesture.connect_pressed(move |_, _, x, y| {
            pop_for_gesture.set_pointing_to(Some(&Rectangle::new(x as i32, y as i32, 1, 1)));
            pop_for_gesture.popup();
        });
        row.add_controller(gesture);

        // "Set as Compilation Root" button
        let pop_for_root = popover.clone();
        let path_root = file_path.clone();
        let set_root_cb = self.on_set_root.clone();
        root_btn.connect_clicked(move |_| {
            pop_for_root.popdown();
            if let Some(f) = set_root_cb.borrow().as_ref() {
                f(path_root.clone());
            }
        });

        // "Delete" button: confirm then fire callback
        let pop_for_del = popover.clone();
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
