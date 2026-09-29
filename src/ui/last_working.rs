//! "It worked a few minutes ago — what did I change?" Remembers the open files
//! as they were the last time the document compiled without problems, and turns
//! the difference from now into something a person can read, roll back to, or
//! paste into a help request.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::diff_render::simple_diff;
use super::error_panel::{CompileError, Severity};

const CONTEXT: usize = 2;
const NO_DIFF: &str = "(no differences)";

#[derive(Clone)]
pub struct Snapshot {
    pub files: Vec<(PathBuf, String)>,
    pub taken: Instant,
}

/// The most recent clean-compile snapshot, shared between whoever records it
/// (the compile-done handler) and whoever reads it (the Problems panel).
#[derive(Clone, Default)]
pub struct LastWorking {
    inner: Rc<RefCell<Option<Snapshot>>>,
}

impl LastWorking {
    pub fn record(&self, mut files: Vec<(PathBuf, String)>) {
        if files.is_empty() {
            return;
        }
        files.sort_by(|a, b| a.0.cmp(&b.0));
        *self.inner.borrow_mut() = Some(Snapshot {
            files,
            taken: Instant::now(),
        });
    }

    pub fn get(&self) -> Option<Snapshot> {
        self.inner.borrow().clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub file: PathBuf,
    /// The file as it was when the document last worked.
    pub old: String,
    /// Readable `+ `/`- `/`  ` lines around what differs.
    pub diff: String,
}

/// The files that differ from the snapshot, with those holding a problem first.
/// Only files still open are compared; a file that was closed can't be rolled back.
pub fn changes(
    snap: &Snapshot,
    current: &[(PathBuf, String)],
    problem_files: &[PathBuf],
) -> Vec<Change> {
    let mut out: Vec<Change> = snap
        .files
        .iter()
        .filter_map(|(path, old)| {
            let (_, now) = current.iter().find(|(p, _)| p == path)?;
            (now != old).then(|| Change {
                file: path.clone(),
                old: old.clone(),
                diff: context_diff(old, now),
            })
        })
        .collect();
    out.sort_by_key(|c| !problem_files.contains(&c.file));
    out
}

/// A line diff of just the part that changed. `simple_diff` looks at no more
/// than its first 600 lines, so trimming the identical start and end first is
/// what lets a change near the bottom of a long document show up at all.
fn context_diff(old: &str, new: &str) -> String {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let max_common = a.len().min(b.len());
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suffix = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take(max_common - prefix)
        .take_while(|(x, y)| x == y)
        .count();

    let start = prefix.saturating_sub(CONTEXT);
    let a_end = a.len() - suffix + CONTEXT.min(suffix);
    let b_end = b.len() - suffix + CONTEXT.min(suffix);
    let diff = simple_diff(&a[start..a_end].join("\n"), &b[start..b_end].join("\n"));
    if diff == NO_DIFF {
        return "(only spacing changed)".to_string();
    }

    let mut out = String::new();
    if start > 0 {
        out.push_str("...\n");
    }
    out.push_str(&diff);
    if suffix > CONTEXT {
        out.push_str("...\n");
    }
    out
}

/// Keeps the first `max` lines and says how many were left out.
pub fn cap_lines(text: &str, max: usize) -> String {
    let total = text.lines().count();
    if total <= max {
        return text.to_string();
    }
    let mut out: String = text.lines().take(max).collect::<Vec<_>>().join("\n");
    let more = total - max;
    out.push_str(&format!(
        "\n... and {more} more line{}\n",
        if more == 1 { "" } else { "s" }
    ));
    out
}

/// The changes as one block of text, each file under its name when more than
/// one changed.
pub fn combined_diff(changes: &[Change]) -> String {
    let named = changes.len() > 1;
    let mut out = String::new();
    for c in changes {
        if named {
            let name = c.file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            out.push_str(&format!("\u{2014} {name} \u{2014}\n"));
        }
        out.push_str(&c.diff);
        if !c.diff.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

pub fn age_text(age: Duration) -> String {
    let mins = age.as_secs() / 60;
    let plural = |n: u64, unit: &str| format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" });
    match mins {
        0 => "less than a minute ago".to_string(),
        1..=59 => plural(mins, "minute"),
        60..=1439 => plural(mins / 60, "hour"),
        _ => plural(mins / 1440, "day"),
    }
}

/// A short plain-text summary someone else can read without seeing the
/// document: the problems, the exact lines, and what changed since it worked.
/// It deliberately holds no more of the document than those lines.
pub fn help_request(
    version: &str,
    errors: &[CompileError],
    line_text: &dyn Fn(&Path, u32) -> Option<String>,
    changes: &[Change],
    age: Option<Duration>,
) -> String {
    const MAX_PROBLEMS: usize = 5;
    const MAX_DIFF_LINES: usize = 40;

    let problems: Vec<&CompileError> = errors
        .iter()
        .filter(|e| e.severity == Severity::Error)
        .collect();
    let mut out = format!("I'm stuck in Zerkalo {version}.\n\n");
    out.push_str(&format!("Problems ({}):\n", problems.len()));
    for (i, e) in problems.iter().take(MAX_PROBLEMS).enumerate() {
        let name = e.file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        out.push_str(&format!(
            "{}. {name}, line {}: {}\n",
            i + 1,
            e.line,
            e.message
        ));
        if let Some(line) = line_text(&e.file, e.line) {
            let line = line.trim();
            if !line.is_empty() {
                out.push_str(&format!("   The line: {line}\n"));
            }
        }
        if let Some(first) = e.technical.lines().next().filter(|l| !l.is_empty()) {
            out.push_str(&format!("   Engine said: {first}\n"));
        }
    }
    if problems.len() > MAX_PROBLEMS {
        let more = problems.len() - MAX_PROBLEMS;
        out.push_str(&format!("   ... and {more} more\n"));
    }

    if !changes.is_empty() {
        let when = age.map(age_text).unwrap_or_else(|| "earlier".to_string());
        out.push_str(&format!(
            "\nIt last compiled without problems {when}. Changed since then:\n"
        ));
        out.push_str(&cap_lines(&combined_diff(changes), MAX_DIFF_LINES));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic_catalog::Kind;

    fn snap(files: &[(&str, &str)]) -> Snapshot {
        Snapshot {
            files: files
                .iter()
                .map(|(p, t)| (PathBuf::from(p), t.to_string()))
                .collect(),
            taken: Instant::now(),
        }
    }

    fn cur(files: &[(&str, &str)]) -> Vec<(PathBuf, String)> {
        files
            .iter()
            .map(|(p, t)| (PathBuf::from(p), t.to_string()))
            .collect()
    }

    fn problem(file: &str, line: u32, message: &str) -> CompileError {
        CompileError {
            file: PathBuf::from(file),
            line,
            col: 1,
            end_line: line,
            end_col: 1,
            origin: None,
            message: message.to_string(),
            advice: String::new(),
            kind: Kind::UnknownVariable,
            hints: Vec::new(),
            technical: "error: unknown variable: x\n more".to_string(),
            severity: Severity::Error,
        }
    }

    #[test]
    fn identical_files_report_no_changes() {
        let s = snap(&[("a.typ", "one\ntwo\n")]);
        assert!(changes(&s, &cur(&[("a.typ", "one\ntwo\n")]), &[]).is_empty());
    }

    #[test]
    fn a_changed_line_shows_as_removed_and_added_with_context() {
        let s = snap(&[("a.typ", "a\nb\nc\nd\ne\n")]);
        let c = changes(&s, &cur(&[("a.typ", "a\nb\nC\nd\ne\n")]), &[]);
        assert_eq!(c.len(), 1);
        assert!(c[0].diff.contains("- c"));
        assert!(c[0].diff.contains("+ C"));
        assert!(c[0].diff.contains("  b"));
        assert_eq!(c[0].old, "a\nb\nc\nd\ne\n");
    }

    #[test]
    fn a_change_deep_in_a_long_document_is_not_missed() {
        let base: Vec<String> = (0..1500).map(|i| format!("line {i}")).collect();
        let old = base.join("\n");
        let mut edited = base.clone();
        edited[1400] = "line 1400 edited".to_string();
        let new = edited.join("\n");
        let s = snap(&[("a.typ", &old)]);
        let c = changes(&s, &cur(&[("a.typ", &new)]), &[]);
        assert!(c[0].diff.contains("- line 1400\n"), "{}", c[0].diff);
        assert!(c[0].diff.contains("+ line 1400 edited"));
        assert!(c[0].diff.starts_with("...\n"));
        assert!(c[0].diff.lines().count() < 20);
    }

    #[test]
    fn a_whitespace_only_change_says_so() {
        let s = snap(&[("a.typ", "one\ntwo\n")]);
        let c = changes(&s, &cur(&[("a.typ", "one\ntwo")]), &[]);
        assert_eq!(c[0].diff, "(only spacing changed)");
    }

    #[test]
    fn appended_and_deleted_text_are_both_shown() {
        let s = snap(&[("a.typ", "a\nb\n")]);
        let added = changes(&s, &cur(&[("a.typ", "a\nb\nc\n")]), &[]);
        assert!(added[0].diff.contains("+ c"));
        let removed = changes(&s, &cur(&[("a.typ", "a\n")]), &[]);
        assert!(removed[0].diff.contains("- b"));
        let emptied = changes(&s, &cur(&[("a.typ", "")]), &[]);
        assert!(emptied[0].diff.contains("- a") && emptied[0].diff.contains("- b"));
    }

    #[test]
    fn files_with_problems_come_first_and_closed_files_are_skipped() {
        let s = snap(&[("a.typ", "1"), ("b.typ", "1"), ("gone.typ", "1")]);
        let now = cur(&[("a.typ", "2"), ("b.typ", "2")]);
        let c = changes(&s, &now, &[PathBuf::from("b.typ")]);
        let names: Vec<_> = c.iter().map(|c| c.file.to_str().unwrap()).collect();
        assert_eq!(names, ["b.typ", "a.typ"]);
    }

    #[test]
    fn only_the_last_snapshot_is_kept_and_empty_ones_are_ignored() {
        let lw = LastWorking::default();
        assert!(lw.get().is_none());
        lw.record(cur(&[("a.typ", "first")]));
        lw.record(Vec::new());
        assert_eq!(lw.get().unwrap().files[0].1, "first");
        lw.record(cur(&[("a.typ", "second")]));
        assert_eq!(lw.get().unwrap().files[0].1, "second");
    }

    #[test]
    fn ages_read_naturally() {
        assert_eq!(age_text(Duration::from_secs(20)), "less than a minute ago");
        assert_eq!(age_text(Duration::from_secs(60)), "1 minute ago");
        assert_eq!(age_text(Duration::from_secs(12 * 60)), "12 minutes ago");
        assert_eq!(age_text(Duration::from_secs(3600)), "1 hour ago");
        assert_eq!(age_text(Duration::from_secs(5 * 3600)), "5 hours ago");
        assert_eq!(age_text(Duration::from_secs(3 * 86400)), "3 days ago");
    }

    #[test]
    fn cap_lines_says_how_many_were_left_out() {
        let text = (0..10).map(|i| format!("l{i}\n")).collect::<String>();
        assert_eq!(cap_lines(&text, 20), text);
        let capped = cap_lines(&text, 4);
        assert!(capped.starts_with("l0\nl1\nl2\nl3\n"));
        assert!(capped.contains("... and 6 more lines"));
        assert!(cap_lines(&text, 9).contains("... and 1 more line\n"));
    }

    #[test]
    fn combined_diff_names_files_only_when_there_are_several() {
        let one = vec![Change {
            file: "a.typ".into(),
            old: String::new(),
            diff: "+ x\n".into(),
        }];
        assert_eq!(combined_diff(&one), "+ x\n");
        let two = vec![
            one[0].clone(),
            Change {
                file: "dir/b.typ".into(),
                old: String::new(),
                diff: "- y".into(),
            },
        ];
        let text = combined_diff(&two);
        assert!(text.contains("\u{2014} a.typ \u{2014}\n+ x\n"));
        assert!(text.contains("\u{2014} b.typ \u{2014}\n- y\n"));
    }

    #[test]
    fn help_request_holds_the_problem_the_line_and_the_change() {
        let errors = vec![problem(
            "/p/main.typ",
            12,
            "Zerkalo doesn't know what “x” means",
        )];
        let ch = vec![Change {
            file: "/p/main.typ".into(),
            old: String::new(),
            diff: "- ok\n+ #x\n".into(),
        }];
        let text = help_request(
            "0.37.0",
            &errors,
            &|_, _| Some("  see #x here  ".to_string()),
            &ch,
            Some(Duration::from_secs(5 * 60)),
        );
        assert!(text.contains("Zerkalo 0.37.0"));
        assert!(text.contains("main.typ, line 12: Zerkalo doesn't know"));
        assert!(text.contains("The line: see #x here\n"));
        assert!(text.contains("Engine said: error: unknown variable: x\n"));
        assert!(!text.contains("more\n   "));
        assert!(text.contains("5 minutes ago"));
        assert!(text.contains("- ok\n+ #x"));
    }

    #[test]
    fn help_request_limits_how_much_it_says_and_skips_notes() {
        let mut errors: Vec<CompileError> =
            (1..=8).map(|i| problem("m.typ", i, "Problem")).collect();
        let mut note = problem("m.typ", 99, "Just a note");
        note.severity = Severity::Warning;
        errors.push(note);
        let text = help_request("1.0.0", &errors, &|_, _| None, &[], None);
        assert!(text.contains("Problems (8):"));
        assert!(text.contains("5. m.typ, line 5"));
        assert!(!text.contains("6. m.typ"));
        assert!(text.contains("... and 3 more"));
        assert!(!text.contains("Just a note"));
        assert!(!text.contains("Changed since"));
    }
}
