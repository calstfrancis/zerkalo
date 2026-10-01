use super::*;

impl LibraryWindow {
    pub(super) fn populate_filter_list(&self) {
        while let Some(child) = self.filter_list.first_child() {
            self.filter_list.remove(&child);
        }

        self.filter_list.append(&make_filter_row(
            "all",
            "view-list-symbolic",
            "All Documents",
            self.library.borrow().doc_count(&LibraryFilter::All).ok(),
        ));
        self.filter_list.append(&make_filter_row(
            "recent",
            "document-open-recent-symbolic",
            "Recent",
            self.library.borrow().doc_count(&LibraryFilter::Recent).ok(),
        ));
        self.filter_list.append(&make_filter_row(
            "untagged",
            "edit-clear-symbolic",
            "Untagged",
            self.library
                .borrow()
                .doc_count(&LibraryFilter::Untagged)
                .ok(),
        ));

        let projects = self.library.borrow().all_projects().unwrap_or_default();
        {
            let this = self.clone();
            self.filter_list.append(&self.add_header(
                "Projects",
                "fond-accent-library",
                "New project",
                move || this.create_project_dialog(),
            ));
        }
        if projects.is_empty() {
            self.filter_list
                .append(&hint_row("Group documents that belong together"));
        } else {
            for p in projects {
                let count = self
                    .library
                    .borrow()
                    .doc_count(&LibraryFilter::Project(p.id))
                    .ok();
                let filter_row = make_filter_row(
                    &format!("project:{}", p.id),
                    "folder-symbolic",
                    &p.name,
                    count,
                );
                let gesture = gtk4::GestureClick::new();
                gesture.set_button(3);
                let this = self.clone();
                let pid = p.id;
                let pname = p.name.clone();
                let row_weak = filter_row.downgrade();
                gesture.connect_pressed(move |g, _, x, y| {
                    g.set_state(gtk4::EventSequenceState::Claimed);
                    if let Some(row) = row_weak.upgrade() {
                        this.show_project_menu(&row, pid, &pname, x, y);
                    }
                });
                filter_row.add_controller(gesture);
                self.filter_list.append(&filter_row);
            }
        }

        let all_cats = self
            .library
            .borrow()
            .all_categories_structured()
            .unwrap_or_default();
        {
            let this = self.clone();
            self.filter_list.append(&self.add_header(
                "Categories",
                "fond-accent-library",
                "New category",
                move || this.create_category_dialog(),
            ));
        }
        if all_cats.is_empty() {
            self.filter_list
                .append(&hint_row("Sort documents into kinds"));
        } else {
            // Partition into parents (have children), children (have parent), standalone
            let parent_names: std::collections::HashSet<String> =
                all_cats.iter().filter_map(|c| c.parent.clone()).collect();
            let cats_with_children: std::collections::HashSet<String> = all_cats
                .iter()
                .filter(|c| parent_names.contains(&c.name))
                .map(|c| c.name.clone())
                .collect();

            // Emit parent rows first, then their children, then standalones
            let mut emitted: std::collections::HashSet<String> = std::collections::HashSet::new();
            for cat in &all_cats {
                if cat.parent.is_none() && cats_with_children.contains(&cat.name) {
                    // Parent category row
                    let has_children = true;
                    let cat_count = self
                        .library
                        .borrow()
                        .doc_count(&LibraryFilter::CategoryGroup(cat.name.clone()))
                        .ok();
                    let filter_row = make_category_filter_row(
                        &format!("category-group:{}", cat.name),
                        &cat.color_hex
                            .clone()
                            .unwrap_or_else(|| stable_palette_color(&cat.name).to_string()),
                        &cat.name,
                        cat_count,
                    );
                    // Parent rows: drop rejected with toast
                    let drop =
                        DropTarget::new(gtk4::glib::Type::STRING, gtk4::gdk::DragAction::COPY);
                    let toast_overlay = self.toast_overlay.clone();
                    drop.connect_drop(move |_, _, _, _| {
                        let toast = adw::Toast::new("Drop onto a specific subcategory");
                        toast_overlay.add_toast(toast);
                        false
                    });
                    filter_row.add_controller(drop);
                    let gesture = gtk4::GestureClick::new();
                    gesture.set_button(3);
                    let this = self.clone();
                    let cat_name = cat.name.clone();
                    let row_weak = filter_row.downgrade();
                    gesture.connect_pressed(move |g, _, x, y| {
                        g.set_state(gtk4::EventSequenceState::Claimed);
                        if let Some(row) = row_weak.upgrade() {
                            this.show_category_menu(&row, &cat_name, has_children, x, y);
                        }
                    });
                    filter_row.add_controller(gesture);
                    self.filter_list.append(&filter_row);
                    emitted.insert(cat.name.clone());

                    // Children of this parent
                    for child in &all_cats {
                        if child.parent.as_deref() == Some(&cat.name) {
                            let child_count = self
                                .library
                                .borrow()
                                .doc_count(&LibraryFilter::Category(child.name.clone()))
                                .ok();
                            let child_row = make_category_filter_row_indented(
                                &format!("category:{}", child.name),
                                &child.color_hex.clone().unwrap_or_else(|| {
                                    stable_palette_color(&child.name).to_string()
                                }),
                                &child.name,
                                child_count,
                                16,
                            );
                            let drop2 = DropTarget::new(
                                gtk4::glib::Type::STRING,
                                gtk4::gdk::DragAction::COPY,
                            );
                            let this2 = self.clone();
                            let cname2 = child.name.clone();
                            drop2.connect_drop(move |_, value, _, _| {
                                if let Ok(id_str) = value.get::<String>() {
                                    if let Ok(doc_id) = id_str.parse::<i64>() {
                                        this2
                                            .library
                                            .borrow_mut()
                                            .add_doc_categories(
                                                doc_id,
                                                std::slice::from_ref(&cname2),
                                            )
                                            .ok();
                                        this2.refresh();
                                        return true;
                                    }
                                }
                                false
                            });
                            child_row.add_controller(drop2);
                            let gesture2 = gtk4::GestureClick::new();
                            gesture2.set_button(3);
                            let this2 = self.clone();
                            let cname2 = child.name.clone();
                            let row_weak2 = child_row.downgrade();
                            gesture2.connect_pressed(move |g, _, x, y| {
                                g.set_state(gtk4::EventSequenceState::Claimed);
                                if let Some(row) = row_weak2.upgrade() {
                                    this2.show_category_menu(&row, &cname2, false, x, y);
                                }
                            });
                            child_row.add_controller(gesture2);
                            self.filter_list.append(&child_row);
                            emitted.insert(child.name.clone());
                        }
                    }
                }
            }
            // Standalone categories (no parent, no children)
            for cat in &all_cats {
                if emitted.contains(&cat.name) {
                    continue;
                }
                let cat_count = self
                    .library
                    .borrow()
                    .doc_count(&LibraryFilter::Category(cat.name.clone()))
                    .ok();
                let filter_row = make_category_filter_row(
                    &format!("category:{}", cat.name),
                    &cat.color_hex
                        .clone()
                        .unwrap_or_else(|| stable_palette_color(&cat.name).to_string()),
                    &cat.name,
                    cat_count,
                );
                let drop = DropTarget::new(gtk4::glib::Type::STRING, gtk4::gdk::DragAction::COPY);
                let this = self.clone();
                let cat_name = cat.name.clone();
                drop.connect_drop(move |_, value, _, _| {
                    if let Ok(id_str) = value.get::<String>() {
                        if let Ok(doc_id) = id_str.parse::<i64>() {
                            this.library
                                .borrow_mut()
                                .add_doc_categories(doc_id, std::slice::from_ref(&cat_name))
                                .ok();
                            this.refresh();
                            return true;
                        }
                    }
                    false
                });
                filter_row.add_controller(drop);
                let gesture = gtk4::GestureClick::new();
                gesture.set_button(3);
                let this = self.clone();
                let cat_name = cat.name.clone();
                let row_weak = filter_row.downgrade();
                gesture.connect_pressed(move |g, _, x, y| {
                    g.set_state(gtk4::EventSequenceState::Claimed);
                    if let Some(row) = row_weak.upgrade() {
                        this.show_category_menu(&row, &cat_name, false, x, y);
                    }
                });
                filter_row.add_controller(gesture);
                self.filter_list.append(&filter_row);
            }
        }

        let tags_with_counts = self
            .library
            .borrow()
            .all_tags_with_counts()
            .unwrap_or_default();
        {
            let this = self.clone();
            self.filter_list.append(&self.add_header(
                "Tags",
                "fond-accent-pinned",
                "Manage tags",
                move || this.show_manage_tags(),
            ));
        }
        if tags_with_counts.is_empty() {
            self.filter_list
                .append(&hint_row("Mark documents with your own words"));
        } else {
            for (t, _) in tags_with_counts.iter() {
                let count = self
                    .library
                    .borrow()
                    .doc_count(&LibraryFilter::Tag(t.id))
                    .ok();
                self.filter_list
                    .append(&make_tag_filter_row(t.id, &t.name, &t.color_hex, count));
            }
        }

        let authors = self
            .library
            .borrow()
            .all_authors_with_counts()
            .unwrap_or_default();
        if !authors.is_empty() {
            let expanded = *self.authors_expanded.borrow();
            self.filter_list
                .append(&self.authors_header_row(authors.len(), expanded));
            if expanded {
                for (a, count) in &authors {
                    self.filter_list
                        .append(&make_author_filter_row(a.id, &a.name, Some(*count)));
                }
            }
        }

        // Repopulate the fixed bottom list (Trash / Archive)
        while let Some(child) = self.bottom_filter_list.first_child() {
            self.bottom_filter_list.remove(&child);
        }
        self.bottom_filter_list.append(&make_filter_row(
            "trash",
            "user-trash-symbolic",
            "Trash",
            self.library.borrow().doc_count(&LibraryFilter::Trash).ok(),
        ));
        self.bottom_filter_list.append(&make_filter_row(
            "archive",
            "view-archive-symbolic",
            "Archive",
            self.library
                .borrow()
                .doc_count(&LibraryFilter::Archive)
                .ok(),
        ));

        self.restore_sidebar_selection();
    }

