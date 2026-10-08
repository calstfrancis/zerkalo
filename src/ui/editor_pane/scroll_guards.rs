//! The click, focus and clipboard handling around the editor view: dismissing
//! suggestions when the text is clicked, and the viewport-holding workarounds
//! for GTK's scroll-to-cursor behaviour (see `ZERKALO_TRACE_SCROLL`).

use super::*;

/// What the guards share with the rest of the tab's wiring.
pub(super) struct ScrollGuardInputs {
    pub(super) lsp_popup: LspPopup,
    pub(super) bib_popup: BibPopup,
    pub(super) ghost_label: Label,
    pub(super) ghost_item: Rc<RefCell<Option<CompletionItem>>>,
    pub(super) ghost_bib_entry: Rc<RefCell<Option<PopupEntry>>>,
    pub(super) active_error_popup: Rc<RefCell<Option<Popover>>>,
    pub(super) hold_position: Rc<Cell<Option<(f64, f64)>>>,
    pub(super) hold_until: Rc<Cell<Instant>>,
    pub(super) saved_scroll: Rc<Cell<f64>>,
    pub(super) saved_hscroll: Rc<Cell<f64>>,
}

impl EditorPane {
    pub(super) fn wire_scroll_guards(
        &self,
        view: &View,
        buffer: &Buffer,
        scroll: &ScrolledWindow,
        inputs: &ScrollGuardInputs,
        pause_tracking: impl Fn() + Clone + 'static,
    ) {
        let view = view.clone();
        let buffer = buffer.clone();
        let scroll = scroll.clone();
        let ScrollGuardInputs {
            lsp_popup,
            bib_popup,
            ghost_label,
            ghost_item,
            ghost_bib_entry,
            active_error_popup,
            hold_position,
            hold_until,
            saved_scroll,
            saved_hscroll,
        } = inputs;
        // Suppress GTK's built-in focus-in cursor snap.
        // GtkTextView calls scroll_mark_onscreen(insert) when it gains keyboard
        // focus, which can violently snap the viewport to the cursor's OLD position
        // when the user has scrolled elsewhere. We save the scroll position just
        // before each click (GestureClick::pressed fires before GtkTextView's own
        // button-press handler, which is what triggers focus-in and the snap) and
        // restore it in idle after GTK's focus-in handler runs.
        // saved_scroll is shared with the right-click gesture above — see comment there.
        {
            // saved_scroll/saved_hscroll follow every scroll (see the adjustment
            // handlers above), so nothing here needs to sample the position.

            // On left-click, suppress the focus-snap only when the view is
            // actually gaining focus. If it already has focus the click is
            // intentional navigation and should scroll to the cursor normally.
            // button=1 only — button=0 would steal the right-click spell gesture's sequence.
            let any_click = GestureClick::new();
            any_click.set_button(1);
            {
                let sc = scroll.clone();
                let pause = pause_tracking.clone();
                let view_fc = view.clone();
                let lsp_popup_click = lsp_popup.clone();
                let bib_popup_click = bib_popup.clone();
                let ghost_click = ghost_label.clone();
                let ghost_item_click = ghost_item.clone();
                let ghost_bib_click = ghost_bib_entry.clone();
                let hint_click = self.lsp_status_label.clone();
                let active_error_popup_click = active_error_popup.clone();
                let hold_pos_click = hold_position.clone();
                let hold_until_click = hold_until.clone();
                any_click.connect_pressed(move |_, _, _, _| {
                    // Clicking anywhere in the text dismisses a suggestion —
                    // the popovers are autohide(false) (they must not steal the
                    // keyboard while you type), so they'd otherwise sit there.
                    lsp_popup_click.hide();
                    bib_popup_click.hide();
                    // See the matching comment on the hover handler above for why this
                    // can't be `if let Some(p) = ...borrow_mut().take() { p.popdown(); }`
                    // directly — same double-borrow panic, same fix.
                    let taken = active_error_popup_click.borrow_mut().take();
                    if let Some(p) = taken {
                        p.popdown();
                    }
                    clear_citation_ghost(&ghost_click, &ghost_bib_click, &hint_click);
                    clear_ghost(&ghost_click, &ghost_item_click, &hint_click);
                    if !view_fc.has_focus() {
                        // View is gaining focus → GTK will snap to insert mark. A
                        // single "capture now, restore once at the next zero-delay
                        // timeout" used to live here, but that only works if GTK's
                        // snap is strictly slower than our restore — and dismissing
                        // a popover/menu by clicking elsewhere in the view (rather
                        // than e.g. pressing Escape) fires this same focus-gain
                        // path, where the snap can already be synchronously done by
                        // the time this handler runs. Use the same continuous-hold
                        // mechanism as paste and spell-suggestion-accept instead:
                        // reassert the captured position on every adjustment change
                        // for a short window, which is correct regardless of when
                        // GTK's own snap actually lands.
                        let val = sc.vadjustment().value();
                        let hval = sc.hadjustment().value();
                        pause();
                        hold_pos_click.set(Some((val, hval)));
                        hold_until_click.set(Instant::now() + PASTE_HOLD);
                        let release = hold_pos_click.clone();
                        glib::timeout_add_local_once(PASTE_HOLD, move || release.set(None));
                    }
                });
            }
            view.add_controller(any_click);

            let focus_ctrl = EventControllerFocus::new();
            {
                // Pause tracking as focus leaves too: the snap can fire on the
                // way out (when a context menu takes focus), and recording it
                // would make the restore on the way back in aim at it. Focus
                // leaving the editor at all — a click in the sidebar, the
                // preview, another window — also means any suggestion on screen
                // is stale, so drop it.
                let pause = pause_tracking.clone();
                let lsp_popup_focus = lsp_popup.clone();
                let bib_popup_focus = bib_popup.clone();
                let ghost_focus = ghost_label.clone();
                let ghost_item_focus = ghost_item.clone();
                let ghost_bib_focus = ghost_bib_entry.clone();
                let hint_focus = self.lsp_status_label.clone();
                let active_error_popup_focus = active_error_popup.clone();
                focus_ctrl.connect_leave(move |_| {
                    pause();
                    lsp_popup_focus.hide();
                    bib_popup_focus.hide();
                    // Same double-borrow hazard and fix as the hover handler above.
                    let taken = active_error_popup_focus.borrow_mut().take();
                    if let Some(p) = taken {
                        p.popdown();
                    }
                    clear_citation_ghost(&ghost_focus, &ghost_bib_focus, &hint_focus);
                    clear_ghost(&ghost_focus, &ghost_item_focus, &hint_focus);
                });
            }
            {
                let sc_enter = scroll.clone();
                let sv_enter = saved_scroll.clone();
                let sh_enter = saved_hscroll.clone();
                let pause = pause_tracking.clone();
                let jumping = self.jumping.clone();
                focus_ctrl.connect_enter(move |_| {
                    if jumping.get() {
                        return;
                    }
                    // Use the tracked position rather than the current scroll. GTK can
                    // snap the view to the cursor synchronously before this signal fires
                    // (e.g. on context-menu dismiss), so reading the adjustment here
                    // would restore the snapped position rather than where the user was.
                    let val = sv_enter.get();
                    let hval = sh_enter.get();
                    if val < 0.0 {
                        return;
                    }
                    let sc = sc_enter.clone();
                    pause();
                    glib::timeout_add_local_once(Duration::ZERO, move || {
                        sc.vadjustment().set_value(val);
                        sc.hadjustment().set_value(hval);
                    });
                });
            }
            view.add_controller(focus_ctrl);
        }

        // Copying must never move the viewport. GtkTextView scrolls to the
        // insert mark after a clipboard action, and taking Copy from the
        // right-click menu adds a focus round-trip that can snap it as well —
        // together they threw the view to wherever the cursor happened to be
        // (usually the top of the file) on a plain copy. Pin the position
        // across both, for cut too, since a cut happens where the user already
        // is. Paste is deliberately left alone: scrolling to the insertion
        // point is the correct thing there, since that's where the text landed.
        {
            let pin = {
                let scroll = scroll.clone();
                let pause = pause_tracking.clone();
                move || {
                    let val = scroll.vadjustment().value();
                    let hval = scroll.hadjustment().value();
                    let sc = scroll.clone();
                    pause();
                    glib::timeout_add_local_once(Duration::ZERO, move || {
                        sc.vadjustment().set_value(val);
                        sc.hadjustment().set_value(hval);
                    });
                }
            };
            let pin_cut = pin.clone();
            view.connect_copy_clipboard(move |_| pin());
            view.connect_cut_clipboard(move |_| pin_cut());

            // Paste inserts at the cursor, so the right viewport is the one the
            // user is already looking at. Hold it while GTK's animation plays
            // out — unless the paste landed off-screen, where following it is
            // the correct behaviour.
            {
                let scroll = scroll.clone();
                let view_p = view.clone();
                let _buf_p = buffer.clone();
                let held = hold_position.clone();
                let held_until = hold_until.clone();
                let pause = pause_tracking.clone();
                buffer.connect_paste_done(move |buf, _| {
                    let cursor = buf.iter_at_offset(buf.cursor_position());
                    let loc = view_p.iter_location(&cursor);
                    let (_, wy) =
                        view_p.buffer_to_window_coords(TextWindowType::Widget, loc.x(), loc.y());
                    let on_screen = wy >= 0 && wy <= view_p.allocated_height();
                    if !on_screen {
                        return;
                    }
                    held.set(Some((
                        scroll.vadjustment().value(),
                        scroll.hadjustment().value(),
                    )));
                    held_until.set(Instant::now() + PASTE_HOLD);
                    pause();
                    // Release the hold once the animation is spent, so ordinary
                    // scrolling works again immediately afterwards.
                    let held_release = held.clone();
                    glib::timeout_add_local_once(PASTE_HOLD, move || held_release.set(None));
                });
            }
        }
    }
}
