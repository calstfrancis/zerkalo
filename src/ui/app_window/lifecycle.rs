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

use super::super::editor_pane::EditorPane;
use super::super::error_panel::{CompileError, Severity};
use super::ERROR_GRACE;
use crate::config::Config;
use crate::lsp::{DiagSeverity, LspClient};
use crate::ui::notify::Say;

/// What the startup and lifecycle wiring needs from `AppWindow::new`.
pub(super) struct LifecycleCtx {
    pub(super) window: adw::ApplicationWindow,
    pub(super) editor_pane: EditorPane,
    pub(super) toast_overlay: adw::ToastOverlay,
    pub(super) current_config: Rc<RefCell<Config>>,
    pub(super) project_root: PathBuf,
    pub(super) auto_save_idle_ms: Rc<RefCell<u64>>,
    pub(super) sync_btn: gtk4::Button,
    pub(super) sync_badge: gtk4::Label,
    /// "New from template…" menu button — the welcome window's "Get Started"
    /// fires this directly rather than just closing, so a first-time user
    /// lands in document creation instead of a blank window.
    pub(super) menu_new_template_item: gtk4::Button,
    pub(super) menu_new_item: gtk4::Button,
    pub(super) lsp_client: Rc<RefCell<Option<LspClient>>>,
    pub(super) problems: super::super::problems::Problems,
    pub(super) last_completion_request: Rc<RefCell<Option<u64>>>,
    pub(super) last_edit_instant: Rc<RefCell<Option<std::time::Instant>>>,
    /// The config as loaded at startup, for the one-shot intro check.
    pub(super) shown_simple_intro: bool,
    /// Set by `AppWindow::new` once the guided tour exists — it's built after
    /// `wire_startup` runs, so this is how the welcome window's dismiss
    /// handler (registered here, fired later) reaches something that doesn't
    /// exist yet at the time this function runs. `FnOnce` because it only
    /// ever fires once, on the very first launch.
    pub(super) start_tour_after_welcome: Rc<RefCell<Option<Box<dyn FnOnce()>>>>,
}

/// How long after the last keystroke autosave writes the document: long enough
/// not to save mid-word, short enough that little is ever at risk.
const AUTOSAVE_DELAY: Duration = Duration::from_secs(3);

/// Saves every modified document, when the status bar's autosave is on. Called
/// after a pause in typing and whenever the user moves away from what they were
/// writing. A failure shows in the editor's save-problem bar, which stays until
/// the save goes through — the idle check retries every second.
pub(super) fn autosave_now(editor: &EditorPane) {
    if editor.autosave_enabled() {
        editor.save_all_modified();
    }
}