    /// Puts the sidebar's highlight back on the current view.
    pub(super) fn restore_sidebar_selection(&self) {
        // Put the highlight back on the view the user is in. This used to
        // select the first row unconditionally, which the selection handler
        // took as a click on "All Documents" — so archiving, deleting or
        // dropping onto a category threw the user out of the view they were in.
        let current = self.current_filter.borrow().clone();
        let wanted = filter_row_name(&current);
        let find = |list: &ListBox| {
            let mut i = 0;
            while let Some(row) = list.row_at_index(i) {
                if row.widget_name().as_str() == wanted {
                    return Some(row);
                }
                i += 1;
            }
            None
        };
        let authors = self
            .library
            .borrow()
            .all_authors_with_counts()
            .unwrap_or_default();
        let (top, bottom) = (find(&self.filter_list), find(&self.bottom_filter_list));
        *self.inhibit_select.borrow_mut() = true;
        match (top, bottom) {
            (Some(row), _) => {
                self.bottom_filter_list.unselect_all();
                self.filter_list.select_row(Some(&row));
            }
            (None, Some(row)) => {
                self.filter_list.unselect_all();
                self.bottom_filter_list.select_row(Some(&row));
            }
            (None, None) => {
                self.filter_list.unselect_all();
                self.bottom_filter_list.unselect_all();
                // The view's own row is missing. If that's only because the
                // Authors section is folded, stay in the view; if what it
                // showed has been deleted, go back to All Documents.
                let author_folded = matches!(
                    current,
                    LibraryFilter::Author(id) if authors.iter().any(|(a, _)| a.id == id)
                );
                if !author_folded {
                    *self.current_filter.borrow_mut() = LibraryFilter::All;
                    if let Some(first) = self.filter_list.row_at_index(0) {
                        self.filter_list.select_row(Some(&first));
                    }
                }
            }
        }
        *self.inhibit_select.borrow_mut() = false;
    }

