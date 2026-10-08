//! Spell check: the underline pass, suggestions on right-click, and autocorrect.

use super::*;

pub(super) fn ensure_spell_tag(buffer: &Buffer) {
    let table = buffer.tag_table();
    if table.lookup("zerkalo-spell").is_none() {
        let tag = TextTag::new(Some("zerkalo-spell"));
        tag.set_underline(gtk4::pango::Underline::Error);
        tag.set_underline_rgba(Some(&gtk4::gdk::RGBA::new(0.22, 0.55, 0.97, 1.0)));
        table.add(&tag);
    }
}

pub(super) fn clear_spell_tags(buffer: &Buffer) {
    ensure_spell_tag(buffer);
    let (s, e) = buffer.bounds();
    // With spell check off this runs on every keystroke; skip the sweep when
    // there is nothing tagged to remove.
    if let Some(tag) = buffer.tag_table().lookup("zerkalo-spell") {
        let mut probe = s;
        if !probe.has_tag(&tag) && !probe.forward_to_tag_toggle(Some(&tag)) {
            return;
        }
    }
    buffer.remove_tag_by_name("zerkalo-spell", &s, &e);
}

// Remove the spell-error tag from every occurrence of `word` in the buffer.
// Uses forward_to_tag_toggle to skip directly between tagged ranges — O(k) in
// the number of misspelled-word ranges, not O(N) in buffer length.
pub(super) fn remove_spell_word_tags(buffer: &Buffer, tag: &gtk4::TextTag, word: &str) {
    let target = word.to_lowercase();
    let (mut it, e) = buffer.bounds();
    loop {
        if !it.has_tag(tag) {
            if !it.forward_to_tag_toggle(Some(tag)) {
                break;
            }
            if it >= e {
                break;
            }
        }
        let ws = it;
        let mut we = it;
        if !we.forward_to_tag_toggle(Some(tag)) {
            we = e;
        }
        let w = buffer.text(&ws, &we, false).to_string();
        if w.to_lowercase() == target {
            buffer.remove_tag(tag, &ws, &we);
        }
        it = we;
        if it >= e {
            break;
        }
    }
}

pub(super) fn apply_spell_tags(
    buffer: &Buffer,
    words: &[(usize, usize, String)],
    misspelled: &HashMap<String, Vec<String>>,
) {
    ensure_spell_tag(buffer);
    let (s, e) = buffer.bounds();
    buffer.remove_tag_by_name("zerkalo-spell", &s, &e);

    for (start, end, word) in words {
        if misspelled.contains_key(&word.to_lowercase()) {
            let iter_start = buffer.iter_at_offset(*start as i32);
            let iter_end = buffer.iter_at_offset(*end as i32);
            buffer.apply_tag_by_name("zerkalo-spell", &iter_start, &iter_end);
        }
    }
}

impl EditorPane {
    pub fn set_spell_enabled(&self, enabled: bool) {
        self.spell_checker.borrow_mut().enabled = enabled;
        if !enabled {
            // Clone buffers out of the borrow before GTK tag ops — remove_tag_by_name
            // can cascade through GtkSourceView signals and re-enter code that tries
            // a conflicting borrow on state, causing a BorrowError panic.
            let buffers: Vec<_> = {
                let state = self.state.borrow();
                state.tabs.values().map(|t| t.buffer.clone()).collect()
            };
            for buffer in &buffers {
                clear_spell_tags(buffer);
            }
        } else {
            self.recheck_all_buffers();
        }
    }

    pub fn set_spell_autocorrect(&self, enabled: bool) {
        self.spell_checker.borrow_mut().autocorrect = enabled;
        set_autocorrect_label(&self.autocorrect_label, enabled);
    }

