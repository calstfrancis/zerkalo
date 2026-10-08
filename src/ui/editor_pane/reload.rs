//! Replacing an open file's text from disk without losing the cursor, scroll position or unsaved typing.

use super::*;

/// Replaces a buffer's whole text with `new_text`, touching only the stretch
/// that actually differs.
///
/// Deleting everything and typing it back looks the same on paper but isn't:
/// the cursor ends up at the very end of the document, every tag and mark is
/// gone, and — worst — the emptied view clamps its scroll position to zero, so
/// the writer finds themselves looking at line 1 of a document they were
/// halfway down. Whole-text rewrites come from Replace All, restoring a
/// snapshot, applying a style, a changed file on disk and more, so all of them
/// go through here. Everything outside the changed stretch keeps its place;
/// a cursor inside it lands at the same distance into the new text.
/// Which open tabs should take new text from disk: those whose saved file now
/// differs from the buffer and that hold no unsaved typing. The second list is
/// the tabs that differ but have unsaved typing, which are left untouched.
pub(super) fn tabs_to_refresh(
    tabs: &[(PathBuf, String)],
    modified: impl Fn(&std::path::Path) -> bool,
    read: impl Fn(&std::path::Path) -> Option<String>,
) -> (Vec<(PathBuf, String)>, Vec<PathBuf>) {
    let mut refresh = Vec::new();
    let mut skipped = Vec::new();
    for (path, buffer) in tabs {
        let Some(disk) = read(path) else { continue };
        if &disk == buffer {
            continue;
        }
        if modified(path) {
            skipped.push(path.clone());
        } else {
            refresh.push((path.clone(), disk));
        }
    }
    (refresh, skipped)
}

pub(super) fn replace_text_minimally(buffer: &Buffer, new_text: &str) {
    let (s, e) = buffer.bounds();
    // Hidden text included: simple mode makes the template invisible, and
    // dropping it here would delete it.
    let old = buffer.text(&s, &e, true);
    let Some((prefix, old_end, inserted)) = changed_span(&old, new_text) else {
        return;
    };
    let cursor = buffer.cursor_position().max(0) as usize;
    let inserted_len = inserted.chars().count();

    buffer.begin_user_action();
    if old_end > prefix {
        let mut from = buffer.iter_at_offset(prefix as i32);
        let mut to = buffer.iter_at_offset(old_end as i32);
        buffer.delete(&mut from, &mut to);
    }
    if !inserted.is_empty() {
        let mut at = buffer.iter_at_offset(prefix as i32);
        buffer.insert(&mut at, &inserted);
    }
    if cursor > prefix && cursor < old_end {
        let target = prefix + (cursor - prefix).min(inserted_len);
        buffer.place_cursor(&buffer.iter_at_offset(target as i32));
    }
    buffer.end_user_action();
}

/// The stretch of `old` that has to change to become `new`, as character
/// offsets: `(start, old_end, replacement)` — delete `start..old_end`, insert
/// `replacement` there. `None` when the two are identical.
pub(super) fn changed_span(old: &str, new: &str) -> Option<(usize, usize, String)> {
    if old == new {
        return None;
    }
    let old: Vec<char> = old.chars().collect();
    let new: Vec<char> = new.chars().collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    Some((
        prefix,
        old.len() - suffix,
        new[prefix..new.len() - suffix].iter().collect(),
    ))
}

impl EditorPane {
    /// Replaces a tab's whole text as one undoable step.
    pub(super) fn replace_buffer_content(&self, path: &PathBuf, new_content: &str) {
        // Clone the buffer before dropping the borrow; set_text fires
        // connect_changed which calls borrow_mut — holding the borrow here
        // causes a RefCell double-borrow panic.
        let buffer_opt = {
            let state = self.state.borrow();
            state.tabs.get(path).map(|tab| tab.buffer.clone())
        };
        if let Some(buffer) = buffer_opt {
            replace_text_minimally(&buffer, new_content);
            let sm = *self.simple_mode.borrow();
            apply_simple_mode_tag(&buffer, sm);
        }
    }

