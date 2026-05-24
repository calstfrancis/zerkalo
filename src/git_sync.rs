use std::path::Path;
use std::process::Command;

use chrono::Local;

// ── Public types ─────────────────────────────────────────────────────────────

pub struct SyncResult {
    pub committed: bool,
    pub pushed: bool,
    pub commit_message: String,
    pub error: Option<String>,
}

// ── Query helpers ─────────────────────────────────────────────────────────────

/// Returns true if the repo has at least one remote configured.
pub fn has_remote(repo_path: &Path) -> bool {
    Command::new("git")
        .args(["-C", path_str(repo_path), "remote"])
        .output()
        .map(|out| !out.stdout.trim_ascii().is_empty())
        .unwrap_or(false)
}

/// Returns the name of the current branch (falls back to "main").
pub fn current_branch(repo_path: &Path) -> String {
    Command::new("git")
        .args(["-C", path_str(repo_path), "rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_else(|_| "main".to_string())
}

/// Returns display names of files changed since the last commit.
pub fn changed_files(repo_path: &Path) -> Vec<String> {
    let Ok(out) = Command::new("git")
        .args(["-C", path_str(repo_path), "status", "--porcelain"])
        .output()
    else {
        return Vec::new();
    };

    if !out.status.success() {
        return Vec::new();
    }

    let mut names: Vec<String> = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if line.len() < 4 {
            continue;
        }
        // "XY path" or "XY old -> new" for renames
        let entry = &line[3..];
        let filename = entry.split(" -> ").last().unwrap_or(entry).trim();
        let basename = Path::new(filename)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(filename)
            .to_string();
        if !names.contains(&basename) {
            names.push(basename);
        }
    }
    names
}

/// Build a human-readable commit message from the changed file list.
pub fn craft_message(changed: &[String]) -> String {
    let ts = Local::now().format("%Y-%m-%d %H:%M").to_string();
    match changed.len() {
        0 => format!("Auto-save: {ts}"),
        1 => format!("Edited {}: {ts}", changed[0]),
        _ => {
            // Limit to first 5 names to keep the subject line short
            let shown: Vec<&str> = changed.iter().take(5).map(String::as_str).collect();
            let suffix = if changed.len() > 5 {
                format!(" (+{})", changed.len() - 5)
            } else {
                String::new()
            };
            format!("Edits to {}{}\n\n{ts}", shown.join(", "), suffix)
        }
    }
}

// ── Write operations ─────────────────────────────────────────────────────────

/// Add a remote named "origin".
pub fn add_remote(repo_path: &Path, url: &str) -> Result<(), String> {
    run_git(repo_path, &["remote", "add", "origin", url])
}

/// Stage everything, commit with an auto-crafted message, and push.
pub fn sync(repo_path: &Path) -> SyncResult {
    let changed = changed_files(repo_path);
    let msg = craft_message(&changed);

    // git add .
    if let Err(e) = run_git(repo_path, &["add", "."]) {
        return SyncResult {
            committed: false,
            pushed: false,
            commit_message: msg,
            error: Some(format!("git add: {e}")),
        };
    }

    // git commit
    let committed = match Command::new("git")
        .args(["-C", path_str(repo_path), "commit", "-m", &msg])
        .output()
    {
        Err(e) => {
            return SyncResult {
                committed: false,
                pushed: false,
                commit_message: msg,
                error: Some(format!("git commit: {e}")),
            };
        }
        Ok(out) if !out.status.success() => {
            let text = lossy_combined(&out);
            if text.contains("nothing to commit") {
                false
            } else {
                return SyncResult {
                    committed: false,
                    pushed: false,
                    commit_message: msg,
                    error: Some(text),
                };
            }
        }
        Ok(_) => true,
    };

    // git push -u origin <branch>
    let branch = current_branch(repo_path);
    let push = Command::new("git")
        .args(["-C", path_str(repo_path), "push", "-u", "origin", &branch])
        .output();

    match push {
        Err(e) => SyncResult {
            committed,
            pushed: false,
            commit_message: msg,
            error: Some(format!("git push: {e}")),
        },
        Ok(out) if !out.status.success() => SyncResult {
            committed,
            pushed: false,
            commit_message: msg,
            error: Some(format!("git push: {}", lossy_combined(&out))),
        },
        Ok(_) => SyncResult {
            committed,
            pushed: true,
            commit_message: msg,
            error: None,
        },
    }
}

// ── Internals ─────────────────────────────────────────────────────────────────

fn path_str(p: &Path) -> &str {
    p.to_str().unwrap_or(".")
}

fn run_git(repo_path: &Path, args: &[&str]) -> Result<(), String> {
    let mut cmd_args = vec!["-C", path_str(repo_path)];
    cmd_args.extend_from_slice(args);
    let out = Command::new("git")
        .args(&cmd_args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(lossy_combined(&out))
    }
}

fn lossy_combined(out: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !stderr.is_empty() {
        stderr
    } else {
        stdout
    }
}
