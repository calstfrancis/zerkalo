//! Word counts, reading time, the word-count goal and the per-keystroke modified/word-count wiring.

use super::*;

/// Settle time before recomputing the status bar's section word count.
pub(super) const SECTION_WC_DEBOUNCE: Duration = Duration::from_millis(200);

pub(super) type Cites<'a> = Option<&'a crate::word_cites::CitationWords>;

pub(super) fn count_words(text: &str) -> u32 {
    count_content_words(text) as u32
}

/// The citations `text` counts, if it asked for them (see `word_cites`).
pub(crate) fn cites_for(
    text: &str,
    doc: &std::path::Path,
    project_root: Option<&std::path::Path>,
) -> Option<std::sync::Arc<crate::word_cites::CitationWords>> {
    if !crate::word_cites::counts_citations(text) {
        return None;
    }
    let configured = project_root
        .and_then(crate::config::ProjectConfig::load)
        .and_then(|p| p.bib_path)
        .or_else(|| crate::config::Config::load().ok().and_then(|c| c.bib_path));
    crate::word_cites::for_document(text, doc, project_root, configured.as_deref())
}

/// Words in the whole project, counting citations as the active document
/// does — a chapter file carries no marker of its own, the root does.
pub(super) fn count_project_words(root: &std::path::Path, cites: Cites) -> u32 {
    crate::project::collect_typ_files(root)
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .map(|c| count_content_words_in(&c, cites) as u32)
        .sum()
}

