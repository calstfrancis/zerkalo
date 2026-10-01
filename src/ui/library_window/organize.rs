use super::*;

/// What a checkbox shows for "how many of the chosen documents have this".
fn show_membership(check: &CheckButton, have: usize, total: usize) {
    check.set_active(have == total && total > 0);
    check.set_inconsistent(have > 0 && have < total);
}

impl LibraryWindow {
    /// The one place a document (or a whole selection) is organized: its
    /// labels, its projects and whether it's pinned, in one popover that
    /// applies each change the moment it's made. It replaces a separate
    /// dialog for each of these.
    ///
    /// The list behind it is only redrawn once the popover closes — redrawing
    /// sooner would destroy the row it hangs from.
    pub(super) fn show_organize(&self, ids: Vec<i64>, anchor: &gtk4::Widget) {
        if ids.is_empty() {
            return;
        }
        let total = ids.len();
        let dirty = Rc::new(std::cell::Cell::new(false));
        let syncing = Rc::new(std::cell::Cell::new(false));

        let popover = Popover::new();
        popover.set_parent(anchor);
        popover.set_has_arrow(true);

        let vbox = GtkBox::new(Orientation::Vertical, 8);
        vbox.set_width_request(300);
        vbox.set_margin_top(12);
        vbox.set_margin_bottom(12);
        vbox.set_margin_start(12);
        vbox.set_margin_end(12);

        let heading = Label::new(Some(&if total == 1 {
            self.library
                .borrow()
                .doc_by_id(ids[0])
                .ok()
                .flatten()
                .map(|d| d.title)
                .unwrap_or_default()
        } else {
            format!("{total} documents")
        }));
        heading.add_css_class("heading");
        heading.set_halign(Align::Start);
        heading.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        vbox.append(&heading);

        // ── labels ──────────────────────────────────────────────────────
        let entry = Entry::new();
        entry.set_placeholder_text(Some("Find or create a label…"));
        vbox.append(&entry);

        let create_btn = Button::new();
        create_btn.add_css_class("flat");
        create_btn.set_halign(Align::Start);
        create_btn.set_visible(false);
        vbox.append(&create_btn);

        let list = ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::None);
        list.add_css_class("fond-list");
        let scroll = ScrolledWindow::new();
        scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        scroll.set_propagate_natural_height(true);
        scroll.set_max_content_height(220);
        scroll.set_child(Some(&list));
        vbox.append(&scroll);

        let membership = self
            .library
            .borrow()
            .label_memberships(&ids)
            .unwrap_or_default();
        // (label id, name, its checkbox) for every row in the list.
        let rows: Rc<RefCell<Vec<(i64, String, CheckButton)>>> = Rc::new(RefCell::new(Vec::new()));

        let add_row: Rc<dyn Fn(&crate::library::Label, usize)> = {
            let this = self.clone();
            let ids = ids.clone();
            let list = list.clone();
            let rows = rows.clone();
            let dirty = dirty.clone();
            let syncing = syncing.clone();
            Rc::new(move |label, have| {
                let check = CheckButton::new();
                let content = GtkBox::new(Orientation::Horizontal, 8);
                content.append(&crate::ui::styles::fond_cue(Some(&label_color(label))));
                let name = Label::new(Some(&label.name));
                name.set_halign(Align::Start);
                name.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                content.append(&name);
                check.set_child(Some(&content));
                show_membership(&check, have, total);
                let row = ListBoxRow::new();
                row.set_selectable(false);
                row.set_child(Some(&check));
                // Matched against what's typed in the box.
                row.set_widget_name(&label.name.to_lowercase());
                list.append(&row);
                rows.borrow_mut()
                    .push((label.id, label.name.clone(), check.clone()));

                let this = this.clone();
                let ids = ids.clone();
                let id = label.id;
                let dirty = dirty.clone();
                let syncing = syncing.clone();
                check.connect_toggled(move |c| {
                    if syncing.get() {
                        return;
                    }
                    // Clicking a partly-set box makes it fully set, and again
                    // clears it; either way it stops being "some".
                    c.set_inconsistent(false);
                    {
                        let mut lib = this.library.borrow_mut();
                        if c.is_active() {
                            lib.add_labels(&ids, &[id]).ok();
                        } else {
                            lib.remove_labels(&ids, &[id]).ok();
                        }
                    }
                    dirty.set(true);
                });
            })
        };
        for label in self.library.borrow().all_labels().unwrap_or_default() {
            let have = membership.get(&label.id).copied().unwrap_or(0);
            add_row(&label, have);
        }

