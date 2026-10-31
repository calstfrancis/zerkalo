//! A plain-text record of how a project was written, version by version, from
//! its first saved version to its last — for showing a teacher that the writing
//! grew over time rather than arriving all at once.
//!
//! It is built from the project folder's own history (the one the online backup
//! keeps), so every version carries the id git gave it. Anyone can check the
//! file against the backup itself: the ids and dates match `git log`. When
//! Zerkalo has kept its private record of typing sessions and pastes
//! (`activity`), that is added.
//!
//! What it can and cannot show is said at the top of the file, in the same plain
//! words: a version is a save or a backup, not a keystroke, and a pasted block
//! shows up as one big addition.
//!
//! The record comes in two readings of the same history: the exact changes, line
//! by line, and a plain one that says in sentences what each version added.

use std::path::Path;

use chrono::{DateTime, FixedOffset, NaiveDate};

use crate::activity::ActivityLog;
use crate::git_sync::git_cmd;

const MARK: &str = "\u{1}VERSION\u{1}";
const SEP: char = '\u{1f}';
/// A single version adding this many lines is pointed out in the summary.
const LARGE_ADDITION: usize = 150;
/// Sessions listed in full; older ones are summed up in a line.
const SESSIONS_LISTED: usize = 60;
/// Lines of added text shown for one file in the plain reading.
const PLAIN_ADDED_SHOWN: usize = 12;

struct FileChange {
    name: String,
    added: Vec<String>,
    removed: usize,
}

struct Version {
    id: String,
    when: DateTime<FixedOffset>,
    message: String,
    added: usize,
    removed: usize,
    diff: String,
    files: Vec<FileChange>,
}

/// Who it is from and the private record to add; all optional.
#[derive(Default)]
pub struct Extras {
    pub author: String,
    pub course: String,
    pub activity: ActivityLog,
}

/// One version, as the window's list shows it.
#[derive(Debug)]
pub struct Item {
    pub label: String,
    pub day: NaiveDate,
    pub added: usize,
    pub removed: usize,
}

/// The finished record, with the few numbers a window shows beside it.
#[derive(Debug)]
pub struct History {
    /// The exact changes, line by line.
    pub text: String,
    /// The same history in sentences.
    pub plain: String,
    pub items: Vec<Item>,
    pub versions: usize,
    pub days: usize,
    pub added: usize,
    pub removed: usize,
    /// Files with changes made since the last backup, which are not in the
    /// history yet.
    pub unsaved_files: usize,
}

