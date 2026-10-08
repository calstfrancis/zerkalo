//! Word counts, reading time, the word-count goal and the per-keystroke modified/word-count wiring.

use super::*;

/// Settle time before recomputing the status bar's section word count.
pub(super) const SECTION_WC_DEBOUNCE: Duration = Duration::from_millis(200);

pub(super) fn count_words(text: &str) -> u32 {
    count_content_words(text) as u32
}

pub(super) fn count_project_words(root: &std::path::Path) -> u32 {
    crate::project::collect_typ_files(root)
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .map(|c| count_content_words(&c) as u32)
        .sum()
}

pub(super) fn wc_str_with_delta(text: &str, session_start: u32) -> String {
    let words = count_content_words(text) as u32;
    let reading = if words < 200 {
        "< 1 min".to_string()
    } else {
        format!("{} min", words / 200)
    };
    if words > session_start {
        let delta = words - session_start;
        format!("{words} words (+{delta}) · {reading} read")
    } else {
        format!("{words} words · {reading} read")
    }
}

pub(super) fn set_wc_text_with_session(label: &Label, text: &str, session_start: u32) {
    label.set_text(&wc_str_with_delta(text, session_start));
}

pub(super) fn count_content_words(text: &str) -> usize {
    strip_typst_markup(&strip_zerkalo_blocks(text))
        .split_whitespace()
        .count()
}

pub(super) fn skip_balanced_typst(chars: &[char], start: usize, n: usize) -> usize {
    let open = chars[start];
    let close = match open {
        '[' => ']',
        '(' => ')',
        '{' => '}',
        _ => return start + 1,
    };
    let mut i = start + 1;
    let mut depth = 1usize;
    while i < n && depth > 0 {
        if chars[i] == open {
            depth += 1;
        } else if chars[i] == close {
            depth -= 1;
        }
        i += 1;
    }
    i
}

pub(super) fn update_goal_ring(ring: &DrawingArea, frac: &Rc<Cell<f64>>, text: &str, goal: u32) {
    if goal == 0 {
        ring.set_visible(false);
        return;
    }
    let words = count_content_words(text);
    let fraction = (words as f64 / goal as f64).min(1.0);
    frac.set(fraction);
    ring.queue_draw();
    ring.set_visible(true);
    ring.set_tooltip_text(Some(&format!(
        "{words} / {goal} words ({:.0}%)",
        fraction * 100.0
    )));
}

pub(super) fn parse_goal_comment(content: &str) -> Option<u32> {
    for line in content.lines().take(20) {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("// @zerkalo-goal:") {
            if let Ok(n) = rest.trim().parse::<u32>() {
                return Some(n);
            }
        }
    }
    None
}

pub(super) fn section_heading_level(text: &str) -> Option<usize> {
    let trimmed = text.trim_start();
    let lvl = trimmed.chars().take_while(|c| *c == '=').count();
    if lvl > 0 && trimmed[lvl..].starts_with(' ') {
        Some(lvl)
    } else {
        None
    }
}

/// Count words in a line of Typst text, treating `#lorem(N)` as N words.
pub(super) fn count_words_typst(text: &str) -> u32 {
    let mut count = 0u32;
    let mut remaining = text;
    while !remaining.is_empty() {
        if let Some(pos) = remaining.find("#lorem(") {
            // Count words before #lorem
            count += remaining[..pos].split_whitespace().count() as u32;
            let after = &remaining[pos + 7..];
            if let Some(end) = after.find(')') {
                if let Ok(n) = after[..end].trim().parse::<u32>() {
                    count += n;
                }
                remaining = &after[end + 1..];
            } else {
                break;
            }
        } else {
            count += remaining.split_whitespace().count() as u32;
            break;
        }
    }
    count
}

/// Words in the section containing `cursor_line`. Reads the buffer once and
/// works on borrowed lines — the previous version issued three `buf.text()`
/// calls per line (one per pass), which on a long document meant thousands of
/// GTK round-trips and allocations every time the cursor changed line.
pub(super) fn section_word_count_for_line(
    buf: &sourceview5::Buffer,
    cursor_line: i32,
) -> Option<u32> {
    let (s, e) = buf.bounds();
    let text = buf.text(&s, &e, false).to_string();
    let lines: Vec<&str> = text.lines().collect();
    let total = lines.len();
    let cursor_line = (cursor_line.max(0) as usize).min(total.saturating_sub(1));
    if total == 0 {
        return None;
    }

    let (sec_start, sec_level) = (0..=cursor_line)
        .rev()
        .find_map(|ln| section_heading_level(lines[ln]).map(|lvl| (ln, lvl)))?;

    let sec_end = ((sec_start + 1)..total)
        .find(|&ln| section_heading_level(lines[ln]).is_some_and(|lvl| lvl <= sec_level))
        .unwrap_or(total);

    Some(
        lines[sec_start..sec_end]
            .iter()
            .map(|l| count_words_typst(l))
            .sum(),
    )
}

