//! Find and replace in the active buffer: every match highlighted, the
//! current one in the accent colour and centred, marks beside the scrollbar,
//! and the counts kept up to date while you edit. What counts as a match is
//! `crate::text_search`, shared with Find in files.

use crate::text_search::{Found, Query};
use crate::ui::find_bar::{FindStep, MIN_TYPED};

use super::*;

const MATCH_TAG: &str = "zerkalo-search-match";
const CURRENT_TAG: &str = "zerkalo-search-current";
const FIND_MARK: &str = "zerkalo-find";
/// Beyond this many matches the scrollbar marks stop being drawn; they'd be
/// a solid bar anyway.
const MARK_LIMIT: usize = 2000;
const EDIT_PAUSE: std::time::Duration = std::time::Duration::from_millis(250);

/// The find bar's state for the open documents.
#[derive(Default)]
pub(super) struct FindState {
    /// Where the cursor was when the bar opened: search-as-you-type looks
    /// from here, so each letter refines the match instead of moving on.
    anchor: Cell<Option<i32>>,
    /// The match the current-match colour is on, as char offsets.
    current: Cell<Option<(i32, i32)>>,
    /// The matches last found, and in which buffer, for the scrollbar marks.
    matches: RefCell<(Option<Buffer>, Vec<(i32, i32)>)>,
    /// Edits to the searched buffer refresh the matches while the bar is open.
    watch: RefCell<Option<(Buffer, glib::SignalHandlerId)>>,
    refresh: RefCell<Option<glib::SourceId>>,
    on_current: RefCell<Option<Box<dyn Fn(PathBuf, usize)>>>,
    on_all_files: RefCell<Option<Box<dyn Fn(String)>>>,
}

/// The match `step` lands on, given the cursor, where typing started from,
/// and the match already current.
pub(super) fn pick_match(
    matches: &[(i32, i32)],
    step: FindStep,
    cursor: i32,
    anchor: Option<i32>,
    current: Option<(i32, i32)>,
) -> usize {
    match step {
        FindStep::Typed => {
            let from = anchor.unwrap_or(cursor);
            matches.iter().position(|(s, _)| *s >= from).unwrap_or(0)
        }
        FindStep::Next => matches
            .iter()
            .position(|m| m.0 >= cursor && Some(*m) != current)
            .unwrap_or(0),
        FindStep::Previous => matches
            .iter()
            .rposition(|(s, _)| *s < cursor)
            .unwrap_or(matches.len() - 1),
    }
}

fn whole_text(buffer: &Buffer) -> String {
    buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string()
}

/// The matches in the text you can see: simple mode's hidden preamble is
/// left out, so the bar never lands on (or replaces) text it can't show.
fn visible_matches(buffer: &Buffer, q: &Query, text: &str) -> Vec<Found> {
    let mut found = q.find(text);
    if let Some(hidden) = buffer
        .tag_table()
        .lookup(SIMPLE_TAG)
        .filter(|t| t.is_invisible())
    {
        found.retain(|f| !buffer.iter_at_offset(f.start as i32).has_tag(&hidden));
    }
    found
}

fn spans(found: &[Found]) -> Vec<(i32, i32)> {
    found
        .iter()
        .map(|f| (f.start as i32, f.end as i32))
        .collect()
}

fn rgba((r, g, b): (f64, f64, f64), a: f32) -> gtk4::gdk::RGBA {
    gtk4::gdk::RGBA::new(r as f32, g as f32, b as f32, a)
}

const FALLBACK_ACCENT: (f64, f64, f64) = (0.21, 0.52, 0.89);

