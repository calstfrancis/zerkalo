use super::*;

impl LibraryWindow {
    pub(super) fn populate_doc_list(&self) {
        while let Some(child) = self.doc_list.first_child() {
            self.doc_list.remove(&child);
        }
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
            let searching = !search.is_empty();
            self.empty_page.set_description(Some(if searching {
                "Try a different search"
            } else {
                "Nothing here yet"
            }));
            self.empty_new_doc_btn.set_visible(!searching);
            self.empty_clear_search_btn.set_visible(searching);
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
                self.doc_list.append(&row);
            }
        }
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
        // the right edge. What was here before was a three-line card with a
        // 32px file icon, four coloured chips and a line of notes — the titles
        // are what a library is scanned for, and they were the smallest thing
        // on the row. Notes are the tooltip now; the tags stay clickable.
        //
        // Compact mode is the same row under a `.compact-mode` list (fond.css
        // tightens the metrics), not a second row built by hand.
        let hbox = GtkBox::new(Orientation::Horizontal, 8);
        hbox.set_margin_start(10);
        hbox.set_margin_end(10);

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
            let cat_lbl = Label::new(Some(&cat.name));
            cat_lbl.add_css_class("fond-row-detail");
            cat_lbl.set_tooltip_text(Some(&format!("Show only {}", cat.name)));
            hbox.append(&cat_lbl);
            let cat_click = gtk4::GestureClick::new();
            cat_click.set_button(1);
            let this_cat = self.clone();
            let cat_name = cat.name.clone();
            let cat_lbl_ref = cat_lbl.clone();
            cat_click.connect_pressed(move |g, _, _, _| {
                g.set_state(gtk4::EventSequenceState::Claimed);
                cat_lbl_ref.add_css_class("chip-active");
                let lbl_weak = cat_lbl_ref.downgrade();
                glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
                    if let Some(l) = lbl_weak.upgrade() {
                        l.remove_css_class("chip-active");
                    }
                });
                *this_cat.current_filter.borrow_mut() = LibraryFilter::Category(cat_name.clone());
                this_cat.populate_doc_list();
                let row_name = format!("category:{}", cat_name);
                let mut i = 0;
                while let Some(row) = this_cat.filter_list.row_at_index(i) {
                    if row.widget_name().as_str() == row_name {
                        this_cat.filter_list.select_row(Some(&row));
                        return;
                    }
                    i += 1;
                }
            });
            cat_lbl.add_controller(cat_click);
        }
        for tag in tags.iter().take(4) {
            let chip = Label::new(Some(&tag.name));
            chip.add_css_class("fond-row-detail");
            chip.set_tooltip_text(Some(&format!("Show only {}", tag.name)));
            hbox.append(&chip);
            let chip_click = gtk4::GestureClick::new();
            chip_click.set_button(1);
            let this_chip = self.clone();
            let tag_id = tag.id;
            let chip_ref = chip.clone();
            chip_click.connect_pressed(move |g, _, _, _| {
                g.set_state(gtk4::EventSequenceState::Claimed);
                chip_ref.add_css_class("chip-active");
                let chip_weak = chip_ref.downgrade();
                glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
                    if let Some(c) = chip_weak.upgrade() {
                        c.remove_css_class("chip-active");
                    }
                });
                *this_chip.current_filter.borrow_mut() = LibraryFilter::Tag(tag_id);
                this_chip.populate_doc_list();
                let tag_name = format!("tag:{}", tag_id);
                let mut i = 0;
                while let Some(row) = this_chip.filter_list.row_at_index(i) {
                    if row.widget_name().as_str() == tag_name {
                        this_chip.filter_list.select_row(Some(&row));
                        return;
                    }
                    i += 1;
                }
                let mut j = 0;
                while let Some(row) = this_chip.bottom_filter_list.row_at_index(j) {
                    if row.widget_name().as_str() == tag_name {
                        this_chip.bottom_filter_list.select_row(Some(&row));
                        return;
                    }
                    j += 1;
                }
            });
            chip.add_controller(chip_click);
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

        if self.selection.borrow().contains(&doc.id) {
            row.add_css_class("doc-selected");
        }

        row.set_child(Some(&hbox));

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

        // Ctrl+click multi-select
        let ctrl_click = gtk4::GestureClick::new();
        ctrl_click.set_button(1);
        let this = self.clone();
        let doc_id = doc.id;
        ctrl_click.connect_pressed(move |g, _, _, _| {
            let mods = g.current_event_state();
            if mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK) {
                g.set_state(gtk4::EventSequenceState::Claimed);
                {
                    let mut sel = this.selection.borrow_mut();
                    if sel.contains(&doc_id) {
                        sel.remove(&doc_id);
                    } else {
                        sel.insert(doc_id);
                    }
                }
                this.update_action_bar();
                this.populate_doc_list();
            }
        });
        row.add_controller(ctrl_click);

        // Right-click context menu
        let gesture = gtk4::GestureClick::new();
        gesture.set_button(3);
        let this = self.clone();
        let doc_clone = doc.clone();
        let row_weak = row.downgrade();
        gesture.connect_pressed(move |g, _, x, y| {
            g.set_state(gtk4::EventSequenceState::Claimed);
            if let Some(row) = row_weak.upgrade() {
                this.show_doc_menu(&row, &doc_clone, x, y);
            }
        });
        row.add_controller(gesture);

        row
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
