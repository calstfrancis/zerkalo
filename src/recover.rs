//! Getting something back: the logic behind ☰ → **Recover…**.
//!
//! Zerkalo keeps several safety nets — a copy each time you save, the history
//! that is backed up online, the Trash, saved copies made when you replace this
//! computer's files with GitHub's. This module gathers them into answers a person
//! can act on: one timeline of earlier versions of a document, the saved copies
//! and what is in them, and a plain statement of how safe the work is.
//!
//! Nothing here ever changes a document. Bringing something back always makes a
//! **new file beside the original** (`restore_copy_path`); replacing the text in
//! the editor is the caller's choice, and goes through the editor's undo.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Datelike, Local, NaiveDateTime, TimeZone};

use crate::git_sync::{git_cmd, git_ok, has_commits, head_branch, primary_remote};

// ── Earlier versions of one document ─────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A copy Zerkalo made when you saved, kept on this computer.
    Saved,
    /// A version in the history that hasn't been backed up online yet.
    OnThisComputer,
    /// A version in the history that is safely backed up online.
    BackedUp,
}

impl Kind {
    pub fn label(&self) -> &'static str {
        match self {
            Kind::Saved => "Saved on this computer",
            Kind::OnThisComputer => "Kept on this computer",
            Kind::BackedUp => "Backed up online",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Version {
    /// Seconds since 1970.
    pub when: i64,
    pub kind: Kind,
    /// The snapshot file, or the commit hash.
    pub id: String,
}

/// Parses a snapshot file stem (`20261003T161005.123`) as a local time.
fn snapshot_time(stem: &str) -> Option<i64> {
    let naive = NaiveDateTime::parse_from_str(stem.get(..15)?, "%Y%m%dT%H%M%S").ok()?;
    Local
        .from_local_datetime(&naive)
        .earliest()
        .map(|d| d.timestamp())
}

/// Every earlier version of `file` that can be found, newest first: the copies
/// made on each save, and the history of the folder (labelled by whether it has
/// reached the online backup).
pub fn versions(project_root: &Path, file: &Path) -> Vec<Version> {
    let mut out: Vec<Version> = Vec::new();

    let dir = crate::ui::snapshot_dialog::snapshot_dir(project_root, file);
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("typ") {
                continue;
            }
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if let Some(when) = snapshot_time(stem) {
                out.push(Version {
                    when,
                    kind: Kind::Saved,
                    id: p.to_string_lossy().into_owned(),
                });
            }
        }
    }

    let Some(parent) = file.parent() else {
        out.sort_by_key(|v| std::cmp::Reverse(v.when));
        return out;
    };
    let name = file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    if let Some(log) = git_ok(
        parent,
        &["log", "-n", "200", "--format=%H%x1f%ct", "--", name],
    ) {
        // What hasn't been sent online yet.
        let unpushed: std::collections::HashSet<String> = primary_remote(parent)
            .and_then(|remote| {
                let branch = head_branch(parent);
                git_ok(parent, &["rev-list", &format!("{remote}/{branch}..HEAD")])
            })
            .map(|t| t.lines().map(str::to_string).collect())
            .unwrap_or_else(|| {
                // No online copy to compare with: nothing is backed up.
                git_ok(parent, &["rev-list", "HEAD"])
                    .map(|t| t.lines().map(str::to_string).collect())
                    .unwrap_or_default()
            });
        for line in log.lines() {
            let mut parts = line.split('\u{1f}');
            let (Some(hash), Some(ts)) = (parts.next(), parts.next()) else {
                continue;
            };
            let Ok(when) = ts.parse::<i64>() else {
                continue;
            };
            out.push(Version {
                when,
                kind: if unpushed.contains(hash) {
                    Kind::OnThisComputer
                } else {
                    Kind::BackedUp
                },
                id: hash.to_string(),
            });
        }
    }

    out.sort_by_key(|v| std::cmp::Reverse(v.when));
    out
}

