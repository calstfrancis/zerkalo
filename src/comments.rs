//! Inline comment threads, anchored to a line in a document rather than
//! edited into the Typst source — a sidecar file (`<stem>.comments.toml`,
//! matching `template_dialog/sidecar.rs`'s `<stem>.zerkalo.toml` convention)
//! so a comment can never corrupt compiled output and survives export
//! round-trips untouched.
//!
//! v1 scope, per `KILLER-APP-PLAN.md` Phase 11's own design pass:
//! comments only (threaded, resolvable) — not suggested edits, which need a
//! diff/patch model this doesn't attempt.
//!
//! Phase 12 (`KILLER-APP-PLAN.md`) extends a `Comment` with an optional
//! [`Suggestion`] — a proposed insertion or deletion, the model Word's
//! `<w:ins>`/`<w:del>` track-changes runs map onto on DOCX import. Both
//! kinds keep their text visible in the document from the moment they're
//! imported (Typst has no track-changes rendering, so "review in context"
//! means the proposed text is just... there); accepting or rejecting is
//! what decides whether it stays. See [`suggestion_removes_text`].

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CommentReply {
    pub body: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum SuggestionKind {
    Insertion,
    Deletion,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub enum SuggestionStatus {
    #[default]
    Pending,
    Accepted,
    Rejected,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Suggestion {
    pub kind: SuggestionKind,
    /// The exact substring proposed for insertion or deletion, as it reads
    /// in the document right now — both kinds are inlined into the text on
    /// import so a reviewer sees them in context.
    pub text: String,
    #[serde(default)]
    pub status: SuggestionStatus,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Comment {
    pub id: u64,
    /// 1-indexed, matching every other line-number convention in this
    /// codebase (`outline_panel`, `jump_to_line`, etc.).
    pub anchor_line: u32,
    /// The anchor line's content at creation time — not just a length-N
    /// substring, the whole line — used to re-locate the comment if edits
    /// elsewhere in the document shift line numbers. See `relocate`.
    pub anchor_snippet: String,
    pub body: String,
    pub created_at: String,
    #[serde(default)]
    pub resolved: bool,
    #[serde(default)]
    pub replies: Vec<CommentReply>,
    #[serde(default)]
    pub suggestion: Option<Suggestion>,
}

/// What accepting or rejecting a suggestion means for the live document:
/// whether its `text` should now be removed. Both kinds start out visible
/// (inlined at import time) — an insertion is undone by *rejecting* it,
/// a deletion is carried out by *accepting* it. Pure and unit-tested on its
/// own since the actual removal happens in the GTK layer, which isn't.
pub fn suggestion_removes_text(kind: &SuggestionKind, accepted: bool) -> bool {
    match kind {
        SuggestionKind::Insertion => !accepted,
        SuggestionKind::Deletion => accepted,
    }
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct CommentThread {
    #[serde(default)]
    pub comments: Vec<Comment>,
    #[serde(default)]
    next_id: u64,
}

impl CommentThread {
    pub fn load(typ_path: &Path) -> Self {
        let path = sidecar_path(typ_path);
        match std::fs::read_to_string(&path) {
            Ok(s) => toml::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, typ_path: &Path) {
        let path = sidecar_path(typ_path);
        if let Ok(s) = toml::to_string_pretty(self) {
            crate::error::atomic_write(&path, s.as_bytes()).ok();
        }
    }

    pub fn add(&mut self, anchor_line: u32, anchor_snippet: String, body: String) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.comments.push(Comment {
            id,
            anchor_line,
            anchor_snippet,
            body,
            created_at: now_str(),
            resolved: false,
            replies: Vec::new(),
            suggestion: None,
        });
        id
    }

    /// Same shape as `add`, but attaches a pending [`Suggestion`] — used by
    /// DOCX import to turn a `<w:ins>`/`<w:del>` run into a reviewable entry.
    pub fn add_suggestion(
        &mut self,
        anchor_line: u32,
        anchor_snippet: String,
        kind: SuggestionKind,
        text: String,
        body: String,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.comments.push(Comment {
            id,
            anchor_line,
            anchor_snippet,
            body,
            created_at: now_str(),
            resolved: false,
            replies: Vec::new(),
            suggestion: Some(Suggestion {
                kind,
                text,
                status: SuggestionStatus::Pending,
            }),
        });
        id
    }

    /// Sets a suggestion's accept/reject status and, since a resolved
    /// suggestion has nothing left to review, marks the comment resolved
    /// too. No-op on a plain comment (`suggestion` is `None`).
    pub fn set_suggestion_status(&mut self, id: u64, status: SuggestionStatus) {
        if let Some(c) = self.comments.iter_mut().find(|c| c.id == id) {
            if let Some(s) = c.suggestion.as_mut() {
                s.status = status;
                c.resolved = true;
            }
        }
    }

    pub fn reply(&mut self, id: u64, body: String) {
        if let Some(c) = self.comments.iter_mut().find(|c| c.id == id) {
            c.replies.push(CommentReply {
                body,
                created_at: now_str(),
            });
        }
    }

    pub fn set_resolved(&mut self, id: u64, resolved: bool) {
        if let Some(c) = self.comments.iter_mut().find(|c| c.id == id) {
            c.resolved = resolved;
        }
    }

    pub fn delete(&mut self, id: u64) {
        self.comments.retain(|c| c.id != id);
    }

    /// Re-locates every comment's anchor against `content`'s current lines.
    /// Returns the ids of comments whose snippet could no longer be found
    /// anywhere in the document (its line kept, best-effort, but the
    /// caller should show these visibly as "anchor lost" rather than
    /// silently trusting a stale line number).
    pub fn relocate_all(&mut self, content: &str) -> Vec<u64> {
        let lines: Vec<&str> = content.lines().collect();
        let mut lost = Vec::new();
        for c in &mut self.comments {
            match relocate(&lines, c.anchor_line, &c.anchor_snippet) {
                Some(new_line) => c.anchor_line = new_line,
                None => lost.push(c.id),
            }
        }
        lost
    }
}

pub fn sidecar_path(typ_path: &Path) -> PathBuf {
    let stem = typ_path.file_stem().unwrap_or_default();
    let dir = typ_path.parent().unwrap_or_else(|| Path::new("."));
    dir.join(format!("{}.comments.toml", stem.to_string_lossy()))
}

fn now_str() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
}

/// Finds the best current line for `snippet`, searching outward from
/// `hint_line` (cheap, and correctly biased toward the nearest match when
/// the same line text recurs elsewhere in the document) before falling back
/// to a full-document scan. Returns `None` if `snippet` can't be found
/// anywhere — e.g. the commented text was deleted — rather than guessing.
fn relocate(lines: &[&str], hint_line: u32, snippet: &str) -> Option<u32> {
    if lines.is_empty() {
        return None;
    }
    if snippet.is_empty() {
        return Some(hint_line.clamp(1, lines.len() as u32));
    }

    let hint_idx = hint_line.saturating_sub(1) as usize;
    if let Some(line) = lines.get(hint_idx) {
        if *line == snippet {
            return Some(hint_line);
        }
    }

    // Outward search from the hint, alternating sides, so the nearest match
    // to where the comment used to be wins over a farther-away duplicate.
    let mut offset: usize = 1;
    loop {
        let before = hint_idx.checked_sub(offset);
        let after = hint_idx + offset;
        let mut any_in_range = false;

        if let Some(i) = before {
            any_in_range = true;
            if lines[i] == snippet {
                return Some((i + 1) as u32);
            }
        }
        if after < lines.len() {
            any_in_range = true;
            if lines[after] == snippet {
                return Some((after + 1) as u32);
            }
        }
        if !any_in_range {
            return None;
        }
        offset += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_assigns_increasing_ids_and_defaults_unresolved() {
        let mut t = CommentThread::default();
        let id1 = t.add(3, "= Intro".into(), "needs a citation".into());
        let id2 = t.add(10, "more text".into(), "rephrase this".into());
        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert!(!t.comments[0].resolved);
        assert!(!t.comments[1].resolved);
    }

    #[test]
    fn reply_appends_to_the_right_comment_only() {
        let mut t = CommentThread::default();
        let id1 = t.add(1, "a".into(), "first".into());
        let id2 = t.add(2, "b".into(), "second".into());
        t.reply(id2, "responding".into());
        assert!(t
            .comments
            .iter()
            .find(|c| c.id == id1)
            .unwrap()
            .replies
            .is_empty());
        assert_eq!(
            t.comments
                .iter()
                .find(|c| c.id == id2)
                .unwrap()
                .replies
                .len(),
            1
        );
    }

    #[test]
    fn set_resolved_toggles_only_the_target_comment() {
        let mut t = CommentThread::default();
        let id1 = t.add(1, "a".into(), "x".into());
        let id2 = t.add(2, "b".into(), "y".into());
        t.set_resolved(id1, true);
        assert!(t.comments.iter().find(|c| c.id == id1).unwrap().resolved);
        assert!(!t.comments.iter().find(|c| c.id == id2).unwrap().resolved);
    }

    #[test]
    fn delete_removes_only_the_target_comment() {
        let mut t = CommentThread::default();
        let id1 = t.add(1, "a".into(), "x".into());
        let id2 = t.add(2, "b".into(), "y".into());
        t.delete(id1);
        assert_eq!(t.comments.len(), 1);
        assert_eq!(t.comments[0].id, id2);
    }

    #[test]
    fn sidecar_round_trips_through_toml_including_nested_replies() {
        let dir = tempfile::tempdir().unwrap();
        let typ_path = dir.path().join("doc.typ");
        std::fs::write(&typ_path, "content").unwrap();

        let mut t = CommentThread::default();
        let id = t.add(5, "= Section".into(), "clarify this".into());
        t.reply(id, "done, see rev 2".into());
        t.set_resolved(id, true);
        t.save(&typ_path);

        let loaded = CommentThread::load(&typ_path);
        assert_eq!(loaded.comments.len(), 1);
        assert_eq!(loaded.comments[0].body, "clarify this");
        assert_eq!(loaded.comments[0].replies.len(), 1);
        assert_eq!(loaded.comments[0].replies[0].body, "done, see rev 2");
        assert!(loaded.comments[0].resolved);
    }

    #[test]
    fn sidecar_path_matches_the_stem_dot_comments_dot_toml_convention() {
        let path = sidecar_path(Path::new("/a/b/document.typ"));
        assert_eq!(path, PathBuf::from("/a/b/document.comments.toml"));
    }

    #[test]
    fn load_of_a_missing_sidecar_returns_an_empty_thread_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let typ_path = dir.path().join("nope.typ");
        let t = CommentThread::load(&typ_path);
        assert!(t.comments.is_empty());
    }

    #[test]
    fn relocate_keeps_the_line_when_unchanged() {
        let lines = vec!["a", "b", "c"];
        assert_eq!(relocate(&lines, 2, "b"), Some(2));
    }

    #[test]
    fn relocate_finds_the_nearest_match_when_lines_shift() {
        // "target" moved from line 3 to line 5 (two lines inserted above it).
        let lines = vec!["x", "y", "new1", "new2", "target", "z"];
        assert_eq!(relocate(&lines, 3, "target"), Some(5));
    }

    #[test]
    fn relocate_prefers_the_nearer_of_two_identical_lines() {
        let lines = vec!["dup", "a", "b", "dup", "c"];
        // Hint was line 1 (the first "dup"); a line shift means the actual
        // comment now belongs to whichever "dup" is closer to the hint.
        assert_eq!(relocate(&lines, 1, "dup"), Some(1));
    }

    #[test]
    fn relocate_returns_none_when_the_commented_text_is_gone() {
        let lines = vec!["completely", "different", "content"];
        assert_eq!(relocate(&lines, 2, "the original line"), None);
    }

    #[test]
    fn relocate_on_an_empty_document_returns_none() {
        let lines: Vec<&str> = vec![];
        assert_eq!(relocate(&lines, 1, "anything"), None);
    }

    #[test]
    fn add_suggestion_attaches_a_pending_suggestion() {
        let mut t = CommentThread::default();
        let id = t.add_suggestion(
            4,
            "line text".into(),
            SuggestionKind::Insertion,
            "new text".into(),
            String::new(),
        );
        let c = t.comments.iter().find(|c| c.id == id).unwrap();
        let s = c.suggestion.as_ref().unwrap();
        assert_eq!(s.kind, SuggestionKind::Insertion);
        assert_eq!(s.text, "new text");
        assert_eq!(s.status, SuggestionStatus::Pending);
        assert!(!c.resolved);
    }

    #[test]
    fn plain_comments_have_no_suggestion() {
        let mut t = CommentThread::default();
        let id = t.add(1, "a".into(), "just a note".into());
        assert!(t
            .comments
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .suggestion
            .is_none());
    }

    #[test]
    fn set_suggestion_status_resolves_the_comment_too() {
        let mut t = CommentThread::default();
        let id = t.add_suggestion(
            1,
            "x".into(),
            SuggestionKind::Deletion,
            "old text".into(),
            String::new(),
        );
        t.set_suggestion_status(id, SuggestionStatus::Accepted);
        let c = t.comments.iter().find(|c| c.id == id).unwrap();
        assert_eq!(
            c.suggestion.as_ref().unwrap().status,
            SuggestionStatus::Accepted
        );
        assert!(c.resolved);
    }

    #[test]
    fn set_suggestion_status_is_a_noop_on_a_plain_comment() {
        let mut t = CommentThread::default();
        let id = t.add(1, "x".into(), "note".into());
        t.set_suggestion_status(id, SuggestionStatus::Accepted);
        assert!(!t.comments.iter().find(|c| c.id == id).unwrap().resolved);
    }

    #[test]
    fn suggestion_sidecar_round_trips_through_toml() {
        let dir = tempfile::tempdir().unwrap();
        let typ_path = dir.path().join("doc.typ");
        std::fs::write(&typ_path, "content").unwrap();

        let mut t = CommentThread::default();
        let id = t.add_suggestion(
            2,
            "= Intro".into(),
            SuggestionKind::Deletion,
            "obsolete phrase".into(),
            String::new(),
        );
        t.set_suggestion_status(id, SuggestionStatus::Rejected);
        t.save(&typ_path);

        let loaded = CommentThread::load(&typ_path);
        let s = loaded.comments[0].suggestion.as_ref().unwrap();
        assert_eq!(s.kind, SuggestionKind::Deletion);
        assert_eq!(s.text, "obsolete phrase");
        assert_eq!(s.status, SuggestionStatus::Rejected);
    }

    #[test]
    fn accepting_an_insertion_keeps_it_but_rejecting_removes_it() {
        assert!(!suggestion_removes_text(&SuggestionKind::Insertion, true));
        assert!(suggestion_removes_text(&SuggestionKind::Insertion, false));
    }

    #[test]
    fn accepting_a_deletion_removes_it_but_rejecting_keeps_it() {
        assert!(suggestion_removes_text(&SuggestionKind::Deletion, true));
        assert!(!suggestion_removes_text(&SuggestionKind::Deletion, false));
    }

    #[test]
    fn relocate_all_reports_lost_anchors_and_updates_found_ones() {
        let mut t = CommentThread::default();
        let found_id = t.add(1, "keep me".into(), "still here".into());
        let lost_id = t.add(2, "delete me".into(), "gone".into());

        let lost = t.relocate_all("intro\nkeep me\nmore text");
        assert_eq!(lost, vec![lost_id]);
        assert_eq!(
            t.comments
                .iter()
                .find(|c| c.id == found_id)
                .unwrap()
                .anchor_line,
            2
        );
    }
}
