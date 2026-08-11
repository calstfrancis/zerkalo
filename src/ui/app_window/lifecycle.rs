//! Startup and lifecycle: the missing-tool and unreadable-settings warnings,
//! the welcome window and its chained setup wizard, idle auto-backup, and the
//! language server's deferred initialisation and diagnostics poll.
//! Split out of `AppWindow::new`.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use libadwaita as adw;

use crate::config::Config;
use crate::lsp::{DiagSeverity, LspClient};
use super::super::editor_pane::EditorPane;
use super::super::error_panel::{CompileError, ErrorPanel, Severity, humanize};

/// What the startup and lifecycle wiring needs from `AppWindow::new`.
pub(super) struct LifecycleCtx {
    pub(super) window: adw::ApplicationWindow,
    pub(super) editor_pane: EditorPane,
    pub(super) error_panel: ErrorPanel,
    pub(super) toast_overlay: adw::ToastOverlay,
    pub(super) current_config: Rc<RefCell<Config>>,
    pub(super) project_root: PathBuf,
    pub(super) auto_save_idle_ms: Rc<RefCell<u64>>,
    pub(super) sync_btn: gtk4::Button,
    pub(super) lsp_client: Rc<RefCell<Option<LspClient>>>,
    pub(super) lsp_has_diags: Rc<RefCell<bool>>,
    pub(super) last_completion_request: Rc<RefCell<Option<u64>>>,
    pub(super) last_edit_instant: Rc<RefCell<Option<std::time::Instant>>>,
    /// The config as loaded at startup, for the one-shot intro check.
    pub(super) shown_simple_intro: bool,
}