    /// Like `open_file` but forces a buffer refresh if the file is already open.
    pub fn reload_file(&self, path: PathBuf, content: &str) {
        self.reload_tab(path, content, true);
    }

    /// After files changed on disk underneath open tabs — a pull from GitHub, a
    /// replace with GitHub's copy — brings every open tab that has no unsaved
    /// typing up to date with what is now on disk, without moving the cursor
    /// or switching tabs. Without this an open document kept showing its old
    /// text, and the next save would have written that old text over what
    /// arrived. Returns the tabs refreshed and the ones left alone because
    /// they hold typing that isn't saved.
    pub fn refresh_open_from_disk(&self) -> (Vec<PathBuf>, Vec<PathBuf>) {
        let (refresh, skipped) = tabs_to_refresh(
            &self.all_tab_texts(),
            |p| self.is_modified(p),
            |p| std::fs::read_to_string(p).ok(),
        );
        let mut done = Vec::new();
        for (path, content) in refresh {
            self.reload_tab(path.clone(), &content, false);
            // The buffer now equals the file, so it holds nothing unsaved (the
            // text swap itself would otherwise mark the tab as edited).
            self.mark_saved(&path);
            done.push(path);
        }
        (done, skipped)
    }

    pub(super) fn reload_tab(&self, path: PathBuf, content: &str, focus: bool) {
        // Clone the buffer out before releasing the borrow — set_text fires
        // connect_changed which re-borrows state, causing a double-borrow panic.
        let existing = {
            let state = self.state.borrow();
            state
                .tabs
                .get(&path)
                .map(|tab| (tab.buffer.clone(), tab.notebook_page.clone()))
        };
        if let Some((buffer, scroll)) = existing {
            // The text on disk replaces the buffer's, so what was undoable no
            // longer applies — but only the changed stretch is touched, so the
            // cursor and scroll position stay where the writer left them.
            buffer.begin_irreversible_action();
            replace_text_minimally(&buffer, content);
            buffer.end_irreversible_action();
            buffer.set_modified(false);
            {
                let sm = *self.simple_mode.borrow();
                apply_simple_mode_tag(&buffer, sm);
            }
            if focus {
                if let Some(n) = self.notebook.page_num(&scroll) {
                    self.notebook.set_current_page(Some(n));
                }
            }
            return;
        }
        if focus {
            self.open_file(path, content);
        }
    }

    /// Replace only the preamble region of an already-open file, preserving the
    /// undo stack for everything below the body marker.
    pub fn splice_preamble(&self, path: PathBuf, full_new_content: &str) {
        pub(super) const BODY_MARKERS: &[&str] = &[
            "// \u{2500}\u{2500} Document body",
            "// \u{2500}\u{2500} Chapters",
        ];
        let existing = {
            let state = self.state.borrow();
            state
                .tabs
                .get(&path)
                .map(|tab| (tab.buffer.clone(), tab.notebook_page.clone()))
        };
        let Some((buffer, scroll)) = existing else {
            self.open_file(path, full_new_content);
            return;
        };
        if let Some(n) = self.notebook.page_num(&scroll) {
            self.notebook.set_current_page(Some(n));
        }
        let current_text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        let marker = BODY_MARKERS.iter().find(|m| current_text.contains(*m));
        match marker {
            Some(m) => {
                let body_byte = current_text.find(m).unwrap();
                let new_preamble = full_new_content
                    .find(m)
                    .map(|pos| &full_new_content[..pos])
                    .unwrap_or(full_new_content);
                let body: String = current_text[body_byte..].to_string();
                replace_text_minimally(&buffer, &format!("{new_preamble}{body}"));
                {
                    let sm = *self.simple_mode.borrow();
                    apply_simple_mode_tag(&buffer, sm);
                }
            }
            None => {
                replace_text_minimally(&buffer, full_new_content);
                {
                    let sm = *self.simple_mode.borrow();
                    apply_simple_mode_tag(&buffer, sm);
                }
            }
        }
    }
}