/// The text of `version` of `file`.
pub fn text_of(version: &Version, file: &Path) -> Option<String> {
    match version.kind {
        Kind::Saved => std::fs::read_to_string(&version.id).ok(),
        Kind::OnThisComputer | Kind::BackedUp => {
            let parent = file.parent()?;
            let name = file.file_name()?.to_str()?;
            let out = git_cmd(parent)
                .args(["show", &format!("{}:./{name}", version.id)])
                .output()
                .ok()
                .filter(|o| o.status.success())?;
            Some(String::from_utf8_lossy(&out.stdout).into_owned())
        }
    }
}

// ── Wording ──────────────────────────────────────────────────────────────────

/// "Today 4:10 pm", "Yesterday 9:02 am", "Mon 2:15 pm", "28 Sep, 2:15 pm".
pub fn friendly_when(ts: i64, now: DateTime<Local>) -> String {
    let Some(when) = Local.timestamp_opt(ts, 0).earliest() else {
        return String::new();
    };
    let clock = when.format("%-I:%M %P").to_string();
    let days = now
        .date_naive()
        .signed_duration_since(when.date_naive())
        .num_days();
    match days {
        0 => format!("Today {clock}"),
        1 => format!("Yesterday {clock}"),
        2..=6 => format!("{} {clock}", when.format("%a")),
        _ if when.year() == now.year() => format!("{}, {clock}", when.format("%-d %b")),
        _ => format!("{}, {clock}", when.format("%-d %b %Y")),
    }
}

fn word_counts(text: &str) -> HashMap<String, i64> {
    let mut m: HashMap<String, i64> = HashMap::new();
    for w in text.split_whitespace() {
        *m.entry(crate::cite_search::fold(w)).or_default() += 1;
    }
    m
}

/// How `version` differs from `current`, in words: (would come back, would go).
pub fn word_changes(version: &str, current: &str) -> (usize, usize) {
    let (v, c) = (word_counts(version), word_counts(current));
    let back: i64 = v
        .iter()
        .map(|(w, n)| (n - c.get(w).copied().unwrap_or(0)).max(0))
        .sum();
    let gone: i64 = c
        .iter()
        .map(|(w, n)| (n - v.get(w).copied().unwrap_or(0)).max(0))
        .sum();
    (back as usize, gone as usize)
}

/// "Brings back about 120 words and takes out about 12", or that it's identical.
pub fn describe_changes(version: &str, current: &str) -> String {
    let (back, gone) = word_changes(version, current);
    let words = |n: usize| format!("{n} word{}", if n == 1 { "" } else { "s" });
    match (back, gone) {
        (0, 0) => "Same words as the document now.".to_string(),
        (b, 0) => format!("Brings back {}.", words(b)),
        (0, g) => format!("Takes out {} that are in the document now.", words(g)),
        (b, g) => format!(
            "Brings back {} and takes out {} that are in the document now.",
            words(b),
            words(g)
        ),
    }
}

/// Whether `text` contains every word of `query` (any order, case and accents
/// ignored) — how "find where that paragraph went" matches earlier versions.
pub fn text_matches(text: &str, query: &str) -> bool {
    let words: Vec<String> = query
        .split_whitespace()
        .map(crate::cite_search::fold)
        .collect();
    if words.is_empty() {
        return true;
    }
    let hay = crate::cite_search::fold(text);
    words.iter().all(|w| hay.contains(w.as_str()))
}

// ── Bringing something back, as a copy ───────────────────────────────────────

