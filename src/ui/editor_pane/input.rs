//! Keyboard handling: the popup/ghost key controller and the editing shortcuts
//! (auto-pair, comment, bold/italic, duplicate, undo, word navigation).
//!
//! Every controller is capture-phase, so they run before GtkTextView's own key
//! bindings, in the order they were added to the view — which is the only thing
//! deciding who wins when two want the same key. That order, first to last:
//!
//! 1. `wire_cursor_tracking` — records that a key was pressed (never consumes it)
//! 2. `wire_key_controller` here: suggestion popups and ghost text (Tab, Esc,
//!    Return, arrows), then auto-pair, Ctrl+/, Ctrl+B/I, Ctrl+D, Ctrl+Enter,
//!    Ctrl+Z/Y, Ctrl+Arrow word navigation and heading jumps
//! 3. `writing::wire` — list Enter/Tab, paste-a-link, move lines, expand selection
//! 4. `wire_autocorrect` — bubble phase, so it runs after GTK's own handling
//!
//! A new shortcut goes in the group it belongs to; if it can clash with a key
//! above, check this order first.

use super::*;

/// A shortcut's letter regardless of Caps Lock, Shift or keyboard layout: on a
/// non-Latin layout the key's Latin letter is taken from the layout's first group.
pub(super) fn base_key(key: gtk4::gdk::Key, keycode: u32) -> gtk4::gdk::Key {
    if key.to_unicode().is_some_and(|c| c.is_ascii()) {
        return key.to_lower();
    }
    if let Some(display) = gtk4::gdk::Display::default() {
        if let Some(maps) = display.map_keycode(keycode) {
            if let Some((_, latin)) = maps
                .iter()
                .find(|(_, k)| k.to_unicode().is_some_and(|c| c.is_ascii_alphabetic()))
            {
                return latin.to_lower();
            }
        }
    }
    key
}