/// The two highlight tags, coloured for the current theme: every match in
/// the style scheme's own search colour, the current one in the accent
/// colour, above every other tag.
fn search_tags(view: &View, buffer: &Buffer) -> (TextTag, TextTag) {
    let table = buffer.tag_table();
    let get = |name: &str| {
        table.lookup(name).unwrap_or_else(|| {
            let tag = TextTag::new(Some(name));
            table.add(&tag);
            tag
        })
    };
    let fresh = table.lookup(CURRENT_TAG).is_none();
    let all = get(MATCH_TAG);
    let current = get(CURRENT_TAG);
    if fresh {
        // A tag added later (spelling, comments) goes on top by default.
        let (a, c) = (all.downgrade(), current.downgrade());
        table.connect_tag_added(move |table, _| {
            if let (Some(a), Some(c)) = (a.upgrade(), c.upgrade()) {
                a.set_priority(table.size() - 1);
                c.set_priority(table.size() - 1);
            }
        });
    }
    let accent = crate::ui::theme::rgb(view, "accent_bg_color").unwrap_or(FALLBACK_ACCENT);
    let accent_fg = crate::ui::theme::rgb(view, "accent_fg_color").unwrap_or((1.0, 1.0, 1.0));
    let scheme_bg = buffer
        .style_scheme()
        .and_then(|s| s.style("search-match"))
        .filter(|st| st.property::<bool>("background-set"))
        .and_then(|st| st.property::<Option<String>>("background"))
        .and_then(|c| gtk4::gdk::RGBA::parse(c.as_str()).ok());
    all.set_background_rgba(Some(&scheme_bg.unwrap_or_else(|| rgba(accent, 0.25))));
    current.set_background_rgba(Some(&rgba(accent, 1.0)));
    current.set_foreground_rgba(Some(&rgba(accent_fg, 1.0)));
    all.set_priority(table.size() - 1);
    current.set_priority(table.size() - 1);
    (all, current)
}

impl EditorPane {
    /// Ctrl+F: opens the bar (with the selection, if there is one), and once
    /// it is open moves to the next match.
    pub fn find_open_or_next(&self) {
        self.find_bar.open_or_next(self.selection_for_find());
    }

    /// Ctrl+H: opens the bar with Replace showing.
    pub fn find_open_replace(&self) {
        self.find_bar.open_replace(self.selection_for_find());
    }

    pub(super) fn find_toggle(&self) {
        self.find_bar.toggle(self.selection_for_find());
    }

    /// Where the preview should follow the current match.
    pub fn set_on_find_current(&self, f: impl Fn(PathBuf, usize) + 'static) {
        *self.find.on_current.borrow_mut() = Some(Box::new(f));
    }

    /// Hands a search over to Find in files.
    pub fn set_on_find_in_files(&self, f: impl Fn(String) + 'static) {
        *self.find.on_all_files.borrow_mut() = Some(Box::new(f));
    }

    pub(super) fn find_in_files(&self, text: &str) {
        if let Some(f) = self.find.on_all_files.borrow().as_ref() {
            f(text.to_string());
        }
    }

    /// A short, single-line selection is what you're about to search for.
    fn selection_for_find(&self) -> Option<String> {
        let (_, buffer) = self.active_view_buffer()?;
        let (s, e) = buffer.selection_bounds()?;
        let text = buffer.text(&s, &e, false).to_string();
        (!text.contains('\n') && !text.trim().is_empty() && text.chars().count() <= 200)
            .then_some(text)
    }

    pub(super) fn remember_find_anchor(&self, open: bool) {
        let at = open
            .then(|| {
                self.active_view_buffer().map(|(_, b)| {
                    b.selection_bounds()
                        .map_or(b.cursor_position(), |(s, _)| s.offset())
                })
            })
            .flatten();
        self.find.anchor.set(at);
        self.find.current.set(None);
    }

    fn find_query(&self, text: &str) -> Result<Query, regex::Error> {
        Query::new(text, self.find_bar.options())
    }