/// The history of the project at `repo`, or of one file in it when `only` is
/// given. Folders Zerkalo keeps for itself are left out. The error is a
/// sentence fit to show to the person.
pub fn build(
    repo: &Path,
    only: Option<&Path>,
    today: &str,
    extras: &Extras,
) -> Result<History, String> {
    let format = format!("--format={MARK}%H{SEP}%aI{SEP}%s");
    let mut cmd = git_cmd(repo);
    cmd.args([
        "log",
        "--reverse",
        "-p",
        "--no-color",
        "--no-ext-diff",
        "--no-renames",
        "-U3",
        &format,
        "--",
    ]);
    match only {
        Some(file) => {
            let rel = file.strip_prefix(repo).unwrap_or(file);
            cmd.arg(rel);
        }
        None => {
            cmd.args([".", ":(exclude).zerkalo", ":(exclude).git"]);
        }
    }
    let out = cmd.output().map_err(|_| NO_GIT.to_string())?;
    if !out.status.success() {
        return Err(NO_HISTORY.to_string());
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let versions = parse(&text);
    if versions.is_empty() {
        return Err(NO_HISTORY.to_string());
    }
    let remote = crate::git_sync::list_remotes(repo)
        .first()
        .and_then(|r| crate::git_sync::get_remote_url(repo, r))
        .map(|u| without_credentials(&u));
    let name = repo
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let scope = only
        .and_then(|f| f.file_name())
        .map(|n| n.to_string_lossy().into_owned());

    let top = top(
        &Facts {
            project: &name,
            file: scope.as_deref(),
            remote: remote.as_deref(),
            repo,
            today,
        },
        extras,
        &versions,
    );
    let mut days: Vec<_> = versions.iter().map(day).collect();
    days.dedup();
    Ok(History {
        text: format!("{top}{}", exact(&versions)),
        plain: format!("{top}{}", plain(&versions)),
        items: versions
            .iter()
            .map(|v| Item {
                label: v.when.format("%-d %b, %-I:%M %P").to_string(),
                day: day(v),
                added: v.added,
                removed: v.removed,
            })
            .collect(),
        versions: versions.len(),
        days: days.len(),
        added: versions.iter().map(|v| v.added).sum(),
        removed: versions.iter().map(|v| v.removed).sum(),
        unsaved_files: crate::git_sync::changed_files(repo)
            .iter()
            .filter(|n| n.ends_with(".typ") || n.ends_with(".bib"))
            .count(),
    })
}

const NO_GIT: &str = "Zerkalo couldn't run git, which keeps the history of your writing.";
const NO_HISTORY: &str = "There is no history to export yet. A project's history starts \
    with its first backup — use the backup button, and every backup after that adds a \
    version.";

fn parse(text: &str) -> Vec<Version> {
    let mut out = Vec::new();
    for chunk in text.split(MARK).skip(1) {
        let (head, diff) = chunk.split_once('\n').unwrap_or((chunk, ""));
        let mut parts = head.split(SEP);
        let (Some(id), Some(when), Some(message)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let Ok(when) = DateTime::parse_from_rfc3339(when) else {
            continue;
        };
        let diff = diff.trim_start_matches('\n');
        let (mut added, mut removed) = (0, 0);
        let mut files: Vec<FileChange> = Vec::new();
        for line in diff.lines() {
            if let Some(rest) = line.strip_prefix("diff --git ") {
                let name = rest.rsplit_once(" b/").map_or(rest, |(_, b)| b).to_string();
                files.push(FileChange {
                    name,
                    added: Vec::new(),
                    removed: 0,
                });
                continue;
            }
            if line.starts_with("+++") || line.starts_with("---") {
                continue;
            }
            if let Some(text) = line.strip_prefix('+') {
                added += 1;
                if let Some(f) = files.last_mut() {
                    f.added.push(text.to_string());
                }
            } else if line.starts_with('-') {
                removed += 1;
                if let Some(f) = files.last_mut() {
                    f.removed += 1;
                }
            }
        }
        out.push(Version {
            id: id.to_string(),
            when,
            message: message.to_string(),
            added,
            removed,
            diff: diff.trim_end().to_string(),
            files,
        });
    }
    out
}

/// A remote address with any `user:token@` part taken out, so the file can be
/// handed over without handing over a sign-in.
fn without_credentials(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    let (authority, path) = rest.split_once('/').map_or((rest, ""), |(a, p)| (a, p));
    let host = authority.rsplit('@').next().unwrap_or(authority);
    if path.is_empty() {
        format!("{scheme}://{host}")
    } else {
        format!("{scheme}://{host}/{path}")
    }
}

/// `https://github.com/owner/repo/commits` for a GitHub address in either
/// of its usual spellings.
fn github_commits_page(remote: &str) -> Option<String> {
    let rest = remote
        .split_once("github.com/")
        .or_else(|| remote.split_once("github.com:"))?
        .1;
    let path = rest.trim_end_matches('/').trim_end_matches(".git");
    (path.matches('/').count() == 1).then(|| format!("https://github.com/{path}/commits"))
}

fn day(v: &Version) -> NaiveDate {
    v.when.date_naive()
}

fn clock(t: &DateTime<FixedOffset>) -> String {
    t.format("%-I:%M %P").to_string()
}

fn minutes(secs: i64) -> String {
    let m = (secs + 30) / 60;
    match (m / 60, m % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

fn words_of(chars: usize) -> usize {
    chars.div_ceil(6)
}

struct Facts<'a> {
    project: &'a str,
    file: Option<&'a str>,
    remote: Option<&'a str>,
    repo: &'a Path,
    today: &'a str,
}

/// Everything before the versions: who and what this is, the summary, how to
/// check it, and the record of typing.
fn top(facts: &Facts, extras: &Extras, versions: &[Version]) -> String {
    let first = &versions[0];
    let last = &versions[versions.len() - 1];
    let added: usize = versions.iter().map(|v| v.added).sum();
    let removed: usize = versions.iter().map(|v| v.removed).sum();
    let mut days: Vec<_> = versions.iter().map(day).collect();
    days.dedup();
    let when = |v: &Version| v.when.format("%-d %B %Y, %-I:%M %P (%:z)").to_string();

    let mut s = String::from("HOW THIS WAS WRITTEN\n====================\n\n");
    if !extras.author.trim().is_empty() {
        s.push_str(&format!("Written by:   {}\n", extras.author.trim()));
    }
    if !extras.course.trim().is_empty() {
        s.push_str(&format!("For:          {}\n", extras.course.trim()));
    }
    s.push_str(&format!("Project:      {}\n", facts.project));
    if let Some(f) = facts.file {
        s.push_str(&format!("Document:     {f}\n"));
    }
    s.push_str(&format!(
        "Made:         {}, with Zerkalo {}\n",
        facts.today,
        env!("CARGO_PKG_VERSION")
    ));
    if let Some(r) = facts.remote {
        s.push_str(&format!("Online copy:  {r}\n"));
    }
    s.push_str(&format!(
        "\nFirst version: {}\nLast version:  {}\n",
        when(first),
        when(last)
    ));
    s.push_str(&format!(
        "Versions:      {}, over {} {}\n",
        versions.len(),
        days.len(),
        if days.len() == 1 {
            "day"
        } else {
            "different days"
        }
    ));
    s.push_str(&format!(
        "In all:        {added} lines written, {removed} lines taken out or rewritten\n"
    ));
    if let Some((i, v)) = versions.iter().enumerate().max_by_key(|(_, v)| v.added) {
        if v.added >= LARGE_ADDITION {
            s.push_str(&format!(
                "Largest step:  version {} added {} lines in one go ({})\n",
                i + 1,
                v.added,
                v.when.format("%-d %B %Y")
            ));
        }
    }

    s.push_str(
        "\nReading this record\n-------------------\n\
         Each version below is one saved step in the project's history: when it was \
         saved, then what changed. A version is a save or a backup, not every \
         keystroke, so a stretch of typing between two saves appears together. Text \
         pasted in appears as one large addition.\n",
    );

    s.push_str(&checking(facts, last));
    s.push_str(&typing(&extras.activity));
    s.push('\n');
    s
}

fn checking(facts: &Facts, last: &Version) -> String {
    let mut s = String::from("\nHow to check it\n---------------\n");
    s.push_str(
        "Every version has an id that git works out from the writing in it and the \
         version before it, so no version can be changed or slipped in without every \
         id after it changing too. To check this record against the real history:\n\n",
    );
    s.push_str(&format!(
        "  1. In a terminal, in the project folder ({}), run:\n\
         \x20       git log --reverse --format='%H %aI %s'\n\
         \x20    The ids and dates are the ones listed below.\n",
        facts.repo.display()
    ));
    if let Some(page) = facts.remote.and_then(github_commits_page) {
        s.push_str(&format!(
            "  2. Open {page}\n\
             \x20    GitHub shows when each version reached it. That time is set by \
             GitHub, not by the writer's computer.\n"
        ));
    } else if let Some(r) = facts.remote {
        s.push_str(&format!(
            "  2. The online copy at {r} lists the same versions and when each reached it.\n"
        ));
    }
    s.push_str(&format!("\nLatest version id: {}\n", last.id));
    s
}

fn typing(log: &ActivityLog) -> String {
    let mut s = String::from("\nTyping sessions\n---------------\n");
    if log.is_empty() {
        s.push_str(
            "None recorded yet. Zerkalo keeps a private record of when writing happened \
             on the computer it is used on; the saved versions below are the record of \
             anything written before that began.\n",
        );
        return s;
    }
    let first = log.sessions.first().map(|x| x.start);
    s.push_str(&format!(
        "Kept by Zerkalo on the computer it was written on{}. Only times and amounts \
         are kept, never the text itself.\n\n",
        first.map_or(String::new(), |t| format!(
            ", since {}",
            t.format("%-d %B %Y")
        ))
    ));
    let typed: usize = log.sessions.iter().map(|x| x.typed_chars).sum();
    let pasted: usize = log.sessions.iter().map(|x| x.pasted_chars).sum();
    let active: i64 = log.sessions.iter().map(|x| x.active_secs).sum();
    s.push_str(&format!(
        "In all: {} {}, {} of writing, about {} words typed and about {} words pasted in.\n\n",
        log.sessions.len(),
        if log.sessions.len() == 1 {
            "session"
        } else {
            "sessions"
        },
        minutes(active),
        words_of(typed),
        words_of(pasted)
    ));
    let skipped = log.sessions.len().saturating_sub(SESSIONS_LISTED);
    if skipped > 0 {
        s.push_str(&format!("  ({skipped} earlier sessions not listed)\n"));
    }
    for x in log.sessions.iter().skip(skipped) {
        s.push_str(&format!(
            "  {:<8} {} – {}   {:>9}   typed about {} words{}\n",
            x.start.format("%-d %b"),
            clock(&x.start),
            clock(&x.end),
            minutes(x.active_secs),
            words_of(x.typed_chars),
            if x.pasted_chars > 0 {
                format!(", pasted about {}", words_of(x.pasted_chars))
            } else {
                String::new()
            }
        ));
    }
    if !log.pastes.is_empty() {
        s.push_str("\nLarger pieces of text pasted in\n");
        for p in &log.pastes {
            s.push_str(&format!(
                "  {:<8} {}   in {}: about {} words\n",
                p.when.format("%-d %b"),
                clock(&p.when),
                p.file,
                p.words
            ));
        }
        s.push_str(
            "Pasted text may be the writer's own notes or another part of the same \
             document; the record shows that text was pasted, not where it came from.\n",
        );
    }
    s
}

fn exact(versions: &[Version]) -> String {
    let total = versions.len();
    let mut s = String::new();
    for (i, v) in versions.iter().enumerate() {
        s.push_str(&"=".repeat(60));
        s.push('\n');
        s.push_str(&format!(
            "Version {} of {}  —  {}\n",
            i + 1,
            total,
            v.when.format("%-d %B %Y, %-I:%M %P (%:z)")
        ));
        s.push_str(&format!(
            "Id {}   +{} / -{} lines\n",
            &v.id[..v.id.len().min(10)],
            v.added,
            v.removed
        ));
        if !v.message.trim().is_empty() {
            s.push_str(&format!("Note: {}\n", v.message.trim()));
        }
        s.push_str(&"=".repeat(60));
        s.push('\n');
        if v.diff.is_empty() {
            s.push_str("(no text changed in this version)\n");
        } else {
            s.push_str(&v.diff);
            s.push('\n');
        }
        s.push('\n');
    }
    s
}

fn plain(versions: &[Version]) -> String {
    let total = versions.len();
    let mut s = String::new();
    for (i, v) in versions.iter().enumerate() {
        s.push_str(&"=".repeat(60));
        s.push('\n');
        s.push_str(&format!(
            "Version {} of {}  —  {}\n",
            i + 1,
            total,
            v.when.format("%A %-d %B %Y, %-I:%M %P")
        ));
        s.push_str(&"=".repeat(60));
        s.push('\n');
        if v.files.is_empty() {
            s.push_str("No text changed in this version.\n\n");
            continue;
        }
        for f in &v.files {
            s.push_str(&format!(
                "\nIn {}: {} {} added{}\n",
                f.name,
                f.added.len(),
                if f.added.len() == 1 { "line" } else { "lines" },
                if f.removed > 0 {
                    format!(", {} taken out or rewritten", f.removed)
                } else {
                    String::new()
                }
            ));
            let text: Vec<&String> = f.added.iter().filter(|l| !l.trim().is_empty()).collect();
            for line in text.iter().take(PLAIN_ADDED_SHOWN) {
                s.push_str(&format!("  | {line}\n"));
            }
            if text.len() > PLAIN_ADDED_SHOWN {
                s.push_str(&format!(
                    "  … and {} more lines\n",
                    text.len() - PLAIN_ADDED_SHOWN
                ));
            }
        }
        s.push('\n');
    }
    s
}

/// Typst source that sets `text` as a page, with added lines in green and
/// removed lines in red, for saving the record as a PDF.
pub fn to_typst(text: &str) -> String {
    fn quote(line: &str) -> String {
        let mut q = String::from("\"");
        for c in line.chars() {
            match c {
                '\\' => q.push_str("\\\\"),
                '"' => q.push_str("\\\""),
                c => q.push(c),
            }
        }
        q.push('"');
        q
    }
    let mut out = String::from(
        "#set page(margin: 1.8cm, numbering: \"1\")\n\
         #set text(size: 8.5pt, font: (\"DejaVu Sans Mono\", \"Liberation Mono\"))\n\
         #set par(leading: 0.5em, spacing: 0.5em)\n",
    );
    for line in text.lines() {
        if line.is_empty() {
            out.push_str("#v(0.7em)\n");
            continue;
        }
        let heading = line.starts_with("Version ") && line.contains(" of ");
        let weight = if heading { "\"bold\"" } else { "\"regular\"" };
        let fill = if line.starts_with('+') && !line.starts_with("+++") {
            "rgb(\"#116b11\")"
        } else if line.starts_with('-') && !line.starts_with("---") {
            "rgb(\"#9a1010\")"
        } else {
            "black"
        };
        out.push_str(&format!(
            "#text(fill: {fill}, weight: {weight})[#{}]\\\n",
            quote(line)
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str], date: &str) {
        let ok = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=T", "-c", "user.email=t@e.com"])
            .args(args)
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?}");
    }

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        git(p, &["init", "-q"], "2026-03-01T10:00:00+00:00");
        std::fs::write(p.join("essay.typ"), "= Essay\nFirst line.\n").unwrap();
        git(p, &["add", "."], "2026-03-01T10:00:00+00:00");
        git(
            p,
            &["commit", "-q", "-m", "Auto-save: essay.typ"],
            "2026-03-01T10:00:00+00:00",
        );
        std::fs::write(p.join("essay.typ"), "= Essay\nFirst line.\nSecond line.\n").unwrap();
        std::fs::create_dir_all(p.join(".zerkalo")).unwrap();
        std::fs::write(p.join(".zerkalo/x.toml"), "a=1\n").unwrap();
        git(p, &["add", "."], "2026-03-04T21:30:00+00:00");
        git(
            p,
            &["commit", "-q", "-m", "Auto-save: essay.typ"],
            "2026-03-04T21:30:00+00:00",
        );
        dir
    }

    #[test]
    fn a_projects_versions_come_out_in_order_with_their_changes() {
        let dir = project();
        let h = build(dir.path(), None, "1 June 2026", &Extras::default()).unwrap();
        let text = h.text;
        assert!(
            text.contains("Versions:      2, over 2 different days"),
            "{text}"
        );
        let v1 = text.find("Version 1 of 2").unwrap();
        let v2 = text.find("Version 2 of 2").unwrap();
        assert!(v1 < v2);
        assert!(text.contains("+Second line."));
        assert!(
            text.contains("In all:        3 lines written, 0 lines taken out"),
            "{text}"
        );
        assert!(!text.contains("x.toml"));
        assert_eq!(h.items.len(), 2);
        assert!(text.contains("Latest version id: "));
    }

    #[test]
    fn the_plain_reading_says_what_each_version_added() {
        let dir = project();
        let h = build(dir.path(), None, "1 June 2026", &Extras::default()).unwrap();
        assert!(
            h.plain.contains("In essay.typ: 1 line added"),
            "{}",
            h.plain
        );
        assert!(h.plain.contains("  | Second line."));
        assert!(!h.plain.contains("diff --git"));
    }

    #[test]
    fn name_course_and_typing_sessions_are_added_when_given() {
        let dir = project();
        let mut log = ActivityLog::default();
        log.sessions.push(crate::activity::Session {
            start: DateTime::parse_from_rfc3339("2026-03-04T21:00:00+00:00").unwrap(),
            end: DateTime::parse_from_rfc3339("2026-03-04T22:00:00+00:00").unwrap(),
            active_secs: 3000,
            typed_chars: 3000,
            pasted_chars: 0,
        });
        let extras = Extras {
            author: "A. Student".into(),
            course: "ENG 101".into(),
            activity: log,
        };
        let text = build(dir.path(), None, "today", &extras).unwrap().text;
        assert!(text.contains("Written by:   A. Student"));
        assert!(text.contains("For:          ENG 101"));
        assert!(text.contains("50 min of writing"), "{text}");
        assert!(text.contains("about 500 words typed"), "{text}");
    }

    #[test]
    fn a_folder_with_no_history_says_what_to_do() {
        let dir = tempfile::tempdir().unwrap();
        let err = build(dir.path(), None, "today", &Extras::default()).unwrap_err();
        assert!(err.contains("backup button"));
    }

    #[test]
    fn a_sign_in_in_the_online_address_is_left_out() {
        assert_eq!(
            without_credentials("https://user:ghp_secret@github.com/a/b.git"),
            "https://github.com/a/b.git"
        );
        assert_eq!(
            without_credentials("https://github.com/a/b.git"),
            "https://github.com/a/b.git"
        );
        assert_eq!(
            without_credentials("git@github.com:a/b.git"),
            "git@github.com:a/b.git"
        );
    }

    #[test]
    fn github_addresses_become_a_page_to_look_at() {
        for url in [
            "https://github.com/a/b.git",
            "git@github.com:a/b.git",
            "https://github.com/a/b",
        ] {
            assert_eq!(
                github_commits_page(url).as_deref(),
                Some("https://github.com/a/b/commits"),
                "{url}"
            );
        }
        assert_eq!(github_commits_page("https://example.com/x.git"), None);
    }

    #[test]
    fn the_pdf_source_quotes_awkward_lines() {
        let src = to_typst("+say \"hi\" \\ there\n\n-gone\nplain");
        assert!(src.contains("say \\\"hi\\\" \\\\ there"));
        assert!(src.contains("#v(0.7em)"));
    }
}