impl EditorPane {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn wire_key_controller(
        &self,
        view: &View,
        buffer: &Buffer,
        bib_popup: &BibPopup,
        lsp_popup: &LspPopup,
        ac_mark: &Rc<RefCell<Option<gtk4::TextMark>>>,
        lsp_mark: &Rc<RefCell<Option<gtk4::TextMark>>>,
        completing: &Rc<RefCell<bool>>,
        lsp_completing: &Rc<RefCell<bool>>,
        ghost_item: &Rc<RefCell<Option<CompletionItem>>>,
        ghost_label: &Label,
        completion_suppressed_at: &Rc<Cell<i32>>,
        ghost_bib_entry: &Rc<RefCell<Option<PopupEntry>>>,
    ) -> (Rc<Cell<Option<(f64, f64)>>>, Rc<Cell<Instant>>) {
        let bib_popup_key = bib_popup.clone();
        let lsp_popup_key = lsp_popup.clone();
        let buf_key = buffer.clone();
        let mark_key = ac_mark.clone();
        let lsp_mark_key = lsp_mark.clone();
        let completing_key = completing.clone();
        let lsp_completing_key = lsp_completing.clone();
        let view_key = view.clone();
        let bib_active_key = self.bib_active.clone();
        let ghost_item_key = ghost_item.clone();
        let ghost_label_key = ghost_label.clone();
        let hint_lbl_key = self.lsp_status_label.clone();
        let suppressed_key = completion_suppressed_at.clone();
        let lsp_mark_suppress = lsp_mark.clone();
        let ghost_bib_key = ghost_bib_entry.clone();
        let view_bib_key = view.clone();

        let key_ctrl = EventControllerKey::new();
        key_ctrl.set_propagation_phase(PropagationPhase::Capture);
        key_ctrl.connect_key_pressed(move |_, key, _, _mods| {
            use gtk4::gdk::Key;

            // Tab accepts the inline ghost suggestion even when no list is up —
            // the fish-shell gesture, and the whole point of showing the ghost
            // before the list appears. Escape dismisses just the ghost, leaving
            // what the user actually typed alone.
            if !lsp_popup_key.is_visible()
                && !bib_popup_key.is_visible()
                && ghost_label_key.is_visible()
            {
                // A citation ghost is taken the citation way — same key, same
                // feel, different insertion.
                if key == Key::Tab {
                    let entry = ghost_bib_key.borrow().clone();
                    if let Some(entry) = entry {
                        clear_citation_ghost(&ghost_label_key, &ghost_bib_key, &hint_lbl_key);
                        *bib_active_key.borrow_mut() = false;
                        do_bib_complete(
                            &buf_key,
                            &mark_key,
                            &completing_key,
                            &bib_popup_key,
                            &view_bib_key,
                            &entry,
                        );
                        return glib::Propagation::Stop;
                    }
                }
                match key {
                    Key::Tab => {
                        let item = ghost_item_key.borrow().clone();
                        if let Some(i) = item {
                            clear_ghost(&ghost_label_key, &ghost_item_key, &hint_lbl_key);
                            do_lsp_complete(
                                &buf_key,
                                &lsp_mark_key,
                                &lsp_completing_key,
                                &lsp_popup_key,
                                &view_key,
                                i,
                            );
                            return glib::Propagation::Stop;
                        }
                    }
                    Key::Escape => {
                        suppress_current_completion(&buf_key, &lsp_mark_suppress, &suppressed_key);
                        clear_citation_ghost(&ghost_label_key, &ghost_bib_key, &hint_lbl_key);
                        clear_ghost(&ghost_label_key, &ghost_item_key, &hint_lbl_key);
                        return glib::Propagation::Stop;
                    }
                    _ => {}
                }
            }

            // LSP popup takes priority
            if lsp_popup_key.is_visible() {
                return match key {
                    Key::Escape => {
                        // Dismiss, and leave what was typed alone. Escape used to
                        // delete back to the `#`, which threw away the user's own
                        // text for the crime of not wanting a suggestion — and it
                        // made "quiet for this word" impossible, there being no
                        // word left to be quiet about.
                        suppress_current_completion(&buf_key, &lsp_mark_suppress, &suppressed_key);
                        lsp_popup_key.hide();
                        clear_ghost(&ghost_label_key, &ghost_item_key, &hint_lbl_key);
                        glib::Propagation::Stop
                    }
                    Key::Tab => {
                        let item = lsp_popup_key
                            .selected_item()
                            .or_else(|| lsp_popup_key.first_item());
                        if let Some(i) = item {
                            clear_ghost(&ghost_label_key, &ghost_item_key, &hint_lbl_key);
                            do_lsp_complete(
                                &buf_key,
                                &lsp_mark_key,
                                &lsp_completing_key,
                                &lsp_popup_key,
                                &view_key,
                                i,
                            );
                        }
                        glib::Propagation::Stop
                    }
                    Key::Return => {
                        if let Some(i) = lsp_popup_key.selected_item() {
                            clear_ghost(&ghost_label_key, &ghost_item_key, &hint_lbl_key);
                            do_lsp_complete(
                                &buf_key,
                                &lsp_mark_key,
                                &lsp_completing_key,
                                &lsp_popup_key,
                                &view_key,
                                i,
                            );
                            glib::Propagation::Stop
                        } else {
                            glib::Propagation::Proceed
                        }
                    }
                    Key::Down => {
                        lsp_popup_key.move_selection(1);
                        glib::Propagation::Stop
                    }
                    Key::Up => {
                        lsp_popup_key.move_selection(-1);
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                };
            }

            // Bib popup
            if !bib_popup_key.is_visible() {
                return glib::Propagation::Proceed;
            }
            match key {
                Key::Escape => {
                    *bib_active_key.borrow_mut() = false;
                    dismiss_popup_only(&bib_popup_key, &buf_key, &mark_key);
                    glib::Propagation::Stop
                }
                Key::Tab => {
                    let chosen = bib_popup_key
                        .selected_entry()
                        .or_else(|| bib_popup_key.first_filtered_entry());
                    if let Some(entry) = chosen {
                        *bib_active_key.borrow_mut() = false;
                        do_bib_complete(
                            &buf_key,
                            &mark_key,
                            &completing_key,
                            &bib_popup_key,
                            &view_key,
                            &entry,
                        );
                    }
                    glib::Propagation::Stop
                }
                Key::Return => {
                    if let Some(entry) = bib_popup_key.selected_entry() {
                        *bib_active_key.borrow_mut() = false;
                        do_bib_complete(
                            &buf_key,
                            &mark_key,
                            &completing_key,
                            &bib_popup_key,
                            &view_key,
                            &entry,
                        );
                        glib::Propagation::Stop
                    } else {
                        glib::Propagation::Proceed
                    }
                }
                Key::Down => {
                    bib_popup_key.move_selection(1);
                    glib::Propagation::Stop
                }
                Key::Up => {
                    bib_popup_key.move_selection(-1);
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        view.add_controller(key_ctrl);

        // ── Auto-pair brackets and quotes ─────────────────────────────────────
        // What gets paired is decided by `autopair::decide`, from what sits
        // around the cursor — see there for the rules.
        {
            let buf_pair = buffer.clone();
            let pair_ctrl = EventControllerKey::new();
            pair_ctrl.set_propagation_phase(PropagationPhase::Capture);
            pair_ctrl.connect_key_pressed(move |_, key, _, mods| {
                use gtk4::gdk::Key;
                // Don't interfere when modifier keys are held (shortcuts)
                if mods.intersects(
                    gtk4::gdk::ModifierType::CONTROL_MASK | gtk4::gdk::ModifierType::ALT_MASK,
                ) {
                    return glib::Propagation::Proceed;
                }

                // Backspace between an empty pair takes both halves.
                if key == Key::BackSpace {
                    if buf_pair.has_selection() {
                        return glib::Propagation::Proceed;
                    }
                    let pos = buf_pair.cursor_position();
                    if pos < 1 {
                        return glib::Propagation::Proceed;
                    }
                    let mut from = buf_pair.iter_at_offset(pos - 1);
                    let mid = buf_pair.iter_at_offset(pos);
                    if mid.is_end() {
                        return glib::Propagation::Proceed;
                    }
                    let mut to = buf_pair.iter_at_offset(pos + 1);
                    if autopair::is_empty_pair(from.char(), mid.char()) {
                        buf_pair.begin_user_action();
                        buf_pair.delete(&mut from, &mut to);
                        buf_pair.end_user_action();
                        return glib::Propagation::Stop;
                    }
                    return glib::Propagation::Proceed;
                }

                let typed = match key {
                    Key::parenleft => '(',
                    Key::bracketleft => '[',
                    Key::braceleft => '{',
                    Key::quotedbl => '"',
                    Key::dollar => '$',
                    Key::parenright => ')',
                    Key::bracketright => ']',
                    Key::braceright => '}',
                    _ => return glib::Propagation::Proceed,
                };

                // With a selection, an opening pair character wraps the
                // selected text instead of replacing it (the same behavior
                // as VS Code, Sublime, etc.) — e.g. selecting a word and
                // typing `"` surrounds it in one quote each side, rather
                // than deleting it and leaving a lone `"`. A closing bracket
                // alone falls through to GTK's normal replace-selection
                // behavior, unchanged.
                if let Some(close) = autopair::partner(typed) {
                    if let Some((start, end)) = buf_pair.selection_bounds() {
                        let start_off = start.offset();
                        let end_off = end.offset();
                        buf_pair.begin_user_action();
                        let mut end_iter = buf_pair.iter_at_offset(end_off);
                        buf_pair.insert(&mut end_iter, &close.to_string());
                        let mut start_iter = buf_pair.iter_at_offset(start_off);
                        buf_pair.insert(&mut start_iter, &typed.to_string());
                        buf_pair.end_user_action();
                        // Re-select the original text, now shifted right by
                        // the inserted opening character, so the wrap can be
                        // chained (select again, wrap again) or the
                        // selection simply continues to read naturally.
                        let new_start = buf_pair.iter_at_offset(start_off + 1);
                        let new_end = buf_pair.iter_at_offset(end_off + 1);
                        buf_pair.select_range(&new_start, &new_end);
                        return glib::Propagation::Stop;
                    }
                }
                if buf_pair.has_selection() {
                    return glib::Propagation::Proceed;
                }

                let pos = buf_pair.cursor_position();
                let cursor = buf_pair.iter_at_offset(pos);
                let next = if cursor.is_end() || cursor.ends_line() {
                    None
                } else {
                    Some(cursor.char())
                };
                let before = text_before_in_paragraph(&buf_pair, &cursor);

                match autopair::decide(typed, &before, next) {
                    autopair::Action::SkipOver => {
                        let ahead = buf_pair.iter_at_offset(pos + 1);
                        buf_pair.place_cursor(&ahead);
                        glib::Propagation::Stop
                    }
                    autopair::Action::Pair => {
                        let close = autopair::partner(typed).unwrap_or(typed);
                        buf_pair.begin_user_action();
                        buf_pair.insert_at_cursor(&format!("{typed}{close}"));
                        // Move cursor back one character to sit between the pair
                        let iter = buf_pair.iter_at_offset(pos + 1);
                        buf_pair.place_cursor(&iter);
                        buf_pair.end_user_action();
                        glib::Propagation::Stop
                    }
                    autopair::Action::Single => glib::Propagation::Proceed,
                }
            });
            view.add_controller(pair_ctrl);
        }

        // ── Comment toggle (Ctrl+/) ───────────────────────────────────────────
        {
            let buf_cmt = buffer.clone();
            let cmt_ctrl = EventControllerKey::new();
            cmt_ctrl.set_propagation_phase(PropagationPhase::Capture);
            cmt_ctrl.connect_key_pressed(move |_, key, keycode, mods| {
                let key = base_key(key, keycode);
                use gtk4::gdk::Key;
                let ctrl = mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
                if !ctrl || key != Key::slash {
                    return glib::Propagation::Proceed;
                }

                let (first_line, last_line) = if let Some((s, e)) = buf_cmt.selection_bounds() {
                    let end_line = if e.line_offset() == 0 && e.line() > s.line() {
                        e.line() - 1
                    } else {
                        e.line()
                    };
                    (s.line(), end_line)
                } else {
                    let line = buf_cmt.iter_at_offset(buf_cmt.cursor_position()).line();
                    (line, line)
                };

                // Determine whether all non-empty lines start with "//"
                let all_commented = (first_line..=last_line).all(|ln| {
                    if let Some(it) = buf_cmt.iter_at_line(ln) {
                        let mut end = it;
                        end.forward_to_line_end();
                        let line_text = buf_cmt.text(&it, &end, false).to_string();
                        line_text.trim_start().is_empty()
                            || line_text.trim_start().starts_with("//")
                    } else {
                        true
                    }
                });

                buf_cmt.begin_user_action();
                for ln in (first_line..=last_line).rev() {
                    let Some(line_start) = buf_cmt.iter_at_line(ln) else {
                        continue;
                    };
                    let mut line_end = line_start;
                    line_end.forward_to_line_end();
                    let line_text = buf_cmt.text(&line_start, &line_end, false).to_string();
                    if line_text.trim_start().is_empty() {
                        continue;
                    }

                    if all_commented {
                        // Remove "// " or "//" prefix
                        let stripped = line_text.trim_start();
                        let indent_len = (line_text.len() - stripped.len()) as i32;
                        if let Some(mut del_start) = buf_cmt.iter_at_line_offset(ln, indent_len) {
                            let remove = if stripped.starts_with("// ") { 3 } else { 2 };
                            let mut del_end = del_start;
                            del_end.forward_chars(remove);
                            buf_cmt.delete(&mut del_start, &mut del_end);
                        }
                    } else {
                        // Insert "// " at indent level
                        let stripped = line_text.trim_start();
                        let indent_len = (line_text.len() - stripped.len()) as i32;
                        if let Some(mut ins) = buf_cmt.iter_at_line_offset(ln, indent_len) {
                            buf_cmt.insert(&mut ins, "// ");
                        }
                    }
                }
                buf_cmt.end_user_action();
                glib::Propagation::Stop
            });
            view.add_controller(cmt_ctrl);
        }

        // ── Bold (Ctrl+B) / Italic (Ctrl+I) ─────────────────────────────────
        {
            let buf_bi = buffer.clone();
            let bi_ctrl = EventControllerKey::new();
            bi_ctrl.set_propagation_phase(PropagationPhase::Capture);
            bi_ctrl.connect_key_pressed(move |_, key, keycode, mods| {
                let key = base_key(key, keycode);
                use gtk4::gdk::Key;
                let ctrl = mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
                let shift = mods.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
                if !ctrl || shift {
                    return glib::Propagation::Proceed;
                }
                let marker = match key {
                    Key::b => "*",
                    Key::i => "_",
                    _ => return glib::Propagation::Proceed,
                };
                let mlen = marker.len() as i32;
                if let Some((sel_s, sel_e)) = buf_bi.selection_bounds() {
                    let start_off = sel_s.offset();
                    let end_off = sel_e.offset();
                    let text = buf_bi.text(&sel_s, &sel_e, false).to_string();
                    buf_bi.begin_user_action();
                    if text.starts_with(marker)
                        && text.ends_with(marker)
                        && text.len() > 2 * marker.len()
                    {
                        let inner = text[marker.len()..text.len() - marker.len()].to_string();
                        let inner_len = inner.chars().count() as i32;
                        let mut s = buf_bi.iter_at_offset(start_off);
                        let mut e = buf_bi.iter_at_offset(end_off);
                        buf_bi.delete(&mut s, &mut e);
                        let mut ins = buf_bi.iter_at_offset(start_off);
                        buf_bi.insert(&mut ins, &inner);
                        let ns = buf_bi.iter_at_offset(start_off);
                        let ne = buf_bi.iter_at_offset(start_off + inner_len);
                        buf_bi.select_range(&ns, &ne);
                    } else {
                        let tlen = text.chars().count() as i32;
                        let mut s = buf_bi.iter_at_offset(start_off);
                        let mut e = buf_bi.iter_at_offset(end_off);
                        buf_bi.delete(&mut s, &mut e);
                        let mut ins = buf_bi.iter_at_offset(start_off);
                        buf_bi.insert(&mut ins, &format!("{marker}{text}{marker}"));
                        let ns = buf_bi.iter_at_offset(start_off + mlen);
                        let ne = buf_bi.iter_at_offset(start_off + mlen + tlen);
                        buf_bi.select_range(&ns, &ne);
                    }
                    buf_bi.end_user_action();
                } else {
                    buf_bi.begin_user_action();
                    let pos = buf_bi.cursor_position();
                    let mut ins = buf_bi.iter_at_offset(pos);
                    buf_bi.insert(&mut ins, &format!("{marker}{marker}"));
                    let cursor = buf_bi.iter_at_offset(pos + mlen);
                    buf_bi.place_cursor(&cursor);
                    buf_bi.end_user_action();
                }
                glib::Propagation::Stop
            });
            view.add_controller(bi_ctrl);
        }

        // ── Select all (Ctrl+A) ──────────────────────────────────────────────
        {
            let buf_all = buffer.clone();
            let all_ctrl = EventControllerKey::new();
            all_ctrl.set_propagation_phase(PropagationPhase::Capture);
            all_ctrl.connect_key_pressed(move |_, key, keycode, mods| {
                let key = base_key(key, keycode);
                let plain_ctrl = mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK)
                    && !mods.intersects(
                        gtk4::gdk::ModifierType::SHIFT_MASK | gtk4::gdk::ModifierType::ALT_MASK,
                    );
                if !plain_ctrl || key != gtk4::gdk::Key::a {
                    return glib::Propagation::Proceed;
                }
                match hidden_template_end(&buf_all, 0) {
                    Some(body) => {
                        buf_all.select_range(&buf_all.end_iter(), &body);
                        glib::Propagation::Stop
                    }
                    None => glib::Propagation::Proceed,
                }
            });
            view.add_controller(all_ctrl);
        }

        // ── Duplicate line / selection (Ctrl+D) ──────────────────────────────
        {
            let buf_dup = buffer.clone();
            let dup_ctrl = EventControllerKey::new();
            dup_ctrl.set_propagation_phase(PropagationPhase::Capture);
            dup_ctrl.connect_key_pressed(move |_, key, keycode, mods| {
                let key = base_key(key, keycode);
                use gtk4::gdk::Key;
                let ctrl = mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
                let shift = mods.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
                if !ctrl || shift || key != Key::d {
                    return glib::Propagation::Proceed;
                }
                buf_dup.begin_user_action();
                if let Some((sel_s, sel_e)) = buf_dup.selection_bounds() {
                    let text = buf_dup.text(&sel_s, &sel_e, false).to_string();
                    let mut ins = sel_e;
                    buf_dup.insert(&mut ins, &text);
                } else {
                    let cursor_pos = buf_dup.cursor_position();
                    let cursor = buf_dup.iter_at_offset(cursor_pos);
                    let ln = cursor.line();
                    let Some(line_start) = buf_dup.iter_at_line(ln) else {
                        buf_dup.end_user_action();
                        return glib::Propagation::Stop;
                    };
                    let mut line_end = line_start;
                    if !line_end.ends_line() {
                        line_end.forward_to_line_end();
                    }
                    let text = buf_dup.text(&line_start, &line_end, false).to_string();
                    let mut ins = line_end;
                    buf_dup.insert(&mut ins, &format!("\n{text}"));
                }
                buf_dup.end_user_action();
                glib::Propagation::Stop
            });
            view.add_controller(dup_ctrl);
        }

        // ── Page break (Ctrl+Enter) ───────────────────────────────────────────
        {
            let buf_pb = buffer.clone();
            let pb_ctrl = EventControllerKey::new();
            pb_ctrl.set_propagation_phase(PropagationPhase::Capture);
            pb_ctrl.connect_key_pressed(move |_, key, _, mods| {
                use gtk4::gdk::Key;
                let ctrl = mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
                if !ctrl || key != Key::Return {
                    return glib::Propagation::Proceed;
                }
                buf_pb.begin_user_action();
                buf_pb.insert_at_cursor("\n#pagebreak()\n");
                buf_pb.end_user_action();
                glib::Propagation::Stop
            });
            view.add_controller(pb_ctrl);
        }

        // ── Undo / Redo keyboard shortcuts ───────────────────────────────────
        // GTK4 GtkTextView has built-in Ctrl+Z / Ctrl+Shift+Z bindings, but we add
        // explicit handling here so our nav_ctrl (Capture phase) can also update the
        // button sensitivity immediately rather than waiting for the next idle cycle.
        {
            let buf_undo = buffer.clone();
            let undo_ctrl = EventControllerKey::new();
            undo_ctrl.set_propagation_phase(PropagationPhase::Capture);
            undo_ctrl.connect_key_pressed(move |_, key, keycode, mods| {
                let key = base_key(key, keycode);
                use gtk4::gdk::Key;
                let ctrl = mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
                let shift = mods.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
                let alt = mods.contains(gtk4::gdk::ModifierType::ALT_MASK);
                if !ctrl || alt {
                    return glib::Propagation::Proceed;
                }
                if key == Key::z {
                    if shift {
                        if buf_undo.can_redo() {
                            buf_undo.redo();
                        }
                    } else {
                        if buf_undo.can_undo() {
                            buf_undo.undo();
                        }
                    }
                } else if key == Key::y && !shift {
                    if buf_undo.can_redo() {
                        buf_undo.redo();
                    }
                } else {
                    return glib::Propagation::Proceed;
                }
                glib::Propagation::Stop
            });
            view.add_controller(undo_ctrl);
        }

        // ── Typst-aware word navigation (Ctrl+Left/Right) ────────────────────
        // GTK's default word boundaries stop at '#' and '@', forcing two presses to
        // skip past `#set`, `@citation`, etc.  This controller intercepts Ctrl+arrow
        // and moves to the true end of the token (including the sigil character).
        {
            let buf_nav = buffer.clone();
            let view_nav = view.clone();
            let nav_ctrl = EventControllerKey::new();
            nav_ctrl.set_propagation_phase(PropagationPhase::Capture);
            nav_ctrl.connect_key_pressed(move |_, key, _, mods| {
                use gtk4::gdk::Key;
                let ctrl = mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
                let shift = mods.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
                let alt = mods.contains(gtk4::gdk::ModifierType::ALT_MASK);
                if !ctrl || alt {
                    return glib::Propagation::Proceed;
                }

                // ── Heading jump: Ctrl+Shift+Up / Ctrl+Shift+Down ────────────
                if shift && (key == Key::Up || key == Key::Down) {
                    let pos = buf_nav.cursor_position();
                    let cur_line = buf_nav.iter_at_offset(pos).line();
                    let line_count = buf_nav.line_count();
                    let target_line = if key == Key::Up {
                        (0..cur_line)
                            .rev()
                            .find(|&ln| is_heading_line(&buf_nav, ln))
                    } else {
                        (cur_line + 1..line_count).find(|&ln| is_heading_line(&buf_nav, ln))
                    };
                    if let Some(ln) = target_line {
                        if let Some(it) = buf_nav.iter_at_line(ln) {
                            buf_nav.place_cursor(&it);
                            view_nav.scroll_to_mark(&buf_nav.get_insert(), 0.1, false, 0.0, 0.3);
                        }
                    }
                    return glib::Propagation::Stop;
                }

                // ── Typst-aware word movement: Ctrl+Left / Ctrl+Right ────────
                // Ctrl+Shift+Left/Right use the same boundaries and extend the
                // selection, so selecting by word stops where moving by word does.
                if key != Key::Left && key != Key::Right {
                    return glib::Propagation::Proceed;
                }

                let pos = buf_nav.cursor_position();
                let mut it = buf_nav.iter_at_offset(pos);
                let forward = key == Key::Right;

                if forward {
                    // Skip leading whitespace first (mirrors GtkTextView default)
                    while !it.is_end() && it.char().is_whitespace() {
                        it.forward_char();
                    }
                    // If we're now on '#' or '@' (Typst sigils), absorb the sigil so
                    // the next word_end lands after the whole `#keyword` or `@key`.
                    if matches!(it.char(), '#' | '@') {
                        it.forward_char();
                    }
                    it.forward_word_end();
                } else {
                    // Skip whitespace before the cursor (the character *before* the
                    // iter, so a one-letter word is not stepped over with it).
                    loop {
                        let mut before = it;
                        if !before.backward_char() || !before.char().is_whitespace() {
                            break;
                        }
                        it = before;
                    }
                    it.backward_word_start();
                    // If the character just before the new position is '#' or '@', absorb it
                    let mut probe = it;
                    if probe.backward_char() && matches!(probe.char(), '#' | '@') {
                        it = probe;
                    }
                }

                if shift {
                    buf_nav.move_mark(&buf_nav.get_insert(), &it);
                } else {
                    buf_nav.place_cursor(&it);
                }
                view_nav.scroll_to_mark(&buf_nav.get_insert(), 0.07, false, 0.0, 0.5);
                glib::Propagation::Stop
            });
            view.add_controller(nav_ctrl);
        }

        // Viewport hold, shared by every edit that hands focus back to the view
        // and so provokes GTK's scroll-to-mark animation: paste, and applying a
        // spell suggestion from either popover. The vadjustment/hadjustment
        // handlers that honour these live further down.
        let hold_position: Rc<Cell<Option<(f64, f64)>>> = Rc::new(Cell::new(None));
        let hold_until: Rc<Cell<Instant>> = Rc::new(Cell::new(Instant::now()));

        (hold_position, hold_until)
    }
}