fn unique_beside(file: &Path, stem_suffix: &str) -> PathBuf {
    let dir = file.parent().unwrap_or(Path::new("."));
    let stem = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("typ");
    let first = dir.join(format!("{stem} ({stem_suffix}).{ext}"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{stem} ({stem_suffix} {n}).{ext}")))
        .find(|p| !p.exists())
        .expect("a free name exists")
}

/// A name for the copy of an earlier version: beside the original, never an
/// existing file — `essay (earlier 3 Oct 1610).typ`.
pub fn restore_copy_path(file: &Path, when: i64) -> PathBuf {
    let stamp = Local
        .timestamp_opt(when, 0)
        .earliest()
        .map(|d| d.format("%-d %b %H%M").to_string())
        .unwrap_or_else(|| "earlier".to_string());
    unique_beside(file, &format!("earlier {stamp}"))
}

/// Writes `text` as a new file beside `file`. The original is untouched.
pub fn bring_back_as_copy(file: &Path, when: i64, text: &str) -> std::io::Result<PathBuf> {
    let path = restore_copy_path(file, when);
    crate::bibliography::write_atomic(&path, text)?;
    Ok(path)
}

// ── Saved copies (made when replacing this computer's files with GitHub's) ──

#[derive(Clone, Debug)]
pub struct SavedCopy {
    pub branch: String,
    pub when: i64,
    /// Files (relative to the folder) that differ from what is here now.
    pub files: Vec<String>,
}

fn repo_root(dir: &Path) -> Option<PathBuf> {
    git_ok(dir, &["rev-parse", "--show-toplevel"]).map(PathBuf::from)
}

/// The saved copies Zerkalo made before replacing this computer's files with
/// GitHub's, newest first, with the files in each that differ from now.
pub fn saved_copies(project_root: &Path) -> Vec<SavedCopy> {
    if !has_commits(project_root) {
        return Vec::new();
    }
    let Some(list) = git_ok(
        project_root,
        &[
            "for-each-ref",
            "--sort=-creatordate",
            "--format=%(refname:short)%1f%(creatordate:unix)",
            "refs/heads/before-replace-*",
        ],
    ) else {
        return Vec::new();
    };
    list.lines()
        .filter_map(|line| {
            let mut p = line.split('\u{1f}');
            let branch = p.next()?.to_string();
            let when = p.next()?.parse().ok()?;
            let files = git_ok(project_root, &["diff", "--name-only", "HEAD", &branch])
                .map(|t| {
                    t.lines()
                        .filter(|f| !f.starts_with(".zerkalo/") && !f.starts_with('.'))
                        .take(50)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            Some(SavedCopy {
                branch,
                when,
                files,
            })
        })
        .collect()
}

/// Brings one file out of a saved copy as a new file beside where it lives (or
/// lived). Nothing existing is touched.
pub fn bring_back_from_saved_copy(
    project_root: &Path,
    branch: &str,
    relative: &str,
) -> Result<PathBuf, String> {
    let root = repo_root(project_root).ok_or("This folder isn't a backup folder.")?;
    let out = git_cmd(&root)
        .args(["show", &format!("{branch}:{relative}")])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("That file isn't in the saved copy.".into());
    }
    let target = root.join(relative);
    let path = unique_beside(&target, "recovered");
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    crate::bibliography::write_atomic(&path, &String::from_utf8_lossy(&out.stdout))
        .map_err(|e| e.to_string())?;
    Ok(path)
}

// ── How safe is the work? ────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// There is an online backup set up for this folder.
    pub connected: bool,
    /// Versions kept here that haven't been sent online.
    pub waiting: usize,
    /// Files with edits not yet kept as a version.
    pub loose: usize,
    /// When the online copy was last added to (seconds since 1970).
    pub last_backup: Option<i64>,
}

pub fn status(project_root: &Path) -> Status {
    let mut s = Status {
        loose: crate::git_sync::changed_files(project_root).len(),
        ..Default::default()
    };
    let Some(remote) = primary_remote(project_root) else {
        return s;
    };
    s.connected = true;
    let branch = head_branch(project_root);
    let upstream = format!("{remote}/{branch}");
    if git_ok(
        project_root,
        &[
            "rev-parse",
            "--verify",
            "-q",
            &format!("refs/remotes/{upstream}"),
        ],
    )
    .is_some()
    {
        s.waiting = git_ok(
            project_root,
            &["rev-list", "--count", &format!("{upstream}..HEAD")],
        )
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);
        s.last_backup = git_ok(project_root, &["log", "-1", "--format=%ct", &upstream])
            .and_then(|t| t.parse().ok());
    } else {
        // Connected, but nothing has ever been sent.
        s.waiting = git_ok(project_root, &["rev-list", "--count", "HEAD"])
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);
    }
    s
}