    pub fn do_find(&self, text: &str, step: FindStep) {
        let Some((view, buffer)) = self.active_view_buffer() else {
            return;
        };
        if text.is_empty() || (step == FindStep::Typed && text.chars().count() < MIN_TYPED) {
            self.clear_search_highlight();
            self.find_bar.set_entry_error(false);
            self.find_bar.set_result(
                if text.is_empty() {
                    ""
                } else {
                    "Enter to search"
                },
                false,
            );
            return;
        }
        let q = match self.find_query(text) {
            Ok(q) => q,
            Err(_) => {
                self.clear_search_highlight();
                self.find_bar.set_entry_error(true);
                self.find_bar
                    .set_result("That pattern isn't quite right", false);
                return;
            }
        };
        self.find_bar.set_entry_error(false);
        self.watch_for_edits(&buffer);
        let matches = spans(&visible_matches(&buffer, &q, &whole_text(&buffer)));
        let say = step != FindStep::Typed;
        if matches.is_empty() {
            self.paint_matches(&view, &buffer, &matches, None);
            self.find_bar.set_result("No results", say);
            return;
        }

        let idx = pick_match(
            &matches,
            step,
            buffer.cursor_position(),
            self.find.anchor.get(),
            self.find.current.get(),
        );
        let (start_off, end_off) = matches[idx];
        let start = buffer.iter_at_offset(start_off);
        // The cursor, not a selection: a selection is painted over the
        // current-match colour.
        buffer.place_cursor(&start);
        self.paint_matches(&view, &buffer, &matches, Some((start_off, end_off)));

        // A mark that lives on: GTK waits until the lines up to it are laid
        // out before scrolling, and deleting the mark right after asking (as
        // this used to) cancels that scroll, so long documents never centred.
        let mark = match buffer.mark(FIND_MARK) {
            Some(mark) => {
                buffer.move_mark(&mark, &start);
                mark
            }
            None => buffer.create_mark(Some(FIND_MARK), &start, true),
        };
        view.scroll_to_mark(&mark, 0.0, true, 0.0, 0.5);

        self.find_bar
            .set_result(&format!("{} of {}", idx + 1, matches.len()), say);
        if let (Some(path), Some(f)) = (
            self.get_active_path(),
            self.find.on_current.borrow().as_ref(),
        ) {
            f(path, start_off as usize);
        }
    }

    /// Highlights `matches`, marks `current`, and redraws the scrollbar marks.
    fn paint_matches(
        &self,
        view: &View,
        buffer: &Buffer,
        matches: &[(i32, i32)],
        current: Option<(i32, i32)>,
    ) {
        let (all, cur) = search_tags(view, buffer);
        let (s, e) = buffer.bounds();
        buffer.remove_tag(&all, &s, &e);
        buffer.remove_tag(&cur, &s, &e);
        for &(a, b) in matches {
            buffer.apply_tag(&all, &buffer.iter_at_offset(a), &buffer.iter_at_offset(b));
        }
        if let Some((a, b)) = current {
            buffer.apply_tag(&cur, &buffer.iter_at_offset(a), &buffer.iter_at_offset(b));
        }
        self.find.current.set(current);
        *self.find.matches.borrow_mut() = (Some(buffer.clone()), matches.to_vec());
        self.find_marks.queue_draw();
    }

    /// While the bar is open, edits to the searched document refresh the
    /// highlights and the count, without moving the cursor or the view.
    fn watch_for_edits(&self, buffer: &Buffer) {
        let same = self
            .find
            .watch
            .borrow()
            .as_ref()
            .is_some_and(|(b, _)| b == buffer);
        if same {
            return;
        }
        self.stop_watching_edits();
        let ep = self.clone();
        let id = buffer.connect_changed(move |_| {
            if let Some(id) = ep.find.refresh.borrow_mut().take() {
                id.remove();
            }
            let ep2 = ep.clone();
            let id = glib::timeout_add_local_once(EDIT_PAUSE, move || {
                ep2.find.refresh.borrow_mut().take();
                ep2.refresh_find();
            });
            *ep.find.refresh.borrow_mut() = Some(id);
        });
        *self.find.watch.borrow_mut() = Some((buffer.clone(), id));
    }

    fn stop_watching_edits(&self) {
        if let Some((buffer, id)) = self.find.watch.borrow_mut().take() {
            buffer.disconnect(id);
        }
        if let Some(id) = self.find.refresh.borrow_mut().take() {
            id.remove();
        }
    }

