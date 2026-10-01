use super::*;

/// The parts of one list row that selection and hover act on.
pub(super) struct RowParts {
    pub row: ListBoxRow,
    pub check: CheckButton,
    pub more: gtk4::MenuButton,
}

/// Shows or hides a row's controls without moving anything: they are faded,
/// and stop taking clicks while faded so the empty margin of a row still opens
/// the document.
fn reveal(parts: &RowParts, check: bool, more: bool) {
    parts.check.set_opacity(if check { 1.0 } else { 0.0 });
    parts.check.set_can_target(check);
    parts.more.set_opacity(if more { 1.0 } else { 0.0 });
    parts.more.set_can_target(more);
}

impl LibraryWindow {
    pub(super) fn populate_doc_list(&self) {
        while let Some(child) = self.doc_list.first_child() {
            self.doc_list.remove(&child);
        }
        self.row_widgets.borrow_mut().clear();
        self.list_order.borrow_mut().clear();
        let search = self.search_entry.text().to_string();
        let filter = self.current_filter.borrow().clone();
        let sort = self.current_sort.borrow().clone();
        let project_reorder = match &filter {
            LibraryFilter::Project(pid) => Some(*pid),
            _ => None,
        };
        let docs = self
            .library
            .borrow()
            .documents(filter, &search, sort)
            .unwrap_or_default();

        if docs.is_empty() {
            let state = empty_state(&self.current_filter.borrow(), &search);
            self.empty_page.set_icon_name(Some(state.icon));
            self.empty_page.set_title(&state.title);
            self.empty_page.set_description(Some(&state.description));
            self.empty_new_doc_btn.set_visible(state.offer_new);
            self.empty_import_btn.set_visible(state.offer_import);
            self.empty_clear_search_btn.set_visible(state.offer_clear);
            self.doc_list_stack.set_visible_child_name("empty");
            return;
        }
        self.doc_list_stack.set_visible_child_name("docs");

        let cat_colors: HashMap<String, String> = self
            .library
            .borrow()
            .all_categories_with_colors()
            .unwrap_or_default()
            .into_iter()
            .map(|(name, color)| {
                let color = color.unwrap_or_else(|| stable_palette_color(&name).to_string());
                (name, color)
            })
            .collect();
        let authors_by_doc = self.library.borrow().authors_by_doc().unwrap_or_default();
        let mode = self.view_mode.borrow().clone();

        if mode == ViewMode::Compact {
            self.doc_list.add_css_class("compact-mode");
        } else {
            self.doc_list.remove_css_class("compact-mode");
        }

        // Pinned documents are a section of their own, announced the way every
        // other section in the suite is — a dot, a small-caps title and a count.
        // A bare separator between the two groups said less and looked like a
        // gap rather than a heading.
        let (pinned, rest): (Vec<_>, Vec<_>) = docs.into_iter().partition(|d| d.pinned);
        let groups: [(&str, &str, Vec<crate::library::Document>); 2] = [
            ("Pinned", "fond-accent-pinned", pinned),
            ("Documents", "fond-accent-library", rest),
        ];

        for (title, accent, group) in groups {
            if group.is_empty() {
                continue;
            }
            self.doc_list
                .append(&section_row(title, accent, group.len()));
            let last_idx = group.len() - 1;
            for (i, doc) in group.into_iter().enumerate() {
                let tags = self.library.borrow().doc_tags(doc.id).unwrap_or_default();
                let categories = self
                    .library
                    .borrow()
                    .doc_categories(doc.id)
                    .unwrap_or_default();
                let row = self.make_doc_row(
                    &doc,
                    &tags,
                    &categories,
                    project_reorder,
                    mode.clone(),
                    &cat_colors,
                    authors_by_doc.get(&doc.id).map_or(&[], |v| v.as_slice()),
                );
                if i == 0 {
                    row.add_css_class("fond-card-first");
                }
                if i == last_idx {
                    row.add_css_class("fond-card-last");
                }
                self.list_order.borrow_mut().push(doc.id);
                self.doc_list.append(&row);
            }
        }
        // Rows are new; carry the current selection onto them.
        self.sync_selection_ui();
    }

