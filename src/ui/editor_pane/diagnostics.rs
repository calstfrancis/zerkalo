//! Compile errors and warnings in the text: underline marks, their colours and the one-click fixes.

use super::*;

/// Re-apply squiggles and gutter marks for a single tab. Split out of
/// `mark_diagnostics` so an edit can refresh only the buffer that changed —
/// the full-buffer tag sweep below costs proportional to document length, and
/// doing it for every open tab on every keystroke was the bulk of typing lag.
pub(super) fn mark_diagnostics_for_tab(
    path: &Path,
    buffer: &Buffer,
    diag_dot: &TabMark,
    diagnostics: &[DiagMark],
) {
    let (buf_start, buf_end) = buffer.bounds();
    ensure_diag_tags(buffer);
    refresh_diag_colors(buffer, diag_dot);
    buffer.remove_tag_by_name("zerkalo-diag-error", &buf_start, &buf_end);
    buffer.remove_tag_by_name("zerkalo-diag-warning", &buf_start, &buf_end);
    buffer.remove_source_marks(&buf_start, &buf_end, Some("zerkalo-error"));
    buffer.remove_source_marks(&buf_start, &buf_end, Some("zerkalo-warning"));
    let has_errors = diagnostics.iter().any(|m| m.file == path && m.is_error);
    diag_dot.set_visible(has_errors);
    for m in diagnostics.iter().filter(|m| m.file == path) {
        let line_idx = m.line.saturating_sub(1) as i32;
        let Some(line_start) = buffer.iter_at_line(line_idx) else {
            continue;
        };
        let mut line_end = line_start;
        if !line_end.ends_line() {
            line_end.forward_to_line_end();
        }
        let (from, to) = diag_span(&line_start, &line_end, m);
        let tag = if m.is_error {
            "zerkalo-diag-error"
        } else {
            "zerkalo-diag-warning"
        };
        buffer.apply_tag_by_name(tag, &from, &to);
        let category = if m.is_error {
            "zerkalo-error"
        } else {
            "zerkalo-warning"
        };
        buffer.create_source_mark(None, category, &line_start);
    }
}

/// The characters to underline: the reported range, cut off at the end of its
/// first line so a diagnostic on a whole multi-line block doesn't underline the
/// block. A report with no extent (or none we could place) falls back to the
/// whole line, as every diagnostic used to.
pub(super) fn diag_span(
    line_start: &gtk4::TextIter,
    line_end: &gtk4::TextIter,
    m: &DiagMark,
) -> (gtk4::TextIter, gtk4::TextIter) {
    let line_len = line_end.line_offset();
    let start = m.col.saturating_sub(1) as i32;
    let end = if m.end_line == m.line {
        m.end_col.saturating_sub(1) as i32
    } else {
        line_len
    }
    .min(line_len);
    if end <= start || start >= line_len {
        return (*line_start, *line_end);
    }
    let mut from = *line_start;
    from.set_line_offset(start);
    let mut to = *line_start;
    to.set_line_offset(end);
    (from, to)
}

/// Applies one edit to `buffer` as a single undo step. Offsets are characters,
/// which is what `GtkTextBuffer` counts in.
pub(super) fn apply_edit(buffer: &Buffer, edit: &crate::diagnostic_catalog::Edit) {
    buffer.begin_user_action();
    if edit.end > edit.start {
        let mut start = buffer.iter_at_offset(edit.start as i32);
        let mut end = buffer.iter_at_offset(edit.end as i32);
        buffer.delete(&mut start, &mut end);
    }
    if !edit.text.is_empty() {
        let mut at = buffer.iter_at_offset(edit.start as i32);
        buffer.insert(&mut at, &edit.text);
    }
    buffer.end_user_action();
}

/// Underline colours come from the theme's named error/warning colours, looked
/// up each time the marks are re-applied so a light/dark or accent change is
/// picked up on the next diagnostics pass instead of being frozen at creation.
pub(super) fn refresh_diag_colors(buffer: &Buffer, mark: &TabMark) {
    let table = buffer.tag_table();
    for (tag_name, color_name) in [
        ("zerkalo-diag-error", "error_color"),
        ("zerkalo-diag-warning", "warning_color"),
    ] {
        if let (Some(tag), Some((r, g, b))) = (
            table.lookup(tag_name),
            crate::ui::theme::rgb(mark.widget(), color_name),
        ) {
            tag.set_underline_rgba(Some(&gtk4::gdk::RGBA::new(
                r as f32, g as f32, b as f32, 1.0,
            )));
        }
    }
}