/// The three-line answer to "is my work safe?".
/// The one calm line the status bar keeps showing: is my writing safe?
/// `unsaved` — some open document has edits not yet written to disk.
pub fn safety_line(unsaved: bool, autosave: bool, s: &Status, now: DateTime<Local>) -> String {
    if unsaved {
        return if autosave {
            "Saving as you write".to_string()
        } else {
            "Not saved yet".to_string()
        };
    }
    if !s.connected {
        return "Saved on this computer".to_string();
    }
    match s.last_backup {
        Some(ts) if s.waiting == 0 && s.loose == 0 => {
            format!("Saved \u{b7} backed up {}", ago(ts, now))
        }
        Some(ts) => format!("Saved \u{b7} last backed up {}", ago(ts, now)),
        None => "Saved \u{b7} not backed up online yet".to_string(),
    }
}

/// "just now", "12 min ago", "3 hours ago", "yesterday", "5 days ago".
fn ago(ts: i64, now: DateTime<Local>) -> String {
    let secs = (now.timestamp() - ts).max(0);
    match secs {
        0..=89 => "just now".to_string(),
        90..=3_599 => format!("{} min ago", (secs + 30) / 60),
        3_600..=86_399 => {
            let h = (secs + 1_800) / 3_600;
            format!("{h} hour{} ago", if h == 1 { "" } else { "s" })
        }
        86_400..=172_799 => "yesterday".to_string(),
        _ => format!("{} days ago", secs / 86_400),
    }
}

