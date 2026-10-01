use super::*;

/// Every right-click context menu in this window (document, project,
/// category rows) builds a plain `Popover` and needs to show it from a
/// button-3 `connect_pressed` handler, which runs before that same click's
/// button-*release* has landed. Calling `popover.popup()` synchronously
/// there starts the autohide grab immediately, and the release then reads as
/// an outside click and dismisses the popover the instant it arrives —
/// visible as the menu opening and closing instantly on a normal fast
/// right-click. A short real delay survives this regardless of how the
/// backend batches input events (an idle-priority deferral does not — see
/// editor_pane.rs's spell-suggestions popover for the fuller writeup and why
/// that weaker fix wasn't enough on its own).
pub(super) fn popup_after_click(popover: &Popover) {
    let popover = popover.clone();
    glib::timeout_add_local_once(Duration::from_millis(40), move || {
        popover.popup();
    });
}

impl LibraryWindow {
    /// Asks where a missing document's file went and points the library at it.
    fn locate_dialog(&self, doc: &crate::library::Document) {
        let dialog = gtk4::FileDialog::new();
        dialog.set_title(&format!("Locate \u{201c}{}\u{201d}", doc.title));
        let filter = gtk4::FileFilter::new();
        filter.add_pattern("*.typ");
        filter.set_name(Some("Typst files"));
        let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));
        dialog.set_initial_folder(Some(&gtk4::gio::File::for_path(&self.work_dir)));
        let this = self.clone();
        let (id, title) = (doc.id, doc.title.clone());
        dialog.open(
            Some(&self.window),
            gtk4::gio::Cancellable::NONE,
            move |res| {
                let Some(path) = res.ok().and_then(|f| f.path()) else {
                    return;
                };
                let moved = this.library.borrow_mut().update_path(id, &path);
                match moved {
                    Ok(()) => {
                        this.refresh();
                        let toast = adw::Toast::new(&format!("Found \u{201c}{title}\u{201d}"));
                        toast.set_use_markup(false);
                        this.toast_overlay.add_toast(toast);
                    }
                    Err(_) => {
                        // That file is already listed as another document.
                        let toast = adw::Toast::new(
                            "That file is already in the Library as another document.",
                        );
                        this.toast_overlay.add_toast(toast);
                    }
                }
            },
        );
    }

    /// The `doc.*` actions the document menus (the row's ⋯ button and its
    /// right-click) call. Each takes the document's id as its target, looks the
    /// document up fresh, and does what the old hand-built popover's buttons did.
    pub(super) fn install_doc_actions(&self) {
        use crate::library::Document;
        let group = gtk4::gio::SimpleActionGroup::new();
        let add = |name: &str, run: Box<dyn Fn(&LibraryWindow, Document)>| {
            let action = gtk4::gio::SimpleAction::new(name, Some(glib::VariantTy::INT64));
            let this = self.clone();
            action.connect_activate(move |_, param| {
                let Some(id) = param.and_then(|p| p.get::<i64>()) else {
                    return;
                };
                let doc = this.library.borrow().doc_by_id(id).ok().flatten();
                if let Some(doc) = doc {
                    run(&this, doc);
                }
            });
            group.add_action(&action);
        };

        add("open", Box::new(|t, d| t.open_doc_by_id(d.id)));
        add("rename", Box::new(|t, d| t.rename_doc_dialog(&d)));
        add("locate", Box::new(|t, d| t.locate_dialog(&d)));
        add("export", Box::new(|t, d| t.export_doc_dialog(&d)));
        add("organize", Box::new(|t, d| t.organize_from_row(d.id)));
        add("notes", Box::new(|t, d| t.edit_notes_dialog(&d)));
        add("move-in", Box::new(|t, d| t.move_into_work_dir(&d)));
        add(
            "set-root",
            Box::new(|t, d| {
                let pid = match *t.current_filter.borrow() {
                    LibraryFilter::Project(pid) => Some(pid),
                    _ => None,
                };
                if let Some(pid) = pid {
                    t.library
                        .borrow_mut()
                        .set_project_root(pid, Some(d.id))
                        .ok();
                }
            }),
        );
        add(
            "pin",
            Box::new(|t, d| {
                t.library.borrow_mut().set_pinned(d.id, !d.pinned).ok();
                t.populate_doc_list();
            }),
        );
        add(
            "archive",
            Box::new(|t, d| {
                t.library.borrow_mut().set_archived(d.id, !d.archived).ok();
                t.refresh();
                if !d.archived {
                    let undo = t.clone();
                    let id = d.id;
                    t.toast_with_undo(
                        &format!("Archived \u{201c}{}\u{201d}", d.title),
                        move || {
                            undo.library.borrow_mut().set_archived(id, false).ok();
                            undo.refresh();
                        },
                    );
                }
            }),
        );
        add(
            "remove",
            Box::new(|t, d| {
                let this = t.clone();
                let id = d.id;
                crate::ui::confirm::confirm_destructive(
                    Some(t.window.upcast_ref()),
                    "Remove from list?",
                    "The document's file on disk isn't touched — this only removes it from \
                     this list. Zerkalo will find it again if you open it or its folder is \
                     rescanned.",
                    "Remove",
                    move || {
                        this.library.borrow_mut().remove_document(id).ok();
                        this.refresh();
                    },
                );
            }),
        );
        add("delete", Box::new(|t, d| t.trash_document(d.id)));
        add(
            "restore",
            Box::new(|t, d| {
                t.library.borrow_mut().restore_from_trash(d.id).ok();
                t.refresh();
            }),
        );
        add(
            "delete-forever",
            Box::new(|t, d| t.permanent_delete_dialog(&d)),
        );

        self.window.insert_action_group("doc", Some(&group));
    }

    /// The menu for one document, grouped by what the items do rather than
    /// listed flat: using it, organizing it, putting it away, and the rarely
    /// wanted rest under More.
    pub(super) fn doc_menu_model(&self, doc_id: i64) -> Option<gtk4::gio::Menu> {
        use gtk4::gio::{Menu, MenuItem};
        let doc = self.library.borrow().doc_by_id(doc_id).ok().flatten()?;
        let id = doc_id.to_variant();
        let item = |label: &str, action: &str| {
            let it = MenuItem::new(Some(label), None);
            it.set_action_and_target_value(Some(action), Some(&id));
            it
        };
        let menu = Menu::new();

        if *self.current_filter.borrow() == LibraryFilter::Trash {
            let s = Menu::new();
            s.append_item(&item("Restore", "doc.restore"));
            s.append_item(&item("Delete Forever…", "doc.delete-forever"));
            menu.append_section(None, &s);
            return Some(menu);
        }

        let open = Menu::new();
        if doc.path.exists() {
            open.append_item(&item("Open", "doc.open"));
            open.append_item(&item("Rename…", "doc.rename"));
            open.append_item(&item("Export…", "doc.export"));
        } else {
            // Nothing to open or export until the file is found again.
            open.append_item(&item("Locate File…", "doc.locate"));
            open.append_item(&item("Rename…", "doc.rename"));
        }
        menu.append_section(None, &open);

        let keep = Menu::new();
        keep.append_item(&item("Organize…", "doc.organize"));
        keep.append_item(&item("Edit Notes…", "doc.notes"));
        keep.append_item(&item(
            if doc.pinned { "Unpin" } else { "Pin to Top" },
            "doc.pin",
        ));
        menu.append_section(None, &keep);

        let away = Menu::new();
        away.append_item(&item(
            if doc.archived { "Unarchive" } else { "Archive" },
            "doc.archive",
        ));
        away.append_item(&item("Delete", "doc.delete"));
        menu.append_section(None, &away);

        let more = Menu::new();
        if matches!(*self.current_filter.borrow(), LibraryFilter::Project(_)) {
            more.append_item(&item("Set as Project Root", "doc.set-root"));
        }
        if !doc.path.starts_with(&self.work_dir) {
            more.append_item(&item("Move into Zerkalo Folder…", "doc.move-in"));
        }
        more.append_item(&item("Remove from list…", "doc.remove"));
        let rest = Menu::new();
        rest.append_submenu(Some("More"), &more);
        menu.append_section(None, &rest);

        Some(menu)
    }

    /// Moves a document to the Trash, with an Undo toast — what the menu's
    /// Delete does, and what dropping a document on the Trash row does.
    pub(super) fn trash_document(&self, doc_id: i64) {
        let Some(doc) = self.library.borrow().doc_by_id(doc_id).ok().flatten() else {
            return;
        };
        let moved = self.library.borrow_mut().move_to_trash(doc_id);
        match moved {
            Err(e) => {
                tracing::error!("move_to_trash failed: {e}");
                self.toast_overlay.add_toast(adw::Toast::new(&format!(
                    "Couldn't move to the trash — {}.",
                    e.user_message()
                )));
            }
            Ok(()) => {
                let undo = self.clone();
                self.toast_with_undo(
                    &format!("Moved \u{201c}{}\u{201d} to Trash", doc.title),
                    move || {
                        undo.library.borrow_mut().restore_from_trash(doc_id).ok();
                        undo.refresh();
                    },
                );
            }
        }
        self.refresh();
    }

    /// Opens the Organize popover for a document from its menu — for the whole
    /// selection if the document is part of one.
    pub(super) fn organize_from_row(&self, doc_id: i64) {
        let ids: Vec<i64> = {
            let sel = self.selection.borrow();
            if sel.len() > 1 && sel.contains(&doc_id) {
                sel.iter().copied().collect()
            } else {
                vec![doc_id]
            }
        };
        let anchor = self
            .row_widgets
            .borrow()
            .get(&doc_id)
            .map(|p| p.row.clone().upcast::<gtk4::Widget>());
        if let Some(anchor) = anchor {
            // After the menu that asked for this has finished closing, or it
            // takes the popover down with it.
            let this = self.clone();
            glib::timeout_add_local_once(Duration::from_millis(60), move || {
                this.show_organize(ids, &anchor);
            });
        }
    }

    /// Gives a sidebar row a ⋯ menu that appears on hover or focus, and the
    /// same menu on right-click, so renaming, recolouring and deleting aren't
    /// hidden behind a gesture nobody is told about.
    pub(super) fn attach_row_menu(
        &self,
        row: &ListBoxRow,
        hbox: &GtkBox,
        model: impl Fn() -> Option<gtk4::gio::Menu> + 'static,
    ) {
        let model: Rc<dyn Fn() -> Option<gtk4::gio::Menu>> = Rc::new(model);
        let more = gtk4::MenuButton::new();
        more.set_icon_name("view-more-symbolic");
        more.add_css_class("flat");
        more.add_css_class("circular");
        more.add_css_class("row-more");
        more.set_valign(Align::Center);
        more.set_tooltip_text(Some("More actions"));
        more.update_property(&[gtk4::accessible::Property::Label("More actions")]);
        {
            let model = model.clone();
            more.set_create_popup_func(move |btn| btn.set_menu_model(model().as_ref()));
        }
        more.set_opacity(0.0);
        more.set_can_target(false);
        hbox.append(&more);

        let show: Rc<dyn Fn(bool)> = {
            let m = more.clone();
            Rc::new(move |on: bool| {
                m.set_opacity(if on { 1.0 } else { 0.0 });
                m.set_can_target(on);
            })
        };
        let hover = gtk4::EventControllerMotion::new();
        let focus = gtk4::EventControllerFocus::new();
        {
            let f = show.clone();
            hover.connect_enter(move |_, _, _| f(true));
            let f = show.clone();
            hover.connect_leave(move |_| f(false));
            let f = show.clone();
            focus.connect_enter(move |_| f(true));
            let f = show.clone();
            focus.connect_leave(move |_| f(false));
        }
        row.add_controller(hover);
        row.add_controller(focus);

        let gesture = gtk4::GestureClick::new();
        gesture.set_button(3);
        let row_weak = row.downgrade();
        gesture.connect_pressed(move |g, _, x, y| {
            g.set_state(gtk4::EventSequenceState::Claimed);
            if let (Some(row), Some(menu)) = (row_weak.upgrade(), model()) {
                popup_menu_at(&row, &menu, x, y);
            }
        });
        row.add_controller(gesture);
    }

    /// Lets a document row be dropped on `row`, with the row lit while it's
    /// over it so it's clear where it will land.
    pub(super) fn add_doc_drop_target(&self, row: &ListBoxRow, on_drop: impl Fn(i64) + 'static) {
        let drop = DropTarget::new(glib::Type::STRING, gtk4::gdk::DragAction::COPY);
        drop.connect_enter(|target, _, _| {
            if let Some(w) = target.widget() {
                w.add_css_class("drop-hover");
            }
            gtk4::gdk::DragAction::COPY
        });
        drop.connect_leave(|target| {
            if let Some(w) = target.widget() {
                w.remove_css_class("drop-hover");
            }
        });
        drop.connect_drop(move |target, value, _, _| {
            if let Some(w) = target.widget() {
                w.remove_css_class("drop-hover");
            }
            match value
                .get::<String>()
                .ok()
                .and_then(|s| s.parse::<i64>().ok())
            {
                Some(id) => {
                    on_drop(id);
                    true
                }
                None => false,
            }
        });
        row.add_controller(drop);
    }

    /// The `project.*` actions: what a project's ⋯ menu does.
    pub(super) fn install_project_actions(&self) {
        let group = gtk4::gio::SimpleActionGroup::new();
        let add = |name: &str, run: Box<dyn Fn(&LibraryWindow, i64)>| {
            let action = gtk4::gio::SimpleAction::new(name, Some(glib::VariantTy::INT64));
            let this = self.clone();
            action.connect_activate(move |_, param| {
                if let Some(id) = param.and_then(|p| p.get::<i64>()) {
                    run(&this, id);
                }
            });
            group.add_action(&action);
        };
        add(
            "open-root",
            Box::new(|t, pid| {
                let root = t.library.borrow().project_root_path(pid).ok().flatten();
                if let Some(path) = root {
                    t.library.borrow_mut().touch_opened(&path).ok();
                    if let Some(cb) = t.on_open.borrow().as_ref() {
                        cb(path);
                    }
                }
            }),
        );
        add(
            "rename",
            Box::new(|t, pid| {
                let name = t.filter_label(&LibraryFilter::Project(pid));
                t.rename_project_dialog(pid, &name);
            }),
        );
        add(
            "delete",
            Box::new(|t, pid| {
                let this = t.clone();
                let name = t.filter_label(&LibraryFilter::Project(pid));
                let n = t
                    .library
                    .borrow()
                    .doc_count(&LibraryFilter::Project(pid))
                    .unwrap_or(0);
                crate::ui::confirm::confirm_destructive(
                    Some(t.window.upcast_ref()),
                    "Delete project?",
                    &format!(
                        "\u{201c}{name}\u{201d} will go; its {n} document{} stay in the Library, \
                         just no longer grouped.",
                        if n == 1 { "" } else { "s" }
                    ),
                    "Delete",
                    move || {
                        this.library.borrow_mut().delete_project(pid).ok();
                        if *this.current_filter.borrow() == LibraryFilter::Project(pid) {
                            *this.current_filter.borrow_mut() = LibraryFilter::All;
                        }
                        this.refresh();
                    },
                );
            }),
        );
        self.window.insert_action_group("project", Some(&group));
    }

    pub(super) fn project_menu_model(&self, project_id: i64) -> Option<gtk4::gio::Menu> {
        use gtk4::gio::{Menu, MenuItem};
        let id = project_id.to_variant();
        let item = |label: &str, action: &str| {
            let it = MenuItem::new(Some(label), None);
            it.set_action_and_target_value(Some(action), Some(&id));
            it
        };
        let menu = Menu::new();
        if let Ok(Some(_)) = self.library.borrow().project_root_path(project_id) {
            let s = Menu::new();
            s.append_item(&item("Open Root File", "project.open-root"));
            menu.append_section(None, &s);
        }
        let s = Menu::new();
        s.append_item(&item("Rename…", "project.rename"));
        s.append_item(&item("Delete…", "project.delete"));
        menu.append_section(None, &s);
        Some(menu)
    }

    /// The `label.*` actions: what a label's ⋯ menu does.
    pub(super) fn install_label_actions(&self) {
        let group = gtk4::gio::SimpleActionGroup::new();
        let add = |name: &str, run: Box<dyn Fn(&LibraryWindow, i64)>| {
            let action = gtk4::gio::SimpleAction::new(name, Some(glib::VariantTy::INT64));
            let this = self.clone();
            action.connect_activate(move |_, param| {
                if let Some(id) = param.and_then(|p| p.get::<i64>()) {
                    run(&this, id);
                }
            });
            group.add_action(&action);
        };
        add("rename", Box::new(|t, id| t.rename_label_dialog(id)));
        add("color", Box::new(|t, id| t.label_color_dialog(id)));
        add(
            "delete",
            Box::new(|t, id| {
                let this = t.clone();
                let name = t.filter_label(&LibraryFilter::Label(id));
                let n = t
                    .library
                    .borrow()
                    .doc_count(&LibraryFilter::Label(id))
                    .unwrap_or(0);
                crate::ui::confirm::confirm_destructive(
                    Some(t.window.upcast_ref()),
                    "Delete label?",
                    &format!(
                        "\u{201c}{name}\u{201d} will be taken off its {n} document{}. The \
                         documents themselves aren't touched.",
                        if n == 1 { "" } else { "s" }
                    ),
                    "Delete",
                    move || {
                        this.library.borrow_mut().delete_label(id).ok();
                        if *this.current_filter.borrow() == LibraryFilter::Label(id) {
                            *this.current_filter.borrow_mut() = LibraryFilter::All;
                        }
                        this.refresh();
                    },
                );
            }),
        );
        self.window.insert_action_group("label", Some(&group));
    }

    pub(super) fn label_menu_model(&self, label_id: i64) -> Option<gtk4::gio::Menu> {
        use gtk4::gio::{Menu, MenuItem};
        let id = label_id.to_variant();
        let item = |label: &str, action: &str| {
            let it = MenuItem::new(Some(label), None);
            it.set_action_and_target_value(Some(action), Some(&id));
            it
        };
        let menu = Menu::new();
        let s = Menu::new();
        s.append_item(&item("Rename…", "label.rename"));
        s.append_item(&item("Change Color…", "label.color"));
        menu.append_section(None, &s);
        let s = Menu::new();
        s.append_item(&item("Delete…", "label.delete"));
        menu.append_section(None, &s);
        Some(menu)
    }
}

/// Pops a menu up at a point inside `parent` — what a right-click does.
pub(super) fn popup_menu_at(
    parent: &impl IsA<gtk4::Widget>,
    model: &gtk4::gio::Menu,
    x: f64,
    y: f64,
) {
    let popover = gtk4::PopoverMenu::from_model(Some(model));
    popover.set_parent(parent);
    popover.set_has_arrow(true);
    popover.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    popover.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    popup_after_click(popover.upcast_ref());
}