    pub fn set_on_autocorrect_toggle(&self, f: impl Fn(bool) + 'static) {
        *self.on_autocorrect_toggle.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_spell_languages(&self, langs: Vec<String>) {
        self.spell_checker.borrow_mut().languages = langs;
        self.recheck_all_buffers();
    }

    pub(super) fn recheck_all_buffers(&self) {
        let sc = self.spell_checker.borrow();
        if !sc.enabled {
            return;
        }
        let languages = sc.languages.clone();
        let ignored = sc.ignored();
        drop(sc);

        let state = self.state.borrow();
        for tab in state.tabs.values() {
            let (s, e) = tab.buffer.bounds();
            let text = tab.buffer.text(&s, &e, true).to_string();
            let buffer = tab.buffer.clone();
            let langs = languages.clone();
            let ig = ignored.clone();

            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            std::thread::spawn(move || {
                let words = crate::spellcheck::extract_words(&text);
                let unique: Vec<String> = {
                    let mut seen = HashSet::new();
                    words
                        .iter()
                        .filter(|(_, _, w)| {
                            !ig.contains(&w.to_lowercase()) && seen.insert(w.to_lowercase())
                        })
                        .map(|(_, _, w)| w.clone())
                        .collect()
                };
                let unique_refs: Vec<&str> = unique.iter().map(|s| s.as_str()).collect();
                let misspelled = crate::spellcheck::check_words_batch(&unique_refs, &langs);
                let _ = tx.send((words, misspelled));
            });

            glib::timeout_add_local(Duration::from_millis(50), move || match rx.try_recv() {
                Ok((words, misspelled)) => {
                    apply_spell_tags(&buffer, &words, &misspelled);
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
            });
        }
    }

    pub(super) fn wire_spell_suggestions(
        &self,
        tab: &TabContext,
        hold_position: &Rc<Cell<Option<(f64, f64)>>>,
        hold_until: &Rc<Cell<Instant>>,
    ) {
        // ── Alt+Enter: open spell suggestions for word under cursor ─────────────
        {
            let spell_ae = self.spell_checker.clone();
            let buf_ae = tab.buffer.clone();
            let view_ae = tab.view.clone();
            let scroll_ae = tab.scroll.clone();
            let hold_pos_ae = hold_position.clone();
            let hold_until_ae = hold_until.clone();
            let ae_ctrl = EventControllerKey::new();
            ae_ctrl.connect_key_pressed(move |_, key, _, mods| {
                use gtk4::gdk::{Key, ModifierType};
                if key != Key::Return && key != Key::KP_Enter {
                    return glib::Propagation::Proceed;
                }
                if !mods.contains(ModifierType::ALT_MASK) {
                    return glib::Propagation::Proceed;
                }

                let sc = spell_ae.borrow();
                if !sc.enabled {
                    return glib::Propagation::Proceed;
                }

                let buf = &buf_ae;
                let pos = buf.cursor_position();
                let iter = buf.iter_at_offset(pos);
                let table = buf.tag_table();
                let Some(tag) = table.lookup("zerkalo-spell") else {
                    return glib::Propagation::Proceed;
                };
                if !iter.has_tag(&tag) {
                    return glib::Propagation::Proceed;
                }

                let mut word_start = iter;
                loop {
                    let mut prev = word_start;
                    if !prev.backward_char() {
                        break;
                    }
                    if !prev.char().is_alphabetic() {
                        break;
                    }
                    word_start = prev;
                }
                let mut word_end = iter;
                while word_end.char().is_alphabetic() {
                    if !word_end.forward_char() {
                        break;
                    }
                }
                let word = buf.text(&word_start, &word_end, false).to_string();
                if word.is_empty() {
                    return glib::Propagation::Proceed;
                }

                let already_ignored = sc.is_ignored(&word);
                let lang = sc.primary_language().to_string();
                drop(sc);

                // Position popover at cursor
                let (cx, cy) = {
                    let iter2 = buf.iter_at_offset(pos);
                    let rect = view_ae.iter_location(&iter2);
                    (rect.x() + rect.width() / 2, rect.y() + rect.height())
                };
                let popover = Popover::new();
                popover.set_parent(&view_ae);
                popover.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(cx, cy, 1, 1)));
                popover.set_has_arrow(true);
                popover.set_autohide(true);

                let vbox = GtkBox::new(Orientation::Vertical, 2);
                vbox.set_margin_top(6);
                vbox.set_margin_bottom(6);
                vbox.set_margin_start(4);
                vbox.set_margin_end(4);

                // Open on a placeholder and fill the list when hunspell replies.
                // Asking it inline blocked the main loop for the whole fork,
                // exec and wait before the menu could even appear.
                let pending = Label::new(Some("Checking\u{2026}"));
                pending.add_css_class("dim-label");
                pending.set_margin_top(4);
                pending.set_margin_bottom(4);
                vbox.append(&pending);

                let pop_close = popover.clone();
                popover.connect_closed(move |_| {
                    pop_close.unparent();
                });
                popover.set_child(Some(&vbox));
                popover.popup();
                popover.grab_focus();

                // Offsets, not TextIters: the reply lands after this handler
                // returns, and any edit in between invalidates an iterator.
                let ws_off = word_start.offset();
                let we_off = word_end.offset();

                let rx = Self::spawn_spelling_suggestions(&word, lang, already_ignored);
                let vbox_fill = vbox.clone();
                let pending_fill = pending.clone();
                let popover_fill = popover.clone();
                let buf_fill = buf_ae.clone();
                let scroll_fill = scroll_ae.clone();
                let hold_pos_fill = hold_pos_ae.clone();
                let hold_until_fill = hold_until_ae.clone();
                let word_fill = word.clone();
                glib::timeout_add_local(Duration::from_millis(30), move || {
                    let suggestions = match rx.try_recv() {
                        Ok(s) => s,
                        Err(std::sync::mpsc::TryRecvError::Empty) => {
                            // Nothing to fill if the user already dismissed it.
                            if !popover_fill.is_visible() {
                                return glib::ControlFlow::Break;
                            }
                            return glib::ControlFlow::Continue;
                        }
                        Err(_) => return glib::ControlFlow::Break,
                    };
                    if !popover_fill.is_visible() {
                        return glib::ControlFlow::Break;
                    }
                    vbox_fill.remove(&pending_fill);

                    if suggestions.is_empty() {
                        let lbl = Label::new(Some("No suggestions"));
                        lbl.add_css_class("dim-label");
                        lbl.set_margin_top(4);
                        lbl.set_margin_bottom(4);
                        vbox_fill.append(&lbl);
                    } else {
                        for sugg in suggestions.iter().take(6) {
                            let btn = Button::with_label(sugg);
                            btn.add_css_class("flat");
                            let buf2 = buf_fill.clone();
                            let s = sugg.clone();
                            let pop2 = popover_fill.clone();
                            let scroll_sg = scroll_fill.clone();
                            let hold_p = hold_pos_fill.clone();
                            let hold_u = hold_until_fill.clone();
                            let expected = word_fill.clone();
                            btn.connect_clicked(move |_| {
                                let vpos = scroll_sg.vadjustment().value();
                                let hpos = scroll_sg.hadjustment().value();
                                hold_p.set(Some((vpos, hpos)));
                                hold_u.set(Instant::now() + PASTE_HOLD);

                                let mut a = buf2.iter_at_offset(ws_off);
                                let mut b = buf2.iter_at_offset(we_off);
                                // The tab.buffer may have changed while the menu was
                                // open; only replace if the word is still there.
                                if buf2.text(&a, &b, false) == expected.as_str() {
                                    buf2.begin_user_action();
                                    buf2.delete(&mut a, &mut b);
                                    buf2.insert(&mut a, &s);
                                    buf2.end_user_action();
                                }
                                pop2.popdown();

                                let release = hold_p.clone();
                                glib::timeout_add_local_once(PASTE_HOLD, move || release.set(None));
                            });
                            vbox_fill.append(&btn);
                        }
                    }
                    glib::ControlFlow::Break
                });
                glib::Propagation::Stop
            });
            tab.view.add_controller(ae_ctrl);
        }
    }