pub(super) fn ensure_diag_tags(buffer: &Buffer) {
    let table = buffer.tag_table();
    if table.lookup("zerkalo-diag-error").is_none() {
        let tag = TextTag::new(Some("zerkalo-diag-error"));
        tag.set_underline(gtk4::pango::Underline::Error);
        tag.set_underline_rgba(Some(&gtk4::gdk::RGBA::new(0.9, 0.2, 0.2, 1.0)));
        table.add(&tag);
    }
    if table.lookup("zerkalo-diag-warning").is_none() {
        let tag = TextTag::new(Some("zerkalo-diag-warning"));
        tag.set_underline(gtk4::pango::Underline::SingleLine);
        tag.set_underline_rgba(Some(&gtk4::gdk::RGBA::new(0.85, 0.72, 0.1, 1.0)));
        table.add(&tag);
    }
}

/// How long after the last edit before diagnostic squiggles are re-applied.
/// Long enough that a burst of typing costs one sweep, not one per keystroke.
pub(super) const DIAG_REMARK_DEBOUNCE: Duration = Duration::from_millis(250);

impl EditorPane {
    /// Underline the exact text at fault for each diagnostic, with a gutter
    /// mark on its first line. Call after compile or LSP diagnostics.
    pub fn mark_diagnostics(&self, diagnostics: &[DiagMark]) {
        *self.last_diagnostics.borrow_mut() = diagnostics.to_vec();
        // Collect buffer/widget refs while holding borrow, then drop it before GTK ops.
        // GTK buffer ops (apply_tag, create_source_mark) fire synchronous signals that
        // can cascade back into Zerkalo callbacks that try borrow_mut — holding borrow
        // across them causes a BorrowError → SIGABRT.
        let tabs: Vec<(PathBuf, Buffer, TabMark)> = {
            let state = self.state.borrow();
            state
                .tabs
                .iter()
                .map(|(p, t)| (p.clone(), t.buffer.clone(), t.diag_dot.clone()))
                .collect()
        };
        for (path, buffer, diag_dot) in &tabs {
            mark_diagnostics_for_tab(path, buffer, diag_dot, diagnostics);
        }
    }

    pub fn clear_diagnostic_marks(&self) {
        self.last_diagnostics.borrow_mut().clear();
        let tabs: Vec<(Buffer, TabMark)> = {
            let state = self.state.borrow();
            state
                .tabs
                .values()
                .map(|t| (t.buffer.clone(), t.diag_dot.clone()))
                .collect()
        };
        for (buffer, diag_dot) in &tabs {
            let (start, end) = buffer.bounds();
            ensure_diag_tags(buffer);
            buffer.remove_tag_by_name("zerkalo-diag-error", &start, &end);
            buffer.remove_tag_by_name("zerkalo-diag-warning", &start, &end);
            buffer.remove_source_marks(&start, &end, Some("zerkalo-error"));
            buffer.remove_source_marks(&start, &end, Some("zerkalo-warning"));
            diag_dot.set_visible(false);
        }
    }

    /// Applies the one-click fix for `mark` to the buffer open for its file
    /// (not whichever tab is active) as one undoable edit that touches only the
    /// characters that change, so the cursor, scroll position and marks
    /// elsewhere survive.
    pub fn apply_fix(&self, mark: &DiagMark) -> FixOutcome {
        use crate::diagnostic_catalog::{fix_for, fix_helped, Site};
        let buffer = {
            let state = self.state.borrow();
            match state.tabs.get(&mark.file) {
                Some(t) => t.buffer.clone(),
                None => return FixOutcome::NotApplicable,
            }
        };
        let Some(fix) = fix_for(mark.kind) else {
            return FixOutcome::NotApplicable;
        };
        let (s, e) = buffer.bounds();
        let before = buffer.text(&s, &e, true).to_string();
        let site = Site::new(&before, mark.line, mark.col);
        let Some(edit) = (fix.apply)(&site) else {
            return FixOutcome::NotApplicable;
        };
        let after = edit.apply_to(&before);
        if after == before {
            return FixOutcome::NotApplicable;
        }
        apply_edit(&buffer, &edit);
        let sm = *self.simple_mode.borrow();
        apply_simple_mode_tag(&buffer, sm);
        if fix_helped(mark.kind, &before, &after) {
            FixOutcome::Fixed
        } else {
            FixOutcome::DidNotHelp
        }
    }
}
