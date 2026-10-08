//! Cursor tracking: position label, section and heading context, typing-scoped scrolling, and undo/redo button state.

use super::*;

// Builds a breadcrumb path string for the cursor position, e.g. "Intro › Methods".
// Scans backward collecting the first heading at each level encountered.
pub(super) fn build_heading_path(buf: &sourceview5::Buffer, line_idx: i32) -> String {
    let mut path: Vec<(u32, String)> = Vec::new();
    let mut min_level: u32 = u32::MAX;
    let mut check = line_idx;
    while check >= 0 {
        if let Some(iter) = buf.iter_at_line(check) {
            let mut end = iter;
            end.forward_to_line_end();
            let text = buf.text(&iter, &end, false).to_string();
            if text.starts_with('=') {
                let level = text.chars().take_while(|&c| c == '=').count() as u32;
                let content = text[level as usize..].trim().to_string();
                if path.is_empty() || level < min_level {
                    path.push((level, content));
                    min_level = level;
                    if level == 1 {
                        break;
                    }
                }
            }
        }
        check -= 1;
    }
    path.reverse();
    path.into_iter()
        .map(|(_, t)| t)
        .collect::<Vec<_>>()
        .join(" / ")
}

// Returns the 1-based line number of the nearest heading at or above `line_idx`,
// or u32::MAX if none found.
pub(super) fn find_heading_line_for(buf: &sourceview5::Buffer, line_idx: i32) -> u32 {
    let mut check = line_idx;
    while check >= 0 {
        if let Some(iter) = buf.iter_at_line(check) {
            let mut end = iter;
            end.forward_to_line_end();
            let text = buf.text(&iter, &end, false);
            if text.starts_with('=') {
                return (check + 1) as u32;
            }
        }
        check -= 1;
    }
    u32::MAX
}

pub(super) fn is_heading_line(buf: &sourceview5::Buffer, ln: i32) -> bool {
    if let Some(it) = buf.iter_at_line(ln) {
        let mut end = it;
        end.forward_to_line_end();
        let text = buf.text(&it, &end, false);
        return text.starts_with('=');
    }
    false
}