pub fn status_lines(s: &Status, now: DateTime<Local>) -> Vec<String> {
    let mut lines = vec!["Saved on this computer.".to_string()];
    if s.loose > 0 {
        lines.push(format!(
            "{} file{} with edits not yet kept as a version — the sync button does that.",
            s.loose,
            if s.loose == 1 { "" } else { "s" }
        ));
    }
    if !s.connected {
        lines.push(
            "Not backed up online yet — ☰ → Set Up Zerkalo will connect GitHub or a folder."
                .to_string(),
        );
    } else if s.waiting > 0 {
        lines.push(format!(
            "{} version{} kept here but not backed up online yet — press the sync button.",
            s.waiting,
            if s.waiting == 1 { "" } else { "s" }
        ));
    } else if let Some(t) = s.last_backup {
        lines.push(format!(
            "Backed up online. Last added to: {}.",
            friendly_when(t, now)
        ));
    } else {
        lines.push("Backed up online.".to_string());
    }
    lines
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_safety_line_always_says_something_reassuring() {
        let now = Local.timestamp_opt(1_800_000_000, 0).unwrap();
        let none = Status::default();
        assert_eq!(safety_line(true, true, &none, now), "Saving as you write");
        assert_eq!(safety_line(true, false, &none, now), "Not saved yet");
        assert_eq!(
            safety_line(false, true, &none, now),
            "Saved on this computer"
        );
        let online = |last, waiting, loose| Status {
            connected: true,
            waiting,
            loose,
            last_backup: last,
        };
        assert_eq!(
            safety_line(false, true, &online(Some(1_800_000_000 - 720), 0, 0), now),
            "Saved \u{b7} backed up 12 min ago"
        );
        assert_eq!(
            safety_line(false, true, &online(Some(1_800_000_000 - 720), 2, 0), now),
            "Saved \u{b7} last backed up 12 min ago"
        );
        assert_eq!(
            safety_line(false, true, &online(None, 0, 0), now),
            "Saved \u{b7} not backed up online yet"
        );
        assert_eq!(ago(1_800_000_000 - 10, now), "just now");
        assert_eq!(ago(1_800_000_000 - 7_200, now), "2 hours ago");
        assert_eq!(ago(1_800_000_000 - 100_000, now), "yesterday");
        assert_eq!(ago(1_800_000_000 - 400_000, now), "4 days ago");
    }

    use super::*;

    fn g(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn repo() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        g(d.path(), &["init", "-b", "main"]);
        g(d.path(), &["config", "user.email", "t@e.org"]);
        g(d.path(), &["config", "user.name", "T"]);
        g(d.path(), &["config", "commit.gpgsign", "false"]);
        d
    }

    fn commit(d: &Path, name: &str, text: &str) {
        std::fs::write(d.join(name), text).unwrap();
        g(d, &["add", "."]);
        g(d, &["commit", "-m", "x"]);
    }

    fn at(y: i32, m: u32, d: u32, h: u32, mi: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, h, mi, 0).unwrap()
    }

    #[test]
    fn times_read_the_way_people_say_them() {
        let now = at(2026, 10, 3, 16, 30);
        assert_eq!(
            friendly_when(at(2026, 10, 3, 16, 10).timestamp(), now),
            "Today 4:10 pm"
        );
        assert_eq!(
            friendly_when(at(2026, 10, 2, 9, 2).timestamp(), now),
            "Yesterday 9:02 am"
        );
        assert_eq!(
            friendly_when(at(2026, 9, 30, 14, 15).timestamp(), now),
            "Wed 2:15 pm"
        );
        assert_eq!(
            friendly_when(at(2026, 9, 12, 8, 5).timestamp(), now),
            "12 Sep, 8:05 am"
        );
        assert_eq!(
            friendly_when(at(2025, 12, 1, 8, 5).timestamp(), now),
            "1 Dec 2025, 8:05 am"
        );
    }

    #[test]
    fn changes_are_described_in_words() {
        assert_eq!(
            describe_changes("a b c", "a b c"),
            "Same words as the document now."
        );
        assert_eq!(
            describe_changes("a b c d e", "a b c"),
            "Brings back 2 words."
        );
        assert_eq!(
            describe_changes("a b", "a b c d x"),
            "Takes out 3 words that are in the document now."
        );
        assert!(describe_changes("old words here", "new words there").contains("and takes out"));
    }

    #[test]
    fn finding_where_a_phrase_went_ignores_case_accents_and_order() {
        assert!(text_matches(
            "The Žižek paragraph about grace",
            "zizek GRACE"
        ));
        assert!(text_matches("grace and then zizek", "zizek grace"));
        assert!(!text_matches("nothing here", "grace"));
        assert!(text_matches("anything", ""));
    }

    #[test]
    fn a_copy_never_takes_an_existing_name() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("essay.typ");
        std::fs::write(&f, "now").unwrap();
        let when = at(2026, 10, 3, 16, 10).timestamp();
        let a = restore_copy_path(&f, when);
        assert_eq!(
            a.file_name().unwrap().to_str().unwrap(),
            "essay (earlier 3 Oct 1610).typ"
        );
        std::fs::write(&a, "x").unwrap();
        let b = restore_copy_path(&f, when);
        assert_ne!(a, b);
        assert!(b.to_str().unwrap().contains("earlier 3 Oct 1610 2"));
    }

    #[test]
    fn bringing_back_makes_a_new_file_and_leaves_the_original_alone() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("essay.typ");
        std::fs::write(&f, "current words").unwrap();
        let copy = bring_back_as_copy(&f, 1_790_000_000, "old words").unwrap();
        assert_eq!(std::fs::read_to_string(&copy).unwrap(), "old words");
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "current words");
        assert_ne!(copy, f);
    }

    #[test]
    fn the_timeline_merges_history_and_saves_and_says_what_is_backed_up() {
        let remote = tempfile::tempdir().unwrap();
        g(remote.path(), &["init", "--bare", "-b", "main"]);
        let d = repo();
        commit(d.path(), "essay.typ", "one\n");
        g(
            d.path(),
            &["remote", "add", "origin", remote.path().to_str().unwrap()],
        );
        g(d.path(), &["push", "-u", "origin", "main"]);
        commit(d.path(), "essay.typ", "one\ntwo\n");
        let file = d.path().join("essay.typ");

        let v = versions(d.path(), &file);
        let kinds: Vec<_> = v.iter().map(|v| v.kind.clone()).collect();
        assert!(kinds.contains(&Kind::BackedUp) && kinds.contains(&Kind::OnThisComputer));
        // Newest first, and the unpushed one is the newest.
        assert_eq!(v[0].kind, Kind::OnThisComputer);
        assert_eq!(text_of(&v[0], &file).unwrap(), "one\ntwo\n");
        assert_eq!(text_of(v.last().unwrap(), &file).unwrap(), "one\n");
    }

    #[test]
    fn status_tells_the_truth_at_each_stage() {
        let d = repo();
        commit(d.path(), "a.typ", "x\n");
        let now = Local::now();
        // No online backup.
        let s = status(d.path());
        assert!(!s.connected);
        assert!(status_lines(&s, now)
            .iter()
            .any(|l| l.contains("Not backed up online yet")));
        // Connected and sent.
        let remote = tempfile::tempdir().unwrap();
        g(remote.path(), &["init", "--bare", "-b", "main"]);
        g(
            d.path(),
            &["remote", "add", "origin", remote.path().to_str().unwrap()],
        );
        g(d.path(), &["push", "-u", "origin", "main"]);
        let s = status(d.path());
        assert!(s.connected && s.waiting == 0 && s.last_backup.is_some());
        assert!(status_lines(&s, now)
            .iter()
            .any(|l| l.starts_with("Backed up online")));
        // A new version not yet sent, and a loose edit.
        commit(d.path(), "a.typ", "x\ny\n");
        std::fs::write(d.path().join("a.typ"), "x\ny\nz\n").unwrap();
        let s = status(d.path());
        assert_eq!((s.waiting, s.loose), (1, 1));
        let lines = status_lines(&s, now).join(" ");
        assert!(lines.contains("not backed up online yet") && lines.contains("not yet kept"));
    }

    #[test]
    fn saved_copies_list_their_differing_files_and_bring_one_back_as_a_copy() {
        let remote = tempfile::tempdir().unwrap();
        g(remote.path(), &["init", "--bare", "-b", "main"]);
        let d = repo();
        commit(d.path(), "essay.typ", "online\n");
        g(
            d.path(),
            &["remote", "add", "origin", remote.path().to_str().unwrap()],
        );
        g(d.path(), &["push", "-u", "origin", "main"]);
        // Something only here, kept by a force replace.
        std::fs::write(d.path().join("essay.typ"), "mine, different\n").unwrap();
        std::fs::write(d.path().join("only-here.typ"), "mine alone\n").unwrap();
        let r = crate::git_sync::force_pull(d.path(), None);
        assert!(r.error.is_none(), "{r:?}");

        let copies = saved_copies(d.path());
        assert_eq!(copies.len(), 1);
        assert!(copies[0].files.contains(&"essay.typ".to_string()));
        assert!(copies[0].files.contains(&"only-here.typ".to_string()));

        let path =
            bring_back_from_saved_copy(d.path(), &copies[0].branch, "only-here.typ").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "mine alone\n");
        assert!(path.to_str().unwrap().contains("(recovered)"));
        // The live file is untouched.
        assert_eq!(
            std::fs::read_to_string(d.path().join("essay.typ")).unwrap(),
            "online\n"
        );
        // A second bring-back never overwrites the first.
        let again =
            bring_back_from_saved_copy(d.path(), &copies[0].branch, "only-here.typ").unwrap();
        assert_ne!(path, again);
    }

    #[test]
    fn snapshot_file_names_read_as_local_times() {
        let t = snapshot_time("20261003T161005.123").unwrap();
        let d = Local.timestamp_opt(t, 0).unwrap();
        assert_eq!(
            d.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2026-10-03 16:10:05"
        );
        assert!(snapshot_time("garbage").is_none());
    }
}
