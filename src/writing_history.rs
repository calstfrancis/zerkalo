//! A plain-text record of how a project was written, version by version, from
//! its first saved version to its last — for showing a teacher that the writing
//! grew over time rather than arriving all at once.
//!
//! It is built from the project folder's own history (the one the online backup
//! keeps), so every version carries the id git gave it. Anyone can check the
//! file against the backup itself: the ids and dates match `git log`.
//!
//! What it can and cannot show is said at the top of the file, in the same plain
//! words: a version is a save or a backup, not a keystroke, and a pasted block
//! shows up as one big addition.

use std::path::Path;

use crate::git_sync::git_cmd;

const MARK: &str = "\u{1}VERSION\u{1}";
const SEP: char = '\u{1f}';
/// A single version adding this many lines is pointed out in the summary.
const LARGE_ADDITION: usize = 150;

struct Version {
    id: String,
    when: chrono::DateTime<chrono::FixedOffset>,
    message: String,
    added: usize,
    removed: usize,
    diff: String,
}

/// The finished file, with the few numbers a window shows beside it.
#[derive(Debug)]
pub struct History {
    pub text: String,
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
pub fn build(repo: &Path, only: Option<&Path>, today: &str) -> Result<History, String> {
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
    let mut days: Vec<_> = versions.iter().map(day).collect();
    days.dedup();
    Ok(History {
        text: render(&name, scope.as_deref(), remote.as_deref(), today, &versions),
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
        let Ok(when) = chrono::DateTime::parse_from_rfc3339(when) else {
            continue;
        };
        let diff = diff.trim_start_matches('\n');
        let (mut added, mut removed) = (0, 0);
        for line in diff.lines() {
            if line.starts_with("+++") || line.starts_with("---") {
                continue;
            }
            if line.starts_with('+') {
                added += 1;
            } else if line.starts_with('-') {
                removed += 1;
            }
        }
        out.push(Version {
            id: id.to_string(),
            when,
            message: message.to_string(),
            added,
            removed,
            diff: diff.trim_end().to_string(),
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

fn day(v: &Version) -> chrono::NaiveDate {
    v.when.date_naive()
}

fn render(
    project: &str,
    file: Option<&str>,
    remote: Option<&str>,
    today: &str,
    versions: &[Version],
) -> String {
    let first = &versions[0];
    let last = &versions[versions.len() - 1];
    let added: usize = versions.iter().map(|v| v.added).sum();
    let removed: usize = versions.iter().map(|v| v.removed).sum();
    let mut days: Vec<_> = versions.iter().map(day).collect();
    days.dedup();
    let when = |v: &Version| v.when.format("%-d %B %Y, %H:%M (%:z)").to_string();
    let largest = versions.iter().enumerate().max_by_key(|(_, v)| v.added);

    let mut s = String::new();
    s.push_str("HOW THIS WAS WRITTEN\n");
    s.push_str("====================\n\n");
    s.push_str(&format!("Project:      {project}\n"));
    if let Some(f) = file {
        s.push_str(&format!("Document:     {f}\n"));
    }
    s.push_str(&format!(
        "Made:         {today}, with Zerkalo {}\n",
        env!("CARGO_PKG_VERSION")
    ));
    if let Some(r) = remote {
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
    if let Some((i, v)) = largest {
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
        "\nReading this file\n-----------------\n\
         Each version below is one saved step in the project's history: when it was \
         saved, then exactly what changed. Lines starting with + were added; lines \
         starting with - were taken out. Read from the top, it shows the text \
         arriving, being reworked and growing.\n\n\
         A version is a save or a backup, not every keystroke, so a stretch of \
         typing between two saves appears together. Text pasted in appears as one \
         large addition.\n\n\
         To check this file, compare it with the project's history itself: every \
         version's id and date here is the same as in `git log` for the project \
         folder",
    );
    if remote.is_some() {
        s.push_str(", and the online copy shows when each version arrived there");
    }
    s.push_str(".\n\n");

    let total = versions.len();
    for (i, v) in versions.iter().enumerate() {
        s.push_str(&"=".repeat(72));
        s.push('\n');
        s.push_str(&format!("Version {} of {}  —  {}\n", i + 1, total, when(v)));
        s.push_str(&format!(
            "Id {}   +{} / -{} lines\n",
            &v.id[..v.id.len().min(10)],
            v.added,
            v.removed
        ));
        if !v.message.trim().is_empty() {
            s.push_str(&format!("Note: {}\n", v.message.trim()));
        }
        s.push_str(&"=".repeat(72));
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

    #[test]
    fn a_projects_versions_come_out_in_order_with_their_changes() {
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

        let text = build(p, None, "1 June 2026").unwrap().text;
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
    }

    #[test]
    fn a_folder_with_no_history_says_what_to_do() {
        let dir = tempfile::tempdir().unwrap();
        let err = build(dir.path(), None, "today").unwrap_err();
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
}