    /// A category or tag as it appears on a row: its colour as a dot, then its
    /// name, and a click shows only the documents that carry it.
    fn filter_chip(&self, name: &str, color: &str, target: LibraryFilter) -> GtkBox {
        let chip = GtkBox::new(Orientation::Horizontal, 4);
        chip.append(&crate::ui::styles::fond_cue(Some(color)));
        let label = Label::new(Some(name));
        label.add_css_class("fond-row-detail");
        chip.append(&label);
        chip.set_tooltip_text(Some(&format!("Show only {name}")));
        let click = gtk4::GestureClick::new();
        click.set_button(1);
        let this = self.clone();
        click.connect_pressed(move |g, _, _, _| {
            g.set_state(gtk4::EventSequenceState::Claimed);
            this.go_to_filter(target.clone());
        });
        chip.add_controller(click);
        chip
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn make_doc_row(
        &self,
        doc: &crate::library::Document,
        tags: &[crate::library::Tag],
        categories: &[crate::library::Category],
        project_reorder: Option<i64>,
        mode: ViewMode,
        cat_colors: &HashMap<String, String>,
        cites: &[String],
    ) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.set_widget_name(&doc.id.to_string());
        row.add_css_class("fond-card");
        row.add_css_class("fond-row");

        // One line per document: a cue in the category's colour, the title, the
        // category and tags as dim reference text, and the date and length at
        // the right edge. A checkbox at the left and a ⋯ menu at the right
        // appear when the row is hovered or focused — they are what make
        // selecting and acting on documents discoverable without hiding it all
        // behind Ctrl+click and a right-click.
        //
        // Compact mode is the same row under a `.compact-mode` list (fond.css
        // tightens the metrics), not a second row built by hand.
        let hbox = GtkBox::new(Orientation::Horizontal, 8);
        hbox.set_margin_start(10);
        hbox.set_margin_end(6);

        let check = CheckButton::new();
        check.set_valign(Align::Center);
        check.set_tooltip_text(Some("Select"));
        check.update_property(&[gtk4::accessible::Property::Label(&format!(
            "Select {}",
            doc.title
        ))]);
        hbox.append(&check);

        if doc.pinned {
            let pin = Image::from_icon_name("view-pin-symbolic");
            pin.set_pixel_size(12);
            pin.add_css_class("fond-row-meta");
            hbox.append(&pin);
        }

        let cue_color = categories.first().map(|cat| {
            cat_colors
                .get(&cat.name)
                .map(|s| s.to_string())
                .unwrap_or_else(|| stable_palette_color(&cat.name).to_string())
        });
        hbox.append(&crate::ui::styles::fond_cue(cue_color.as_deref()));

        let title = Label::new(Some(&doc.title));
        title.add_css_class("fond-row-title");
        title.set_halign(Align::Start);
        title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        hbox.append(&title);

        for cat in categories {
            let color = cat_colors
                .get(&cat.name)
                .cloned()
                .unwrap_or_else(|| stable_palette_color(&cat.name).to_string());
            hbox.append(&self.filter_chip(
                &cat.name,
                &color,
                LibraryFilter::Category(cat.name.clone()),
            ));
        }
        for tag in tags.iter().take(4) {
            hbox.append(&self.filter_chip(&tag.name, &tag.color_hex, LibraryFilter::Tag(tag.id)));
        }

        let mut tip = doc
            .notes
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .to_string();
        if !cites.is_empty() {
            if !tip.is_empty() {
                tip.push_str("\n\n");
            }
            tip.push_str("Cites: ");
            tip.push_str(&cites.iter().take(8).cloned().collect::<Vec<_>>().join("; "));
            if cites.len() > 8 {
                tip.push_str(&format!(" and {} more", cites.len() - 8));
            }
        }
        if !tip.is_empty() {
            hbox.set_tooltip_text(Some(&tip));
        }

        let spacer = GtkBox::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        hbox.append(&spacer);

        if doc.archived {
            let badge = Label::new(Some("archived"));
            badge.add_css_class("fond-row-meta");
            hbox.append(&badge);
        }

        // A document with notes says so, rather than hiding them in a tooltip
        // nobody knows to look for.
        if doc.notes.as_deref().is_some_and(|n| !n.trim().is_empty()) {
            let note = Image::from_icon_name("document-edit-symbolic");
            note.set_pixel_size(12);
            note.add_css_class("fond-row-meta");
            note.set_tooltip_text(Some("Has notes — hover the row to read them"));
            hbox.append(&note);
        }

        // The word count reads every file in the list, so it is worth having
        // only where there is room to read it.
        let mut meta_text = format_date(&doc.modified_at);
        if mode != ViewMode::Compact {
            let word_count = count_prose_words(std::path::Path::new(&doc.path));
            if word_count > 0 {
                meta_text = format!("{} \u{b7} {} words", meta_text, word_count);
            }
        }
        let meta = Label::new(Some(&meta_text));
        meta.add_css_class("fond-row-meta");
        meta.set_halign(Align::End);
        hbox.append(&meta);

        let more = gtk4::MenuButton::new();
        more.set_icon_name("view-more-symbolic");
        more.add_css_class("flat");
        more.add_css_class("circular");
        more.set_valign(Align::Center);
        more.set_tooltip_text(Some("More actions"));
        more.update_property(&[gtk4::accessible::Property::Label(&format!(
            "Actions for {}",
            doc.title
        ))]);
        {
            let this = self.clone();
            let id = doc.id;
            more.set_create_popup_func(move |btn| {
                btn.set_menu_model(this.doc_menu_model(id).as_ref());
            });
        }
        hbox.append(&more);

        row.set_child(Some(&hbox));

        let parts = RowParts {
            row: row.clone(),
            check: check.clone(),
            more: more.clone(),
        };
        reveal(&parts, false, false);
        {
            let this = self.clone();
            let id = doc.id;
            let syncing = self.syncing_checks.clone();
            check.connect_toggled(move |c| {
                if !syncing.get() {
                    this.set_selected(id, c.is_active());
                }
            });
        }
        // Hover and keyboard focus both reveal the controls; a selection in
        // progress keeps every row's checkbox showing.
        let hover = gtk4::EventControllerMotion::new();
        let focus = gtk4::EventControllerFocus::new();
        {
            let (c, m) = (check.clone(), more.clone());
            hover.connect_enter(move |_, _, _| {
                c.set_opacity(1.0);
                c.set_can_target(true);
                m.set_opacity(1.0);
                m.set_can_target(true);
            });
            let (c, m, this) = (check.clone(), more.clone(), self.clone());
            hover.connect_leave(move |_| {
                let keep = !this.selection.borrow().is_empty();
                c.set_opacity(if keep { 1.0 } else { 0.0 });
                c.set_can_target(keep);
                m.set_opacity(0.0);
                m.set_can_target(false);
            });
            let (c, m) = (check.clone(), more.clone());
            focus.connect_enter(move |_| {
                c.set_opacity(1.0);
                c.set_can_target(true);
                m.set_opacity(1.0);
                m.set_can_target(true);
            });
            let (c, m, this) = (check.clone(), more.clone(), self.clone());
            focus.connect_leave(move |_| {
                let keep = !this.selection.borrow().is_empty();
                c.set_opacity(if keep { 1.0 } else { 0.0 });
                c.set_can_target(keep);
                m.set_opacity(0.0);
                m.set_can_target(false);
            });
        }
        row.add_controller(hover);
        row.add_controller(focus);
        self.row_widgets.borrow_mut().insert(doc.id, parts);

        // Drag source — carry doc ID as a string for drop-on-category
        let drag_source = DragSource::new();
        drag_source.set_actions(gtk4::gdk::DragAction::COPY);
        let id_str = doc.id.to_string();
        drag_source.connect_prepare(move |_, _, _| {
            Some(gtk4::gdk::ContentProvider::for_value(&id_str.to_value()))
        });
        row.add_controller(drag_source);

        if let Some(pid) = project_reorder {
            let drop = DropTarget::new(gtk4::glib::Type::STRING, gtk4::gdk::DragAction::COPY);
            let this = self.clone();
            let target_doc_id = doc.id;
            drop.connect_drop(move |_, value, _, _| {
                if let Ok(id_str) = value.get::<String>() {
                    if let Ok(dragged_id) = id_str.parse::<i64>() {
                        if dragged_id != target_doc_id {
                            if let Ok(Some(target_pos)) = this
                                .library
                                .borrow()
                                .position_in_project(pid, target_doc_id)
                            {
                                this.library
                                    .borrow_mut()
                                    .move_doc_in_project(pid, dragged_id, target_pos)
                                    .ok();
                                this.populate_doc_list();
                            }
                        }
                    }
                }
                false
            });
            row.add_controller(drop);
        }

        // Ctrl+click toggles, Shift+click extends from the last one clicked,
        // and once anything is selected a plain click toggles too, so a run of
        // checkbox clicks doesn't suddenly open a document.
        let click = gtk4::GestureClick::new();
        click.set_button(1);
        let this = self.clone();
        let doc_id = doc.id;
        click.connect_pressed(move |g, _, _, _| {
            let mods = g.current_event_state();
            if mods.contains(gtk4::gdk::ModifierType::SHIFT_MASK) {
                g.set_state(gtk4::EventSequenceState::Claimed);
                this.select_range_to(doc_id);
            } else if mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK)
                || !this.selection.borrow().is_empty()
            {
                g.set_state(gtk4::EventSequenceState::Claimed);
                this.toggle_selected(doc_id);
            }
        });
        row.add_controller(click);

