//! Find and replace in the active buffer, with every match highlighted.

use super::*;

pub(super) fn ensure_search_tag(buffer: &Buffer) {
    let table = buffer.tag_table();
    if table.lookup("zerkalo-search-current").is_none() {
        let tag = TextTag::new(Some("zerkalo-search-current"));
        tag.set_background_rgba(Some(&gtk4::gdk::RGBA::new(1.0, 0.6, 0.0, 0.65)));
        tag.set_foreground_rgba(Some(&gtk4::gdk::RGBA::new(0.0, 0.0, 0.0, 1.0)));
        table.add(&tag);
        // Newly-added tags already get top priority, but make it explicit so
        // it stays visible even if another tag is added after this one later.
        tag.set_priority(table.size() - 1);
    }
}

impl EditorPane {
    pub fn toggle_find(&self) {
        self.find_bar.toggle();
    }

    pub fn do_find(&self, text: &str, forward: bool) {
        if text.is_empty() {
            self.find_bar.set_result("");
            return;
        }
        let Some((view, buffer)) = self.active_view_buffer() else {
            return;
        };
        let case_sensitive = self.find_bar.is_case_sensitive();
        let whole_word = self.find_bar.is_whole_word();
        let regex_mode = self.find_bar.is_regex_mode();
        let cursor_pos = buffer.cursor_position();

        let matches: Vec<(i32, i32)> = if regex_mode || whole_word {
            let full_text = buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), false)
                .to_string();
            let pattern = if whole_word {
                format!("\\b{}\\b", regex::escape(text))
            } else {
                text.to_string()
            };
            let re_result = if case_sensitive {
                regex::Regex::new(&pattern)
            } else {
                regex::Regex::new(&format!("(?i){}", pattern))
            };
            match re_result {
                Err(_) => {
                    self.find_bar.set_entry_error(true);
                    self.find_bar.set_result("That pattern isn't quite right");
                    return;
                }
                Ok(re) => {
                    self.find_bar.set_entry_error(false);
                    re.find_iter(&full_text)
                        .map(|m| {
                            let char_start = full_text[..m.start()].chars().count() as i32;
                            let char_end = full_text[..m.end()].chars().count() as i32;
                            (char_start, char_end)
                        })
                        .collect()
                }
            }
        } else {
            let flags = if case_sensitive {
                TextSearchFlags::TEXT_ONLY
            } else {
                TextSearchFlags::TEXT_ONLY | TextSearchFlags::CASE_INSENSITIVE
            };
            let mut v = Vec::new();
            let mut it = buffer.start_iter();
            while let Some((s, e)) = it.forward_search(text, flags, None) {
                let advance = e;
                v.push((s.offset(), e.offset()));
                it = advance;
            }
            self.find_bar.set_entry_error(false);
            v
        };

        if matches.is_empty() {
            self.find_bar.set_result("No results");
            return;
        }

        // Pick the next (or previous) match relative to cursor, with wrap-around
        let idx = if forward {
            matches
                .iter()
                .position(|(s, _)| *s > cursor_pos)
                .unwrap_or(0)
        } else {
            matches
                .iter()
                .rposition(|(_, e)| *e < cursor_pos)
                .unwrap_or(matches.len() - 1)
        };

        self.highlight_all_matches(&buffer, text, case_sensitive, whole_word, regex_mode);

        let (start_off, end_off) = matches[idx];
        let start = buffer.iter_at_offset(start_off);
        let end = buffer.iter_at_offset(end_off);
        // Place insertion cursor at start so next forward search skips past current match
        buffer.select_range(&end, &start);

        // A bright background tag makes the current match obvious even where the
        // native selection color is low-contrast against the editor theme.
        ensure_search_tag(&buffer);
        let (buf_start, buf_end) = buffer.bounds();
        buffer.remove_tag_by_name("zerkalo-search-current", &buf_start, &buf_end);
        buffer.apply_tag_by_name("zerkalo-search-current", &start, &end);

        // scroll_to_iter can silently no-op if the view hasn't validated line
        // heights for this part of the buffer yet (a known GTK timing issue) —
        // deferring to the next idle iteration, after layout has settled, and
        // scrolling via a mark (which survives that iteration) makes this
        // reliable. use_align + yalign 0.5 centers the match instead of just
        // nudging it into view at the edge.
        let mark = buffer.create_mark(None::<&str>, &start, true);
        let view_idle = view.clone();
        let buffer_idle = buffer.clone();
        glib::idle_add_local_once(move || {
            view_idle.scroll_to_mark(&mark, 0.0, true, 0.0, 0.5);
            buffer_idle.delete_mark(&mark);
        });

        self.find_bar
            .set_result(&format!("{} of {}", idx + 1, matches.len()));
    }

    pub(super) fn highlight_all_matches(
        &self,
        buffer: &Buffer,
        text: &str,
        case_sensitive: bool,
        whole_word: bool,
        regex_mode: bool,
    ) {
        let mut slot = self.search_context.borrow_mut();
        let reuse = slot
            .as_ref()
            .is_some_and(|c| c.buffer().as_ptr() == buffer.as_ptr());
        if !reuse {
            *slot = Some(sourceview5::SearchContext::new(
                buffer,
                None::<&sourceview5::SearchSettings>,
            ));
        }
        let Some(ctx) = slot.as_ref() else { return };
        let settings = ctx.settings();
        settings.set_search_text(Some(text));
        settings.set_case_sensitive(case_sensitive);
        settings.set_at_word_boundaries(whole_word);
        settings.set_regex_enabled(regex_mode);
        settings.set_wrap_around(true);
        ctx.set_highlight(true);
    }

    pub fn clear_search_highlight(&self) {
        *self.search_context.borrow_mut() = None;
        if let Some((_, buffer)) = self.active_view_buffer() {
            let (start, end) = buffer.bounds();
            buffer.remove_tag_by_name("zerkalo-search-current", &start, &end);
        }
    }

    pub fn do_replace_one(&self, find: &str, replace: &str) {
        if find.is_empty() {
            return;
        }
        let Some((_view, buffer)) = self.active_view_buffer() else {
            return;
        };
        let case_sensitive = self.find_bar.is_case_sensitive();
        if let Some((sel_start, sel_end)) = buffer.selection_bounds() {
            let selected = buffer.text(&sel_start, &sel_end, false).to_string();
            let matches = if case_sensitive {
                selected == find
            } else {
                selected.to_lowercase() == find.to_lowercase()
            };
            if matches {
                let offset = sel_start.offset();
                let mut s = sel_start;
                let mut e = sel_end;
                buffer.begin_user_action();
                buffer.delete(&mut s, &mut e);
                let mut ins = buffer.iter_at_offset(offset);
                buffer.insert(&mut ins, replace);
                buffer.end_user_action();
            }
        }
        self.do_find(find, true);
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

    pub fn do_replace_all(&self, find: &str, replace: &str) {
        if find.is_empty() {
            return;
        }
        let Some((_view, buffer)) = self.active_view_buffer() else {
            return;
        };
        let case_sensitive = self.find_bar.is_case_sensitive();
        let whole_word = self.find_bar.is_whole_word();
        let regex_mode = self.find_bar.is_regex_mode();
        let mut count: usize = 0;

        if regex_mode || whole_word {
            // include_hidden_chars=true: simple mode marks the preamble invisible; without
            // this flag buf.text() drops it and the full-buffer delete+reinsert wipes it.
            let full_text = buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), true)
                .to_string();
            let pattern = if whole_word {
                format!("\\b{}\\b", regex::escape(find))
            } else {
                find.to_string()
            };
            let re_result = if case_sensitive {
                regex::Regex::new(&pattern)
            } else {
                regex::Regex::new(&format!("(?i){}", pattern))
            };
            match re_result {
                Err(_) => {
                    self.find_bar.set_entry_error(true);
                    self.find_bar.set_result("That pattern isn't quite right");
                    return;
                }
                Ok(re) => {
                    self.find_bar.set_entry_error(false);
                    let new_text = re.replace_all(&full_text, replace);
                    count = re.find_iter(&full_text).count();
                    replace_text_minimally(&buffer, &new_text);
                    let sm = *self.simple_mode.borrow();
                    apply_simple_mode_tag(&buffer, sm);
                }
            }
        } else {
            let flags = if case_sensitive {
                TextSearchFlags::TEXT_ONLY
            } else {
                TextSearchFlags::TEXT_ONLY | TextSearchFlags::CASE_INSENSITIVE
            };
            self.find_bar.set_entry_error(false);
            buffer.begin_user_action();
            let mut iter = buffer.start_iter();
            while let Some((mut start, mut end)) = iter.forward_search(find, flags, None) {
                let offset = start.offset();
                buffer.delete(&mut start, &mut end);
                let mut ins = buffer.iter_at_offset(offset);
                buffer.insert(&mut ins, replace);
                iter = buffer.iter_at_offset(offset + replace.chars().count() as i32);
                count += 1;
            }
            buffer.end_user_action();
        }
        self.find_bar.set_result(&format!("Replaced {count}"));
    }
}