    pub(super) fn find_on_tab_switch(&self) {
        if !self.find_bar.is_open() {
            return;
        }
        let Some((_, buffer)) = self.active_view_buffer() else {
            return;
        };
        self.remember_find_anchor(true);
        self.watch_for_edits(&buffer);
        self.refresh_find();
    }

    fn refresh_find(&self) {
        let text = self.find_bar.find_entry.text().to_string();
        if !self.find_bar.is_open() || text.chars().count() < MIN_TYPED {
            return;
        }
        let Some((view, buffer)) = self.active_view_buffer() else {
            return;
        };
        let Ok(q) = self.find_query(&text) else {
            return;
        };
        let matches = spans(&visible_matches(&buffer, &q, &whole_text(&buffer)));
        let was = self.find.current.get();
        let idx = was.and_then(|(s, _)| matches.iter().position(|m| m.0 >= s));
        self.paint_matches(&view, &buffer, &matches, idx.map(|i| matches[i]));
        self.find_bar.set_result(
            &match (matches.len(), idx) {
                (0, _) => "No results".to_string(),
                (n, Some(i)) => format!("{} of {n}", i + 1),
                (n, None) => format!("{n} found"),
            },
            false,
        );
    }

    pub fn clear_search_highlight(&self) {
        self.stop_watching_edits();
        self.find.current.set(None);
        self.find.matches.borrow_mut().1.clear();
        self.find_marks.queue_draw();
        let buffers: Vec<Buffer> = self
            .state
            .borrow()
            .tabs
            .values()
            .map(|t| t.buffer.clone())
            .collect();
        for buffer in buffers {
            let (start, end) = buffer.bounds();
            buffer.remove_tag_by_name(MATCH_TAG, &start, &end);
            buffer.remove_tag_by_name(CURRENT_TAG, &start, &end);
        }
    }

    /// The marks beside the scrollbar: where each match sits in the
    /// document, the current one stronger.
    pub(super) fn draw_find_marks(&self, area: &gtk4::DrawingArea, cr: &gtk4::cairo::Context) {
        let Some((view, buffer)) = self.active_view_buffer() else {
            return;
        };
        let found = self.find.matches.borrow();
        let (Some(of), matches) = (&found.0, &found.1) else {
            return;
        };
        if of != &buffer || matches.is_empty() || matches.len() > MARK_LIMIT {
            return;
        }
        let Some(bounds) = view.compute_bounds(area) else {
            return;
        };
        let upper = view.vadjustment().map_or(0.0, |a| a.upper());
        if upper <= 0.0 {
            return;
        }
        let (top, height) = (bounds.y() as f64, bounds.height() as f64);
        let width = area.width() as f64;
        let accent = crate::ui::theme::rgb(view.upcast_ref::<gtk4::Widget>(), "accent_bg_color")
            .unwrap_or(FALLBACK_ACCENT);
        let current = self.find.current.get();
        for &(s, e) in matches {
            let (y, _) = view.line_yrange(&buffer.iter_at_offset(s));
            let at = top + (y as f64 / upper) * height;
            let strong = current == Some((s, e));
            cr.set_source_rgba(
                accent.0,
                accent.1,
                accent.2,
                if strong { 1.0 } else { 0.55 },
            );
            cr.rectangle(
                0.0,
                at.min(top + height - 3.0),
                width,
                if strong { 4.0 } else { 2.0 },
            );
            let _ = cr.fill();
        }
    }