        // Right-click context menu — the same menu as the ⋯ button.
        let gesture = gtk4::GestureClick::new();
        gesture.set_button(3);
        let this = self.clone();
        let row_weak = row.downgrade();
        let doc_id = doc.id;
        gesture.connect_pressed(move |g, _, x, y| {
            g.set_state(gtk4::EventSequenceState::Claimed);
            let (Some(row), Some(model)) = (row_weak.upgrade(), this.doc_menu_model(doc_id)) else {
                return;
            };
            let popover = gtk4::PopoverMenu::from_model(Some(&model));
            popover.set_parent(&row);
            popover.set_has_arrow(true);
            popover.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            popover.connect_closed(|p| {
                let p = p.clone();
                glib::idle_add_local_once(move || p.unparent());
            });
            popup_after_click(popover.upcast_ref());
        });
        row.add_controller(gesture);

        row
    }

    pub(super) fn set_selected(&self, id: i64, on: bool) {
        {
            let mut sel = self.selection.borrow_mut();
            if on {
                sel.insert(id);
            } else {
                sel.remove(&id);
            }
        }
        *self.anchor.borrow_mut() = Some(id);
        self.sync_selection_ui();
    }

    pub(super) fn toggle_selected(&self, id: i64) {
        let on = !self.selection.borrow().contains(&id);
        self.set_selected(id, on);
    }

    /// Selects everything between the last document clicked and `id`, in the
    /// order the list shows them.
    pub(super) fn select_range_to(&self, id: i64) {
        let order = self.list_order.borrow().clone();
        let anchor = self.anchor.borrow().unwrap_or(id);
        let from = order.iter().position(|x| *x == anchor);
        let to = order.iter().position(|x| *x == id);
        match (from, to) {
            (Some(a), Some(b)) => {
                let mut sel = self.selection.borrow_mut();
                for doc in &order[a.min(b)..=a.max(b)] {
                    sel.insert(*doc);
                }
            }
            _ => {
                self.selection.borrow_mut().insert(id);
                *self.anchor.borrow_mut() = Some(id);
            }
        }
        self.sync_selection_ui();
    }

    pub(super) fn select_all_visible(&self) {
        let order = self.list_order.borrow().clone();
        self.selection.borrow_mut().extend(order);
        self.sync_selection_ui();
    }

    pub(super) fn clear_selection(&self) {
        self.selection.borrow_mut().clear();
        *self.anchor.borrow_mut() = None;
        self.sync_selection_ui();
    }

    /// Brings every row's checkbox, highlight and visible controls into line
    /// with the selection — in place, without rebuilding the list.
    pub(super) fn sync_selection_ui(&self) {
        let selected = self.selection.borrow().clone();
        let any = !selected.is_empty();
        for (id, parts) in self.row_widgets.borrow().iter() {
            let on = selected.contains(id);
            if parts.check.is_active() != on {
                self.syncing_checks.set(true);
                parts.check.set_active(on);
                self.syncing_checks.set(false);
            }
            if on {
                parts.row.add_css_class("doc-selected");
            } else {
                parts.row.remove_css_class("doc-selected");
            }
            let hovered = parts.row.state_flags().contains(gtk4::StateFlags::PRELIGHT);
            reveal(parts, any || hovered, hovered);
        }
        self.update_action_bar();
    }

    /// Ctrl+A selects every listed document and Escape lets go of the
    /// selection — neither when typing in the search box, which wants both.
    pub(super) fn install_selection_keys(&self) {
        let key = gtk4::EventControllerKey::new();
        let this = self.clone();
        key.connect_key_pressed(move |_, key, _, mods| {
            let typing = gtk4::prelude::GtkWindowExt::focus(&this.window)
                .is_some_and(|w| w.is::<gtk4::Text>() || w.is::<TextView>());
            if typing {
                return glib::Propagation::Proceed;
            }
            let ctrl = mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
            if ctrl && (key == gtk4::gdk::Key::a || key == gtk4::gdk::Key::A) {
                this.select_all_visible();
                return glib::Propagation::Stop;
            }
            if key == gtk4::gdk::Key::Escape && !this.selection.borrow().is_empty() {
                this.clear_selection();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.window.add_controller(key);
    }

    pub(super) fn open_doc_by_id(&self, doc_id: i64) {
        let doc = self.library.borrow().doc_by_id(doc_id).ok().flatten();
        if let Some(doc) = doc {
            let path = doc.path.clone();
            self.library.borrow_mut().touch_opened(&path).ok();
            if let Some(cb) = self.on_open.borrow().as_ref() {
                cb(path);
            }
        }
    }
}

/// What the list says when it has nothing to show — specific to where the user
/// is, so an empty view explains itself and offers the one next step that
/// fits, instead of a bare "Nothing here yet".
pub(super) struct EmptyState {
    pub icon: &'static str,
    pub title: String,
    pub description: String,
    pub offer_new: bool,
    pub offer_import: bool,
    pub offer_clear: bool,
}

pub(super) fn empty_state(filter: &LibraryFilter, search: &str) -> EmptyState {
    let plain = |icon, title: &str, description: &str| EmptyState {
        icon,
        title: title.to_string(),
        description: description.to_string(),
        offer_new: false,
        offer_import: false,
        offer_clear: false,
    };
    let search = search.trim();
    if !search.is_empty() {
        return EmptyState {
            offer_clear: true,
            ..plain(
                "system-search-symbolic",
                &format!("Nothing matches \u{201c}{search}\u{201d}"),
                "Searched titles, categories, tags and the authors a document cites.",
            )
        };
    }
    match filter {
        LibraryFilter::All => EmptyState {
            offer_new: true,
            offer_import: true,
            ..plain(
                "folder-open-symbolic",
                "Your documents will appear here",
                "Start a new one, or bring in something you've already written.",
            )
        },
        LibraryFilter::Recent => plain(
            "document-open-recent-symbolic",
            "Nothing opened yet",
            "Documents you open show up here, most recent first.",
        ),
        LibraryFilter::Untagged => plain(
            "emblem-ok-symbolic",
            "Every document has a tag",
            "Documents without a tag would be listed here.",
        ),
        LibraryFilter::Project(_) => plain(
            "folder-symbolic",
            "This project is empty",
            "Open a document's \u{22ef} menu and choose Organize \u{203a} Add to Project, or select several documents and use Add to Project.",
        ),
        LibraryFilter::Category(_) | LibraryFilter::CategoryGroup(_) => plain(
            "folder-symbolic",
            "Nothing in this category yet",
            "Drag a document onto the category in the sidebar, or select documents and choose Categorize.",
        ),
        LibraryFilter::Tag(_) => plain(
            "tag-symbolic",
            "No documents have this tag",
            "Select documents and choose Tag to add it.",
        ),
        LibraryFilter::Author(_) => plain(
            "avatar-default-symbolic",
            "No document cites this author",
            "Authors come from the citations in your documents, so this list follows what you write.",
        ),
        LibraryFilter::Archive => plain(
            "view-archive-symbolic",
            "Nothing archived",
            "Archive a document you're done with: it stays safe, but out of the way.",
        ),
        LibraryFilter::Trash => plain(
            "user-trash-symbolic",
            "The Trash is empty",
            "Deleted documents wait here until you delete them for good.",
        ),
    }
}

/// A section header inside the document list: the suite's dot-and-small-caps
/// header, plus the number of documents under it.
pub(super) fn section_row(title: &str, accent: &str, count: usize) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    let bx = crate::ui::styles::fond_section_header(title, accent);
    let meta = crate::ui::styles::fond_section_meta();
    meta.set_text(&format!("\u{b7} {count}"));
    bx.append(&meta);
    row.set_child(Some(&bx));
    row
}

pub(super) fn format_date(iso: &str) -> String {
    use chrono::{DateTime, Datelike, Local};
    let dt = DateTime::parse_from_rfc3339(iso)
        .map(|d| d.with_timezone(&Local))
        .unwrap_or_else(|_| Local::now());
    let now = Local::now();
    let days_ago = (now.date_naive() - dt.date_naive()).num_days();
    if days_ago == 0 {
        "Today".to_string()
    } else if days_ago == 1 {
        "Yesterday".to_string()
    } else if days_ago < 7 {
        dt.format("%A").to_string()
    } else if dt.year() == now.year() {
        dt.format("%b %-d").to_string()
    } else {
        dt.format("%b %-d, %Y").to_string()
    }
}

/// `count_prose_words_uncached`, remembered per file until it changes — the
/// list is rebuilt on every keystroke of a search and every click, and used to
/// read every document in full each time.
pub(super) fn count_prose_words(path: &std::path::Path) -> usize {
    use std::time::SystemTime;
    thread_local! {
        static CACHE: RefCell<HashMap<PathBuf, (SystemTime, u64, usize)>> =
            RefCell::new(HashMap::new());
    }
    let stamp = std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok().map(|t| (t, m.len())));
    if let Some((mtime, len)) = stamp {
        let hit = CACHE.with(|c| c.borrow().get(path).copied());
        if let Some((cached_mtime, cached_len, words)) = hit {
            if cached_mtime == mtime && cached_len == len {
                return words;
            }
        }
    }
    let words = count_prose_words_uncached(path);
    if let Some((mtime, len)) = stamp {
        CACHE.with(|c| {
            c.borrow_mut()
                .insert(path.to_path_buf(), (mtime, len, words))
        });
    }
    words
}

