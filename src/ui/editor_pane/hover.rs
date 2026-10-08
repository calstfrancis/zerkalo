//! The explanation pop-up that opens when the pointer rests on an underlined mistake.

use super::*;

impl EditorPane {
    /// Wires the hover pop-up onto `view` and returns the slot holding the one on
    /// screen, which the click and focus handlers close.
    pub(super) fn wire_error_hover(
        &self,
        view: &View,
        buffer: &Buffer,
        path: &Path,
    ) -> Rc<RefCell<Option<Popover>>> {
        let view = view.clone();
        let buffer = buffer.clone();
        let path = path.to_path_buf();
        // ── Inline error assistant — hover over error-tagged line ─────────────
        // Shared with the any_click handler further down, which dismisses it
        // explicitly — see that handler's own comment for why.
        let active_error_popup: Rc<RefCell<Option<Popover>>> = Rc::new(RefCell::new(None));
        {
            let last_diags = self.last_diagnostics.clone();
            let on_fix_request = self.on_fix_request.clone();
            let view_hover = view.clone();
            let buf_hover = buffer.clone();
            let path_hover = path.clone();
            let active_popup = active_error_popup.clone();

            let motion = EventControllerMotion::new();
            let active_popup_c = active_popup.clone();
            let show_hover = Rc::new(move |x: f64, y: f64| {
                let diags = last_diags.borrow();
                if diags.is_empty() {
                    return;
                }

                let (bx, by) =
                    view_hover.window_to_buffer_coords(TextWindowType::Widget, x as i32, y as i32);
                let Some(iter) = view_hover.iter_at_location(bx, by) else {
                    // Past the end of a line, or below the text: no character is
                    // under the pointer, so the error pop-up has nothing to point at
                    // and closes like it does for any other spot off the error.
                    let taken = active_popup_c.borrow_mut().take();
                    if let Some(p) = taken {
                        p.popdown();
                    }
                    return;
                };
                let line_1based = iter.line() as u32 + 1;

                let tag_table = buf_hover.tag_table();
                let has_error_tag = tag_table
                    .lookup("zerkalo-diag-error")
                    .map(|t| iter.has_tag(&t))
                    .unwrap_or(false);
                if !has_error_tag {
                    // Moved off the error entirely — with autohide(false)
                    // (see why at the popover's own construction below)
                    // nothing else closes this, so it must be done here.
                    //
                    // The extracted Option is bound to its own `let` first,
                    // not matched directly on `.borrow_mut().take()` — Rust
                    // extends that temporary RefMut's lifetime to the end of
                    // the `if let` block otherwise, so it's still held while
                    // popdown() runs. popdown() synchronously fires this
                    // popover's own `connect_closed` handler below, which
                    // does `ap_closed.borrow_mut()` on the same RefCell —
                    // a real double-borrow panic, hit live (RefCell already
                    // borrowed, aborting the whole process, every time the
                    // mouse left an error line with the popup still open).
                    let taken = active_popup_c.borrow_mut().take();
                    if let Some(p) = taken {
                        p.popdown();
                    }
                    return;
                }

                let col_1based = iter.line_offset() as u32 + 1;
                let on_line = |m: &&DiagMark| m.line == line_1based && m.file == path_hover;
                let mark: Option<DiagMark> = diags
                    .iter()
                    .filter(on_line)
                    .find(|m| {
                        col_1based >= m.col
                            && (m.end_line != m.line || col_1based < m.end_col.max(m.col + 1))
                    })
                    .or_else(|| diags.iter().find(on_line))
                    .cloned();
                let Some(mark) = mark else { return };
                let msg = mark.headline.clone();

                // Only create a new popup if none is showing (avoid flicker)
                if active_popup_c.borrow().is_some() {
                    return;
                }

                let (line_start, line_end) = {
                    let mut a = buf_hover.iter_at_line(mark.line as i32 - 1).unwrap_or(iter);
                    let mut b = a;
                    if !b.ends_line() {
                        b.forward_to_line_end();
                    }
                    a.set_line_offset(0);
                    (a, b)
                };
                let line_text = buf_hover.text(&line_start, &line_end, true).to_string();
                let fix = crate::diagnostic_catalog::fix_for(mark.kind).filter(|_| {
                    !mark.in_package
                        && crate::diagnostic_catalog::fix_applies(
                            mark.kind,
                            Some(&line_text),
                            mark.col,
                        )
                });

                let popover = Popover::new();
                popover.set_parent(&view_hover);
                popover.set_has_arrow(true);
                // Not autohide(true): a click meant to place the cursor on
                // error-underlined text — the whole point of an inline error
                // popup is that the error is right where you're about to
                // edit — was instead being swallowed to dismiss the popover,
                // needing a second click to actually reach the text.
                // autohide(false) plus the explicit dismiss in any_click's
                // handler below (same pattern the LSP/citation popups already
                // use, and for the same reason) lets that first click do
                // both at once.
                popover.set_autohide(false);
                // Point at the hovered line's own rectangle and open below it, so the
                // popup never sits on top of the text you are about to click.
                let loc = view_hover.iter_location(&iter);
                let (wx, wy) =
                    view_hover.buffer_to_window_coords(TextWindowType::Widget, loc.x(), loc.y());
                popover.set_position(gtk4::PositionType::Bottom);
                popover.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(
                    wx.max(x as i32 - 1),
                    wy,
                    1,
                    loc.height().max(1),
                )));

                let vbox = GtkBox::new(Orientation::Vertical, 4);
                vbox.set_margin_top(8);
                vbox.set_margin_bottom(8);
                vbox.set_margin_start(10);
                vbox.set_margin_end(10);

                let msg_lbl = Label::new(Some(&msg));
                msg_lbl.set_xalign(0.0);
                msg_lbl.set_wrap(true);
                msg_lbl.set_max_width_chars(50);
                vbox.append(&msg_lbl);

                let explanation = match &fix {
                    Some(fx) => Some(fx.description.clone()),
                    None => Some(mark.advice.clone()).filter(|a| !a.is_empty()),
                };
                if let Some(text) = explanation {
                    vbox.append(&Separator::new(Orientation::Horizontal));
                    let fix_row = GtkBox::new(Orientation::Horizontal, 8);
                    let fix_desc = Label::new(Some(&text));
                    fix_desc.add_css_class("dim-label");
                    fix_desc.set_xalign(0.0);
                    fix_desc.set_wrap(true);
                    fix_desc.set_max_width_chars(40);
                    fix_desc.set_hexpand(true);
                    fix_row.append(&fix_desc);

                    if fix.is_some() {
                        let fix_btn = Button::with_label("Fix It");
                        fix_btn.add_css_class("suggested-action");
                        let request = on_fix_request.clone();
                        let mark_fix = mark.clone();
                        let pop_fix = popover.clone();
                        fix_btn.connect_clicked(move |_| {
                            pop_fix.popdown();
                            if let Some(f) = request.borrow().as_ref() {
                                f(mark_fix.clone());
                            }
                        });
                        fix_row.append(&fix_btn);
                    }
                    vbox.append(&fix_row);
                }

                popover.set_child(Some(&vbox));
                *active_popup_c.borrow_mut() = Some(popover.clone());
                let ap_closed = active_popup_c.clone();
                popover.connect_closed(move |_| {
                    *ap_closed.borrow_mut() = None;
                });
                popover.popup();
            });
            // A popup that opens the instant the pointer crosses an underline covers
            // text people are passing over on their way somewhere else, so it waits
            // for the pointer to rest. Moving while one is showing is handled at once
            // so it closes as soon as the pointer leaves the error.
            let last_pos = Rc::new(Cell::new((0.0f64, 0.0f64)));
            let hover_gen = Rc::new(Cell::new(0u64));
            {
                let showing = active_popup.clone();
                motion.connect_motion(move |_, x, y| {
                    last_pos.set((x, y));
                    let gen = hover_gen.get().wrapping_add(1);
                    hover_gen.set(gen);
                    if showing.borrow().is_some() {
                        show_hover(x, y);
                        return;
                    }
                    let show = show_hover.clone();
                    let gen_cell = hover_gen.clone();
                    let pos = last_pos.clone();
                    glib::timeout_add_local_once(HOVER_DELAY, move || {
                        if gen_cell.get() == gen {
                            let (x, y) = pos.get();
                            show(x, y);
                        }
                    });
                });
            }
            view.add_controller(motion);
        }

        active_error_popup
    }
}
