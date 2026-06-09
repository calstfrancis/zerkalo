use std::path::Path;
use std::process::Command;

use chrono::Local;

pub fn in_flatpak() -> bool {
    std::path::Path::new("/.flatpak-info").exists()
}

/// Returns a `Command` for a host binary, using `flatpak-spawn --host` when
/// running inside a flatpak sandbox so the binary is found on the host.
pub fn host_command(bin: &str) -> Command {
    if in_flatpak() {
        let mut cmd = Command::new("flatpak-spawn");
        cmd.arg("--host").arg(bin);
        cmd
    } else {
        Command::new(bin)
    }
}

/// Returns a `Command` pre-loaded with `git -C <repo>`, using
/// `flatpak-spawn --host git` when running inside a flatpak sandbox.
fn git_cmd(repo_path: &Path) -> Command {
    if in_flatpak() {
        let mut cmd = Command::new("flatpak-spawn");
        cmd.args(["--host", "git", "-C", path_str(repo_path)]);
        cmd
    } else {
        let mut cmd = Command::new("git");
        cmd.args(["-C", path_str(repo_path)]);
        cmd
    }
}

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
    /// True if any push error looks like an authentication failure.
    pub auth_failed: bool,
}

// ── Query helpers ─────────────────────────────────────────────────────────────

/// Returns the git repository root for the given directory, or None if not in a git repo.
pub fn git_repo_root(dir: &Path) -> Option<std::path::PathBuf> {
    let out = git_cmd(dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() { Some(std::path::PathBuf::from(s)) } else { None }
    } else {
        None
    }
}

/// Returns true if the repo has at least one remote configured.
pub fn has_remote(repo_path: &Path) -> bool {
    git_cmd(repo_path)
        .arg("remote")
        .output()
        .map(|out| !out.stdout.trim_ascii().is_empty())
        .unwrap_or(false)
}

/// Returns the names of all configured remotes.
pub fn list_remotes(repo_path: &Path) -> Vec<String> {
    git_cmd(repo_path)
        .arg("remote")
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
    let out = git_cmd(repo_path)
        .args(["remote", "get-url", name])
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
    add_named_remote(repo_path, "backup", target)
}

/// Add (or update) a remote with `name`. Removes any existing remote with that
/// name first. Local paths get a bare repository initialised automatically.
/// The remote is pushed to on every `sync()` call alongside all other remotes.
pub fn add_named_remote(repo_path: &Path, name: &str, url: &str) -> Result<(), String> {
    let resolved = if is_local_path(url) {
        let expanded = shellexpand::tilde(url).into_owned();
        ensure_bare_repo(Path::new(&expanded))?;
        expanded
    } else {
        url.to_string()
    };
    let _ = run_git(repo_path, &["remote", "remove", name]);
    run_git(repo_path, &["remote", "add", name, &resolved])
}

/// Remove a named remote.
pub fn remove_remote(repo_path: &Path, name: &str) -> Result<(), String> {
    run_git(repo_path, &["remote", "remove", name])
}

/// Return all configured remotes except "origin", paired with their push URL.
/// These are the backup / secondary remotes that `sync()` also pushes to.
pub fn list_backup_remotes(repo_path: &Path) -> Vec<(String, String)> {
    list_remotes(repo_path)
        .into_iter()
        .filter(|n| n != "origin")
        .filter_map(|name| {
            let url = get_remote_url(repo_path, &name)?;
            Some((name, url))
        })
        .collect()
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
    git_cmd(repo_path)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_else(|_| "main".to_string())
}