pub(super) fn wc_str_with_delta(text: &str, session_start: u32, cites: Cites) -> String {
    let words = count_content_words_in(text, cites) as u32;
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

pub(super) fn set_wc_text_with_session(
    label: &Label,
    text: &str,
    session_start: u32,
    cites: Cites,
) {
    label.set_text(&wc_str_with_delta(text, session_start, cites));
}

pub(super) fn count_content_words(text: &str) -> usize {
    count_content_words_in(text, None)
}

pub(super) fn count_content_words_in(text: &str, cites: Cites) -> usize {
    strip_typst_markup_with(&strip_zerkalo_blocks(text), cites)
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

pub(super) fn update_goal_ring(
    ring: &DrawingArea,
    frac: &Rc<Cell<f64>>,
    text: &str,
    goal: u32,
    cites: Cites,
) {
    if goal == 0 {
        ring.set_visible(false);
        return;
    }
    let words = count_content_words_in(text, cites);
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

/// Words in a stretch of Typst source, `#lorem(N)` counting as N. Joined text
/// goes through the same stripping as the whole-document count: comment and
/// raw-block state spans lines, so counting line by line would miss it.
pub(crate) fn count_section_words(text: &str, cites: Cites) -> u32 {
    let mut expanded = String::with_capacity(text.len());
    let mut remaining = text;
    while !remaining.is_empty() {
        let Some(pos) = remaining.find("#lorem(") else {
            expanded.push_str(remaining);
            break;
        };
        expanded.push_str(&remaining[..pos]);
        let after = &remaining[pos + 7..];
        let Some(end) = after.find(')') else {
            expanded.push_str(&remaining[pos..]);
            break;
        };
        if let Ok(n) = after[..end].trim().parse::<u32>() {
            for _ in 0..n {
                expanded.push_str("lorem ");
            }
        }
        remaining = &after[end + 1..];
    }
    count_content_words_in(&expanded, cites) as u32
}

/// Words in the section containing `cursor_line`. Reads the buffer once and
/// works on borrowed lines — the previous version issued three `buf.text()`
/// calls per line (one per pass), which on a long document meant thousands of
/// GTK round-trips and allocations every time the cursor changed line.
pub(super) fn section_word_count_for_line(
    buf: &sourceview5::Buffer,
    cursor_line: i32,
    cites: Cites,
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

    Some(count_section_words(
        &lines[sec_start..sec_end].join("\n"),
        cites,
    ))
}

/// Shows the "citations in count" toggle only for a document whose style
/// prints parenthetical citations, bold while they count.
pub(super) fn sync_cite_toggle(
    btn: &Button,
    label: &Label,
    text: &str,
    doc: &std::path::Path,
    project_root: Option<&std::path::Path>,
) {
    let applies = crate::word_cites::applies_to(text, doc, project_root);
    btn.set_visible(applies);
    set_status_toggle(
        btn,
        label,
        "citations in count",
        applies && crate::word_cites::counts_citations(text),
    );
}

impl EditorPane {
    pub(super) fn toggle_count_citations(&self) {
        let (Some(path), Some(content)) = (self.get_active_path(), self.get_active_content())
        else {
            return;
        };
        let on = !crate::word_cites::counts_citations(&content);
        let new_content = crate::word_cites::set_counts_citations(&content, on);
        self.replace_buffer_content(&path, &new_content);
    }

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
            let cites = self.active_cites(&text);
            update_goal_ring(
                &self.goal_ring,
                &self.goal_fraction,
                &text,
                effective,
                cites.as_deref(),
            );
        }
    }

    fn active_cites(&self, text: &str) -> Option<std::sync::Arc<crate::word_cites::CitationWords>> {
        let path = self.get_active_path()?;
        cites_for(text, &path, self.project_root.borrow().as_deref())
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
        for (path, tab) in state.tabs.iter() {
            if self.notebook.page_num(&tab.notebook_page) == Some(current) {
                let (s, e) = tab.buffer.bounds();
                let text = tab.buffer.text(&s, &e, false);
                let cites = cites_for(&text, path, self.project_root.borrow().as_deref());
                let current_words = count_content_words_in(&text, cites.as_deref()) as i32;
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
        let cites_were_counted: Rc<Cell<bool>> = Rc::new(Cell::new(
            cites_for(content, &tab.path, self.project_root.borrow().as_deref()).is_some(),
        ));
        let cite_btn_for_change = self.cite_count_btn.clone();
        let cite_label_for_change = self.cite_count_label.clone();
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
                let root2 = project_root_for_wc.clone();
                let path2 = path_for_change.clone();
                let state2 = state_for_change.clone();
                let counted2 = cites_were_counted.clone();
                let cite_btn2 = cite_btn_for_change.clone();
                let cite_label2 = cite_label_for_change.clone();
                *wc_timer.borrow_mut() = Some(glib::timeout_add_local_once(
                    Duration::from_millis(300),
                    move || {
                        *t.borrow_mut() = None;
                        let (s, e) = buf2.bounds();
                        let text = buf2.text(&s, &e, false);
                        let root = root2.borrow().clone();
                        let cites = cites_for(&text, &path2, root.as_deref());
                        sync_cite_toggle(&cite_btn2, &cite_label2, &text, &path2, root.as_deref());
                        // Switching the toggle moves every count at once; shift the
                        // session's starting point by the same amount so "+N words"
                        // keeps reporting what was typed, not the citations.
                        if counted2.get() != cites.is_some() {
                            let with = count_content_words_in(&text, cites.as_deref()) as i64;
                            let without = count_content_words(&text) as i64;
                            let shift = if cites.is_some() {
                                with - without
                            } else {
                                without - with
                            };
                            let moved = (ss2.get() as i64 + shift).max(0) as u32;
                            ss2.set(moved);
                            if let Some(tab) = state2.borrow_mut().tabs.get_mut(&path2) {
                                tab.session_start_words = moved;
                            }
                            counted2.set(cites.is_some());
                        }
                        let goal = *goal_val2.borrow();
                        if goal > 0 {
                            let was_met = goal_was2.get();
                            update_goal_ring(&goal2, &goal_frac2, &text, goal, cites.as_deref());
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
                        let wc_str = wc_str_with_delta(&text, ss2.get(), cites.as_deref());
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
                let path_proj = path_for_change.clone();
                let buf_proj = buf.clone();
                let t = proj_wc_timer.clone();
                *proj_wc_timer.borrow_mut() = Some(glib::timeout_add_local_once(
                    Duration::from_millis(5000),
                    move || {
                        *t.borrow_mut() = None;
                        let root = root_proj.borrow().clone();
                        if let Some(root) = root {
                            let lbl = wc_lbl_proj.clone();
                            let (s, e) = buf_proj.bounds();
                            let text = buf_proj.text(&s, &e, false).to_string();
                            let doc = path_proj.clone();
                            glib::spawn_future_local(async move {
                                if let Ok(total) = gtk4::gio::spawn_blocking(move || {
                                    let cites = cites_for(&text, &doc, Some(&root));
                                    count_project_words(&root, cites.as_deref())
                                })
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