pub(super) fn count_prose_words_uncached(path: &std::path::Path) -> usize {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    let mut in_code_block = false;
    let mut total = 0usize;
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block || t.starts_with("//") || t.starts_with('#') {
            continue;
        }
        for word in t.split_whitespace() {
            if !word.starts_with('@') && !word.starts_with('<') && !word.starts_with('`') {
                total += 1;
            }
        }
    }
    total
}

#[cfg(test)]
mod empty_tests {
    use super::*;

    #[test]
    fn a_search_with_no_results_offers_to_clear_it_and_names_it() {
        let s = empty_state(&LibraryFilter::All, "magnificat");
        assert!(s.title.contains("magnificat"));
        assert!(s.offer_clear && !s.offer_new && !s.offer_import);
    }

    #[test]
    fn a_blank_search_counts_as_no_search() {
        let s = empty_state(&LibraryFilter::All, "   ");
        assert!(!s.offer_clear && s.offer_new);
    }

    #[test]
    fn an_empty_library_offers_new_and_import_and_nothing_else_does() {
        let all = empty_state(&LibraryFilter::All, "");
        assert!(all.offer_new && all.offer_import);
        for f in [
            LibraryFilter::Recent,
            LibraryFilter::Untagged,
            LibraryFilter::Project(1),
            LibraryFilter::Category("x".into()),
            LibraryFilter::CategoryGroup("x".into()),
            LibraryFilter::Tag(1),
            LibraryFilter::Author(1),
            LibraryFilter::Archive,
            LibraryFilter::Trash,
        ] {
            let s = empty_state(&f, "");
            assert!(!s.offer_new && !s.offer_import && !s.offer_clear, "{f:?}");
            assert!(!s.title.is_empty() && !s.description.is_empty(), "{f:?}");
        }
    }
}