/// Returns display names of files changed since the last commit.
pub fn changed_files(repo_path: &Path) -> Vec<String> {
    let Ok(out) = git_cmd(repo_path)
        .args(["status", "--porcelain"])
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

/// Stage everything, commit with an auto-crafted message, pull from each remote
/// (rebase), then push to every configured remote.
///
/// `github_token` is injected into HTTPS GitHub remote URLs for authentication.
/// `pushed` is true if at least one remote succeeded.
pub fn sync(repo_path: &Path, github_token: Option<&str>) -> SyncResult {
    let changed = changed_files(repo_path);
    let msg = craft_message(&changed);

    if let Err(e) = run_git(repo_path, &["add", "."]) {
        return SyncResult {
            committed: false, pushed: false, commit_message: msg,
            error: Some(format!("git add: {e}")),
            push_errors: Vec::new(), auth_failed: false,
        };
    }

    let committed = match git_cmd(repo_path)
        .args(["commit", "-m", &msg])
        .output()
    {
        Err(e) => return SyncResult {
            committed: false, pushed: false, commit_message: msg,
            error: Some(format!("git commit: {e}")),
            push_errors: Vec::new(), auth_failed: false,
        },
        Ok(out) if !out.status.success() => {
            let text = lossy_combined(&out);
            if text.contains("nothing to commit") { false } else {
                return SyncResult {
                    committed: false, pushed: false, commit_message: msg,
                    error: Some(text), push_errors: Vec::new(), auth_failed: false,
                };
            }
        }
        Ok(_) => true,
    };

    let remotes = list_remotes(repo_path);
    let branch = current_branch(repo_path);
    let mut pushed = false;
    let mut push_errors: Vec<String> = Vec::new();
    let mut auth_failed = false;

    for remote in &remotes {
        let authed_url = github_token
            .and_then(|_tok| get_remote_url(repo_path, remote))
            .and_then(|url| inject_token_into_url(&url, github_token.unwrap_or("")));

        // Pull --rebase before push so diverged histories are handled.
        let pull_remote = authed_url.as_deref().unwrap_or(remote.as_str());
        if let Ok(pull_out) = git_cmd(repo_path)
            .args(["pull", "--rebase", pull_remote, &branch])
            .output()
        {
            if !pull_out.status.success() {
                // Abort the rebase so the repo is left in a clean state.
                let _ = git_cmd(repo_path).args(["rebase", "--abort"]).output();
                let msg = lossy_combined(&pull_out);
                push_errors.push(format!("({remote}) Pull failed: {msg}"));
                continue;
            }
        }

        let push_remote = authed_url.as_deref().unwrap_or(remote.as_str());
        match git_cmd(repo_path)
            .args(["push", "-u", push_remote, &branch])
            .output() {
            Err(e) => push_errors.push(format!("({remote}) {e}")),
            Ok(o) if !o.status.success() => {
                let msg = lossy_combined(&o);
                if is_auth_error(&msg) { auth_failed = true; }
                push_errors.push(format!("({remote}) {msg}"));
            }
            Ok(_) => pushed = true,
        }
    }

    SyncResult { committed, pushed, commit_message: msg, error: None, push_errors, auth_failed }
}

fn inject_token_into_url(url: &str, token: &str) -> Option<String> {
    if token.is_empty() { return None; }
    if url.starts_with("https://github.com/") {
        let rest = &url["https://github.com/".len()..];
        Some(format!("https://{token}@github.com/{rest}"))
    } else if url.starts_with("https://") {
        // Generic HTTPS: insert token@ after https://
        let rest = &url["https://".len()..];
        // Remove any existing user@
        let rest = if let Some(at) = rest.find('@') { &rest[at + 1..] } else { rest };
        Some(format!("https://{token}@{rest}"))
    } else {
        None
    }
}

fn is_auth_error(msg: &str) -> bool {
    msg.contains("Authentication failed")
        || msg.contains("403")
        || msg.contains("401")
        || msg.contains("could not read Username")
        || msg.contains("remote: Invalid username")
}

// ── Internals ─────────────────────────────────────────────────────────────────

fn path_str(p: &Path) -> &str {
    p.to_str().unwrap_or(".")
}

fn run_git(repo_path: &Path, args: &[&str]) -> Result<(), String> {
    let out = git_cmd(repo_path)
        .args(args)
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
