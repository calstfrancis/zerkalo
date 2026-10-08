//! Jumping to a line, text or offset, with the landing spot briefly tinted; and showing the cursor in the preview.

use super::*;

/// The buffer mark `jump_to_line` scrolls to. Named, so one mark per buffer is
/// reused rather than created and destroyed per jump.
pub(super) const JUMP_MARK: &str = "zerkalo-jump";

pub(super) const JUMP_FLASH_TAG: &str = "zerkalo-jump-flash";

/// Briefly tints `start..end` so a jump's landing spot is easy to find. The
/// tint never becomes a selection and goes away on the next edit, click or
/// after a moment, whichever comes first.
pub(super) fn flash_range(view: &View, buffer: &Buffer, start: i32, end: i32) {
    let table = buffer.tag_table();
    let tag = table.lookup(JUMP_FLASH_TAG).unwrap_or_else(|| {
        let tag = TextTag::new(Some(JUMP_FLASH_TAG));
        table.add(&tag);
        tag
    });
    let (r, g, b) = crate::ui::theme::rgb(view, "accent_color").unwrap_or((0.2, 0.5, 0.9));
    tag.set_background_rgba(Some(&gtk4::gdk::RGBA::new(
        r as f32, g as f32, b as f32, 0.28,
    )));
    let (bs, be) = buffer.bounds();
    buffer.remove_tag(&tag, &bs, &be);
    buffer.apply_tag(
        &tag,
        &buffer.iter_at_offset(start),
        &buffer.iter_at_offset(end),
    );

    let clear = {
        let buffer = buffer.clone();
        move || {
            if let Some(tag) = buffer.tag_table().lookup(JUMP_FLASH_TAG) {
                let (s, e) = buffer.bounds();
                buffer.remove_tag(&tag, &s, &e);
            }
        }
    };
    let once = Rc::new(Cell::new(Some(clear)));
    let handler: Rc<Cell<Option<glib::SignalHandlerId>>> = Rc::new(Cell::new(None));
    let finish = {
        let once = once.clone();
        let handler = handler.clone();
        let buffer = buffer.clone();
        Rc::new(move || {
            if let Some(f) = once.take() {
                f();
            }
            if let Some(id) = handler.take() {
                buffer.disconnect(id);
            }
        })
    };
    let id = buffer.connect_changed({
        let finish = finish.clone();
        move |_| {
            let finish = finish.clone();
            glib::idle_add_local_once(move || finish());
        }
    });
    handler.set(Some(id));
    glib::timeout_add_local_once(Duration::from_millis(1400), move || finish());
}

impl EditorPane {
    pub fn next_tab(&self) {
        let n = self.notebook.n_pages();
        if n < 2 {
            return;
        }
        let current = self.notebook.current_page().unwrap_or(0);
        self.notebook.set_current_page(Some((current + 1) % n));
        self.grab_focus();
    }

    pub fn prev_tab(&self) {
        let n = self.notebook.n_pages();
        if n < 2 {
            return;
        }
        let current = self.notebook.current_page().unwrap_or(0);
        let prev = if current == 0 { n - 1 } else { current - 1 };
        self.notebook.set_current_page(Some(prev));
        self.grab_focus();
    }

    /// Jump to the first occurrence of `text` in the active buffer, select it, and scroll to centre.
    pub fn jump_to_text(&self, text: &str) {
        let Some((view, buffer)) = self.active_view_buffer() else {
            return;
        };
        let flags = TextSearchFlags::TEXT_ONLY | TextSearchFlags::CASE_INSENSITIVE;
        let start_iter = buffer.start_iter();
        if let Some((s, e)) = start_iter.forward_search(text, flags, None) {
            buffer.place_cursor(&s);
            flash_range(&view, &buffer, s.offset(), e.offset());
            view.scroll_to_iter(&mut s.clone(), 0.0, true, 0.0, 0.5);
            self.focus_after_jump(&view);
        }
    }

    pub fn jump_to_line(&self, path: &PathBuf, line: u32) {
        self.switch_to_file(path);
        self.reveal_template_if_hidden(path, line.saturating_sub(1) as i32);
        let state = self.state.borrow();
        if let Some(tab) = state.tabs.get(path) {
            let line_idx = line.saturating_sub(1) as i32;
            let line_start = tab.buffer.iter_at_line(line_idx).unwrap_or_else(|| {
                let (_, end) = tab.buffer.bounds();
                end
            });
            let mut line_end = line_start;
            line_end.forward_to_line_end();
            // Place the cursor and flash the line rather than selecting it: a
            // selected heading is replaced by the very next keystroke.
            tab.buffer.place_cursor(&line_start);
            flash_range(
                &tab.view,
                &tab.buffer,
                line_start.offset(),
                line_end.offset(),
            );
            // Scroll to a mark, not to the iter. scroll_to_iter works off the
            // view's current idea of where that line is, which is wrong — and
            // reported as fine — until the view has validated the lines in
            // between; on a tab that was just switched to, or one never
            // scrolled, the jump silently does nothing. GTK holds a mark until
            // the layout is valid and then scrolls to it. One reused mark, not
            // one per jump: a fresh mark would have to be deleted afterwards,
            // and deleting it cancels the very scroll it was created for.
            let mark = match tab.buffer.mark(JUMP_MARK) {
                Some(mark) => {
                    tab.buffer.move_mark(&mark, &line_start);
                    mark
                }
                None => tab.buffer.create_mark(Some(JUMP_MARK), &line_start, false),
            };
            tab.view.scroll_to_mark(&mark, 0.0, true, 0.0, 0.5);
            self.focus_after_jump(&tab.view);
        }
    }

    /// Puts the cursor at `char_offset` in `path` and scrolls it into view —
    /// the landing spot for a click in the preview.
    pub fn jump_to_offset(&self, path: &PathBuf, char_offset: usize) {
        self.switch_to_file(path);
        let line = self
            .state
            .borrow()
            .tabs
            .get(path)
            .map(|tab| tab.buffer.iter_at_offset(char_offset as i32).line());
        let Some(line) = line else { return };
        self.reveal_template_if_hidden(path, line);
        let state = self.state.borrow();
        let Some(tab) = state.tabs.get(path) else {
            return;
        };
        let at = tab.buffer.iter_at_offset(char_offset as i32);
        tab.buffer.place_cursor(&at);
        // A mark, for the same reason as in `jump_to_line`.
        let mark = match tab.buffer.mark(JUMP_MARK) {
            Some(mark) => {
                tab.buffer.move_mark(&mark, &at);
                mark
            }
            None => tab.buffer.create_mark(Some(JUMP_MARK), &at, false),
        };
        tab.view.scroll_to_mark(&mark, 0.1, false, 0.0, 0.0);
        self.focus_after_jump(&tab.view);
    }

    /// Ctrl+click in the editor (or the "Show in preview" command) asks for
    /// the preview to scroll to that spot; this is where it's delivered.
    pub fn set_on_show_in_preview(&self, f: impl Fn(PathBuf, usize) + 'static) {
        *self.on_show_in_preview.borrow_mut() = Some(Box::new(f));
    }

    /// Shows the cursor's position in the preview.
    pub fn show_cursor_in_preview(&self) {
        let Some(path) = self.get_active_path() else {
            return;
        };
        let Some((_, buffer)) = self.active_view_buffer() else {
            return;
        };
        if let Some(f) = self.on_show_in_preview.borrow().as_ref() {
            f(path, buffer.cursor_position().max(0) as usize);
        }
    }

    pub(super) fn focus_after_jump(&self, view: &View) {
        self.jumping.set(true);
        view.grab_focus();
        self.jumping.set(false);
    }
}
