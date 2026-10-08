//! Saving: writing buffers to disk, the modified state, and the save-problem list.

use super::*;

impl EditorPane {
    /// Whether any open document has edits not yet written to disk.
    pub fn any_modified(&self) -> bool {
        self.state.borrow().tabs.values().any(|t| t.modified)
    }

    pub fn mark_saved(&self, path: &PathBuf) {
        let widgets = {
            let mut state = self.state.borrow_mut();
            state.tabs.get_mut(path).map(|tab| {
                tab.modified = false;
                (tab.dot_label.clone(), tab.buffer.clone())
            })
        };
        if let Some((dot_label, buffer)) = widgets {
            dot_label.set_visible(false);
            buffer.set_modified(false);
        }
        if let Some(f) = self.on_modified_changed.borrow().as_ref() {
            f(false);
        }
        if let Some(f) = self.on_file_dirty.borrow().as_ref() {
            f(path.clone(), false);
        }
        self.note_save_result(path.clone(), None);
    }

    /// Whether the tab for `path` has unsaved modifications.
    pub fn is_modified(&self, path: &std::path::Path) -> bool {
        self.state
            .borrow()
            .tabs
            .get(path)
            .map(|t| t.modified)
            .unwrap_or(false)
    }

    /// Returns (path, content) for every tab that has unsaved modifications.
    pub fn modified_buffers(&self) -> Vec<(PathBuf, String)> {
        let state = self.state.borrow();
        state
            .tabs
            .iter()
            .filter(|(_, tab)| tab.modified)
            .map(|(path, tab)| {
                let (s, e) = tab.buffer.bounds();
                (path.clone(), tab.buffer.text(&s, &e, true).to_string())
            })
            .collect()
    }

    /// Returns the paths that failed to write, so callers with a data-loss
    /// stake (e.g. closing the window on "Save All") can tell the difference
    /// between "everything saved" and "some writes failed" instead of
    /// proceeding as if the save silently succeeded.
    pub fn save_all_modified(&self) -> Vec<PathBuf> {
        let mut outcomes: Vec<(PathBuf, Option<String>)> = Vec::new();
        let (saved, failed): (Vec<(TabMark, PathBuf, Buffer)>, Vec<PathBuf>) = {
            let mut state = self.state.borrow_mut();
            let mut saved = Vec::new();
            let mut failed = Vec::new();
            for (path, tab) in state.tabs.iter_mut() {
                if !tab.modified {
                    continue;
                }
                let (start, end) = tab.buffer.bounds();
                let content = tab.buffer.text(&start, &end, true);
                let write = crate::error::atomic_write(path, content.as_bytes());
                outcomes.push((
                    path.clone(),
                    write.as_ref().err().map(crate::error::io_reason),
                ));
                if write.is_ok() {
                    tab.modified = false;
                    saved.push((tab.dot_label.clone(), path.clone(), tab.buffer.clone()));
                } else {
                    failed.push(path.clone());
                }
            }
            (saved, failed)
        };
        self.prune_save_problems();
        for (path, reason) in outcomes {
            self.note_save_result(path, reason);
        }
        let any_saved = !saved.is_empty();
        for (dot_label, path, buffer) in saved {
            dot_label.set_visible(false);
            buffer.set_modified(false);
            crate::auto_save::clear(&path);
            // Same notifications a manual save sends, so the window's unsaved
            // state and the backup badge follow a save made from here too.
            if let Some(f) = self.on_file_dirty.borrow().as_ref() {
                f(path.clone(), false);
            }
        }
        if any_saved && self.get_active_path().is_none_or(|p| !self.is_modified(&p)) {
            if let Some(f) = self.on_modified_changed.borrow().as_ref() {
                f(false);
            }
        }
        failed
    }

    /// `Ok(None)` means there was nothing to save (no active document);
    /// `Err` means a save was attempted and the write itself failed — the
    /// two must not be conflated, or a write failure looks identical to the
    /// normal no-op case and the caller has nothing to show the user.
    pub fn save_current(&self) -> std::io::Result<Option<PathBuf>> {
        let Some(path) = self.get_active_path() else {
            return Ok(None);
        };
        let Some(content) = self.get_active_content() else {
            return Ok(None);
        };
        if let Err(e) = crate::error::atomic_write(&path, content.as_bytes()) {
            self.note_save_result(path, Some(crate::error::io_reason(&e)));
            return Err(e);
        }
        crate::auto_save::clear(&path);
        self.mark_saved(&path);
        Ok(Some(path))
    }

    /// Records that saving `path` failed (`Some(reason)`) or worked (`None`),
    /// and tells the listener only when the set of problems actually changed.
    pub(super) fn note_save_result(&self, path: PathBuf, reason: Option<String>) {
        let changed = {
            let mut problems = self.save_problems.borrow_mut();
            let before = problems.clone();
            problems.retain(|(p, _)| *p != path);
            if let Some(reason) = reason {
                problems.push((path, reason));
            }
            *problems != before
        };
        if changed {
            self.announce_save_problems();
        }
    }

    pub(super) fn prune_save_problems(&self) {
        let changed = {
            let state = self.state.borrow();
            let mut problems = self.save_problems.borrow_mut();
            let before = problems.len();
            problems.retain(|(p, _)| state.tabs.contains_key(p));
            problems.len() != before
        };
        if changed {
            self.announce_save_problems();
        }
    }

    pub(super) fn announce_save_problems(&self) {
        let problems = self.save_problems.borrow().clone();
        if let Some(f) = self.on_save_problems.borrow().as_ref() {
            f(&problems);
        }
    }

    pub fn set_on_save_problems(&self, f: impl Fn(&[(PathBuf, String)]) + 'static) {
        *self.on_save_problems.borrow_mut() = Some(Box::new(f));
    }

    pub fn save_problems(&self) -> Vec<(PathBuf, String)> {
        self.prune_save_problems();
        self.save_problems.borrow().clone()
    }
}