        {
            let entry_c = entry.clone();
            list.set_filter_func(move |row| {
                let needle = entry_c.text().trim().to_lowercase();
                needle.is_empty() || row.widget_name().as_str().contains(&needle)
            });
        }
        let create: Rc<dyn Fn()> = {
            let this = self.clone();
            let ids = ids.clone();
            let entry = entry.clone();
            let rows = rows.clone();
            let add_row = add_row.clone();
            let dirty = dirty.clone();
            let list = list.clone();
            Rc::new(move || {
                let name = entry.text().trim().to_string();
                if name.is_empty() {
                    return;
                }
                let exists = rows
                    .borrow()
                    .iter()
                    .find(|(_, n, _)| n.eq_ignore_ascii_case(&name))
                    .map(|(_, _, c)| c.clone());
                match exists {
                    Some(check) => check.set_active(true),
                    None => {
                        let id = this.library.borrow_mut().create_label(&name);
                        let Ok(id) = id else { return };
                        this.library.borrow_mut().add_labels(&ids, &[id]).ok();
                        if let Some(label) = this
                            .library
                            .borrow()
                            .all_labels()
                            .unwrap_or_default()
                            .into_iter()
                            .find(|l| l.id == id)
                        {
                            add_row(&label, total);
                        }
                        dirty.set(true);
                    }
                }
                entry.set_text("");
                list.invalidate_filter();
            })
        };
        {
            let rows = rows.clone();
            let create_btn = create_btn.clone();
            let list = list.clone();
            entry.connect_changed(move |e| {
                let text = e.text().trim().to_string();
                let exact = rows
                    .borrow()
                    .iter()
                    .any(|(_, n, _)| n.eq_ignore_ascii_case(&text));
                create_btn.set_visible(!text.is_empty() && !exact);
                create_btn.set_label(&format!("Create \u{201c}{text}\u{201d}"));
                list.invalidate_filter();
            });
        }
        {
            let create = create.clone();
            create_btn.connect_clicked(move |_| create());
        }
        {
            // Enter makes the label if it's new, or ticks the one match.
            let create = create.clone();
            let rows = rows.clone();
            entry.connect_activate(move |e| {
                let text = e.text().trim().to_lowercase();
                if text.is_empty() {
                    return;
                }
                let matches: Vec<CheckButton> = rows
                    .borrow()
                    .iter()
                    .filter(|(_, n, _)| n.to_lowercase().contains(&text))
                    .map(|(_, _, c)| c.clone())
                    .collect();
                let exact = rows
                    .borrow()
                    .iter()
                    .any(|(_, n, _)| n.to_lowercase() == text);
                if matches.len() == 1 && !exact {
                    matches[0].set_active(!matches[0].is_active());
                    e.set_text("");
                } else {
                    create();
                }
            });
        }

        // ── projects ────────────────────────────────────────────────────
        let projects = self.library.borrow().all_projects().unwrap_or_default();
        vbox.append(&Separator::new(Orientation::Horizontal));
        let projects_heading = Label::new(Some("Projects"));
        projects_heading.add_css_class("fond-row-meta");
        projects_heading.set_halign(Align::Start);
        vbox.append(&projects_heading);
        if projects.is_empty() {
            let none = Label::new(Some(
                "None yet \u{2014} use + beside Projects in the sidebar",
            ));
            none.add_css_class("fond-row-meta");
            none.set_halign(Align::Start);
            none.set_wrap(true);
            none.set_xalign(0.0);
            vbox.append(&none);
        } else {
            let in_project = self
                .library
                .borrow()
                .project_memberships(&ids)
                .unwrap_or_default();
            let plist = GtkBox::new(Orientation::Vertical, 2);
            for project in projects {
                let check = CheckButton::with_label(&project.name);
                show_membership(
                    &check,
                    in_project.get(&project.id).copied().unwrap_or(0),
                    total,
                );
                let this = self.clone();
                let ids = ids.clone();
                let dirty = dirty.clone();
                let syncing = syncing.clone();
                let pid = project.id;
                check.connect_toggled(move |c| {
                    if syncing.get() {
                        return;
                    }
                    c.set_inconsistent(false);
                    {
                        let mut lib = this.library.borrow_mut();
                        for id in &ids {
                            if c.is_active() {
                                lib.add_doc_to_project(pid, *id).ok();
                            } else {
                                lib.remove_doc_from_project(pid, *id).ok();
                            }
                        }
                    }
                    dirty.set(true);
                });
                plist.append(&check);
            }
            let pscroll = ScrolledWindow::new();
            pscroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
            pscroll.set_propagate_natural_height(true);
            pscroll.set_max_content_height(110);
            pscroll.set_child(Some(&plist));
            vbox.append(&pscroll);
        }

        // ── pin ─────────────────────────────────────────────────────────
        vbox.append(&Separator::new(Orientation::Horizontal));
        let pinned_now = ids
            .iter()
            .filter(|id| {
                self.library
                    .borrow()
                    .doc_by_id(**id)
                    .ok()
                    .flatten()
                    .is_some_and(|d| d.pinned)
            })
            .count();
        let pin = CheckButton::with_label("Pin to top");
        show_membership(&pin, pinned_now, total);
        {
            let this = self.clone();
            let ids = ids.clone();
            let dirty = dirty.clone();
            let syncing = syncing.clone();
            pin.connect_toggled(move |c| {
                if syncing.get() {
                    return;
                }
                c.set_inconsistent(false);
                for id in &ids {
                    this.library
                        .borrow_mut()
                        .set_pinned(*id, c.is_active())
                        .ok();
                }
                dirty.set(true);
            });
        }
        vbox.append(&pin);

        popover.set_child(Some(&vbox));
        {
            let this = self.clone();
            popover.connect_closed(move |p| {
                let p = p.clone();
                glib::idle_add_local_once(move || p.unparent());
                if dirty.get() {
                    this.refresh();
                }
            });
        }
        popover.popup();
        entry.grab_focus();
    }
}