    /// Switches to a view the way clicking its sidebar row would — for the
    /// chips on a document row.
    pub(super) fn go_to_filter(&self, filter: LibraryFilter) {
        *self.current_filter.borrow_mut() = filter;
        self.selection.borrow_mut().clear();
        self.restore_sidebar_selection();
        self.populate_doc_list();
    }

    /// A section header with a `+` at its right edge for making a new one.
    pub(super) fn add_header(
        &self,
        title: &str,
        accent: &str,
        tooltip: &str,
        on_add: impl Fn() + 'static,
    ) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.set_selectable(false);
        row.set_activatable(false);
        let bx = crate::ui::styles::fond_section_header(title, accent);
        let spacer = GtkBox::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        bx.append(&spacer);
        let add = Button::from_icon_name("list-add-symbolic");
        add.add_css_class("flat");
        add.add_css_class("circular");
        add.set_tooltip_text(Some(tooltip));
        add.update_property(&[gtk4::accessible::Property::Label(tooltip)]);
        add.connect_clicked(move |_| on_add());
        bx.append(&add);
        row.set_child(Some(&bx));
        row
    }

    /// The Authors section header: unlike the others it folds, because a
    /// library that cites widely has hundreds of authors and they would bury
    /// the projects and labels above them.
    pub(super) fn authors_header_row(&self, count: usize, expanded: bool) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.set_selectable(false);
        row.set_activatable(false);
        row.set_tooltip_text(Some(
            "Everyone your documents cite, as \"Surname, I.\" — made automatically from each \
             document's citations. Click to show or hide.",
        ));
        let bx = crate::ui::styles::fond_section_header("Authors", "fond-accent-library");
        let meta = crate::ui::styles::fond_section_meta();
        meta.set_text(&format!("\u{b7} {count}"));
        bx.append(&meta);
        let spacer = GtkBox::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        bx.append(&spacer);
        let chevron = Image::from_icon_name(if expanded {
            "pan-down-symbolic"
        } else {
            "pan-end-symbolic"
        });
        chevron.add_css_class("fond-row-meta");
        bx.append(&chevron);
        row.set_child(Some(&bx));
        let click = gtk4::GestureClick::new();
        click.set_button(1);
        let this = self.clone();
        click.connect_pressed(move |g, _, _, _| {
            g.set_state(gtk4::EventSequenceState::Claimed);
            let now = *this.authors_expanded.borrow();
            *this.authors_expanded.borrow_mut() = !now;
            crate::config::update(|c| c.library.authors_open = !now).ok();
            this.populate_filter_list();
        });
        row.add_controller(click);
        row
    }
}

