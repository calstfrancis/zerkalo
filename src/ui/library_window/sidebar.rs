use super::*;

impl LibraryWindow {
    pub(super) fn populate_filter_list(&self) {
        let counts = self.library.borrow().counts().unwrap_or_default();
        while let Some(child) = self.filter_list.first_child() {
            self.filter_list.remove(&child);
        }

        self.filter_list.append(&make_filter_row(
            "all",
            "view-list-symbolic",
            "All Documents",
            Some(counts.all),
        ));
        self.filter_list.append(&make_filter_row(
            "recent",
            "document-open-recent-symbolic",
            "Recent",
            Some(counts.recent),
        ));
        self.filter_list.append(&make_filter_row(
            "unlabelled",
            "edit-clear-symbolic",
            "Needs a label",
            Some(counts.unlabelled),
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
                let count = Some(counts.projects.get(&p.id).copied().unwrap_or(0));
                let (row, hbox) = filter_row_shell(&format!("project:{}", p.id), &p.name, count);
                let img = Image::from_icon_name("folder-symbolic");
                img.set_pixel_size(14);
                img.add_css_class("fond-quiet");
                hbox.prepend(&img);
                let pid = p.id;
                let this = self.clone();
                self.attach_row_menu(&row, &hbox, move || this.project_menu_model(pid));
                let this = self.clone();
                self.add_doc_drop_target(&row, move |doc_id| {
                    this.library
                        .borrow_mut()
                        .add_doc_to_project(pid, doc_id)
                        .ok();
                    this.refresh();
                });
                self.filter_list.append(&row);
            }
        }

        let labels = self.library.borrow().all_labels().unwrap_or_default();
        {
            let this = self.clone();
            self.filter_list.append(&self.add_header(
                "Labels",
                "fond-accent-pinned",
                "New label",
                move || this.create_label_dialog(),
            ));
        }
        if labels.is_empty() {
            self.filter_list
                .append(&hint_row("Mark documents with your own words"));
        } else {
            for label in &labels {
                let count = Some(counts.labels.get(&label.id).copied().unwrap_or(0));
                let (row, hbox) =
                    filter_row_shell(&format!("label:{}", label.id), &label.name, count);
                hbox.prepend(&crate::ui::styles::fond_cue(Some(&label_color(label))));
                let lid = label.id;
                let this = self.clone();
                self.attach_row_menu(&row, &hbox, move || this.label_menu_model(lid));
                let this = self.clone();
                self.add_doc_drop_target(&row, move |doc_id| {
                    this.library.borrow_mut().add_labels(&[doc_id], &[lid]).ok();
                    this.refresh();
                });
                self.filter_list.append(&row);
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
        // Not a drop target: a drag that slipped onto it would move a document's
        // file out of its folder, and nothing about dragging should do that.
        self.bottom_filter_list.append(&make_filter_row(
            "trash",
            "user-trash-symbolic",
            "Trash",
            Some(counts.trash),
        ));
        self.bottom_filter_list.append(&make_filter_row(
            "archive",
            "view-archive-symbolic",
            "Archive",
            Some(counts.archive),
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

    /// What a view is called to the user — for saying what a search is looking
    /// through.
    pub(super) fn filter_label(&self, filter: &LibraryFilter) -> String {
        let lib = self.library.borrow();
        match filter {
            LibraryFilter::All | LibraryFilter::Everywhere => "All Documents".into(),
            LibraryFilter::Recent => "Recent".into(),
            LibraryFilter::Unlabelled => "Needs a label".into(),
            LibraryFilter::Archive => "Archive".into(),
            LibraryFilter::Trash => "Trash".into(),
            LibraryFilter::Project(id) => lib
                .all_projects()
                .unwrap_or_default()
                .into_iter()
                .find(|p| p.id == *id)
                .map_or_else(|| "this project".into(), |p| p.name),
            LibraryFilter::Label(id) => lib
                .all_labels()
                .unwrap_or_default()
                .into_iter()
                .find(|l| l.id == *id)
                .map_or_else(|| "this label".into(), |l| l.name),
            LibraryFilter::Author(id) => lib
                .all_authors_with_counts()
                .unwrap_or_default()
                .into_iter()
                .find(|(a, _)| a.id == *id)
                .map_or_else(|| "this author".into(), |(a, _)| a.name),
        }
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
        LibraryFilter::Unlabelled => "unlabelled".into(),
        LibraryFilter::Trash => "trash".into(),
        LibraryFilter::Everywhere => "everywhere".into(),
        LibraryFilter::Project(id) => format!("project:{id}"),
        LibraryFilter::Label(id) => format!("label:{id}"),
        LibraryFilter::Author(id) => format!("author:{id}"),
    }
}

pub(super) fn parse_filter_name(name: &str) -> LibraryFilter {
    if name == "all" {
        LibraryFilter::All
    } else if name == "recent" {
        LibraryFilter::Recent
    } else if name == "archive" {
        LibraryFilter::Archive
    } else if name == "unlabelled" {
        LibraryFilter::Unlabelled
    } else if name == "trash" {
        LibraryFilter::Trash
    } else if name == "everywhere" {
        LibraryFilter::Everywhere
    } else if let Some(rest) = name.strip_prefix("project:") {
        rest.parse::<i64>()
            .map(LibraryFilter::Project)
            .unwrap_or(LibraryFilter::All)
    } else if let Some(rest) = name.strip_prefix("author:") {
        rest.parse::<i64>()
            .map(LibraryFilter::Author)
            .unwrap_or(LibraryFilter::All)
    } else if let Some(rest) = name.strip_prefix("label:") {
        rest.parse::<i64>()
            .map(LibraryFilter::Label)
            .unwrap_or(LibraryFilter::All)
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
            LibraryFilter::Unlabelled,
            LibraryFilter::Trash,
            LibraryFilter::Everywhere,
            LibraryFilter::Project(7),
            LibraryFilter::Label(3),
            LibraryFilter::Author(12),
        ] {
            assert_eq!(parse_filter_name(&filter_row_name(&f)), f);
        }
    }
}