pub(super) fn wire_startup(ctx: &LifecycleCtx) {
    // ── Startup: warn if required tools are missing ──────────────────────

    // ── Startup: report an unreadable settings file ──────────────────────

    if let Some(problem) = crate::config::take_load_problem() {
        let toast_for_cfg = ctx.toast_overlay.clone();
        let backup = problem
            .backup
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| problem.backup.display().to_string());
        let msg = format!(
            "Settings could not be read ({}). Backed up as {backup}; defaults are in use.",
            problem.error
        );
        glib::timeout_add_local_once(Duration::from_millis(700), move || {
            let t = adw::Toast::new(&msg);
            t.set_timeout(10);
            toast_for_cfg.add_toast(t);
        });
    }

    // ── Startup: optional tools ──────────────────────────────────────────
    //
    // This used to open a modal alert listing sudo commands over the top of a
    // brand-new window — the first thing a first-time user saw, about tools
    // that are now bundled anyway. Missing optional tools are logged and shown
    // in ☰ → Tools; only a missing git, which nothing works without, still
    // interrupts, and in the flatpak there is always one.
    let toast_for_check = ctx.toast_overlay.clone();
    glib::timeout_add_local(Duration::from_millis(900), move || {
        if !crate::git_sync::git_available() {
            tracing::warn!("git not found");
            let t = adw::Toast::new("git isn't installed — saving versions won't work. See ☰ → Tools.");
            t.set_timeout(12);
            toast_for_check.add_toast(t);
        }
        if std::process::Command::new("hunspell").arg("--version").output().is_err() {
            tracing::info!("hunspell not found — spell check disabled");
        }
        if crate::git_sync::host_command("pandoc").arg("--version").output().is_err() {
            tracing::info!("pandoc not found — LaTeX/HTML/EPUB/RTF import disabled");
        }
        let tinymist_ok = ["/app/lib/zerkalo/tinymist", "/usr/lib/zerkalo/tinymist"]
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .map(|p| std::process::Command::new(p).arg("--version").output().is_ok())
            .unwrap_or_else(|| std::process::Command::new("tinymist").arg("--version").output().is_ok());
        if !tinymist_ok {
            tracing::info!("tinymist not found — LSP completions disabled");
        }
        glib::ControlFlow::Break
    });

    // ── Welcome ctx.window + chained setup wizard ────────────────────────────
    // Simple-mode intro is now part of the welcome ctx.window, so we no longer
    // need a separate dialog for it. Mark shown_simple_intro if not already set.
    if !ctx.shown_simple_intro {
        let cfg_for_intro = ctx.current_config.clone();
        cfg_for_intro.borrow_mut().shown_simple_intro = true;
        let _ = cfg_for_intro.borrow().save();
    }

    let win_for_welcome = ctx.window.clone();
    let root_for_welcome = ctx.project_root.clone();
    glib::timeout_add_local(Duration::from_millis(1200), move || {
        if super::super::welcome_window::WelcomeWindow::should_show() {
            let is_first_run = super::super::welcome_window::WelcomeWindow::is_first_run();
            super::super::welcome_window::WelcomeWindow::mark_shown();
            let ww = super::super::welcome_window::WelcomeWindow::new(&win_for_welcome, is_first_run);
            // Chain: after "Get Started", check if setup wizard is needed.
            let win_chain = win_for_welcome.clone();
            let root_chain = root_for_welcome.clone();
            ww.set_on_dismissed(move || {
                if super::super::setup_wizard::SetupWizard::should_show(&root_chain) {
                    super::super::setup_wizard::SetupWizard::new(&win_chain, &root_chain).present();
                }
            });
            ww.present();
        } else if super::super::setup_wizard::SetupWizard::should_show(&root_for_welcome) {
            super::super::setup_wizard::SetupWizard::new(&win_for_welcome, &root_for_welcome).present();
        }
        glib::ControlFlow::Break
    });

    // ── Auto-backup on idle: write modified buffers after idle for ctx.auto_save_idle_ms ──

    let editor_for_autosave = ctx.editor_pane.clone();
    let toast_for_autosave = ctx.toast_overlay.clone();
    let last_edit_for_autosave = ctx.last_edit_instant.clone();
    let idle_ms_for_autosave = ctx.auto_save_idle_ms.clone();
    glib::timeout_add_local(Duration::from_secs(5), move || {
        let idle_threshold = *idle_ms_for_autosave.borrow();
        let elapsed = last_edit_for_autosave
            .borrow()
            .map(|t| t.elapsed().as_millis() as u64);
        if let Some(ms) = elapsed {
            // Autosave even when there are compile errors — the recovery
            // dialog lets the user choose whether to restore.
            if ms >= idle_threshold {
                let buffers: Vec<_> = editor_for_autosave.modified_buffers();
                if !buffers.is_empty() {
                    for (path, content) in &buffers {
                        crate::auto_save::save(path, content);
                    }
                    let t = adw::Toast::new("Autosaved");
                    t.set_timeout(2);
                    toast_for_autosave.add_toast(t);
                    *last_edit_for_autosave.borrow_mut() = None;
                }
            }
        }
        glib::ControlFlow::Continue
    });

    // ── Backup: sync periodically, so non-technical users never have to find
    // the Sync button themselves ─────────────────────────────────────────
    //
    // Only runs once a backup location is configured (the setup wizard or
    // ☰ → Backup Locations…), and only when there's something uncommitted —
    // otherwise it's a no-op check every 30s, not a sync. Quiet: failures
    // show a toast, never a blocking dialog, so an offline user isn't
    // interrupted while writing.
    const AUTO_SYNC_INTERVAL: Duration = Duration::from_secs(12 * 60);
    let editor_for_autosync = ctx.editor_pane.clone();
    let overlay_for_autosync = ctx.toast_overlay.clone();
    let sync_btn_for_autosync = ctx.sync_btn.clone();
    let root_for_autosync = ctx.project_root.clone();
    let last_auto_sync: Rc<RefCell<Option<std::time::Instant>>> = Rc::new(RefCell::new(None));
    glib::timeout_add_local(Duration::from_secs(30), move || {
        let root = editor_for_autosync
            .get_active_path()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .and_then(|dir| crate::git_sync::git_repo_root(&dir))
            .unwrap_or_else(|| root_for_autosync.clone());

        let due = last_auto_sync
            .borrow()
            .map(|t| t.elapsed() >= AUTO_SYNC_INTERVAL)
            .unwrap_or(true);

        if due
            && sync_btn_for_autosync.is_sensitive()
            && crate::git_sync::has_remote(&root)
            && !crate::git_sync::changed_files(&root).is_empty()
        {
            *last_auto_sync.borrow_mut() = Some(std::time::Instant::now());
            editor_for_autosync.save_all_modified();
            sync_btn_for_autosync.set_sensitive(false);
            let btn_done = sync_btn_for_autosync.clone();
            super::sync::auto_sync_quiet(
                root,
                Some(overlay_for_autosync.clone()),
                crate::secret_store::load_github_token(),
                move || btn_done.set_sensitive(true),
            );
        }
        glib::ControlFlow::Continue
    });

    // ── LSP: initialise 500 ms after startup ────────────────────────────

    let lsp_init = ctx.lsp_client.clone();
    let root_for_lsp = ctx.project_root.clone();
    let editor_for_lsp_init = ctx.editor_pane.clone();
    glib::timeout_add_local(Duration::from_millis(500), move || {
        *lsp_init.borrow_mut() = LspClient::new(&root_for_lsp);
        let ready = lsp_init.borrow().is_some();
        editor_for_lsp_init.set_lsp_available(ready);
        if ready {
            tracing::info!("tinymist LSP active");
            editor_for_lsp_init.set_lsp_status("LSP ●");
        } else {
            tracing::info!("tinymist not found — LSP disabled");
            editor_for_lsp_init.set_lsp_status("");
        }
        glib::ControlFlow::Break
    });

    // ── LSP: poll for diagnostics + completions + auto-restart ──────────

    let lsp_poll = ctx.lsp_client.clone();
    let error_panel_for_lsp = ctx.error_panel.clone();
    let editor_for_comp_poll = ctx.editor_pane.clone();
    let editor_for_lsp_diag = ctx.editor_pane.clone();
    let editor_for_lsp_status = ctx.editor_pane.clone();
    let last_req_poll = ctx.last_completion_request.clone();
    let lsp_diags_for_poll = ctx.lsp_has_diags.clone();
    // Grace-period counter: only clear ctx.lsp_has_diags after 3 consecutive
    // empty polls (~1.2 s), preventing flicker between a did_change and the
    // LSP's next diagnostic response.
    let lsp_empty_polls: Rc<RefCell<u8>> = Rc::new(RefCell::new(0));
    glib::timeout_add_local(Duration::from_millis(400), move || {
        // Auto-restart if tinymist crashed
        {
            let mut slot = lsp_poll.borrow_mut();
            if let Some(client) = slot.as_mut() {
                if !client.is_alive() {
                    tracing::warn!("tinymist crashed — restarting");
                    editor_for_lsp_status.set_lsp_status("LSP ↻");
                    let root = client.root.clone();
                    *slot = LspClient::new(&root);
                    if slot.is_some() {
                        editor_for_lsp_status.set_lsp_status("LSP ●");
                    } else {
                        editor_for_lsp_status.set_lsp_status("LSP ✗");
                    }
                }
            }
        }
        // Collect all LSP data in a scoped borrow, then release it before
        // any GTK ops. mark_diagnostics / show_lsp_completions call
        // buffer.create_source_mark / popover.popup, which cascade through
        // GtkSourceView signals that re-enter Zerkalo callbacks — those
        // callbacks may try to borrow ctx.lsp_client, causing a BorrowError
        // panic if the borrow is still held.
        let lsp_data: Option<(Vec<_>, Option<_>)> = {
            let slot = lsp_poll.borrow();
            slot.as_ref().map(|client| (client.poll(), client.poll_completion()))
        };
        if let Some((raw_diags, completion_result)) = lsp_data {
            if !raw_diags.is_empty() {
                *lsp_empty_polls.borrow_mut() = 0;
                *lsp_diags_for_poll.borrow_mut() = true;
                let errors: Vec<CompileError> = raw_diags
                    .into_iter()
                    .map(|d| {
                        let severity = match d.severity {
                            DiagSeverity::Error => Severity::Error,
                            _ => Severity::Warning,
                        };
                        // The language server's diagnostics go through the
                        // same plain-language pass as the compiler's, so the
                        // wording doesn't change depending on which one
                        // happened to report the problem.
                        let (message, advice) = humanize(&d.message);
                        CompileError {
                            file: d.file,
                            line: d.line,
                            col: d.col,
                            message,
                            advice,
                            hints: Vec::new(),
                            technical: d.message,
                            severity,
                        }
                    })
                    .collect();
                let diag_marks: Vec<(std::path::PathBuf, u32, bool, String)> = errors
                    .iter()
                    .map(|e| (e.file.clone(), e.line, matches!(e.severity, Severity::Error), e.message.clone()))
                    .collect();
                let err_count = diag_marks.iter().filter(|(_, _, is_err, _)| *is_err).count() as u32;
                let warn_count = diag_marks.iter().filter(|(_, _, is_err, _)| !*is_err).count() as u32;
                editor_for_lsp_diag.mark_diagnostics(&diag_marks);
                let error_lines: Vec<(std::path::PathBuf, u32)> = errors.iter()
                    .filter(|e| matches!(e.severity, Severity::Error))
                    .map(|e| (e.file.clone(), e.line))
                    .collect();
                editor_for_lsp_diag.mark_error_lines(&error_lines);
                error_panel_for_lsp.show_errors(errors);
                error_panel_for_lsp.widget().set_visible(true);
                editor_for_lsp_diag.set_diag_summary(err_count, warn_count);
            } else {
                let count = {
                    let mut c = lsp_empty_polls.borrow_mut();
                    *c = c.saturating_add(1);
                    *c
                };
                if count >= 3 {
                    *lsp_diags_for_poll.borrow_mut() = false;
                }
            }
            if let Some((id, items)) = completion_result {
                if *last_req_poll.borrow() == Some(id) {
                    editor_for_comp_poll.show_lsp_completions(items);
                }
            }
        }
        glib::ControlFlow::Continue
    });

    // There is deliberately no periodic write-to-disk timer here. One used
    // to call `save_all_modified()` every 30 s, which wrote every modified
    // buffer to its real path, cleared the modified flag, and deleted the
    // recovery copy — so the idle autosave above could never find anything
    // to save and `find_recovery` could never see an autosave newer than
    // the file, making the whole crash-recovery path unreachable. The file
    // on disk now changes only when the user saves.

}