/// The sidebar row name for a filter — the inverse of `parse_filter_name`.
pub(super) fn filter_row_name(filter: &LibraryFilter) -> String {
    match filter {
        LibraryFilter::All => "all".into(),
        LibraryFilter::Recent => "recent".into(),
        LibraryFilter::Archive => "archive".into(),
        LibraryFilter::Untagged => "untagged".into(),
        LibraryFilter::Trash => "trash".into(),
        LibraryFilter::Project(id) => format!("project:{id}"),
        LibraryFilter::Tag(id) => format!("tag:{id}"),
        LibraryFilter::Author(id) => format!("author:{id}"),
        LibraryFilter::CategoryGroup(name) => format!("category-group:{name}"),
        LibraryFilter::Category(name) => format!("category:{name}"),
    }
}

pub(super) fn parse_filter_name(name: &str) -> LibraryFilter {
    if name == "all" {
        LibraryFilter::All
    } else if name == "recent" {
        LibraryFilter::Recent
    } else if name == "archive" {
        LibraryFilter::Archive
    } else if name == "untagged" {
        LibraryFilter::Untagged
    } else if name == "trash" {
        LibraryFilter::Trash
    } else if let Some(rest) = name.strip_prefix("project:") {
        rest.parse::<i64>()
            .map(LibraryFilter::Project)
            .unwrap_or(LibraryFilter::All)
    } else if let Some(rest) = name.strip_prefix("author:") {
        rest.parse::<i64>()
            .map(LibraryFilter::Author)
            .unwrap_or(LibraryFilter::All)
    } else if let Some(rest) = name.strip_prefix("tag:") {
        rest.parse::<i64>()
            .map(LibraryFilter::Tag)
            .unwrap_or(LibraryFilter::All)
    } else if let Some(rest) = name.strip_prefix("category-group:") {
        LibraryFilter::CategoryGroup(rest.to_string())
    } else if let Some(rest) = name.strip_prefix("category:") {
        LibraryFilter::Category(rest.to_string())
    } else {
        LibraryFilter::All
    }
}

