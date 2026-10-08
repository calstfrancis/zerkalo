//! Simple Mode: hiding the template block and keeping it untouchable.

use super::*;

/// Where the hidden template ends, if `offset` falls inside it (Simple Mode).
pub(super) fn hidden_template_end(buf: &Buffer, offset: i32) -> Option<gtk4::TextIter> {
    let tag = buf.tag_table().lookup(SIMPLE_TAG)?;
    if !tag.is_invisible() {
        return None;
    }
    let at = buf.iter_at_offset(offset);
    if !at.has_tag(&tag) {
        return None;
    }
    let mut end = at;
    end.forward_to_tag_toggle(Some(&tag));
    Some(end)
}

pub(super) const SIMPLE_TAG: &str = "zk-simple-hidden";

pub(super) const BODY_SEPARATOR: &str = "// ── Document body";

/// Only the two lines the template generator writes: a line the user typed
/// into (or merged onto) must never be mistaken for the separator and hidden.
pub(super) fn is_separator_line(line: &str) -> bool {
    line.starts_with(BODY_SEPARATOR)
        && (line.ends_with('─') || line.ends_with("yours to edit freely."))
}

pub(super) fn apply_simple_mode_tag(buffer: &Buffer, on: bool) {
    let table = buffer.tag_table();
    let tag = match table.lookup(SIMPLE_TAG) {
        Some(t) => t,
        None => {
            let t = TextTag::new(Some(SIMPLE_TAG));
            t.set_invisible(on);
            table.add(&t);
            t
        }
    };
    tag.set_invisible(on);
    // Hidden text must also be untouchable: Backspace at the start of the body
    // used to eat the hidden newline and fold the first paragraph into the
    // separator comment, which drops it from the PDF.
    tag.set_editable(!on);

    // Always clear any existing span first.
    let (start, end) = buffer.bounds();
    buffer.remove_tag(&tag, &start, &end);

    if !on {
        return;
    }

    // Find the "// ── Document body" separator line.
    let text = buffer.text(&start, &end, false);
    let body_line = text.lines().position(is_separator_line);
    let Some(body_line_idx) = body_line else {
        return;
    };

    // Count consecutive separator lines (typically 2: the explanatory line
    // and the decorative rule beneath it) so we hide those too.
    let sep_count = text
        .lines()
        .skip(body_line_idx)
        .take_while(|l| is_separator_line(l))
        .count();
    let hide_to = body_line_idx + sep_count;

    if hide_to == 0 {
        return;
    }
    let hide_end = match buffer.iter_at_line(hide_to as i32) {
        Some(it) => it,
        None => {
            let (_, e) = buffer.bounds();
            e
        }
    };
    buffer.apply_tag(&tag, &start, &hide_end);
}

impl EditorPane {
    /// Apply simple mode to the current active buffer and update button label.
    /// The button reads "show template" or "hide template" — the action the
    /// next click performs — rather than a name-as-label bold/plain toggle,
    /// since new users found "SIMPLE" ambiguous about what it did. Lowercase
    /// to match the other status-bar words (format bar, focus, search)
    /// beside it — it used to be the one all-caps label in that row.
    pub fn apply_simple_mode(&self, on: bool) {
        *self.simple_mode.borrow_mut() = on;
        self.simple_mode_label
            .set_text(if on { "show template" } else { "hide template" });
        self.apply_simple_mode_to_buffer(on);
        if on && !self.format_bar_visible() && !*self.user_dismissed_format_bar.borrow() {
            self.set_format_bar_visible(true);
            if let Some(f) = self.on_format_bar_toggle.borrow().as_ref() {
                f(true);
            }
        }
        if !on && !self.shown_frontmatter_banner.get() {
            self.shown_frontmatter_banner.set(true);
            self.frontmatter_banner.set_revealed(true);
        }
    }

    pub(super) fn apply_simple_mode_to_buffer(&self, on: bool) {
        let left_margin = if on { 40 } else { 8 };
        let tabs: Vec<_> = {
            let state = self.state.borrow();
            state
                .tabs
                .values()
                .map(|t| (t.buffer.clone(), t.view.clone()))
                .collect()
        };
        for (buffer, view) in &tabs {
            apply_simple_mode_tag(buffer, on);
            view.set_show_line_numbers(!on || self.line_numbers_override.get());
            view.set_left_margin(left_margin);
            view.set_highlight_current_line(!on);
            writing::retune_hanging_tags(view, buffer);
        }
    }

    pub fn set_on_simple_mode_toggle(&self, f: impl Fn(bool) + 'static) {
        *self.on_simple_mode_toggle.borrow_mut() = Some(Box::new(f));
    }

    /// A jump to a line inside the hidden template setup (an error in a
    /// `#set` rule, most often) would land on invisible text, so show the
    /// template first, the same as clicking SHOW TEMPLATE.
    pub(super) fn reveal_template_if_hidden(&self, path: &PathBuf, line_idx: i32) {
        if !*self.simple_mode.borrow() {
            return;
        }
        let hidden = self.state.borrow().tabs.get(path).is_some_and(|tab| {
            let tag = tab.buffer.tag_table().lookup(SIMPLE_TAG);
            let it = tab.buffer.iter_at_line(line_idx);
            matches!((tag, it), (Some(tag), Some(it)) if it.has_tag(&tag))
        });
        if hidden {
            self.apply_simple_mode(false);
            if let Some(f) = self.on_simple_mode_toggle.borrow().as_ref() {
                f(false);
            }
        }
    }
}
