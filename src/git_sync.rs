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

/// Absolute path to a git shipped inside the application, if there is one.
///
/// The flatpak bundles git because the GNOME runtime has none, and reaching
/// the host's git through `flatpak-spawn` means sync works only for users who
/// already installed git themselves — which for the app's main distribution
/// made "run this in a terminal" a prerequisite for saving your work.
pub fn bundled_git() -> Option<&'static str> {
    const CANDIDATES: [&str; 2] = ["/app/bin/git", "/usr/lib/zerkalo/bin/git"];
    CANDIDATES.into_iter().find(|p| Path::new(p).exists())
}

/// Whether a usable git exists at all — bundled, or on the host.
pub fn git_available() -> bool {
    if bundled_git().is_some() {
        return true;
    }
    host_command("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Returns a `Command` pre-loaded with `git -C <repo>`, using the bundled git
/// when present and `flatpak-spawn --host git` otherwise.
pub(crate) fn git_cmd(repo_path: &Path) -> Command {
    let mut cmd = if let Some(path) = bundled_git() {
        let mut cmd = Command::new(path);
        cmd.args(["-C", path_str(repo_path)]);
        cmd
    } else if in_flatpak() {
        let mut cmd = Command::new("flatpak-spawn");
        cmd.args(["--host", "git", "-C", path_str(repo_path)]);
        cmd
    } else {
        let mut cmd = Command::new("git");
        cmd.args(["-C", path_str(repo_path)]);
        cmd
    };
    // Force English output so the substring matches in is_auth_error() and the
    // "nothing to commit" check below are reliable regardless of the user's locale.
    cmd.env("LANG", "C").env("LC_ALL", "C");
    // Zerkalo's commits (made here, and replayed during `pull --rebase`) run
    // from a background thread with no terminal — if the user's git config
    // has commit.gpgsign on, git tries to launch pinentry to unlock the key
    // and fails with "Inappropriate ioctl for device", silently breaking
    // every sync. This overrides gpgsign for Zerkalo's own git invocations
    // only, leaving the user's global config (and their own commits made
    // elsewhere) untouched.
    cmd.args(["-c", "commit.gpgsign=false"]);
    cmd
}

// ── Knowing when a sync is running or has run ────────────────────────────────

static SYNC_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SYNC_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Whether a sync is in the middle of committing and pulling. Anything else
/// that writes into the folder should wait: a file changing between the commit
/// and the pull makes `pull --rebase` refuse, and the push after it is skipped.
pub fn sync_active() -> bool {
    SYNC_ACTIVE.load(std::sync::atomic::Ordering::SeqCst)
}

/// How many syncs have finished — it moves whenever files may have arrived.
pub fn syncs_finished() -> usize {
    SYNC_COUNT.load(std::sync::atomic::Ordering::SeqCst)
}

struct SyncRunning;

impl SyncRunning {
    fn begin() -> Self {
        SYNC_ACTIVE.store(true, std::sync::atomic::Ordering::SeqCst);
        SyncRunning
    }
}

impl Drop for SyncRunning {
    fn drop(&mut self) {
        SYNC_ACTIVE.store(false, std::sync::atomic::Ordering::SeqCst);
        SYNC_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

// ── Public types ─────────────────────────────────────────────────────────────

#[derive(Debug)]
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
        if !s.is_empty() {
            Some(std::path::PathBuf::from(s))
        } else {
            None
        }
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
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    } else {
        None
    }
}

/// Add (or update) a remote named "backup". Removes any existing "backup" first.
/// If `target` is a local path (absolute, or starts with `~`, `./`, or
/// `../`), a bare git repository is initialised there automatically so the
/// path is ready to receive pushes.
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
    Path::new(s).is_absolute() || s.starts_with('~') || s.starts_with("./") || s.starts_with("../")
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
    let Ok(out) = git_cmd(repo_path).args(["status", "--porcelain"]).output() else {
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

/// Stage everything, commit with an auto-crafted message, pull from each remote
/// (rebase), then push to every configured remote.
///
/// `github_token` authenticates HTTPS GitHub remotes via a per-invocation
/// `http.extraHeader` (see [`apply_github_auth`]), never embedded in the
/// remote URL itself. `pushed` is true if at least one remote succeeded.
pub fn sync(repo_path: &Path, github_token: Option<&str>) -> SyncResult {
    let _running = SyncRunning::begin();
    let changed = changed_files(repo_path);
    let msg = craft_message(&changed);

    if let Err(e) = run_git(repo_path, &["add", "."]) {
        return SyncResult {
            committed: false,
            pushed: false,
            commit_message: msg,
            error: Some(format!("git add: {e}")),
            push_errors: Vec::new(),
            auth_failed: false,
        };
    }

    let committed = match git_cmd(repo_path).args(["commit", "-m", &msg]).output() {
        Err(e) => {
            return SyncResult {
                committed: false,
                pushed: false,
                commit_message: msg,
                error: Some(format!("git commit: {e}")),
                push_errors: Vec::new(),
                auth_failed: false,
            }
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
                    auth_failed: false,
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
        let github_auth: Option<&str> = match github_token {
            Some(tok) if !tok.is_empty() => match get_remote_url(repo_path, remote) {
                Some(url) if is_github_https(&url) => Some(tok),
                _ => None,
            },
            _ => None,
        };

        // Pull --rebase before push so diverged histories are handled.
        let mut pull_cmd = git_cmd(repo_path);
        if let Some(tok) = github_auth {
            apply_github_auth(&mut pull_cmd, tok);
        }
        if let Ok(pull_out) = pull_cmd
            .args(["pull", "--rebase", remote.as_str(), &branch])
            .output()
        {
            if !pull_out.status.success() {
                let msg = lossy_combined(&pull_out);
                if rebase_in_progress(repo_path) {
                    // Abort the rebase so the repo is left in a clean state.
                    match git_cmd(repo_path).args(["rebase", "--abort"]).output() {
                        Ok(a) if !a.status.success() => {
                            let abort_msg = lossy_combined(&a);
                            push_errors.push(format!(
                                "({remote}) Pull failed and rebase --abort also failed: {abort_msg}. \
                                 Repository may be in mid-rebase state — run 'git rebase --abort' manually."
                            ));
                        }
                        Err(e) => {
                            push_errors.push(format!(
                                "({remote}) Pull failed and could not run rebase --abort: {e}. \
                                 Repository may be in mid-rebase state — run 'git rebase --abort' manually."
                            ));
                        }
                        Ok(_) => {
                            push_errors.push(format!("({remote}) Pull failed: {msg}"));
                        }
                    }
                    continue;
                }
                if !is_missing_remote_branch(&msg) {
                    push_errors.push(format!("({remote}) Pull failed: {msg}"));
                    continue;
                }
                // Remote has no such branch yet (first sync to an empty repo):
                // nothing was rebased, and the push below creates it.
            }
        }

        let mut push_cmd = git_cmd(repo_path);
        if let Some(tok) = github_auth {
            apply_github_auth(&mut push_cmd, tok);
        }
        match push_cmd
            .args(["push", "-u", remote.as_str(), &branch])
            .output()
        {
            Err(e) => push_errors.push(format!("({remote}) {e}")),
            Ok(o) if !o.status.success() => {
                let msg = lossy_combined(&o);
                if is_auth_error(&msg) {
                    auth_failed = true;
                }
                push_errors.push(format!("({remote}) {msg}"));
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
        auth_failed,
    }
}

// ── Bringing work from another computer here ────────────────────────────────

/// What `pull` did.
#[derive(Debug, Default)]
pub struct PullResult {
    /// How many commits arrived from the online copy.
    pub new_commits: usize,
    /// Files that arrived or changed (names relative to the folder, a few).
    pub changed_files: Vec<String>,
    /// Edits made here but not yet backed up were saved as a version first.
    pub saved_local_edits: bool,
    /// Nothing has been backed up online for this folder yet.
    pub nothing_online: bool,
    /// The online copy and this folder disagree about the same lines; nothing
    /// here was changed.
    pub conflict: bool,
    pub auth_failed: bool,
    pub error: Option<String>,
}

fn git_ok(repo_path: &Path, args: &[&str]) -> Option<String> {
    git_cmd(repo_path)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn has_commits(repo_path: &Path) -> bool {
    git_ok(repo_path, &["rev-parse", "--verify", "-q", "HEAD"]).is_some()
}

/// The branch this folder is on, even before it has a first commit.
fn head_branch(repo_path: &Path) -> String {
    git_ok(repo_path, &["symbolic-ref", "--short", "-q", "HEAD"])
        .filter(|b| !b.is_empty())
        .unwrap_or_else(|| "main".to_string())
}

/// Which branch of `remote` holds the online copy of `branch`: the same name
/// if it exists there, else the remote's own default (`main`, `master`).
fn online_branch(repo_path: &Path, remote: &str, branch: &str) -> Option<String> {
    let exists = |b: &str| {
        git_ok(
            repo_path,
            &[
                "rev-parse",
                "--verify",
                "-q",
                &format!("refs/remotes/{remote}/{b}"),
            ],
        )
        .is_some()
    };
    if exists(branch) {
        return Some(branch.to_string());
    }
    // Only a folder with no history of its own may take the remote's default
    // branch; one with history keeps to its own branch name.
    if has_commits(repo_path) {
        return None;
    }
    git_ok(
        repo_path,
        &[
            "symbolic-ref",
            "--short",
            "-q",
            &format!("refs/remotes/{remote}/HEAD"),
        ],
    )
    .and_then(|r| r.strip_prefix(&format!("{remote}/")).map(str::to_string))
    .filter(|b| exists(b))
    .or_else(|| {
        ["main", "master"]
            .into_iter()
            .find(|b| exists(b))
            .map(str::to_string)
    })
}

fn fetch(repo_path: &Path, remote: &str, github_token: Option<&str>) -> Result<(), (String, bool)> {
    let mut cmd = git_cmd(repo_path);
    if let (Some(tok), Some(url)) = (github_token, get_remote_url(repo_path, remote)) {
        if !tok.is_empty() && is_github_https(&url) {
            apply_github_auth(&mut cmd, tok);
        }
    }
    match cmd.args(["fetch", "--prune", remote]).output() {
        Err(e) => Err((e.to_string(), false)),
        Ok(o) if !o.status.success() => {
            let msg = lossy_combined(&o);
            let auth = is_auth_error(&msg);
            Err((msg, auth))
        }
        Ok(_) => Ok(()),
    }
}

fn primary_remote(repo_path: &Path) -> Option<String> {
    let remotes = list_remotes(repo_path);
    remotes
        .iter()
        .find(|r| r.as_str() == "origin")
        .or_else(|| remotes.first())
        .cloned()
}

/// Newer work waiting online that this folder doesn't have yet.
#[derive(Debug, PartialEq, Eq)]
pub struct Waiting {
    pub changes: usize,
    /// Bringing it here is a plain catch-up: nothing is waiting to be backed up
    /// from this folder (no loose edits, no unpushed versions), so it can be
    /// done without any chance of the two sides disagreeing.
    pub plain_catch_up: bool,
}

/// What is waiting online that this folder doesn't have yet, or `None` if that
/// can't be told (offline, no online copy, not signed in). Only fetches; it
/// changes nothing in the folder.
pub fn waiting_online(repo_path: &Path, github_token: Option<&str>) -> Option<Waiting> {
    let remote = primary_remote(repo_path)?;
    fetch(repo_path, &remote, github_token).ok()?;
    let branch = head_branch(repo_path);
    let theirs = online_branch(repo_path, &remote, &branch)?;
    let range = if has_commits(repo_path) {
        format!("HEAD..{remote}/{theirs}")
    } else {
        format!("{remote}/{theirs}")
    };
    let changes: usize = git_ok(repo_path, &["rev-list", "--count", &range])?
        .parse()
        .ok()?;
    // New files of Zerkalo's own (comment sidecars, library data) sit untracked in
    // the folder all the time, so only edits to files already being backed up
    // count as "work of its own".
    let plain_catch_up = git_ok(
        repo_path,
        &["status", "--porcelain", "--untracked-files=no"],
    )
    .is_some_and(|s| s.is_empty())
        && has_commits(repo_path)
        // Everything here is already part of the online history.
        && git_cmd(repo_path)
            .args([
                "merge-base",
                "--is-ancestor",
                "HEAD",
                &format!("{remote}/{theirs}"),
            ])
            .output()
            .is_ok_and(|o| o.status.success());
    Some(Waiting {
        changes,
        plain_catch_up,
    })
}

/// Brings everything backed up online down to this folder, without sending
/// anything back. Meant for sitting down at another computer: your edits here
/// (if any) are saved as a version first, then the online work is laid
/// underneath them. A disagreement is never forced — the rebase is undone and
/// both sides are left exactly as they were.
pub fn pull(repo_path: &Path, github_token: Option<&str>) -> PullResult {
    let _running = SyncRunning::begin();
    let mut result = PullResult::default();

    let Some(remote) = primary_remote(repo_path) else {
        result.error = Some("This folder isn't connected to an online backup yet.".into());
        return result;
    };
    if let Err((msg, auth)) = fetch(repo_path, &remote, github_token) {
        result.auth_failed = auth;
        result.error = Some(msg);
        return result;
    }

    let branch = head_branch(repo_path);
    let Some(theirs) = online_branch(repo_path, &remote, &branch) else {
        result.nothing_online = true;
        return result;
    };
    let upstream = format!("{remote}/{theirs}");

    // A folder with no history of its own (a fresh install): lay the online
    // copy straight into it. Git refuses, rather than overwrites, if a file
    // here would be replaced by a different one from online.
    if !has_commits(repo_path) {
        result.changed_files = git_ok(repo_path, &["ls-tree", "-r", "--name-only", &upstream])
            .map(|t| t.lines().take(8).map(str::to_string).collect())
            .unwrap_or_default();
        result.new_commits = git_ok(repo_path, &["rev-list", "--count", &upstream])
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);
        if let Err(e) = git_cmd(repo_path)
            .args(["checkout", "-B", &branch, "--track", &upstream])
            .output()
            .map_err(|e| e.to_string())
            .and_then(|o| {
                if o.status.success() {
                    Ok(())
                } else {
                    Err(lossy_combined(&o))
                }
            })
        {
            result = PullResult {
                error: Some(format!(
                    "Couldn't bring the online copy into this folder (a file here would be replaced): {e}"
                )),
                ..Default::default()
            };
        }
        return result;
    }

    // Edits made here and not yet backed up become a version first, so the
    // rebase below has nothing loose to trip over and nothing can be lost.
    let local = changed_files(repo_path);
    if !local.is_empty() {
        if let Err(e) = run_git(repo_path, &["add", "."]) {
            result.error = Some(format!("git add: {e}"));
            return result;
        }
        let msg = craft_message(&local);
        match git_cmd(repo_path).args(["commit", "-m", &msg]).output() {
            Ok(o) if o.status.success() => result.saved_local_edits = true,
            Ok(o) => {
                let text = lossy_combined(&o);
                if !text.contains("nothing to commit") {
                    result.error = Some(text);
                    return result;
                }
            }
            Err(e) => {
                result.error = Some(format!("git commit: {e}"));
                return result;
            }
        }
    }

    let behind: usize = git_ok(
        repo_path,
        &["rev-list", "--count", &format!("HEAD..{upstream}")],
    )
    .and_then(|n| n.parse().ok())
    .unwrap_or(0);
    if behind == 0 {
        return result;
    }
    result.changed_files = git_ok(
        repo_path,
        &["diff", "--name-only", &format!("HEAD...{upstream}")],
    )
    .map(|t| t.lines().take(8).map(str::to_string).collect())
    .unwrap_or_default();

    let out = git_cmd(repo_path)
        .args(["pull", "--rebase", remote.as_str(), &theirs])
        .output();
    match out {
        Err(e) => {
            result.error = Some(e.to_string());
            result.changed_files.clear();
        }
        Ok(o) if !o.status.success() => {
            let msg = lossy_combined(&o);
            result.changed_files.clear();
            if rebase_in_progress(repo_path) {
                let _ = git_cmd(repo_path).args(["rebase", "--abort"]).output();
                result.conflict = true;
            } else {
                result.auth_failed = is_auth_error(&msg);
                result.error = Some(msg);
            }
        }
        Ok(_) => result.new_commits = behind,
    }
    result
}

/// What `force_pull` did.
#[derive(Debug, Default)]
pub struct ForceResult {
    /// Where everything that was only on this computer was kept, if there was
    /// anything to keep.
    pub saved_as: Option<String>,
    /// Files whose contents differed between this computer and online (a few).
    pub replaced_files: Vec<String>,
    pub nothing_online: bool,
    pub auth_failed: bool,
    pub error: Option<String>,
}

/// Makes this folder match the online copy exactly, whatever is different here.
///
/// Nothing is thrown away outright: first everything here (loose edits, versions
/// never backed up, files that exist only here) is committed to a saved copy — a
/// branch named `before-replace-<date>-<time>` — and only then is the folder
/// reset to the online copy. The saved copy stays in the folder's history, so
/// any of it can be brought back.
pub fn force_pull(repo_path: &Path, github_token: Option<&str>) -> ForceResult {
    let _running = SyncRunning::begin();
    let mut result = ForceResult::default();

    let Some(remote) = primary_remote(repo_path) else {
        result.error = Some("This folder isn't connected to an online backup yet.".into());
        return result;
    };
    if let Err((msg, auth)) = fetch(repo_path, &remote, github_token) {
        result.auth_failed = auth;
        result.error = Some(msg);
        return result;
    }
    let branch = head_branch(repo_path);
    let Some(theirs) = online_branch(repo_path, &remote, &branch) else {
        // No history of its own here, or nothing online: nothing to force.
        if has_commits(repo_path) {
            result.nothing_online = true;
        } else {
            let pulled = pull(repo_path, github_token);
            result.error = pulled.error;
            result.nothing_online = pulled.nothing_online;
        }
        return result;
    };
    let upstream = format!("{remote}/{theirs}");

    if has_commits(repo_path) {
        // 1 · Keep everything that is only here.
        let local = changed_files(repo_path);
        if !local.is_empty() {
            if let Err(e) = run_git(repo_path, &["add", "-A"]) {
                result.error = Some(format!("git add: {e}"));
                return result;
            }
            let msg = format!(
                "Saved before replacing with the online copy\n\n{}",
                craft_message(&local)
            );
            if let Err(e) = git_cmd(repo_path)
                .args(["commit", "-m", &msg])
                .output()
                .map_err(|e| e.to_string())
                .and_then(|o| {
                    if o.status.success() || lossy_combined(&o).contains("nothing to commit") {
                        Ok(())
                    } else {
                        Err(lossy_combined(&o))
                    }
                })
            {
                result.error = Some(format!("git commit: {e}"));
                return result;
            }
        }
        result.replaced_files = git_ok(repo_path, &["diff", "--name-only", &upstream, "HEAD"])
            .map(|t| t.lines().take(8).map(str::to_string).collect())
            .unwrap_or_default();
        let differs = git_ok(
            repo_path,
            &["rev-list", "--count", &format!("{upstream}..HEAD")],
        )
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or(0)
            > 0
            || !result.replaced_files.is_empty();
        if differs {
            let stamp = Local::now().format("%Y%m%d-%H%M%S");
            let name = format!("before-replace-{stamp}");
            if let Err(e) = run_git(repo_path, &["branch", &name, "HEAD"]) {
                result.error = Some(format!(
                    "Couldn't keep a saved copy, so nothing was changed: {e}"
                ));
                return result;
            }
            result.saved_as = Some(name);
        }
    }

    // 2 · Now make this folder match the online copy.
    let reset = git_cmd(repo_path)
        .args(["checkout", "-B", &branch, "--track", &upstream, "--force"])
        .output();
    match reset {
        Err(e) => result.error = Some(e.to_string()),
        Ok(o) if !o.status.success() => result.error = Some(lossy_combined(&o)),
        Ok(_) => {}
    }
    result
}

/// Whether `url` is an `https://github.com/...` remote — the only case the
/// stored OAuth token is authorized for. Never send it to any other host.
fn is_github_https(url: &str) -> bool {
    url.starts_with("https://github.com/")
}

/// Authenticates `cmd` as `token`, scoped to `https://github.com/` only, via
/// `GIT_CONFIG_COUNT`/`GIT_CONFIG_KEY_0`/`GIT_CONFIG_VALUE_0` (git >= 2.31)
/// rather than a `-c http.<url>.extraHeader=...` argv entry. Command-line
/// arguments are visible to any local user for the life of the process (this
/// machine's `/proc/<pid>/cmdline` is world-readable, and even with
/// `hidepid` set elsewhere that's not guaranteed) — base64-encoding the
/// token, as the previous argv-based version did, only obscures it, since
/// base64 is trivially reversible. Environment variables land in
/// `/proc/<pid>/environ` instead, which is owner-only regardless of
/// `hidepid`. Never embedded in the remote URL either, so it doesn't show up
/// in `git remote -v`.
fn apply_github_auth(cmd: &mut Command, token: &str) {
    let encoded = base64_encode(format!("x-access-token:{token}").as_bytes());
    cmd.env("GIT_CONFIG_COUNT", "1");
    cmd.env("GIT_CONFIG_KEY_0", "http.https://github.com/.extraHeader");
    cmd.env(
        "GIT_CONFIG_VALUE_0",
        format!("AUTHORIZATION: basic {encoded}"),
    );
}

fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        out.push(ALPHABET[(n >> 18 & 0x3F) as usize] as char);
        out.push(ALPHABET[(n >> 12 & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6 & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Whether a rebase is actually stopped mid-flight. `git pull --rebase` can
/// fail before starting one at all (empty remote, unreachable host), and
/// running `rebase --abort` then fails with "no rebase in progress" — which
/// used to be reported to the user as a scary mid-rebase warning on the very
/// first sync to a brand-new repository.
fn rebase_in_progress(repo_path: &Path) -> bool {
    ["rebase-merge", "rebase-apply"].iter().any(|name| {
        git_cmd(repo_path)
            .args(["rev-parse", "--git-path", name])
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| {
                let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let path = Path::new(&p);
                !p.is_empty()
                    && if path.is_absolute() {
                        path.exists()
                    } else {
                        repo_path.join(path).exists()
                    }
            })
            .unwrap_or(false)
    })
}

/// A pull failure that only means the remote doesn't have this branch yet —
/// the normal case when syncing to a freshly created, empty repository. The
/// push that follows creates the branch, so this must not abort the sync.
fn is_missing_remote_branch(msg: &str) -> bool {
    msg.contains("couldn't find remote ref")
        || msg.contains("Couldn't find remote ref")
        || msg.contains("does not have any commits yet")
        || msg.contains("no such ref was fetched")
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
    if !stderr.is_empty() {
        stderr
    } else {
        stdout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_local_path_recognizes_absolute_and_relative_paths() {
        assert!(is_local_path("/home/user/repo"));
        assert!(is_local_path("~/repo"));
        assert!(is_local_path("./repo"));
        assert!(is_local_path("../repo"));
    }

    #[test]
    fn is_local_path_rejects_urls() {
        assert!(!is_local_path("https://github.com/foo/bar.git"));
        assert!(!is_local_path("git@github.com:foo/bar.git"));
    }

    #[test]
    fn craft_message_no_changes() {
        let msg = craft_message(&[]);
        assert!(msg.starts_with("Auto-save: "));
    }

    #[test]
    fn craft_message_single_file() {
        let msg = craft_message(&["main.typ".to_string()]);
        assert!(msg.starts_with("Edited main.typ: "));
    }

    #[test]
    fn craft_message_multiple_files_lists_up_to_five() {
        let files: Vec<String> = (1..=7).map(|i| format!("f{i}.typ")).collect();
        let msg = craft_message(&files);
        assert!(
            msg.starts_with("Edits to f1.typ, f2.typ, f3.typ, f4.typ, f5.typ (+2)"),
            "got: {msg}"
        );
    }

    #[test]
    fn craft_message_exactly_five_files_no_suffix() {
        let files: Vec<String> = (1..=5).map(|i| format!("f{i}.typ")).collect();
        let msg = craft_message(&files);
        assert!(
            msg.starts_with("Edits to f1.typ, f2.typ, f3.typ, f4.typ, f5.typ\n"),
            "got: {msg}"
        );
        assert!(!msg.contains('+'));
    }

    #[test]
    fn is_github_https_matches_only_github_com() {
        assert!(is_github_https("https://github.com/user/repo.git"));
        assert!(!is_github_https("https://example.com/repo.git"));
        assert!(!is_github_https("git@github.com:user/repo.git"));
    }

    #[test]
    fn apply_github_auth_scopes_header_to_github_and_keeps_token_out_of_argv() {
        let mut cmd = Command::new("git");
        apply_github_auth(&mut cmd, "abc123");

        let envs: std::collections::HashMap<_, _> = cmd.get_envs().collect();
        assert_eq!(
            envs.get(std::ffi::OsStr::new("GIT_CONFIG_KEY_0"))
                .copied()
                .flatten(),
            Some(std::ffi::OsStr::new("http.https://github.com/.extraHeader"))
        );
        let value = envs
            .get(std::ffi::OsStr::new("GIT_CONFIG_VALUE_0"))
            .copied()
            .flatten()
            .expect("GIT_CONFIG_VALUE_0 must be set")
            .to_str()
            .unwrap();
        assert!(value.starts_with("AUTHORIZATION: basic "));
        assert!(
            !value.contains("abc123"),
            "raw token must not appear even in the (env-only) auth header: {value}"
        );

        // The token must never appear in argv at all — env vars, unlike
        // arguments, aren't visible via /proc/<pid>/cmdline.
        let args: Vec<_> = cmd.get_args().collect();
        assert!(
            args.is_empty(),
            "token must not be passed as an argument: {args:?}"
        );
    }

    #[test]
    fn base64_encode_matches_known_vectors() {
        assert_eq!(
            base64_encode(b"x-access-token:abc123"),
            "eC1hY2Nlc3MtdG9rZW46YWJjMTIz"
        );
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"a"), "YQ==");
        assert_eq!(base64_encode(b"ab"), "YWI=");
        assert_eq!(base64_encode(b"abc"), "YWJj");
    }

    #[test]
    fn is_auth_error_detects_common_auth_failures() {
        assert!(is_auth_error("remote: Authentication failed"));
        assert!(is_auth_error(
            "fatal: could not read Username for 'https://...'"
        ));
        assert!(is_auth_error("received 403 Forbidden"));
    }

    #[test]
    fn is_missing_remote_branch_detects_empty_remote() {
        assert!(is_missing_remote_branch(
            "fatal: couldn't find remote ref main"
        ));
        assert!(is_missing_remote_branch(
            "Your configuration specifies to merge with the ref 'main' from the remote, \
             but no such ref was fetched."
        ));
    }

    #[test]
    fn is_missing_remote_branch_false_for_real_failures() {
        assert!(!is_missing_remote_branch("fatal: Authentication failed"));
        assert!(!is_missing_remote_branch(
            "CONFLICT (content): Merge conflict in main.typ"
        ));
        assert!(!is_missing_remote_branch(
            "fatal: could not resolve host: github.com"
        ));
    }

    #[test]
    fn is_auth_error_false_for_unrelated_errors() {
        assert!(!is_auth_error("fatal: not a git repository"));
    }

    // ── Pulling work from another computer, with real git ────────────────────

    fn g(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            lossy_combined(&out)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn identify(dir: &Path) {
        g(dir, &["config", "user.name", "T"]);
        g(dir, &["config", "user.email", "t@example.org"]);
        g(dir, &["config", "commit.gpgsign", "false"]);
    }

    /// An online copy, and one computer that has written and backed up a file.
    fn online_with_one_commit() -> (tempfile::TempDir, tempfile::TempDir) {
        let remote = tempfile::tempdir().unwrap();
        g(remote.path(), &["init", "--bare", "-b", "main"]);
        let a = tempfile::tempdir().unwrap();
        g(a.path(), &["init", "-b", "main"]);
        identify(a.path());
        std::fs::write(a.path().join("essay.typ"), "first draft\n").unwrap();
        g(a.path(), &["add", "."]);
        g(a.path(), &["commit", "-m", "one"]);
        g(
            a.path(),
            &["remote", "add", "origin", remote.path().to_str().unwrap()],
        );
        g(a.path(), &["push", "-u", "origin", "main"]);
        (remote, a)
    }

    #[test]
    fn a_fresh_folder_gets_everything_from_the_online_copy() {
        let (remote, _a) = online_with_one_commit();
        let b = tempfile::tempdir().unwrap();
        g(b.path(), &["init", "-b", "main"]);
        identify(b.path());
        g(
            b.path(),
            &["remote", "add", "origin", remote.path().to_str().unwrap()],
        );
        let r = pull(b.path(), None);
        assert!(r.error.is_none(), "{:?}", r.error);
        assert_eq!(r.new_commits, 1);
        assert_eq!(
            std::fs::read_to_string(b.path().join("essay.typ")).unwrap(),
            "first draft\n"
        );
        assert_eq!(g(b.path(), &["status", "--porcelain"]), "");
    }

    #[test]
    fn work_in_progress_from_the_other_computer_arrives_and_local_edits_are_kept() {
        let (remote, a) = online_with_one_commit();
        let b = tempfile::tempdir().unwrap();
        g(b.path(), &["clone", remote.path().to_str().unwrap(), "."]);
        identify(b.path());
        // Computer A keeps writing and backs it up.
        std::fs::write(a.path().join("essay.typ"), "first draft\nmore from A\n").unwrap();
        std::fs::write(a.path().join("notes.typ"), "from A\n").unwrap();
        g(a.path(), &["add", "."]);
        g(a.path(), &["commit", "-m", "two"]);
        g(a.path(), &["push"]);
        // Computer B has an unrelated edit it hasn't backed up.
        std::fs::write(b.path().join("other.typ"), "B's own work\n").unwrap();
        let remote_head_before = g(remote.path(), &["rev-parse", "main"]);

        let w = waiting_online(b.path(), None).unwrap();
        assert_eq!(w.changes, 1);
        // B only has a new file of its own: still a plain catch-up (the file is
        // simply kept, untouched).
        assert!(w.plain_catch_up);
        let r = pull(b.path(), None);
        assert!(r.error.is_none() && !r.conflict, "{r:?}");
        assert_eq!(r.new_commits, 1);
        assert!(r.saved_local_edits);
        assert!(r.changed_files.contains(&"notes.typ".to_string()));
        assert_eq!(
            std::fs::read_to_string(b.path().join("essay.typ")).unwrap(),
            "first draft\nmore from A\n"
        );
        assert_eq!(
            std::fs::read_to_string(b.path().join("other.typ")).unwrap(),
            "B's own work\n"
        );
        // Nothing was sent back online.
        assert_eq!(g(remote.path(), &["rev-parse", "main"]), remote_head_before);
        assert_eq!(waiting_online(b.path(), None).unwrap().changes, 0);
    }

    #[test]
    fn a_disagreement_changes_nothing_and_says_so() {
        let (remote, a) = online_with_one_commit();
        let b = tempfile::tempdir().unwrap();
        g(b.path(), &["clone", remote.path().to_str().unwrap(), "."]);
        identify(b.path());
        std::fs::write(a.path().join("essay.typ"), "A rewrote this\n").unwrap();
        g(a.path(), &["commit", "-am", "A"]);
        g(a.path(), &["push"]);
        std::fs::write(b.path().join("essay.typ"), "B rewrote this differently\n").unwrap();

        let r = pull(b.path(), None);
        assert!(r.conflict, "{r:?}");
        assert!(r.changed_files.is_empty());
        assert!(!rebase_in_progress(b.path()));
        // B's words are still there, committed as a version, not lost.
        assert_eq!(
            std::fs::read_to_string(b.path().join("essay.typ")).unwrap(),
            "B rewrote this differently\n"
        );
        assert_eq!(g(b.path(), &["status", "--porcelain"]), "");
    }

    #[test]
    fn nothing_online_yet_is_reported_not_treated_as_an_error() {
        let remote = tempfile::tempdir().unwrap();
        g(remote.path(), &["init", "--bare", "-b", "main"]);
        let b = tempfile::tempdir().unwrap();
        g(b.path(), &["init", "-b", "main"]);
        identify(b.path());
        std::fs::write(b.path().join("a.typ"), "x\n").unwrap();
        g(b.path(), &["add", "."]);
        g(b.path(), &["commit", "-m", "x"]);
        g(
            b.path(),
            &["remote", "add", "origin", remote.path().to_str().unwrap()],
        );
        let r = pull(b.path(), None);
        assert!(r.nothing_online && r.error.is_none());
        assert_eq!(waiting_online(b.path(), None), None);
    }

    #[test]
    fn no_online_copy_set_up_is_a_plain_message() {
        let b = tempfile::tempdir().unwrap();
        g(b.path(), &["init", "-b", "main"]);
        let r = pull(b.path(), None);
        assert!(r.error.is_some());
    }

    #[test]
    fn a_clean_folder_that_is_simply_behind_is_a_plain_catch_up() {
        let (remote, a) = online_with_one_commit();
        let b = tempfile::tempdir().unwrap();
        g(b.path(), &["clone", remote.path().to_str().unwrap(), "."]);
        identify(b.path());
        std::fs::write(a.path().join("essay.typ"), "first draft\nand more\n").unwrap();
        g(a.path(), &["commit", "-am", "two"]);
        g(a.path(), &["push"]);
        let w = waiting_online(b.path(), None).unwrap();
        assert_eq!((w.changes, w.plain_catch_up), (1, true));
        // An edit to a file already being backed up means it is no longer plain…
        std::fs::write(b.path().join("essay.typ"), "B edited this\n").unwrap();
        assert!(!waiting_online(b.path(), None).unwrap().plain_catch_up);
        g(b.path(), &["checkout", "--", "essay.typ"]);
        // …and so is a version here that was never backed up.
        std::fs::write(b.path().join("b.typ"), "mine\n").unwrap();
        g(b.path(), &["add", "."]);
        g(b.path(), &["commit", "-m", "mine"]);
        let w = waiting_online(b.path(), None).unwrap();
        assert!(!w.plain_catch_up);
    }

    #[test]
    fn forcing_makes_this_folder_match_online_and_keeps_what_was_only_here() {
        let (remote, a) = online_with_one_commit();
        let b = tempfile::tempdir().unwrap();
        g(b.path(), &["clone", remote.path().to_str().unwrap(), "."]);
        identify(b.path());
        // Online moves on…
        std::fs::write(a.path().join("essay.typ"), "A's version\n").unwrap();
        g(a.path(), &["commit", "-am", "A"]);
        g(a.path(), &["push"]);
        // …while B has a backed-up-nowhere version, a loose edit, and a file of its own.
        std::fs::write(b.path().join("essay.typ"), "B's competing version\n").unwrap();
        g(b.path(), &["commit", "-am", "B"]);
        std::fs::write(
            b.path().join("essay.typ"),
            "B's competing version, edited again\n",
        )
        .unwrap();
        std::fs::write(b.path().join("only-here.typ"), "mine alone\n").unwrap();

        // A normal pull refuses…
        assert!(pull(b.path(), None).conflict);
        // …force replaces, but keeps everything.
        let r = force_pull(b.path(), None);
        assert!(r.error.is_none(), "{r:?}");
        let saved = r.saved_as.clone().expect("a saved copy was made");
        assert!(saved.starts_with("before-replace-"));
        assert!(r.replaced_files.contains(&"essay.typ".to_string()));
        assert_eq!(
            std::fs::read_to_string(b.path().join("essay.typ")).unwrap(),
            "A's version\n"
        );
        assert!(!b.path().join("only-here.typ").exists());
        assert_eq!(g(b.path(), &["status", "--porcelain"]), "");
        // The saved copy still holds all of B's words.
        assert_eq!(
            g(b.path(), &["show", &format!("{saved}:essay.typ")]),
            "B's competing version, edited again"
        );
        assert_eq!(
            g(b.path(), &["show", &format!("{saved}:only-here.typ")]),
            "mine alone"
        );
        // Nothing was sent online.
        assert_eq!(
            g(remote.path(), &["rev-parse", "main"]),
            g(a.path(), &["rev-parse", "HEAD"])
        );
    }

    #[test]
    fn forcing_when_already_identical_changes_nothing_and_keeps_no_copy() {
        let (remote, _a) = online_with_one_commit();
        let b = tempfile::tempdir().unwrap();
        g(b.path(), &["clone", remote.path().to_str().unwrap(), "."]);
        identify(b.path());
        let r = force_pull(b.path(), None);
        assert!(r.error.is_none() && r.saved_as.is_none(), "{r:?}");
    }
}
