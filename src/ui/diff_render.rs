//! Shared "an old version and what changed" rendering, used by both the
//! History panel (git-backed) and the Snapshot dialog (local, per-save).
//!
//! Both features answer the same question for the user — "what's different
//! from what I have now?" — and should look the same doing it: a plain
//! `+ line` / `- line` / `  line` view, never raw unified-diff headers or
//! `@@ -3,2 +3,3 @@` hunk markers, which mean nothing to someone who's never
//! used git.

use gtk4::prelude::*;
use gtk4::TextBuffer;

/// Line-level diff between two full texts, LCS-based, with 2-line context
/// around each change. Output is already in the `+ `/`- `/`  ` convention
/// `render_clean_diff` expects.
pub fn simple_diff(old: &str, new: &str) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let m = old_lines.len().min(600);
    let n = new_lines.len().min(600);
    let old_lines = &old_lines[..m];
    let new_lines = &new_lines[..n];

    // LCS DP table
    let mut dp = vec![vec![0u16; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if old_lines[i - 1] == new_lines[j - 1] {
                dp[i - 1][j - 1] + 1
            } else {
                dp[i - 1][j].max(dp[i][j - 1])
            };
        }
    }

    // Backtrack
    let mut diff: Vec<(char, &str)> = Vec::new();
    let (mut i, mut j) = (m, n);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
            diff.push((' ', old_lines[i - 1]));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            diff.push(('+', new_lines[j - 1]));
            j -= 1;
        } else {
            diff.push(('-', old_lines[i - 1]));
            i -= 1;
        }
    }
    diff.reverse();

    // Render with 2-line context around changes
    let changed: Vec<bool> = diff.iter().map(|(c, _)| *c != ' ').collect();
    let mut show = vec![false; diff.len()];
    for (k, _) in changed.iter().enumerate().filter(|(_, c)| **c) {
        let s = k.saturating_sub(2);
        let e = (k + 3).min(diff.len());
        show[s..e].iter_mut().for_each(|v| *v = true);
    }

    let mut out = String::new();
    let mut gap = false;
    for (idx, (ch, line)) in diff.iter().enumerate() {
        if !show[idx] {
            gap = true;
            continue;
        }
        if gap {
            out.push_str("...\n");
            gap = false;
        }
        match ch {
            '-' => out.push_str(&format!("- {line}\n")),
            '+' => out.push_str(&format!("+ {line}\n")),
            _ => out.push_str(&format!("  {line}\n")),
        }
    }

    if out.is_empty() {
        "(no differences)".to_string()
    } else {
        out
    }
}

/// Cleans a `git show`/`git diff` unified-diff body into the same
/// `+ `/`- `/`  ` convention `simple_diff` produces — drops the file header
/// (`diff --git`, `index`, `---`, `+++`) and hunk markers (`@@ ... @@`)
/// entirely rather than showing them to a non-technical user.
pub fn clean_unified_diff(raw: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for line in raw.lines() {
        if line.starts_with("diff --git")
            || line.starts_with("index ")
            || line.starts_with("--- ")
            || line.starts_with("+++ ")
        {
            continue;
        }
        if line.starts_with("@@") {
            gap = true;
            continue;
        }
        if gap {
            out.push_str("...\n");
            gap = false;
        }
        if let Some(rest) = line.strip_prefix('+') {
            out.push_str(&format!("+ {rest}\n"));
        } else if let Some(rest) = line.strip_prefix('-') {
            out.push_str(&format!("- {rest}\n"));
        } else if let Some(rest) = line.strip_prefix(' ') {
            out.push_str(&format!("  {rest}\n"));
        } else if !line.is_empty() {
            out.push_str(&format!("  {line}\n"));
        }
    }
    if out.is_empty() {
        "(no differences)".to_string()
    } else {
        out
    }
}

/// Renders text already in the `+ `/`- `/`  ` convention into `buf`, tagging
/// added/removed lines with the `added`/`removed` TextTags if the buffer's
/// tag table has them.
pub fn render_clean_diff(buf: &TextBuffer, diff: &str) {
    buf.set_text("");
    let mut iter = buf.start_iter();
    for line in diff.lines() {
        let tag_name = if line.starts_with("+ ") {
            Some("added")
        } else if line.starts_with("- ") {
            Some("removed")
        } else {
            None
        };
        let line_with_nl = format!("{line}\n");
        if let Some(name) = tag_name {
            if let Some(tag) = buf.tag_table().lookup(name) {
                buf.insert_with_tags(&mut iter, &line_with_nl, &[&tag]);
                continue;
            }
        }
        buf.insert(&mut iter, &line_with_nl);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_unified_diff_drops_headers_and_hunk_markers() {
        let raw = "diff --git a/main.typ b/main.typ\nindex abc..def 100644\n--- a/main.typ\n+++ b/main.typ\n@@ -1,2 +1,3 @@\n = First\n+= Second\n-= Old\n";
        let cleaned = clean_unified_diff(raw);
        assert!(!cleaned.contains("diff --git"));
        assert!(!cleaned.contains("@@"));
        assert!(!cleaned.contains("---"));
        assert!(!cleaned.contains("+++"));
        assert!(cleaned.contains("+ = Second"));
        assert!(cleaned.contains("- = Old"));
        assert!(cleaned.contains("  = First"));
    }

    #[test]
    fn simple_diff_reports_no_differences_for_identical_text() {
        assert_eq!(simple_diff("same\n", "same\n"), "(no differences)");
    }
}