impl EditorPane {
    /// Sets the global goal from Settings. A document whose text carries its
    /// own `// @zerkalo-goal:` comment keeps that goal; everything else picks
    /// this one up immediately.
    pub fn apply_word_count_goal(&self, goal: u32) {
        *self.default_word_count_goal.borrow_mut() = goal;
        let text = self.get_active_content();
        let effective = text.as_deref().and_then(parse_goal_comment).unwrap_or(goal);
        *self.word_count_goal.borrow_mut() = effective;
        if effective == 0 {
            self.goal_ring.set_visible(false);
        } else if let Some(text) = text {
            update_goal_ring(&self.goal_ring, &self.goal_fraction, &text, effective);
        }
    }

    pub fn set_session_delta(&self, delta: i32) {
        if delta > 0 {
            self.session_delta_label.set_text(&format!("↑ {delta}"));
            self.session_delta_label
                .add_css_class("session-delta-positive");
            self.session_delta_label.set_visible(true);
        } else {
            self.session_delta_label
                .remove_css_class("session-delta-positive");
            self.session_delta_label.set_visible(false);
        }
    }

    pub fn get_active_session_delta(&self) -> i32 {
        let current = match self.notebook.current_page() {
            Some(p) => p,
            None => return 0,
        };
        let state = self.state.borrow();
        for tab in state.tabs.values() {
            if self.notebook.page_num(&tab.notebook_page) == Some(current) {
                let (s, e) = tab.buffer.bounds();
                let text = tab.buffer.text(&s, &e, false);
                let current_words = count_words(&text) as i32;
                return current_words - tab.session_start_words as i32;
            }
        }
        0
    }

