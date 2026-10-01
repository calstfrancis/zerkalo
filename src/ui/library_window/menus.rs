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
        add("project", Box::new(|t, d| t.add_to_project_dialog(d.id)));
        add(
            "categories",
            Box::new(|t, d| t.edit_categories_dialog(d.id)),
        );
        add("tags", Box::new(|t, d| t.edit_tags_dialog(d.id)));
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
        add(
            "delete",
            Box::new(|t, d| {
                let moved = t.library.borrow_mut().move_to_trash(d.id);
                match moved {
                    Err(e) => {
                        tracing::error!("move_to_trash failed: {e}");
                        t.toast_overlay.add_toast(adw::Toast::new(&format!(
                            "Couldn't move to the trash — {}.",
                            e.user_message()
                        )));
                    }
                    Ok(()) => {
                        let undo = t.clone();
                        let id = d.id;
                        t.toast_with_undo(
                            &format!("Moved \u{201c}{}\u{201d} to Trash", d.title),
                            move || {
                                undo.library.borrow_mut().restore_from_trash(id).ok();
                                undo.refresh();
                            },
                        );
                    }
                }
                t.refresh();
            }),
        );
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

        let organize = Menu::new();
        organize.append_item(&item("Add to Project…", "doc.project"));
        organize.append_item(&item("Edit Categories…", "doc.categories"));
        organize.append_item(&item("Edit Tags…", "doc.tags"));
        organize.append_item(&item("Edit Notes…", "doc.notes"));
        let keep = Menu::new();
        keep.append_submenu(Some("Organize"), &organize);
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

    pub(super) fn show_project_menu(
        &self,
        row: &ListBoxRow,
        project_id: i64,
        project_name: &str,
        x: f64,
        y: f64,
    ) {
        let popover = Popover::new();
        popover.set_parent(row);
        popover.set_has_arrow(true);
        popover.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));

        let vbox = GtkBox::new(Orientation::Vertical, 2);
        vbox.set_margin_top(4);
        vbox.set_margin_bottom(4);
        vbox.set_margin_start(4);
        vbox.set_margin_end(4);

        let mk = |label: &str| -> Button {
            let b = Button::with_label(label);
            b.add_css_class("flat");
            b.set_halign(Align::Fill);
            if let Some(child) = b.child() {
                child.set_halign(Align::Start);
            }
            b
        };

        if let Ok(Some(root_path)) = self.library.borrow().project_root_path(project_id) {
            let open_root = mk("Open Root File");
            let this = self.clone();
            let pop = popover.clone();
            open_root.connect_clicked(move |_| {
                pop.popdown();
                if let Some(cb) = this.on_open.borrow().as_ref() {
                    this.library.borrow_mut().touch_opened(&root_path).ok();
                    cb(root_path.clone());
                }
            });
            vbox.append(&open_root);
            vbox.append(&Separator::new(Orientation::Horizontal));
        }

        let rename_b = mk("Rename Project…");
        {
            let this = self.clone();
            let pop = popover.clone();
            let pname = project_name.to_string();
            rename_b.connect_clicked(move |_| {
                pop.popdown();
                this.rename_project_dialog(project_id, &pname);
            });
        }
        vbox.append(&rename_b);

        let delete_b = mk("Delete Project");
        delete_b.add_css_class("error");
        {
            let this = self.clone();
            let pop = popover.clone();
            delete_b.connect_clicked(move |_| {
                pop.popdown();
                this.library.borrow_mut().delete_project(project_id).ok();
                *this.current_filter.borrow_mut() = LibraryFilter::All;
                this.refresh();
            });
        }
        vbox.append(&delete_b);

        popover.set_child(Some(&vbox));
        popup_after_click(&popover);
    }

    pub(super) fn show_category_menu(
        &self,
        row: &ListBoxRow,
        cat_name: &str,
        has_children: bool,
        x: f64,
        y: f64,
    ) {
        let popover = Popover::new();
        popover.set_parent(row);
        popover.set_has_arrow(true);
        popover.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        let vbox = GtkBox::new(Orientation::Vertical, 2);
        vbox.set_margin_top(4);
        vbox.set_margin_bottom(4);
        vbox.set_margin_start(4);
        vbox.set_margin_end(4);
        let mk = |label: &str| -> Button {
            let b = Button::with_label(label);
            b.add_css_class("flat");
            b.set_halign(Align::Fill);
            if let Some(child) = b.child() {
                child.set_halign(Align::Start);
            }
            b
        };
        let add_sub_b = mk("Add Subcategory…");
        {
            let this = self.clone();
            let pop = popover.clone();
            let cname = cat_name.to_string();
            add_sub_b.connect_clicked(move |_| {
                pop.popdown();
                this.add_subcategory_dialog(&cname);
            });
        }
        vbox.append(&add_sub_b);
        if !has_children {
            let set_parent_b = mk("Set Parent…");
            let this = self.clone();
            let pop = popover.clone();
            let cname = cat_name.to_string();
            set_parent_b.connect_clicked(move |_| {
                pop.popdown();
                this.set_parent_dialog(&cname);
            });
            vbox.append(&set_parent_b);
        }
        let rename_b = mk("Rename Category…");
        {
            let this = self.clone();
            let pop = popover.clone();
            let cname = cat_name.to_string();
            rename_b.connect_clicked(move |_| {
                pop.popdown();
                this.rename_category_dialog(&cname);
            });
        }
        vbox.append(&rename_b);
        let recolor_b = mk("Recolor…");
        {
            let this = self.clone();
            let pop = popover.clone();
            let cname = cat_name.to_string();
            recolor_b.connect_clicked(move |_| {
                pop.popdown();
                this.recolor_category_dialog(&cname);
            });
        }
        vbox.append(&recolor_b);
        let delete_b = mk("Delete Category");
        delete_b.add_css_class("error");
        if has_children {
            delete_b.set_sensitive(false);
            delete_b.add_css_class("dim-label");
            delete_b.set_tooltip_text(Some("Remove subcategories first"));
        } else {
            let this = self.clone();
            let pop = popover.clone();
            let cname = cat_name.to_string();
            delete_b.connect_clicked(move |_| {
                pop.popdown();
                let deleted = this
                    .library
                    .borrow_mut()
                    .force_delete_category_if_no_children(&cname)
                    .unwrap_or(false);
                if !deleted {
                    let toast = adw::Toast::new("Cannot delete: subcategories exist");
                    this.toast_overlay.add_toast(toast);
                }
                *this.current_filter.borrow_mut() = LibraryFilter::All;
                this.refresh();
            });
        }
        vbox.append(&delete_b);
        popover.set_child(Some(&vbox));
        popup_after_click(&popover);
    }
}