impl EditorPane {
    pub(super) fn wire_cursor_tracking(&self, tab: &TabContext) {
        // ── Cursor position tracking + heading detection ──────────────────────

        let cursor_lbl = self.cursor_label.clone();
        let section_wc_lbl = self.section_wc_label.clone();
        let last_section_line: Rc<std::cell::Cell<i32>> = Rc::new(std::cell::Cell::new(-1));
        let section_wc_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        let wc_lbl_for_sel = self.word_count_label.clone();
        let last_wc_for_mark = self.last_wc_text.clone();
        // Extra clones for the selection_bound handler below.
        let wc_lbl_for_sel_bound = wc_lbl_for_sel.clone();
        let last_wc_for_sel_bound = last_wc_for_mark.clone();
        let breadcrumb_lbl = self.breadcrumb_label.clone();
        let lsp_lbl_for_pkg = self.lsp_status_label.clone();
        let on_heading_cb = self.on_cursor_heading.clone();
        let on_moved_cb = self.on_cursor_moved.clone();
        let cursor_moved_gen: Rc<std::cell::Cell<u64>> = Rc::new(std::cell::Cell::new(0));
        let typewriter_gen: Rc<std::cell::Cell<u64>> = Rc::new(std::cell::Cell::new(0));
        let heading_sync_gen: Rc<std::cell::Cell<u64>> = Rc::new(std::cell::Cell::new(0));
        let path_for_heading = tab.path.clone();
        let path_for_moved = tab.path.clone();
        let last_heading_line: Rc<RefCell<u32>> = Rc::new(RefCell::new(u32::MAX));
        let typewriter_for_mark = self.typewriter_scroll.clone();
        let view_for_typewriter = tab.view.clone();
        let scroll_for_typewriter = tab.scroll.clone();
        let crosshair_for_mark = self.typewriter_crosshair.clone();
        let crosshair_timer_for_mark = self.typewriter_crosshair_timer.clone();
        let view_for_scroll_margin = tab.view.clone();
        // Track the cursor's last vertical (window) position the typewriter
        // tab.scroll recentered on, so we only fire when the cursor moves to a
        // new display line (not every column move). This must be the *display*
        // line, not the buffer line: with word wrap on (the default), a single
        // long paragraph spans many display lines but is all one buffer line,
        // so gating on buf line() alone would never recenter while typing
        // within a wrapped paragraph and text would run off screen.
        let last_tw_y: Rc<std::cell::Cell<i32>> = Rc::new(std::cell::Cell::new(i32::MIN));
        // Key presses are what mark an edit as the user's own typing. GTK does not
        // emit mark-set for the insert mark while text is typed, so inferring
        // "typing" from that signal left the flag set until the next click.
        let key_intent: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));
        let pointer_down: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        {
            let keys = EventControllerKey::new();
            keys.set_propagation_phase(PropagationPhase::Capture);
            let intent = key_intent.clone();
            keys.connect_key_pressed(move |_, key, _, mods| {
                use gtk4::gdk::Key;
                let modifier_only = matches!(
                    key,
                    Key::Shift_L
                        | Key::Shift_R
                        | Key::Control_L
                        | Key::Control_R
                        | Key::Alt_L
                        | Key::Alt_R
                        | Key::Super_L
                        | Key::Super_R
                        | Key::Caps_Lock
                );
                if !modifier_only
                    && !mods.intersects(
                        gtk4::gdk::ModifierType::CONTROL_MASK | gtk4::gdk::ModifierType::ALT_MASK,
                    )
                {
                    intent.set(Some(Instant::now()));
                }
                glib::Propagation::Proceed
            });
            tab.view.add_controller(keys);

            let legacy = gtk4::EventControllerLegacy::new();
            legacy.set_propagation_phase(PropagationPhase::Capture);
            let down = pointer_down.clone();
            let intent = key_intent.clone();
            legacy.connect_event(move |_, event| {
                match event.event_type() {
                    gtk4::gdk::EventType::ButtonPress => {
                        down.set(true);
                        intent.set(None);
                    }
                    gtk4::gdk::EventType::ButtonRelease | gtk4::gdk::EventType::GrabBroken => {
                        down.set(false)
                    }
                    _ => {}
                }
                glib::Propagation::Proceed
            });
            tab.view.add_controller(legacy);
        }

        // Scrolling that belongs to typing (keep the cursor off the edges,
        // typewriter recentering). Runs from the edit itself, never from a
        // click, and never while a button is held: scrolling under a held button
        // stretches the selection to whatever slides beneath the pointer.
        let typing_scroll: Rc<dyn Fn(&Buffer)> = {
            let vs = view_for_scroll_margin.clone();
            let typewriter = typewriter_for_mark.clone();
            let last_tw_y = last_tw_y.clone();
            let typewriter_gen = typewriter_gen.clone();
            let sc_tw = scroll_for_typewriter.clone();
            let vt = view_for_typewriter.clone();
            let crosshair = crosshair_for_mark.clone();
            let crosshair_timer = crosshair_timer_for_mark.clone();
            let down = pointer_down.clone();
            Rc::new(move |buf: &Buffer| {
                if down.get() || buf.has_selection() {
                    return;
                }
                let insert_mark = buf.get_insert();
                let cursor = buf.iter_at_mark(&insert_mark);
                // Within 1 viewport of the visible area only: further away means
                // the user scrolled there on purpose.
                let loc = vs.iter_location(&cursor);
                let (_, wy) = vs.buffer_to_window_coords(TextWindowType::Widget, loc.x(), loc.y());
                let view_h = vs.allocated_height();
                if wy > -view_h && wy < 2 * view_h {
                    vs.scroll_to_mark(&insert_mark, 0.15, false, 0.0, 0.5);
                }
                if !*typewriter.borrow() {
                    return;
                }
                let y = loc.y();
                if y == last_tw_y.get() {
                    return;
                }
                last_tw_y.set(y);
                let gen = typewriter_gen.get().wrapping_add(1);
                typewriter_gen.set(gen);
                let gen_rc = typewriter_gen.clone();
                let vt = vt.clone();
                let sc_tw = sc_tw.clone();
                let crosshair = crosshair.clone();
                let crosshair_timer = crosshair_timer.clone();
                let down = down.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(80), move || {
                    if gen_rc.get() != gen || down.get() {
                        return;
                    }
                    // Blank space past the last line, or the end of the document, can
                    // never scroll up to the anchor.
                    let pad = vt.height() / 2;
                    if vt.bottom_margin() != pad {
                        vt.set_bottom_margin(pad);
                    }
                    let mut c = vt.buffer().iter_at_mark(&insert_mark);
                    let h = sc_tw.hadjustment().value();
                    vt.scroll_to_iter(&mut c, 0.0, true, 0.0, 0.45);
                    sc_tw.hadjustment().set_value(h);
                    crosshair.set_visible(true);
                    crosshair.queue_draw();
                    if let Some(id) = crosshair_timer.borrow_mut().take() {
                        id.remove();
                    }
                    let ch = crosshair.clone();
                    let ct = crosshair_timer.clone();
                    let id = glib::timeout_add_local_once(
                        std::time::Duration::from_millis(800),
                        move || {
                            ch.set_visible(false);
                            *ct.borrow_mut() = None;
                        },
                    );
                    *crosshair_timer.borrow_mut() = Some(id);
                });
            })
        };
        {
            let intent = key_intent.clone();
            let down = pointer_down.clone();
            let pending: Rc<Cell<bool>> = Rc::new(Cell::new(false));
            tab.buffer.connect_changed(move |buf| {
                let typed = intent
                    .get()
                    .is_some_and(|t| t.elapsed() < KEY_INTENT_WINDOW);
                if !typed || down.get() || pending.replace(true) {
                    return;
                }
                let buf = buf.clone();
                let run = typing_scroll.clone();
                let pending = pending.clone();
                glib::idle_add_local_once(move || {
                    pending.set(false);
                    run(&buf);
                });
            });
        }

        // Ctrl+click → show that spot in the preview. A modifier, not a plain
        // click: following every click used to yank the preview around while
        // simply placing the cursor (see `set_on_cursor_moved`'s call site).
        {
            let ctrl_click = GestureClick::new();
            ctrl_click.set_button(1);
            let view_cc = tab.view.clone();
            let path_cc = tab.path.clone();
            let cb = self.on_show_in_preview.clone();
            ctrl_click.connect_pressed(move |g, n_press, x, y| {
                if n_press != 1
                    || !g
                        .current_event_state()
                        .contains(gtk4::gdk::ModifierType::CONTROL_MASK)
                {
                    return;
                }
                let (bx, by) =
                    view_cc.window_to_buffer_coords(TextWindowType::Widget, x as i32, y as i32);
                let Some(iter) = view_cc.iter_at_location(bx, by) else {
                    return;
                };
                if let Some(f) = cb.borrow().as_ref() {
                    f(path_cc.clone(), iter.offset().max(0) as usize);
                }
            });
            tab.view.add_controller(ctrl_click);
        }
        tab.buffer.connect_cursor_position_notify(move |buf| {
            {
                if let Some(boundary) = hidden_template_end(buf, buf.cursor_position()) {
                    if buf.has_selection() {
                        buf.move_mark(&buf.get_insert(), &boundary);
                    } else {
                        buf.place_cursor(&boundary);
                    }
                    return;
                }
                let cursor = buf.iter_at_offset(buf.cursor_position());
                let line = cursor.line() + 1;
                let col = cursor.line_offset() + 1;
                cursor_lbl.set_text(&format!("L{line}:C{col}"));
                cursor_lbl.set_tooltip_text(Some(&format!("Line {line}, Column {col}")));

                // Section word count — recompute only when the line changes, and
                // debounced so holding an arrow key doesn't rescan per line.
                let cur_line = cursor.line();
                if cur_line != last_section_line.get() {
                    last_section_line.set(cur_line);
                    if let Some(id) = section_wc_timer.borrow_mut().take() {
                        id.remove();
                    }
                    let lbl = section_wc_lbl.clone();
                    let b = buf.clone();
                    let t = section_wc_timer.clone();
                    *section_wc_timer.borrow_mut() = Some(glib::timeout_add_local_once(
                        SECTION_WC_DEBOUNCE,
                        move || {
                            *t.borrow_mut() = None;
                            if let Some(wc) = section_word_count_for_line(&b, cur_line) {
                                lbl.set_text(&format!("§ {wc}"));
                                lbl.set_tooltip_text(Some("Words in this section"));
                            } else {
                                lbl.set_text("");
                            }
                        },
                    ));
                }

                // Selection word/sentence stats — use cached wc to avoid reading entire tab.buffer
                if let Some((sel_s, sel_e)) = buf.selection_bounds() {
                    let sel_text = buf.text(&sel_s, &sel_e, false).to_string();
                    let word_count = sel_text.split_whitespace().count();
                    let sentence_count = sel_text
                        .split(['.', '!', '?'])
                        .filter(|s| !s.trim().is_empty())
                        .count();
                    wc_lbl_for_sel.set_text(&format!(
                        "{word_count} words, {sentence_count} sentences selected"
                    ));
                } else {
                    // Restore cached word count — no full tab.buffer read needed
                    let cached = last_wc_for_mark.borrow().clone();
                    if !cached.is_empty() {
                        wc_lbl_for_sel.set_text(&cached);
                    }
                }

                // Keyboard-driven, as opposed to a click or a programmatic move.
                let was_typing = key_intent
                    .get()
                    .is_some_and(|t| t.elapsed() < KEY_INTENT_WINDOW)
                    && !pointer_down.get();

                // #import "@preview/pkg:ver" tooltip
                {
                    let line_start = buf
                        .iter_at_line(cursor.line())
                        .unwrap_or_else(|| buf.start_iter());
                    let line_end = {
                        let mut e = line_start;
                        if !e.ends_line() {
                            e.forward_to_line_end();
                        }
                        e
                    };
                    let line_text = buf.text(&line_start, &line_end, false).to_string();
                    let trimmed = line_text.trim();
                    if let Some(rest) = trimmed.strip_prefix("#import \"@preview/") {
                        let pkg_name: String = rest
                            .chars()
                            .take_while(|c| *c != ':' && *c != '"')
                            .collect();
                        // Strip version suffix (e.g. "codly" from "codly:1.0.0")
                        let base_name = pkg_name.split(':').next().unwrap_or(&pkg_name);
                        if let Some((_, desc)) = IMPORT_PACKAGE_TOOLTIPS
                            .iter()
                            .find(|(n, _)| *n == base_name)
                        {
                            let lbl = lsp_lbl_for_pkg.clone();
                            let desc_s = desc.to_string();
                            let pkg_s = base_name.to_string();
                            lbl.set_text(&format!("{pkg_s}: {desc_s}"));
                            glib::timeout_add_local_once(
                                std::time::Duration::from_secs(3),
                                move || {
                                    lbl.set_text("");
                                },
                            );
                        }
                    }
                }

                // Update breadcrumb heading path
                let heading_path = build_heading_path(buf, cursor.line());
                breadcrumb_lbl.set_text(&heading_path);

                // Scan backward for a heading; only tab.scroll preview on keyboard nav (not mouse click).
                // Debounced 200 ms so the preview doesn't jump on every section boundary crossing.
                if was_typing {
                    let heading_line = find_heading_line_for(buf, cursor.line());
                    if heading_line != *last_heading_line.borrow() {
                        *last_heading_line.borrow_mut() = heading_line;
                        if heading_line != u32::MAX && on_heading_cb.borrow().is_some() {
                            let gen = heading_sync_gen.get().wrapping_add(1);
                            heading_sync_gen.set(gen);
                            let gen_rc = heading_sync_gen.clone();
                            let cb_h = on_heading_cb.clone();
                            let path_h = path_for_heading.clone();
                            glib::timeout_add_local_once(
                                std::time::Duration::from_millis(200),
                                move || {
                                    if gen_rc.get() != gen {
                                        return;
                                    }
                                    if let Some(f) = cb_h.borrow().as_ref() {
                                        f(path_h.clone(), heading_line);
                                    }
                                },
                            );
                        }
                    }
                }

                // Debounced reverse sync: notify app_window of cursor position 300ms after it
                // settles. Only fire on keyboard movement (was_typing), not mouse clicks —
                // otherwise a click in the editor jumps the preview to match the clicked line.
                // Uses a generation counter rather than SourceId::remove() — glib 0.18 panics
                // when remove() is called on a source that timeout_add_local_once already removed.
                if was_typing {
                    let line = cursor.line() as u32;
                    let total = buf.line_count() as u32;
                    let gen = cursor_moved_gen.get().wrapping_add(1);
                    cursor_moved_gen.set(gen);
                    let cb = on_moved_cb.clone();
                    let path_m = path_for_moved.clone();
                    let gen_rc = cursor_moved_gen.clone();
                    glib::timeout_add_local_once(
                        std::time::Duration::from_millis(300),
                        move || {
                            if gen_rc.get() == gen {
                                if let Some(f) = cb.borrow().as_ref() {
                                    f(path_m.clone(), line, total);
                                }
                            }
                        },
                    );
                }
            }
        });

        // When the user clicks to deselect, GTK moves the `insert` mark first and
        // `selection_bound` second. The `insert` handler above fires while
        // `selection_bound` is still at the old anchor, making
        // `selection_bounds()` return Some — so it prints "N selected" even
        // though nothing is selected. This second handler fires when
        // `selection_bound` arrives and clears the ghost label.
        tab.buffer.connect_mark_set(move |buf, _iter, mark| {
            if mark.name().as_deref() != Some("selection_bound") {
                return;
            }
            if !buf.has_selection() {
                let cached = last_wc_for_sel_bound.borrow().clone();
                if !cached.is_empty() {
                    wc_lbl_for_sel_bound.set_text(&cached);
                }
            }
        });
    }

    pub(super) fn wire_undo_redo_sensitivity(&self, tab: &TabContext) {
        // `tab.scroll` sits inside the Overlay that is the actual notebook page, so
        // `page_num(&tab.scroll)` is always None — comparing it to the current page left
        // these buttons updating only on a tab switch, stuck greyed out while typing.
        fn is_current_page(nb: &TabHost, scroll: &ScrolledWindow) -> bool {
            nb.nth_page(nb.current_page())
                .is_some_and(|page| scroll.is_ancestor(&page))
        }
        // ── Undo / Redo sensitivity ───────────────────────────────────────────
        // Guard against background-tab interference: only update the shared
        // undo/redo buttons when the notification comes from the active tab's
        // tab.buffer. A background tab's begin_user_action or set_text can fire
        // notify::can-undo and silently grey out the button for the active tab.
        {
            let ub = self.undo_btn.clone();
            let nb_u = self.notebook.clone();
            let sc_u = tab.scroll.clone();
            tab.buffer.connect_can_undo_notify(move |buf| {
                if is_current_page(&nb_u, &sc_u) {
                    ub.set_sensitive(buf.can_undo());
                }
            });
            let rb = self.redo_btn.clone();
            let nb_r = self.notebook.clone();
            let sc_r = tab.scroll.clone();
            tab.buffer.connect_can_redo_notify(move |buf| {
                if is_current_page(&nb_r, &sc_r) {
                    rb.set_sensitive(buf.can_redo());
                }
            });
        }
    }
}
