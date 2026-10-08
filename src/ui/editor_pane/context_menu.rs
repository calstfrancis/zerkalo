//! The right-click handling, spell suggestions on a misspelt word, and the viewport hold used after applying one.

use super::*;

impl EditorPane {
    pub(super) fn wire_right_click_menu(
        &self,
        view: &View,
        buffer: &Buffer,
        scroll: &ScrolledWindow,
        hold_position: &Rc<Cell<Option<(f64, f64)>>>,
        hold_until: &Rc<Cell<Instant>>,
    ) -> (Rc<Cell<f64>>, Rc<Cell<f64>>, impl Fn() + Clone + 'static) {
        //
        // saved_scroll is defined here (not in the focus-snap block below) so
        // the right-click gesture can also update it. If we don't, the sequence:
        //   right-click → GTK snaps scroll → focus_leave saves snapped value
        //   → idle restores real value → dismiss popover → focus_enter restores
        //   wrong (snapped) value → visible jump.
        let saved_scroll: Rc<Cell<f64>> = Rc::new(Cell::new(-1.0));
        let saved_hscroll: Rc<Cell<f64>> = Rc::new(Cell::new(-1.0));

        // Track every scroll, rather than sampling the position on the handful
        // of events (pointer enter/leave, click, focus leave) that used to be
        // the only writers. Anything that scrolled without one of those firing
        // — a wheel scroll with the pointer already inside, Page Down, a jump
        // from the outline — left the saved value stale, usually still at the
        // top of the file where the pointer first entered. The next focus-enter
        // then "restored" that, which is why copying or pasting (both of which
        // hand focus to the clipboard manager and back) threw the view to the
        // top of the document.
        //
        // GTK's focus-snap must not be recorded as the user's position, so
        // tracking pauses around the events that provoke one (a click into the
        // view, a right-click, focus arriving or leaving). The pause is a short
        // deadline rather than a flag cleared on the next tick because the snap
        // doesn't reliably land within one: taking Copy from the right-click
        // menu snapped the view *after* the restore that was meant to undo it,
        // and the snapped position — the cursor, typically still at the top of
        // the file — became the position every later restore aimed at.
        let track_paused_until: Rc<Cell<Instant>> = Rc::new(Cell::new(Instant::now()));
        // Pasting makes GTK animate the viewport to the top of the buffer — an
        // eased curve over a dozen frames, ending at 0, with focus never leaving
        // the editor and the cursor still mid-document. Nothing in Zerkalo asks
        // for it and there's no signal to decline it, so instead the position is
        // *held*: for a moment after a paste, every frame of that animation is
        // put straight back. Snapping back once at the end would be visible;
        // countering each frame means nothing moves at all.
        let pause_tracking = {
            let until = track_paused_until.clone();
            move || until.set(Instant::now() + Duration::from_millis(150))
        };
        // Whether a scroll that just happened is the user's position and not
        // GTK's snap. The snap only happens to a focused view, so anything
        // while focus is elsewhere in the window (the sidebar, the search box,
        // the error panel's Jump button) is real: a wheel scroll there, or a
        // jump from the outline or error panel. Recording only while focused
        // left those out, and the next time focus came back (a right-click, a
        // return from another window) the view was thrown back to the last
        // position it had while focused, often the top of the file. Skipped:
        // the pause windows around clicks and focus changes, and any time one
        // of the view's own menus or popovers has focus, since that is when GTK
        // can snap and a menu can stay open indefinitely. A wheel or touchpad
        // scroll is always the user's, whatever has focus.
        let user_scroll_until: Rc<Cell<Instant>> = Rc::new(Cell::new(Instant::now()));
        {
            let wheel = gtk4::EventControllerScroll::new(
                gtk4::EventControllerScrollFlags::BOTH_AXES
                    | gtk4::EventControllerScrollFlags::KINETIC,
            );
            wheel.set_propagation_phase(PropagationPhase::Capture);
            let mark = user_scroll_until.clone();
            wheel.connect_scroll(move |_, _, _| {
                mark.set(Instant::now() + Duration::from_millis(300));
                glib::Propagation::Proceed
            });
            let mark = user_scroll_until.clone();
            wheel.connect_decelerate(move |_, _, _| {
                mark.set(Instant::now() + Duration::from_millis(2000));
            });
            scroll.add_controller(wheel);
        }
        let is_users_scroll = {
            let view = view.clone();
            let until = track_paused_until.clone();
            let user_until = user_scroll_until.clone();
            move || {
                // A tab that isn't showing isn't being scrolled by anyone;
                // whatever its adjustment does while hidden is layout.
                if !view.is_mapped() {
                    return false;
                }
                let now = Instant::now();
                if now < user_until.get() {
                    return true;
                }
                if now < until.get() {
                    return false;
                }
                let focus = view.root().and_then(|r| r.focus());
                match focus {
                    Some(w) if w == *view.upcast_ref::<gtk4::Widget>() => view.has_focus(),
                    Some(w) => !w.is_ancestor(&view),
                    None => true,
                }
            }
        };
        {
            let sv = saved_scroll.clone();
            let wheel_until = user_scroll_until.clone();
            let view_v = view.clone();
            let held = hold_position.clone();
            let held_until = hold_until.clone();
            let reasserting: Rc<Cell<bool>> = Rc::new(Cell::new(false));
            let record_v = is_users_scroll.clone();
            scroll.vadjustment().connect_value_changed(move |adj| {
                if scroll_trace_on() {
                    eprintln!(
                        "SCROLLTRACE v={:.0} focused={} held={}",
                        adj.value(),
                        view_v.has_focus(),
                        held.get().is_some()
                    );
                }
                if let Some((v, _)) = held.get() {
                    if Instant::now() < held_until.get() {
                        // Re-assert, guarding against our own recursion.
                        if !reasserting.get() && (adj.value() - v).abs() > 0.5 {
                            reasserting.set(true);
                            adj.set_value(v);
                            reasserting.set(false);
                        }
                        return;
                    }
                    held.set(None);
                }
                if record_v() {
                    // Not the very top, unless the editor itself has focus.
                    // Reflowing a hidden or resizing view clamps its scroll to
                    // zero, and a zero recorded as "where the writer was" is
                    // what a later focus-enter restore threw them back to: line
                    // 1 of a document they were halfway down. Actually going to
                    // the top (Ctrl+Home, the wheel) always has either focus or
                    // a wheel event behind it.
                    let wheel = Instant::now() < wheel_until.get();
                    if adj.value() > adj.lower() || wheel || view_v.has_focus() {
                        sv.set(adj.value());
                    }
                }
            });
            let sh = saved_hscroll.clone();
            let held_h = hold_position.clone();
            let held_until_h = hold_until.clone();
            let reasserting_h: Rc<Cell<bool>> = Rc::new(Cell::new(false));
            let record_h = is_users_scroll.clone();
            scroll.hadjustment().connect_value_changed(move |adj| {
                if let Some((_, h)) = held_h.get() {
                    if Instant::now() < held_until_h.get() {
                        if !reasserting_h.get() && (adj.value() - h).abs() > 0.5 {
                            reasserting_h.set(true);
                            adj.set_value(h);
                            reasserting_h.set(false);
                        }
                        return;
                    }
                }
                if record_h() {
                    sh.set(adj.value());
                }
            });
        }

        {
            let spell_rc = self.spell_checker.clone();
            let buf_rc = buffer.clone();
            let view_rc = view.clone();
            let hold_pos_spell = hold_position.clone();
            let hold_until_spell = hold_until.clone();
            let scroll_rc = scroll.clone();
            let pause_rc = pause_tracking.clone();

            // Use connect_pressed, not connect_released. GtkSourceView processes
            // button-3 internally and may grab the pointer before the release
            // event reaches our gesture, so connect_released is unreliable.
            // connect_pressed fires before any widget-level handling.
            let gesture = GestureClick::new();
            gesture.set_button(3); // right button
                                   // Capture phase + claiming the sequence below: GtkTextView has its
                                   // own right-click handler that opens the standard context menu, and
                                   // it was opening *on top of* the spell suggestions, hiding the thing
                                   // the right-click was for. Claiming stops the view ever seeing it.
            gesture.set_propagation_phase(PropagationPhase::Capture);

            gesture.connect_pressed(move |gesture, _, x, y| {
                // Suppress the focus-snap that right-click can trigger even
                // when the view already has focus.
                let scroll_val = scroll_rc.vadjustment().value();
                let hscroll_val = scroll_rc.hadjustment().value();
                {
                    let sc = scroll_rc.clone();
                    pause_rc();
                    glib::timeout_add_local_once(Duration::ZERO, move || {
                        sc.vadjustment().set_value(scroll_val);
                        sc.hadjustment().set_value(hscroll_val);
                    });
                }

                // Move cursor to the right-click position (unless it's inside
                // the current selection). This makes GTK's focus-in scroll-to-mark
                // target a position already in the viewport, so the snap is a no-op.
                let (bx, by) =
                    view_rc.window_to_buffer_coords(TextWindowType::Widget, x as i32, y as i32);
                if let Some(iter) = view_rc.iter_at_location(bx, by) {
                    let ofs = iter.offset();
                    let inside_sel = buf_rc
                        .selection_bounds()
                        .map(|(s, e)| ofs >= s.offset() && ofs <= e.offset())
                        .unwrap_or(false);
                    if !inside_sel {
                        buf_rc.place_cursor(&iter);
                    }
                }

                let sc = spell_rc.borrow();
                if !sc.enabled {
                    return;
                }

                let (bx, by) =
                    view_rc.window_to_buffer_coords(TextWindowType::Widget, x as i32, y as i32);
                let Some(iter) = view_rc.iter_at_location(bx, by) else {
                    return;
                };

                let table = buf_rc.tag_table();
                let Some(tag) = table.lookup("zerkalo-spell") else {
                    return;
                };
                if !iter.has_tag(&tag) {
                    return;
                }

                // Find word boundaries
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
                let word = buf_rc.text(&word_start, &word_end, false).to_string();
                if word.is_empty() {
                    return;
                }

                let already_ignored = sc.is_ignored(&word);
                let lang = sc.primary_language().to_string();
                drop(sc);
                // From here a spell popover is definitely going up, so take the
                // click: no built-in menu, no two menus stacked.
                gesture.set_state(gtk4::EventSequenceState::Claimed);

                let popover = Popover::new();
                popover.set_parent(&view_rc);
                let rect = gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
                popover.set_pointing_to(Some(&rect));
                popover.set_has_arrow(true);

                let vbox = GtkBox::new(Orientation::Vertical, 2);
                vbox.set_margin_top(6);
                vbox.set_margin_bottom(6);
                vbox.set_margin_start(4);
                vbox.set_margin_end(4);

                // Suggestions live in their own box so they can be filled in
                // once hunspell answers, without disturbing the fixed actions
                // below. Asking it inline delayed the menu appearing by the
                // whole fork/exec/wait, on the main loop.
                let sugg_box = GtkBox::new(Orientation::Vertical, 2);
                let pending = Label::new(Some("Checking\u{2026}"));
                pending.add_css_class("dim-label");
                pending.set_margin_top(4);
                pending.set_margin_bottom(4);
                sugg_box.append(&pending);
                vbox.append(&sugg_box);

                // Offsets, not TextIters: the reply arrives after this handler
                // returns, and any edit in between invalidates an iterator.
                let ws_off = word_start.offset();
                let we_off = word_end.offset();

                // Tracks "the user dismissed this popover," set in
                // connect_closed below. `popover.is_visible()` used to serve
                // this purpose, but that's also false during the window
                // between this handler returning and the delayed popup()
                // actually showing it (see the popup() delay's own comment,
                // further down) — a poll tick landing in that window used to
                // read "not visible" as "already dismissed" and cancel itself
                // permanently, before the popover had even appeared, so
                // suggestions that arrived after that point were silently
                // dropped and "Checking…" never updated.
                let dismissed: Rc<Cell<bool>> = Rc::new(Cell::new(false));

                let rx = Self::spawn_spelling_suggestions(&word, lang, already_ignored);
                let sugg_box_fill = sugg_box.clone();
                let pending_fill = pending.clone();
                let popover_fill = popover.clone();
                let dismissed_fill = dismissed.clone();
                let buf_fill = buf_rc.clone();
                let scroll_fill = scroll_rc.clone();
                let hold_pos_fill = hold_pos_spell.clone();
                let hold_until_fill = hold_until_spell.clone();
                let word_fill = word.clone();
                glib::timeout_add_local(Duration::from_millis(30), move || {
                    let suggestions = match rx.try_recv() {
                        Ok(s) => s,
                        Err(std::sync::mpsc::TryRecvError::Empty) => {
                            if dismissed_fill.get() {
                                return glib::ControlFlow::Break;
                            }
                            return glib::ControlFlow::Continue;
                        }
                        Err(_) => return glib::ControlFlow::Break,
                    };
                    if dismissed_fill.get() {
                        return glib::ControlFlow::Break;
                    }
                    sugg_box_fill.remove(&pending_fill);

                    if suggestions.is_empty() {
                        let lbl = Label::new(Some("No suggestions"));
                        lbl.add_css_class("dim-label");
                        lbl.set_margin_top(4);
                        lbl.set_margin_bottom(4);
                        sugg_box_fill.append(&lbl);
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
                                // Popping the popover down hands focus back to the view,
                                // and GTK answers with the same scroll-to-mark animation
                                // that follows a paste. Hold the viewport through it.
                                let vpos = scroll_sg.vadjustment().value();
                                let hpos = scroll_sg.hadjustment().value();
                                hold_p.set(Some((vpos, hpos)));
                                hold_u.set(Instant::now() + PASTE_HOLD);

                                let mut a = buf2.iter_at_offset(ws_off);
                                let mut b = buf2.iter_at_offset(we_off);
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
                            sugg_box_fill.append(&btn);
                        }
                    }
                    glib::ControlFlow::Break
                });

                vbox.append(&Separator::new(Orientation::Horizontal));

                let ignore_btn = Button::with_label("Ignore All");
                ignore_btn.add_css_class("flat");
                let spell_ign = spell_rc.clone();
                let buf_ign = buf_rc.clone();
                let word_ign = word.clone();
                let pop_ign = popover.clone();
                ignore_btn.connect_clicked(move |_| {
                    spell_ign.borrow_mut().ignore(&word_ign);
                    let tag_table = buf_ign.tag_table();
                    if let Some(t) = tag_table.lookup("zerkalo-spell") {
                        remove_spell_word_tags(&buf_ign, &t, &word_ign);
                    }
                    pop_ign.popdown();
                });
                vbox.append(&ignore_btn);

                let add_dict_btn = Button::with_label("Add to Dictionary");
                add_dict_btn.add_css_class("flat");
                let spell_dict = spell_rc.clone();
                let buf_dict = buf_rc.clone();
                let word_dict = word.clone();
                let pop_dict = popover.clone();
                add_dict_btn.connect_clicked(move |_| {
                    spell_dict.borrow_mut().add_to_user_dict(&word_dict);
                    let tag_table = buf_dict.tag_table();
                    if let Some(t) = tag_table.lookup("zerkalo-spell") {
                        remove_spell_word_tags(&buf_dict, &t, &word_dict);
                    }
                    pop_dict.popdown();
                });
                vbox.append(&add_dict_btn);

                if spell_rc.borrow().has_project_dict() {
                    let add_proj_btn = Button::with_label("Add to Project Dictionary");
                    add_proj_btn.add_css_class("flat");
                    let spell_proj = spell_rc.clone();
                    let buf_proj = buf_rc.clone();
                    let word_proj = word.clone();
                    let pop_proj = popover.clone();
                    add_proj_btn.connect_clicked(move |_| {
                        spell_proj.borrow_mut().add_to_project_dict(&word_proj);
                        let tag_table = buf_proj.tag_table();
                        if let Some(t) = tag_table.lookup("zerkalo-spell") {
                            remove_spell_word_tags(&buf_proj, &t, &word_proj);
                        }
                        pop_proj.popdown();
                    });
                    vbox.append(&add_proj_btn);
                }

                popover.set_child(Some(&vbox));

                let pop_close = popover.clone();
                let dismissed_close = dismissed.clone();
                popover.connect_closed(move |_| {
                    dismissed_close.set(true);
                    pop_close.unparent();
                    // Do NOT restore scroll here. The idle in popup() already
                    // anchored the view. Restoring on close fights with whatever
                    // the user clicked to dismiss the popover.
                });

                // Deferred by a short real delay, not called here directly:
                // this whole handler runs on the right-click's button-*press*,
                // with the button still down (connect_pressed, not
                // connect_released — see the comment on this gesture's
                // creation for why). Calling popup() synchronously starts the
                // popover's autohide grab immediately, and the paired
                // button-*release* — which hasn't happened yet and lands back
                // on the editor, outside the popover's own surface — then
                // reads to that grab as an outside click and closes the
                // popover right away, before the suggestions even finish
                // rendering.
                //
                // This used to defer via `glib::idle_add_local_once` instead
                // (zero-delay, next main-loop iteration) on the reasoning that
                // it would run after the release finished dispatching — that
                // held on X11 but not reliably on Wayland, where a fast
                // right-click's press and release can arrive and both get
                // processed within the same main-loop iteration, before any
                // idle source runs at all, so the deferral won zero time
                // against the race it was meant to avoid. A real (not just
                // idle-priority) delay can't lose that race regardless of how
                // the backend batches input events, and 40ms is well under the
                // ~100ms "feels instant" threshold.
                let popover_open = popover.clone();
                glib::timeout_add_local_once(Duration::from_millis(40), move || {
                    popover_open.popup()
                });
            });
            view.add_controller(gesture);
        }

        (saved_scroll, saved_hscroll, pause_tracking)
    }
}