    /// Replaces the current match (with `$1`-style groups in regex mode) and
    /// moves on to the next.
    pub fn do_replace_one(&self, find: &str, replace: &str) {
        if find.is_empty() {
            return;
        }
        let Some((_view, buffer)) = self.active_view_buffer() else {
            return;
        };
        let Ok(q) = self.find_query(find) else {
            self.do_find(find, FindStep::Next);
            return;
        };
        let text = whole_text(&buffer);
        let current = self.find.current.get().and_then(|(s, e)| {
            visible_matches(&buffer, &q, &text)
                .into_iter()
                .find(|f| (f.start as i32, f.end as i32) == (s, e))
        });
        if let Some(f) = current {
            let with = q.replacement(&text, &f, replace);
            let mut start = buffer.iter_at_offset(f.start as i32);
            let mut end = buffer.iter_at_offset(f.end as i32);
            buffer.begin_user_action();
            buffer.delete(&mut start, &mut end);
            let mut at = buffer.iter_at_offset(f.start as i32);
            buffer.insert(&mut at, &with);
            buffer.end_user_action();
            let after = f.start as i32 + with.chars().count() as i32;
            buffer.place_cursor(&buffer.iter_at_offset(after));
            self.find.current.set(None);
        }
        self.do_find(find, FindStep::Next);
    }

    /// Removes the first occurrence of `text` found on `line` (1-indexed) of
    /// the active tab. Used to apply a resolved suggestion (see
    /// `crate::comments::suggestion_removes_text`) directly to the document —
    /// deliberately narrow (one line, exact substring) rather than a general
    /// find/replace, since the caller already knows exactly what it's
    /// looking for and where.
    pub fn remove_text_at_line(&self, line: u32, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        let Some((_view, buffer)) = self.active_view_buffer() else {
            return false;
        };
        let line_idx = line.saturating_sub(1) as i32;
        let Some(line_start) = buffer.iter_at_line(line_idx) else {
            return false;
        };
        let mut line_end = line_start;
        line_end.forward_to_line_end();
        let Some((mut start, mut end)) =
            line_start.forward_search(text, TextSearchFlags::TEXT_ONLY, Some(&line_end))
        else {
            return false;
        };
        buffer.begin_user_action();
        buffer.delete(&mut start, &mut end);
        buffer.end_user_action();
        true
    }

    /// Replaces every match you can see, as one undo step. Text simple mode
    /// hides is left alone.
    pub fn do_replace_all(&self, find: &str, replace: &str) {
        if find.is_empty() {
            return;
        }
        let Some((_view, buffer)) = self.active_view_buffer() else {
            return;
        };
        let q = match self.find_query(find) {
            Ok(q) => q,
            Err(_) => {
                self.find_bar.set_entry_error(true);
                self.find_bar
                    .set_result("That pattern isn't quite right", true);
                return;
            }
        };
        self.find_bar.set_entry_error(false);
        let text = whole_text(&buffer);
        let found = visible_matches(&buffer, &q, &text);
        if !found.is_empty() {
            let new_text = q.replace_found(&text, &found, replace);
            replace_text_minimally(&buffer, &new_text);
            let sm = *self.simple_mode.borrow();
            apply_simple_mode_tag(&buffer, sm);
        }
        self.find.current.set(None);
        self.refresh_find();
        self.find_bar
            .set_result(&format!("Replaced {}", found.len()), true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_refines_from_where_the_search_started() {
        let m = [(2, 5), (10, 13), (20, 23)];
        // The cursor has moved onto a later match; typing still looks from the anchor.
        assert_eq!(
            pick_match(&m, FindStep::Typed, 20, Some(3), Some((20, 23))),
            1
        );
        assert_eq!(pick_match(&m, FindStep::Typed, 0, Some(2), None), 0);
        assert_eq!(
            pick_match(&m, FindStep::Typed, 0, Some(30), None),
            0,
            "wraps"
        );
    }

    #[test]
    fn next_and_previous_step_from_the_current_match_and_wrap() {
        let m = [(2, 5), (10, 13), (20, 23)];
        assert_eq!(pick_match(&m, FindStep::Next, 10, None, Some((10, 13))), 2);
        assert_eq!(pick_match(&m, FindStep::Next, 20, None, Some((20, 23))), 0);
        // After a replacement the cursor sits just past it, with no current match.
        assert_eq!(pick_match(&m, FindStep::Next, 10, None, None), 1);
        assert_eq!(
            pick_match(&m, FindStep::Previous, 10, None, Some((10, 13))),
            0
        );
        assert_eq!(pick_match(&m, FindStep::Previous, 2, None, Some((2, 5))), 2);
    }
}