/// The shell every sidebar filter row shares: the suite's single-line row, a
/// cue or an icon at the left, the name, and a plain count at the right. The
/// count used to be a filled pill, which made a sidebar of quiet names read as
/// a column of badges.
pub(super) fn filter_row_shell(
    name: &str,
    label: &str,
    count: Option<i64>,
) -> (ListBoxRow, GtkBox) {
    let row = ListBoxRow::new();
    row.set_widget_name(name);
    row.add_css_class("fond-row");
    let hbox = GtkBox::new(Orientation::Horizontal, 8);
    hbox.set_margin_start(10);
    hbox.set_margin_end(10);
    let lbl = Label::new(Some(label));
    lbl.set_hexpand(true);
    lbl.set_halign(Align::Start);
    lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    lbl.add_css_class("fond-row-title");
    hbox.append(&lbl);
    if let Some(c) = count {
        let c_lbl = Label::new(Some(&c.to_string()));
        c_lbl.add_css_class("fond-row-meta");
        c_lbl.set_halign(Align::End);
        c_lbl.set_visible(c > 0);
        hbox.append(&c_lbl);
    }
    row.set_child(Some(&hbox));
    (row, hbox)
}

pub(super) fn make_filter_row(
    name: &str,
    icon: &str,
    label: &str,
    count: Option<i64>,
) -> ListBoxRow {
    let (row, hbox) = filter_row_shell(name, label, count);
    let img = Image::from_icon_name(icon);
    img.set_pixel_size(14);
    img.add_css_class("fond-quiet");
    hbox.prepend(&img);
    row
}

pub(super) fn make_category_filter_row(
    name: &str,
    color: &str,
    label: &str,
    count: Option<i64>,
) -> ListBoxRow {
    let (row, hbox) = filter_row_shell(name, label, count);
    hbox.prepend(&crate::ui::styles::fond_cue(Some(color)));
    row
}

pub(super) fn make_category_filter_row_indented(
    name: &str,
    color: &str,
    label: &str,
    count: Option<i64>,
    indent: i32,
) -> ListBoxRow {
    let row = make_category_filter_row(name, color, label, count);
    if let Some(child) = row.child() {
        child.set_margin_start(indent);
    }
    row
}

pub(super) fn make_tag_filter_row(
    tag_id: i64,
    label: &str,
    color: &str,
    count: Option<i64>,
) -> ListBoxRow {
    let (row, hbox) = filter_row_shell(&format!("tag:{tag_id}"), label, count);
    hbox.prepend(&crate::ui::styles::fond_cue(Some(color)));
    row
}

pub(super) fn make_author_filter_row(
    author_id: i64,
    label: &str,
    count: Option<i64>,
) -> ListBoxRow {
    let (row, hbox) = filter_row_shell(&format!("author:{author_id}"), label, count);
    hbox.prepend(&crate::ui::styles::fond_cue(None));
    row
}

/// A dim one-line suggestion under a section that has nothing in it yet.
pub(super) fn hint_row(text: &str) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    let label = Label::new(Some(text));
    label.add_css_class("fond-row-meta");
    label.set_halign(Align::Start);
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.set_margin_start(14);
    label.set_margin_end(8);
    label.set_margin_top(2);
    label.set_margin_bottom(4);
    row.set_child(Some(&label));
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_filter_round_trips_through_its_row_name() {
        for f in [
            LibraryFilter::All,
            LibraryFilter::Recent,
            LibraryFilter::Archive,
            LibraryFilter::Untagged,
            LibraryFilter::Trash,
            LibraryFilter::Project(7),
            LibraryFilter::Tag(3),
            LibraryFilter::Author(12),
            LibraryFilter::Category("Sermons".into()),
            LibraryFilter::CategoryGroup("Liturgy".into()),
        ] {
            assert_eq!(parse_filter_name(&filter_row_name(&f)), f);
        }
    }
}
