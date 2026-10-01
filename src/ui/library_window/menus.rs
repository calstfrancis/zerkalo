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
    pub(super) fn show_doc_menu(
        &self,
        row: &ListBoxRow,
        doc: &crate::library::Document,
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

        let is_trash = *self.current_filter.borrow() == LibraryFilter::Trash;
        if is_trash {
            let restore_b = mk("Restore");
            {
                let this = self.clone();
                let id = doc.id;
                let pop = popover.clone();
                restore_b.connect_clicked(move |_| {
                    pop.popdown();
                    this.library.borrow_mut().restore_from_trash(id).ok();
                    this.refresh();
                });
            }
            vbox.append(&restore_b);

            vbox.append(&Separator::new(Orientation::Horizontal));

            let del_b = mk("Permanently Delete…");
            del_b.add_css_class("error");
            {
                let this = self.clone();
                let doc = doc.clone();
                let pop = popover.clone();
                del_b.connect_clicked(move |_| {
                    pop.popdown();
                    this.permanent_delete_dialog(&doc);
                });
            }
            vbox.append(&del_b);

            popover.set_child(Some(&vbox));
            popup_after_click(&popover);
            return;
        }

        let open_b = mk("Open");
        {
            let this = self.clone();
            let id = doc.id;
            let pop = popover.clone();
            open_b.connect_clicked(move |_| {
                pop.popdown();
                this.open_doc_by_id(id);
            });
        }
        vbox.append(&open_b);

        let export_b = mk("Export…");
        {
            let this = self.clone();
            let doc = doc.clone();
            let pop = popover.clone();
            export_b.connect_clicked(move |_| {
                pop.popdown();
                this.export_doc_dialog(&doc);
            });
        }
        vbox.append(&export_b);

        let rename_b = mk("Rename…");
        {
            let this = self.clone();
            let doc = doc.clone();
            let pop = popover.clone();
            rename_b.connect_clicked(move |_| {
                pop.popdown();
                this.rename_doc_dialog(&doc);
            });
        }
        vbox.append(&rename_b);

        // Only offered for documents saved outside the Zerkalo folder — the
        // common case is a document dragged in from elsewhere, or saved
        // before name-only New Document existed. Those don't reliably see
        // the project's fonts and bibliography.
        if !doc.path.starts_with(&self.work_dir) {
            let move_b = mk("Move into Zerkalo Folder…");
            let this = self.clone();
            let doc = doc.clone();
            let pop = popover.clone();
            move_b.connect_clicked(move |_| {
                pop.popdown();
                this.move_into_work_dir(&doc);
            });
            vbox.append(&move_b);
        }

        let cat_b = mk("Edit Categories…");
        {
            let this = self.clone();
            let id = doc.id;
            let pop = popover.clone();
            cat_b.connect_clicked(move |_| {
                pop.popdown();
                this.edit_categories_dialog(id);
            });
        }
        vbox.append(&cat_b);

        let tags_b = mk("Edit Tags…");
        {
            let this = self.clone();
            let id = doc.id;
            let pop = popover.clone();
            tags_b.connect_clicked(move |_| {
                pop.popdown();
                this.edit_tags_dialog(id);
            });
        }
        vbox.append(&tags_b);

        let notes_b = mk("Edit Notes…");
        {
            let this = self.clone();
            let doc = doc.clone();
            let pop = popover.clone();
            notes_b.connect_clicked(move |_| {
                pop.popdown();
                this.edit_notes_dialog(&doc);
            });
        }
        vbox.append(&notes_b);

        let project_b = mk("Add to Project…");
        {
            let this = self.clone();
            let id = doc.id;
            let pop = popover.clone();
            project_b.connect_clicked(move |_| {
                pop.popdown();
                this.add_to_project_dialog(id);
            });
        }
        vbox.append(&project_b);

        let maybe_pid = match *self.current_filter.borrow() {
            LibraryFilter::Project(pid) => Some(pid),
            _ => None,
        };
        if let Some(pid) = maybe_pid {
            let root_b = mk("Set as Project Root");
            let this = self.clone();
            let id = doc.id;
            let pop = popover.clone();
            root_b.connect_clicked(move |_| {
                pop.popdown();
                this.library
                    .borrow_mut()
                    .set_project_root(pid, Some(id))
                    .ok();
            });
            vbox.append(&root_b);
        }

        let pin_label = if doc.pinned { "Unpin" } else { "Pin to Top" };
        let pin_b = mk(pin_label);
        {
            let this = self.clone();
            let id = doc.id;
            let pinned = doc.pinned;
            let pop = popover.clone();
            pin_b.connect_clicked(move |_| {
                pop.popdown();
                this.library.borrow_mut().set_pinned(id, !pinned).ok();
                this.populate_doc_list();
            });
        }
        vbox.append(&pin_b);

        let arch_label = if doc.archived { "Unarchive" } else { "Archive" };
        let arch_b = mk(arch_label);
        {
            let this = self.clone();
            let id = doc.id;
            let archived = doc.archived;
            let title = doc.title.clone();
            let pop = popover.clone();
            arch_b.connect_clicked(move |_| {
                pop.popdown();
                this.library.borrow_mut().set_archived(id, !archived).ok();
                this.refresh();
                if !archived {
                    let undo = this.clone();
                    this.toast_with_undo(&format!("Archived \u{201c}{title}\u{201d}"), move || {
                        undo.library.borrow_mut().set_archived(id, false).ok();
                        undo.refresh();
                    });
                }
            });
        }
        vbox.append(&arch_b);

        vbox.append(&Separator::new(Orientation::Horizontal));

        let remove_b = mk("Remove from list");
        {
            let this = self.clone();
            let id = doc.id;
            let pop = popover.clone();
            let win = self.window.clone();
            remove_b.connect_clicked(move |_| {
                pop.popdown();
                let this2 = this.clone();
                crate::ui::confirm::confirm_destructive(
                    Some(win.upcast_ref()),
                    "Remove from list?",
                    "The document's file on disk isn't touched — this only removes it from \
                     this list. Zerkalo will find it again if you open it or its folder is \
                     rescanned.",
                    "Remove",
                    move || {
                        this2.library.borrow_mut().remove_document(id).ok();
                        this2.refresh();
                    },
                );
            });
        }
        vbox.append(&remove_b);

        let trash_b = mk("Delete");
        trash_b.add_css_class("error");
        {
            let this = self.clone();
            let id = doc.id;
            let title = doc.title.clone();
            let pop = popover.clone();
            trash_b.connect_clicked(move |_| {
                pop.popdown();
                let moved = this.library.borrow_mut().move_to_trash(id);
                match moved {
                    Err(e) => {
                        tracing::error!("move_to_trash failed: {e}");
                        let toast = adw::Toast::new(&format!(
                            "Couldn't move to the trash — {}.",
                            e.user_message()
                        ));
                        this.toast_overlay.add_toast(toast);
                    }
                    Ok(()) => {
                        let undo = this.clone();
                        this.toast_with_undo(
                            &format!("Moved \u{201c}{title}\u{201d} to Trash"),
                            move || {
                                undo.library.borrow_mut().restore_from_trash(id).ok();
                                undo.refresh();
                            },
                        );
                    }
                }
                this.refresh();
            });
        }
        vbox.append(&trash_b);

        popover.set_child(Some(&vbox));
        popup_after_click(&popover);
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
