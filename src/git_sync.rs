use std::path::Path;
use std::process::Command;

use chrono::Local;

// ── Public types ─────────────────────────────────────────────────────────────

pub struct SyncResult {
    pub committed: bool,
    /// True if at least one remote was pushed successfully.
    pub pushed: bool,
    pub commit_message: String,
    /// Fatal error (add or commit failed before any push).
    pub error: Option<String>,
    /// Non-fatal: per-remote push failures — "(remote_name) reason".
    pub push_errors: Vec<String>,
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

/// Returns the names of all configured remotes.
pub fn list_remotes(repo_path: &Path) -> Vec<String> {
    Command::new("git")
        .args(["-C", path_str(repo_path), "remote"])
        .output()
        .map(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Returns the push URL for a named remote.
pub fn get_remote_url(repo_path: &Path, name: &str) -> Option<String> {
    let out = Command::new("git")
        .args(["-C", path_str(repo_path), "remote", "get-url", name])
        .output()
        .ok()?;
    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    } else {
        None
    }
}

/// Add (or update) a remote named "backup". Removes any existing "backup" first.
/// If `target` is a local path (starts with `/`, `~`, `./`, or `../`), a bare
/// git repository is initialised there automatically so the path is ready to
/// receive pushes.
pub fn add_backup_remote(repo_path: &Path, target: &str) -> Result<(), String> {
    let resolved = if is_local_path(target) {
        let expanded = shellexpand::tilde(target).into_owned();
        ensure_bare_repo(Path::new(&expanded))?;
        expanded
    } else {
        target.to_string()
    };
    let _ = run_git(repo_path, &["remote", "remove", "backup"]);
    run_git(repo_path, &["remote", "add", "backup", &resolved])
}

/// Returns true when the string looks like a filesystem path rather than a git URL.
pub fn is_local_path(s: &str) -> bool {
    s.starts_with('/') || s.starts_with('~') || s.starts_with("./") || s.starts_with("../")
}

/// Ensures `path` contains a bare git repository, creating one if needed.
fn ensure_bare_repo(path: &Path) -> Result<(), String> {
    if path.join("HEAD").exists() {
        return Ok(());
    }
    std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
    run_git(path, &["init", "--bare"])
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

/// Stage everything, commit with an auto-crafted message, then push to every
/// configured remote. `pushed` is true if at least one remote succeeded.
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
            push_errors: Vec::new(),
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
                push_errors: Vec::new(),
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
                    push_errors: Vec::new(),
                };
            }
        }
        Ok(_) => true,
    };

    // Push to every remote
    let remotes = list_remotes(repo_path);
    let branch = current_branch(repo_path);
    let mut pushed = false;
    let mut push_errors: Vec<String> = Vec::new();

    for remote in &remotes {
        let out = Command::new("git")
            .args(["-C", path_str(repo_path), "push", "-u", remote, &branch])
            .output();

        match out {
            Err(e) => push_errors.push(format!("({remote}) {e}")),
            Ok(o) if !o.status.success() => {
                push_errors.push(format!("({remote}) {}", lossy_combined(&o)));
            }
            Ok(_) => pushed = true,
        }
    }

    SyncResult {
        committed,
        pushed,
        commit_message: msg,
        error: None,
        push_errors,
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
    if !stderr.is_empty() { stderr } else { stdout }
}