    /// Looks up hunspell suggestions for `word` on a background thread — the
    /// fork/exec/wait is too slow to run inline on the GTK main thread,
    /// whether triggered from a click (spell popover) or a keystroke
    /// (autocorrect). `already_ignored` short-circuits to no suggestions
    /// without spawning a thread at all, matching what the two popover call
    /// sites already did before this was pulled out.
    ///
    /// Only the lookup itself is shared — each caller's poll loop still
    /// differs (whether it bails when a popover closes, how it applies a
    /// result), so this returns the `Rc`-wrapped receiver for the caller to
    /// poll on its own `glib::timeout_add_local`, rather than taking a
    /// callback.
    pub(super) fn spawn_spelling_suggestions(
        word: &str,
        lang: String,
        already_ignored: bool,
    ) -> Rc<std::sync::mpsc::Receiver<Vec<String>>> {
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<String>>(1);
        let word_bg = word.to_string();
        std::thread::spawn(move || {
            let out = if already_ignored {
                Vec::new()
            } else {
                crate::spellcheck::suggestions_for_word(&word_bg, &lang)
            };
            tx.send(out).ok();
        });
        Rc::new(rx)
    }

    pub(super) fn wire_spellcheck(&self, tab: &TabContext) {
        // ── Spell check: debounced tab.buffer check ───────────────────────────────

        {
            let spell_c = self.spell_checker.clone();
            let spell_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
            let spell_poll_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
            let buf_spell = tab.buffer.clone();

            tab.buffer.connect_changed(move |buf| {
                let enabled = {
                    let sc = spell_c.borrow();
                    sc.enabled
                };
                if !enabled {
                    // Release spell_checker borrow before GTK tag ops — remove_tag_by_name
                    // can cascade through GtkSourceView signals and re-enter this closure
                    // (or another that borrows spell_checker), causing a BorrowError panic.
                    clear_spell_tags(&buf_spell);
                    return;
                }

                // Cancel any pending debounce and in-flight poll timer.
                if let Some(id) = spell_timer.borrow_mut().take() {
                    id.remove();
                }
                if let Some(id) = spell_poll_timer.borrow_mut().take() {
                    id.remove();
                }

                let buf2 = buf.clone();
                let sc2 = spell_c.clone();
                let t = spell_timer.clone();
                let pt = spell_poll_timer.clone();
                let pt2 = spell_poll_timer.clone();

                *spell_timer.borrow_mut() = Some(glib::timeout_add_local_once(
                    Duration::from_millis(700),
                    move || {
                        *t.borrow_mut() = None;

                        let sc = sc2.borrow();
                        if !sc.enabled {
                            clear_spell_tags(&buf2);
                            return;
                        }
                        let langs = sc.languages.clone();
                        let ignored = sc.ignored();
                        drop(sc);

                        let (s, e) = buf2.bounds();
                        let text = buf2.text(&s, &e, true).to_string();
                        let buf3 = buf2.clone();

                        let (tx, rx) = std::sync::mpsc::sync_channel(1);
                        std::thread::spawn(move || {
                            let words = crate::spellcheck::extract_words(&text);
                            let unique: Vec<String> = {
                                let mut seen = HashSet::new();
                                words
                                    .iter()
                                    .filter(|(_, _, w)| {
                                        !ignored.contains(&w.to_lowercase())
                                            && seen.insert(w.to_lowercase())
                                    })
                                    .map(|(_, _, w)| w.clone())
                                    .collect()
                            };
                            let unique_refs: Vec<&str> =
                                unique.iter().map(|s| s.as_str()).collect();
                            let misspelled =
                                crate::spellcheck::check_words_batch(&unique_refs, &langs);
                            let _ = tx.send((words, misspelled));
                        });

                        let poll_id =
                            glib::timeout_add_local(Duration::from_millis(50), move || {
                                match rx.try_recv() {
                                    Ok((words, misspelled)) => {
                                        // Clear the RefCell before returning Break. GLib auto-removes
                                        // the source after the callback, but the RefCell still holds
                                        // the now-dead SourceId. A subsequent connect_changed would call
                                        // id.remove() on it and panic with "Failed to remove source".
                                        *pt2.borrow_mut() = None;
                                        apply_spell_tags(&buf3, &words, &misspelled);
                                        glib::ControlFlow::Break
                                    }
                                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                                        glib::ControlFlow::Continue
                                    }
                                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                        *pt2.borrow_mut() = None;
                                        glib::ControlFlow::Break
                                    }
                                }
                            });
                        *pt.borrow_mut() = Some(poll_id);
                    },
                ));
            });
        }
    }

    pub(super) fn wire_autocorrect(&self, tab: &TabContext) {
        // ── Spell check: autocorrect on word boundary ─────────────────────────

        {
            let spell_ac = self.spell_checker.clone();
            let buf_ac = tab.buffer.clone();

            tab.buffer.connect_changed(move |buf| {
                let sc = spell_ac.borrow();
                if !sc.enabled || !sc.autocorrect {
                    return;
                }

                let cursor = buf.cursor_position();
                if cursor < 2 {
                    return;
                }

                let just_typed = buf.iter_at_offset(cursor - 1);
                let ch = just_typed.char();
                // Only autocorrect when a word-terminating character is typed
                if !matches!(ch, ' ' | '\t' | '\n' | '.' | ',' | ';' | ':' | '!' | '?') {
                    return;
                }

                // Scan backward to find the preceding word
                let word_end = buf.iter_at_offset(cursor - 1);
                let mut word_start = word_end;
                loop {
                    let mut prev = word_start;
                    if !prev.backward_char() {
                        break;
                    }
                    if !prev.char().is_alphabetic() {
                        break;
                    }
                    word_start = prev;
                }
                if word_start == word_end {
                    return;
                }

                let word = buf.text(&word_start, &word_end, false).to_string();
                if word.len() < 3 || sc.is_ignored(&word) {
                    return;
                }

                // Don't autocorrect proper nouns or words already starting with upper
                if word
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
                {
                    return;
                }

                let lang = sc.primary_language().to_string();
                drop(sc);

                // Ask hunspell on a worker thread. This runs from
                // `connect_changed`, i.e. inside a keystroke: doing the
                // fork/exec/wait inline stalled the main loop on every space,
                // period, comma, semicolon, colon, `!` and `?` the user typed.
                //
                // Char offsets rather than TextIter: iterators are invalidated
                // by any later tab.buffer edit, and now the reply arrives well
                // after this handler has returned. The word at those offsets is
                // re-validated before anything is replaced, so keystrokes in
                // the meantime are safely ignored.
                let ws_off = word_start.offset();
                let we_off = word_end.offset();
                let buf_c = buf_ac.clone();
                let rx = Self::spawn_spelling_suggestions(&word, lang, false);

                let word_c = word.clone();
                glib::timeout_add_local(Duration::from_millis(30), move || {
                    let suggestions = match rx.try_recv() {
                        Ok(s) => s,
                        Err(std::sync::mpsc::TryRecvError::Empty) => {
                            return glib::ControlFlow::Continue
                        }
                        Err(_) => return glib::ControlFlow::Break,
                    };
                    // Only apply if edit distance is 1 (very confident replacement)
                    if let Some(best) = suggestions.first() {
                        if crate::spellcheck::levenshtein(
                            &word_c.to_lowercase(),
                            &best.to_lowercase(),
                        ) <= 1
                        {
                            let mut s = buf_c.iter_at_offset(ws_off);
                            let mut e = buf_c.iter_at_offset(we_off);
                            if buf_c.text(&s, &e, false) == word_c.as_str() {
                                buf_c.begin_user_action();
                                buf_c.delete(&mut s, &mut e);
                                buf_c.insert(&mut s, best);
                                buf_c.end_user_action();
                            }
                        }
                    }
                    glib::ControlFlow::Break
                });
            });
        }
    }
}