pub(super) fn wire_startup(ctx: &LifecycleCtx) {
    // ── The calm line: "Saved · backed up 12 min ago" ────────────────────
    // The cheap part (is anything unsaved?) refreshes every few seconds; the
    // online part asks git, so it runs on a thread every half minute.
    {
        use std::sync::mpsc;
        let status: Rc<RefCell<crate::recover::Status>> =
            Rc::new(RefCell::new(crate::recover::Status::default()));
        let (tx, rx) = mpsc::channel::<crate::recover::Status>();
        let root = ctx.project_root.clone();
        let ask = move |tx: mpsc::Sender<crate::recover::Status>| {
            let root = root.clone();
            std::thread::spawn(move || {
                let _ = tx.send(crate::recover::status(&root));
            });
        };
        ask.clone()(tx.clone());
        let editor = ctx.editor_pane.clone();
        let ticks = Rc::new(std::cell::Cell::new(0u32));
        glib::timeout_add_local(Duration::from_secs(3), move || {
            while let Ok(s) = rx.try_recv() {
                *status.borrow_mut() = s;
            }
            ticks.set(ticks.get() + 1);
            if ticks.get().is_multiple_of(10) {
                ask.clone()(tx.clone());
            }
            editor.set_safety_line(&crate::recover::safety_line(
                editor.any_modified(),
                editor.autosave_enabled(),
                &status.borrow(),
                chrono::Local::now(),
            ));
            glib::ControlFlow::Continue
        });
    }

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
            "Your settings couldn't be read — {}. The old file was kept as {backup}, and Zerkalo \
             is using its default settings for now.",
            problem.error
        );
        // Held until nothing else is open over the window, so it isn't lost
        // behind (or piled on top of) the welcome or setup windows.
        super::super::calm::when_calm(&ctx.window, move || {
            let t = adw::Toast::new(&msg);
            t.set_timeout(15);
            toast_for_cfg.say(t);
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
    let window_for_check = ctx.window.clone();
    glib::timeout_add_local(Duration::from_millis(900), move || {
        if !crate::git_sync::git_available() {
            tracing::warn!("git not found");
            let toasts = toast_for_check.clone();
            let window = window_for_check.clone();
            super::super::calm::when_calm(&window_for_check, move || {
                let t = adw::Toast::new(
                    "Your writing saves as usual, but Zerkalo can't keep saved versions or \
                     back up: git isn't installed.",
                );
                t.set_button_label(Some("Show what's missing"));
                let window = window.clone();
                t.connect_button_clicked(move |_| {
                    super::super::tools_window::ToolsWindow::new(&window).present();
                });
                t.set_timeout(0);
                toasts.say(t);
            });
        }
        if crate::spellcheck::SpellChecker::available_languages().is_empty() {
            tracing::info!("no spelling dictionaries found — spell check disabled");
        }
        if crate::git_sync::host_command("pandoc")
            .arg("--version")
            .output()
            .is_err()
        {
            tracing::info!("pandoc not found — LaTeX/HTML/EPUB/RTF import disabled");
        }
        let tinymist_ok = ["/app/lib/zerkalo/tinymist", "/usr/lib/zerkalo/tinymist"]
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .map(|p| {
                std::process::Command::new(p)
                    .arg("--version")
                    .output()
                    .is_ok()
            })
            .unwrap_or_else(|| {
                std::process::Command::new("tinymist")
                    .arg("--version")
                    .output()
                    .is_ok()
            });
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
    let new_template_for_welcome = ctx.menu_new_template_item.clone();
    let new_blank_for_welcome = ctx.menu_new_item.clone();
    let start_tour_after_welcome = ctx.start_tour_after_welcome.clone();
    let editor_for_note = ctx.editor_pane.clone();
    let after_update = ctx.current_config.borrow().comfort.after_update;
    glib::timeout_add_local(Duration::from_millis(1200), move || {
        use super::super::welcome_window::WelcomeWindow;
        let updated_quietly = WelcomeWindow::should_show() && !WelcomeWindow::is_first_run() && {
            // An update says so with a note (or not at all), never a window
            // over the work, unless that's what the person chose. A dev build
            // says nothing: it is a test, not news.
            let dev = env!("CARGO_PKG_VERSION").contains('-');
            match (dev, after_update) {
                (true, _) | (false, crate::config::AfterUpdate::Nothing) => {
                    WelcomeWindow::mark_shown();
                    true
                }
                (false, crate::config::AfterUpdate::Note) => {
                    // Marked shown only once the note is clicked or closed, so
                    // it waits across launches.
                    let note = super::super::update_note::build(
                        win_for_welcome.upcast_ref(),
                        WelcomeWindow::last_seen(),
                    );
                    editor_for_note.status_bar_insert_after_safety(&note);
                    true
                }
                (false, crate::config::AfterUpdate::Window) => false,
            }
        };
        if updated_quietly {
            if super::super::setup_wizard::SetupWizard::should_show(&root_for_welcome) {
                super::super::setup_wizard::SetupWizard::new(&win_for_welcome, &root_for_welcome)
                    .present();
            }
        } else if WelcomeWindow::should_show() {
            let is_first_run = WelcomeWindow::is_first_run();
            // Read before it is overwritten: a person who skipped releases is
            // shown each one they missed.
            let last_seen = WelcomeWindow::last_seen();
            WelcomeWindow::mark_shown();
            let ww = super::super::welcome_window::WelcomeWindow::new(
                &win_for_welcome,
                is_first_run,
                last_seen,
            );
            if is_first_run {
                // "Get Started" leads straight into creating a first document
                // instead of just closing the window on a blank editor. The
                // guided tour starts right alongside it — the template dialog
                // that opens is modal, so the tour sits ready underneath and
                // is simply what the user sees once they finish there
                // (create a document, or cancel back to the blank editor).
                let new_blank = new_blank_for_welcome.clone();
                ww.set_on_blank(move || new_blank.emit_clicked());
                let new_template = new_template_for_welcome.clone();
                let start_tour = start_tour_after_welcome.clone();
                ww.set_on_dismissed(move || {
                    new_template.emit_clicked();
                    if let Some(f) = start_tour.borrow_mut().take() {
                        f();
                    }
                });
            } else {
                // Chain: after "Close", check if setup wizard is needed.
                let win_chain = win_for_welcome.clone();
                let root_chain = root_for_welcome.clone();
                ww.set_on_dismissed(move || {
                    if super::super::setup_wizard::SetupWizard::should_show(&root_chain) {
                        super::super::setup_wizard::SetupWizard::new(&win_chain, &root_chain)
                            .present();
                    }
                });
            }
            ww.present();
        } else if super::super::setup_wizard::SetupWizard::should_show(&root_for_welcome) {
            super::super::setup_wizard::SetupWizard::new(&win_for_welcome, &root_for_welcome)
                .present();
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
                    let all_ok = buffers
                        .iter()
                        .all(|(path, content)| crate::auto_save::save(path, content));
                    // A recovery copy that worked needs no announcement — the
                    // status bar already says how safe the work is.
                    if !all_ok {
                        let t = adw::Toast::new(
                            "Your document is still open here. Zerkalo couldn't keep a recovery \
                             copy of it — the disk may be full.",
                        );
                        t.set_timeout(12);
                        toast_for_autosave.say(t);
                    }
                    *last_edit_for_autosave.borrow_mut() = None;
                }
            }
        }
        glib::ControlFlow::Continue
    });

    // ── Autosave the document itself (status bar toggle) ─────────────────
    {
        let editor = ctx.editor_pane.clone();
        let last_edit = ctx.last_edit_instant.clone();
        glib::timeout_add_local(Duration::from_secs(1), move || {
            let idle = last_edit
                .borrow()
                .is_some_and(|t| t.elapsed() >= AUTOSAVE_DELAY);
            if idle {
                autosave_now(&editor);
            }
            glib::ControlFlow::Continue
        });
    }
    {
        let editor = ctx.editor_pane.clone();
        let cfg = ctx.current_config.clone();
        let left_at: Rc<std::cell::Cell<Option<std::time::Instant>>> =
            Rc::new(std::cell::Cell::new(None));
        ctx.window.connect_is_active_notify(move |w| {
            if !w.is_active() {
                autosave_now(&editor);
                left_at.set(Some(std::time::Instant::now()));
            } else if let Some(t) = left_at.take() {
                // Back after a real break: show where the cursor is.
                if t.elapsed() >= Duration::from_secs(120)
                    && cfg.borrow().comfort.find_cursor_after_break
                {
                    editor.flash_cursor_line();
                }
            }
        });
    }
    // ── A plain clock for how long this sitting has lasted (opt-in) ──────
    {
        let started = std::time::Instant::now();
        let label = gtk4::Label::new(None);
        label.add_css_class("caption");
        label.add_css_class("dim-label");
        label.set_margin_end(8);
        label.set_visible(false);
        label.set_tooltip_text(Some("How long you have been writing this time"));
        ctx.editor_pane.status_bar_insert_after_goal(&label);
        let cfg = ctx.current_config.clone();
        let tick = move || {
            let on = cfg.borrow().comfort.session_clock;
            label.set_visible(on);
            if on {
                label.set_text(&format!(
                    "writing {}",
                    crate::resume::clock(started.elapsed().as_secs())
                ));
            }
        };
        tick();
        glib::timeout_add_local(Duration::from_secs(30), move || {
            tick();
            glib::ControlFlow::Continue
        });
    }

    // ── Backup: sync periodically, so non-technical users never have to find
    // the Sync button themselves ─────────────────────────────────────────
    //
    // Only runs once a backup location is configured (the setup wizard or
    // ☰ → Backup locations…), and only when there's something uncommitted —
    // otherwise it's a no-op check every 30s, not a sync. Quiet: failures
    // show a toast, never a blocking dialog, so an offline user isn't
    // interrupted while writing.
    const AUTO_SYNC_INTERVAL: Duration = Duration::from_secs(5 * 60);
    let editor_for_autosync = ctx.editor_pane.clone();
    let overlay_for_autosync = ctx.toast_overlay.clone();
    let sync_btn_for_autosync = ctx.sync_btn.clone();
    let sync_badge_for_autosync = ctx.sync_badge.clone();
    let root_for_autosync = ctx.project_root.clone();
    let last_auto_sync: Rc<RefCell<Option<std::time::Instant>>> = Rc::new(RefCell::new(None));

    // Paint the badge's initial state right away rather than leaving it
    // blank for up to 30s — reopening the app with unsynced work from last
    // time should show that immediately.
    {
        let editor = ctx.editor_pane.clone();
        let badge = ctx.sync_badge.clone();
        let root_fallback = ctx.project_root.clone();
        glib::idle_add_local_once(move || {
            let root = editor
                .get_active_path()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .and_then(|dir| crate::git_sync::git_repo_root(&dir))
                .unwrap_or(root_fallback);
            super::sync::refresh_badge(&root, &badge);
        });
    }

    glib::timeout_add_local(Duration::from_secs(30), move || {
        let root = editor_for_autosync
            .get_active_path()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .and_then(|dir| crate::git_sync::git_repo_root(&dir))
            .unwrap_or_else(|| root_for_autosync.clone());

        // Reflect "there's something waiting to back up" on every tick, not
        // just when a sync actually fires below — the badge should light up
        // the moment a change lands, not 5 minutes later.
        super::sync::refresh_badge(&root, &sync_badge_for_autosync);
        let dirty =
            crate::git_sync::has_remote(&root) && !crate::git_sync::changed_files(&root).is_empty();

        let due = last_auto_sync
            .borrow()
            .map(|t| t.elapsed() >= AUTO_SYNC_INTERVAL)
            .unwrap_or(true);

        if due && sync_btn_for_autosync.is_sensitive() && dirty {
            *last_auto_sync.borrow_mut() = Some(std::time::Instant::now());
            editor_for_autosync.save_all_modified();
            sync_btn_for_autosync.set_sensitive(false);
            let btn_done = sync_btn_for_autosync.clone();
            super::sync::auto_sync_quiet(
                root,
                Some(overlay_for_autosync.clone()),
                Some(sync_badge_for_autosync.clone()),
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
            editor_for_lsp_init.set_lsp_status("");
        } else {
            tracing::info!("tinymist not found — LSP disabled");
            editor_for_lsp_init.set_lsp_status("");
        }
        glib::ControlFlow::Break
    });

    // ── LSP: poll for diagnostics + completions + auto-restart ──────────

    let lsp_poll = ctx.lsp_client.clone();
    let editor_for_comp_poll = ctx.editor_pane.clone();
    let editor_for_lsp_diag = ctx.editor_pane.clone();
    let editor_for_lsp_status = ctx.editor_pane.clone();
    let last_req_poll = ctx.last_completion_request.clone();
    let problems_for_lsp = ctx.problems.clone();
    glib::timeout_add_local(Duration::from_millis(400), move || {
        // Auto-restart if tinymist crashed
        let mut restarted = false;
        {
            let mut slot = lsp_poll.borrow_mut();
            if let Some(client) = slot.as_mut() {
                if !client.is_alive() {
                    tracing::warn!("tinymist crashed — restarting");
                    editor_for_lsp_status.set_lsp_status("Suggestions restarting \u{21bb}");
                    let root = client.root.clone();
                    restarted = true;
                    *slot = LspClient::new(&root);
                    if slot.is_some() {
                        editor_for_lsp_status.set_lsp_status("");
                    } else {
                        editor_for_lsp_status.set_lsp_status("Suggestions unavailable \u{2717}");
                    }
                }
            }
        }
        if restarted {
            problems_for_lsp.clear_lsp();
        }
        // Collect all LSP data in a scoped borrow, then release it before
        // any GTK ops. mark_diagnostics / show_lsp_completions call
        // buffer.create_source_mark / popover.popup, which cascade through
        // GtkSourceView signals that re-enter Zerkalo callbacks — those
        // callbacks may try to borrow ctx.lsp_client, causing a BorrowError
        // panic if the borrow is still held.
        let lsp_data: Option<(Vec<_>, Option<_>)> = {
            let slot = lsp_poll.borrow();
            slot.as_ref().map(|client| {
                let typing = editor_for_lsp_diag
                    .since_last_edit()
                    .is_some_and(|d| d < ERROR_GRACE);
                let batches = if typing { Vec::new() } else { client.poll() };
                (batches, client.poll_completion())
            })
        };
        if let Some((batches, completion_result)) = lsp_data {
            // The language server's diagnostics go through the same
            // plain-language pass as the compiler's, so the wording doesn't
            // change depending on which one happened to report the problem.
            let reports = batches
                .into_iter()
                .map(|batch| {
                    let errors = batch
                        .diags
                        .into_iter()
                        .map(|d| {
                            let severity = match d.severity {
                                DiagSeverity::Error => Severity::Error,
                                _ => Severity::Warning,
                            };
                            let diagnosis = crate::diagnostic_catalog::diagnose(&d.message, None);
                            CompileError {
                                file: d.file,
                                line: d.line,
                                col: d.col,
                                end_line: d.end_line,
                                end_col: d.end_col,
                                origin: None,
                                message: diagnosis.headline,
                                advice: diagnosis.advice,
                                kind: diagnosis.kind,
                                hints: Vec::new(),
                                technical: d.message,
                                severity,
                            }
                        })
                        .collect();
                    (batch.file, errors)
                })
                .collect();
            problems_for_lsp.set_lsp(reports);
            if let Some((id, items)) = completion_result {
                if *last_req_poll.borrow() == Some(id) {
                    editor_for_comp_poll.show_lsp_completions(items);
                }
            }
        }
        glib::ControlFlow::Continue
    });

    // There is deliberately no *unconditional* write-to-disk timer here. One
    // used to call `save_all_modified()` every 30 s, which wrote every modified
    // buffer to its real path, cleared the modified flag, and deleted the
    // recovery copy — so the idle autosave above could never find anything
    // to save and `find_recovery` could never see an autosave newer than
    // the file, making the whole crash-recovery path unreachable. Writing the
    // document is now the user's choice (the status bar's "autosave", see
    // `autosave_now`): when it's on, the file itself is never more than a few
    // seconds behind, so it is its own recovery; when it's off, the file
    // changes only when the user saves and recovery copies work as before.
}