    pub(super) fn wire_modified_and_word_count(&self, tab: &TabContext, content: &str) {
        // ── Modified flag + word count ────────────────────────────────────────

        let state_for_change = self.state.clone();
        let path_for_change = tab.path.clone();
        let dot_for_change = tab.dot_label.clone();
        let on_change_cb = self.on_change.clone();
        let on_modified_cb = self.on_modified_changed.clone();
        let on_file_dirty_cb = self.on_file_dirty.clone();
        let wc_for_change = self.word_count_label.clone();
        let goal_for_change = self.goal_ring.clone();
        let goal_frac_for_change = self.goal_fraction.clone();
        let goal_val_for_change = self.word_count_goal.clone();
        let goal_celebrating_for_change = self.goal_celebrating.clone();
        let goal_was_met_for_change: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let last_wc_for_change = self.last_wc_text.clone();
        let project_root_for_wc = self.project_root.clone();
        let session_start_for_change: Rc<std::cell::Cell<u32>> =
            Rc::new(std::cell::Cell::new(count_words(content)));
        // SourceId-based debounce timers. Each keystroke cancels the previous
        // pending timer before scheduling a new one, so timers never accumulate
        // in the event loop regardless of typing speed.
        let last_edit_for_change = self.last_edit.clone();
        let wc_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        let comment_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        let comment_spans: Rc<RefCell<Vec<(i32, i32)>>> = Rc::new(RefCell::new(Vec::new()));
        let proj_wc_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        tab.buffer.connect_changed(move |buf| {
            last_edit_for_change.set(Some(std::time::Instant::now()));
            let newly_modified = {
                let mut state = state_for_change.borrow_mut();
                if let Some(tab) = state.tabs.get_mut(&path_for_change) {
                    if !tab.modified {
                        tab.modified = true;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            };
            if newly_modified {
                // GTK widget ops must happen after borrow_mut is released — doing
                // them inside the borrow can cause reentrant signal dispatch that
                // tries to borrow state again, triggering a BorrowMutError panic.
                dot_for_change.set_visible(true);
                if let Some(f) = on_modified_cb.borrow().as_ref() {
                    f(true);
                }
                if let Some(f) = on_file_dirty_cb.borrow().as_ref() {
                    f(path_for_change.clone(), true);
                }
            }
            if let Some(f) = on_change_cb.borrow().as_ref() {
                f();
            }

            // ── Debounced word count (300 ms) ─────────────────────────────────
            if let Some(id) = wc_timer.borrow_mut().take() {
                id.remove();
            }
            {
                let wc2 = wc_for_change.clone();
                let goal2 = goal_for_change.clone();
                let goal_frac2 = goal_frac_for_change.clone();
                let goal_val2 = goal_val_for_change.clone();
                let last_wc2 = last_wc_for_change.clone();
                let ss2 = session_start_for_change.clone();
                let buf2 = buf.clone();
                let t = wc_timer.clone();
                let goal_cel2 = goal_celebrating_for_change.clone();
                let goal_was2 = goal_was_met_for_change.clone();
                *wc_timer.borrow_mut() = Some(glib::timeout_add_local_once(
                    Duration::from_millis(300),
                    move || {
                        *t.borrow_mut() = None;
                        let (s, e) = buf2.bounds();
                        let text = buf2.text(&s, &e, false);
                        let goal = *goal_val2.borrow();
                        if goal > 0 {
                            let was_met = goal_was2.get();
                            update_goal_ring(&goal2, &goal_frac2, &text, goal);
                            let now_met = goal_frac2.get() >= 1.0;
                            goal_was2.set(now_met);
                            if now_met && !was_met {
                                goal_cel2.set(true);
                                goal2.queue_draw();
                                let cel_reset = goal_cel2.clone();
                                let ring_reset = goal2.clone();
                                glib::timeout_add_local_once(
                                    Duration::from_millis(900),
                                    move || {
                                        cel_reset.set(false);
                                        ring_reset.queue_draw();
                                    },
                                );
                            }
                        }
                        let wc_str = wc_str_with_delta(&text, ss2.get());
                        *last_wc2.borrow_mut() = wc_str.clone();
                        wc2.set_text(&wc_str);
                    },
                ));
            }

            // ── Debounced project word count tooltip (5 s) ────────────────────
            if let Some(id) = proj_wc_timer.borrow_mut().take() {
                id.remove();
            }
            {
                let wc_lbl_proj = wc_for_change.clone();
                let root_proj = project_root_for_wc.clone();
                let t = proj_wc_timer.clone();
                *proj_wc_timer.borrow_mut() = Some(glib::timeout_add_local_once(
                    Duration::from_millis(5000),
                    move || {
                        *t.borrow_mut() = None;
                        let root = root_proj.borrow().clone();
                        if let Some(root) = root {
                            let lbl = wc_lbl_proj.clone();
                            glib::spawn_future_local(async move {
                                if let Ok(total) =
                                    gtk4::gio::spawn_blocking(move || count_project_words(&root))
                                        .await
                                {
                                    lbl.set_tooltip_text(Some(&format!(
                                        "Project total: {total} words"
                                    )));
                                }
                            });
                        }
                    },
                ));
            }

            // ── Debounced comment highlights (500 ms) ─────────────────────────
            if let Some(id) = comment_timer.borrow_mut().take() {
                id.remove();
            }
            {
                let buf_comment = buf.clone();
                let t = comment_timer.clone();
                let cache = comment_spans.clone();
                *comment_timer.borrow_mut() = Some(glib::timeout_add_local_once(
                    Duration::from_millis(500),
                    move || {
                        *t.borrow_mut() = None;
                        apply_comment_highlights(&buf_comment, Some(&cache));
                    },
                ));
            }
        });

        // Undoing back to the text on disk makes it unmodified again: GtkSource
        // tracks the saved state, so the unsaved dot follows it rather than
        // staying lit until the next save.
        let state_for_clean = self.state.clone();
        let path_for_clean = tab.path.clone();
        let dot_for_clean = tab.dot_label.clone();
        let on_modified_clean = self.on_modified_changed.clone();
        let on_dirty_clean = self.on_file_dirty.clone();
        tab.buffer.connect_modified_changed(move |buf| {
            if buf.is_modified() {
                return;
            }
            let was_modified = {
                let mut state = state_for_clean.borrow_mut();
                state
                    .tabs
                    .get_mut(&path_for_clean)
                    .is_some_and(|t| std::mem::replace(&mut t.modified, false))
            };
            if !was_modified {
                return;
            }
            dot_for_clean.set_visible(false);
            crate::auto_save::clear(&path_for_clean);
            if let Some(f) = on_dirty_clean.borrow().as_ref() {
                f(path_for_clean.clone(), false);
            }
            if let Some(f) = on_modified_clean.borrow().as_ref() {
                f(false);
            }
        });
    }
}
