//! Bibliography and CV-entry loading (with file watches), the citation panel's
//! insert/choose actions, and the reference manager's insert, jump and
//! project-wide citation-key rename. Split out of `AppWindow::new`.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use gtk4::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

use crate::bibliography;
use crate::config::Config;
use super::super::citation_panel::CitationPanel;
use super::super::editor_pane::EditorPane;
use super::super::ref_manager::RefManager;

/// What the citation/bibliography wiring needs from `AppWindow::new`.
pub(super) struct CitationCtx {
    pub(super) window: adw::ApplicationWindow,
    pub(super) editor_pane: EditorPane,
    pub(super) citation_panel: CitationPanel,
    pub(super) ref_manager: RefManager,
    pub(super) current_config: Rc<RefCell<Config>>,
    pub(super) project_root: PathBuf,
    pub(super) effective_bib: Option<PathBuf>,
    pub(super) effective_cv_elements: Option<PathBuf>,
}

/// Rewrites the active document's `#bibliography(...)` call to point at
/// `path`, so choosing a bibliography source from the citation panel takes
/// effect immediately instead of only updating `Config::bib_path` — which
/// drives the citation panel's own autocomplete/list but nothing in the
/// document itself, leaving it uncompilable (a real bug reported live:
/// picking a new source here didn't touch the code, and the document
/// wouldn't compile until the `#bibliography(...)` line was fixed by hand).
/// A no-op if no document is open. Wrapped as one undoable edit so it can be
/// undone like any other change, since — unlike Update Template Settings,
/// which the user explicitly opens to change the document — this fires as a
/// side effect of a dialog whose primary purpose looks like a Settings
/// action, not a document edit.
fn update_active_document_bib_path(ep: &EditorPane, path: &std::path::Path) {
    let Some(content) = ep.get_active_content() else { return };
    let target = bibliography::bib_target_path(path);
    let new_content = crate::styles::set_bibliography_path(&content, &target.to_string_lossy());
    if new_content != content {
        ep.set_active_content_undoable(&new_content);
    }
}

/// Returns the auto-detected `.bib` slot, which later sections read.
pub(super) fn wire_citations(ctx: &CitationCtx) -> Rc<RefCell<Option<PathBuf>>> {
    // ── Bibliography loading & watch ────────────────────────────────────

    if let Some(ref bp) = ctx.effective_bib {
        let entries = bibliography::load_bib(bp);
        if !entries.is_empty() {
            tracing::info!("Loaded {} bib entries from {}", entries.len(), bp.display());
        }
        ctx.editor_pane.set_bib_entries(entries.clone());
        ctx.citation_panel.load_bib(entries);
        ctx.citation_panel.set_bib_filename(bp.file_name().and_then(|n| n.to_str()));
        ctx.ref_manager.load_bib(bp);

        if bibliography::is_vault_dir(bp) {
            let editor_for_bib = ctx.editor_pane.clone();
            let citation_for_bib = ctx.citation_panel.clone();
            let bib_for_watch = bp.clone();
            // Leaked deliberately: the watch must outlive `wire_citations`
            // (called once from `AppWindow::new`), and there is exactly one
            // per window for the process's lifetime — same lifetime as the
            // window itself, so there is nothing to reclaim it into.
            let watch = crate::vault_watch::start(bp.clone(), move || {
                let entries = bibliography::load_bib(&bib_for_watch);
                tracing::info!("Reloaded {} bib entries from vault", entries.len());
                editor_for_bib.set_bib_entries(entries.clone());
                citation_for_bib.load_bib(entries);
            });
            std::mem::forget(watch);
        } else {
            let editor_for_bib = ctx.editor_pane.clone();
            let citation_for_bib = ctx.citation_panel.clone();
            let bib_for_watch = bp.clone();
            let last_mtime: Rc<RefCell<Option<SystemTime>>> = Rc::new(RefCell::new(
                std::fs::metadata(&bib_for_watch)
                    .and_then(|m| m.modified())
                    .ok(),
            ));
            glib::timeout_add_local(Duration::from_secs(5), move || {
                let current = std::fs::metadata(&bib_for_watch)
                    .and_then(|m| m.modified())
                    .ok();
                let changed = match (*last_mtime.borrow(), current) {
                    (Some(old), Some(new)) => old != new,
                    (None, Some(_)) => true,
                    _ => false,
                };
                if changed {
                    *last_mtime.borrow_mut() = current;
                    let entries = bibliography::load_bib(&bib_for_watch);
                    tracing::info!("Reloaded {} bib entries", entries.len());
                    editor_for_bib.set_bib_entries(entries.clone());
                    citation_for_bib.load_bib(entries);
                }
                glib::ControlFlow::Continue
            });
        }
    }

    // ── Auto-detect .bib when no bib is configured ─────────────────────────
    let auto_detected_bib: Rc<RefCell<Option<std::path::PathBuf>>> = Rc::new(RefCell::new(None));
    if ctx.effective_bib.is_none() {
        if let Ok(mut entries) = std::fs::read_dir(&ctx.project_root) {
            let found = entries.find_map(|e| {
                let path = e.ok()?.path();
                let ext = path.extension().and_then(|x| x.to_str())?;
                if ext.eq_ignore_ascii_case("bib")
                    || ext.eq_ignore_ascii_case("yaml")
                    || ext.eq_ignore_ascii_case("yml")
                {
                    Some(path)
                } else {
                    None
                }
            });
            if let Some(bib_path) = found {
                let entries = bibliography::load_bib(&bib_path);
                ctx.editor_pane.set_bib_entries(entries.clone());
                ctx.citation_panel.load_bib(entries);
                ctx.citation_panel.set_bib_filename(bib_path.file_name().and_then(|n| n.to_str()));
                *auto_detected_bib.borrow_mut() = Some(bib_path);
            }
        }
    }

    // ── CV entries loading & watch ───────────────────────────────────────

    if let Some(ref cvp) = ctx.effective_cv_elements {
        let entries = crate::cv_mode::load_cv_entries(cvp);
        if !entries.is_empty() {
            tracing::info!("Loaded {} CV entries from {}", entries.len(), cvp.display());
        }
        ctx.editor_pane.set_cv_entries(entries.clone());
        ctx.citation_panel.load_cv_entries(entries);
        ctx.citation_panel.set_cv_filename(cvp.file_name().and_then(|n| n.to_str()));

        let editor_for_cv = ctx.editor_pane.clone();
        let citation_for_cv = ctx.citation_panel.clone();
        let cv_for_watch = cvp.clone();
        let last_mtime: Rc<RefCell<Option<SystemTime>>> = Rc::new(RefCell::new(
            std::fs::metadata(&cv_for_watch)
                .and_then(|m| m.modified())
                .ok(),
        ));
        glib::timeout_add_local(Duration::from_secs(5), move || {
            let current = std::fs::metadata(&cv_for_watch)
                .and_then(|m| m.modified())
                .ok();
            let changed = match (*last_mtime.borrow(), current) {
                (Some(old), Some(new)) => old != new,
                (None, Some(_)) => true,
                _ => false,
            };
            if changed {
                *last_mtime.borrow_mut() = current;
                let entries = crate::cv_mode::load_cv_entries(&cv_for_watch);
                tracing::info!("Reloaded {} CV entries", entries.len());
                editor_for_cv.set_cv_entries(entries.clone());
                citation_for_cv.load_cv_entries(entries);
            }
            glib::ControlFlow::Continue
        });
    }

    // ── Citation panel: insert @key / #cv-entry("key") at cursor ──────────

    {
        let ep = ctx.editor_pane.clone();
        ctx.citation_panel.set_on_insert(move |text| ep.insert_at_cursor(&text));
    }

    // ── Citation panel: choose bib file button ────────────────────────────

    {
        let win_for_bib = ctx.window.clone();
        let ep_for_bib = ctx.editor_pane.clone();
        let cp_for_bib = ctx.citation_panel.clone();
        let cfg_for_bib = ctx.current_config.clone();
        let rm_for_bib = ctx.ref_manager.clone();
        ctx.citation_panel.set_on_choose_bib(move || {
            let dialog = gtk4::FileDialog::new();
            dialog.set_title("Choose Bibliography File");
            let filter = gtk4::FileFilter::new();
            filter.set_name(Some("Bibliography files (*.bib, *.yaml, *.yml)"));
            filter.add_pattern("*.bib");
            filter.add_pattern("*.yaml");
            filter.add_pattern("*.yml");
            let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
            filters.append(&filter);
            dialog.set_filters(Some(&filters));
            let win = win_for_bib.clone();
            let ep = ep_for_bib.clone();
            let cp = cp_for_bib.clone();
            let cfg = cfg_for_bib.clone();
            let rm = rm_for_bib.clone();
            dialog.open(Some(&win), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        let entries = bibliography::load_bib(&path);
                        ep.set_bib_entries(entries.clone());
                        cp.load_bib(entries);
                        cp.set_bib_filename(path.file_name().and_then(|n| n.to_str()));
                        rm.load_bib(&path);
                        update_active_document_bib_path(&ep, &path);
                        cfg.borrow_mut().bib_path = Some(path);
                        let _ = cfg.borrow().save();
                    }
                }
            });
        });
    }

    // ── Citation panel + reference manager: start a new bibliography ──────
    // Both surfaces hit the same dead end without this: a first-time user has
    // no `.bib` file yet, and neither panel could previously create one.

    {
        let win = ctx.window.clone();
        let ep = ctx.editor_pane.clone();
        let cp = ctx.citation_panel.clone();
        let cfg = ctx.current_config.clone();
        let rm = ctx.ref_manager.clone();
        ctx.citation_panel.set_on_new_bib(move || {
            open_create_bib_dialog(&win, &ep, &cp, &cfg, &rm);
        });
    }
    {
        let win = ctx.window.clone();
        let ep = ctx.editor_pane.clone();
        let cp = ctx.citation_panel.clone();
        let cfg = ctx.current_config.clone();
        let rm = ctx.ref_manager.clone();
        ctx.ref_manager.set_on_create_bib(move || {
            open_create_bib_dialog(&win, &ep, &cp, &cfg, &rm);
        });
    }

    // ── Citation panel: choose Kartoteka vault folder button ──────────────

    {
        let win_for_vault = ctx.window.clone();
        let ep_for_vault = ctx.editor_pane.clone();
        let cp_for_vault = ctx.citation_panel.clone();
        let cfg_for_vault = ctx.current_config.clone();
        let rm_for_vault = ctx.ref_manager.clone();
        ctx.citation_panel.set_on_choose_vault(move || {
            let dialog = gtk4::FileDialog::new();
            dialog.set_title("Choose Kartoteka Vault Folder");
            let win = win_for_vault.clone();
            let ep = ep_for_vault.clone();
            let cp = cp_for_vault.clone();
            let cfg = cfg_for_vault.clone();
            let rm = rm_for_vault.clone();
            dialog.select_folder(Some(&win), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        let entries = bibliography::load_bib(&path);
                        ep.set_bib_entries(entries.clone());
                        cp.load_bib(entries);
                        cp.set_bib_filename(path.file_name().and_then(|n| n.to_str()));
                        rm.load_bib(&path);
                        update_active_document_bib_path(&ep, &path);
                        cfg.borrow_mut().bib_path = Some(path);
                        let _ = cfg.borrow().save();
                    }
                }
            });
        });
    }

    // ── Citation panel: choose Skrizhal CV element file button ────────────

    {
        let win_for_cv = ctx.window.clone();
        let ep_for_cv = ctx.editor_pane.clone();
        let cp_for_cv = ctx.citation_panel.clone();
        let cfg_for_cv = ctx.current_config.clone();
        ctx.citation_panel.set_on_choose_cv(move || {
            let dialog = gtk4::FileDialog::new();
            dialog.set_title("Choose Skrizhal CV Element File");
            let filter = gtk4::FileFilter::new();
            filter.set_name(Some("YAML files (*.yaml, *.yml)"));
            filter.add_pattern("*.yaml");
            filter.add_pattern("*.yml");
            let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
            filters.append(&filter);
            dialog.set_filters(Some(&filters));
            let win = win_for_cv.clone();
            let ep = ep_for_cv.clone();
            let cp = cp_for_cv.clone();
            let cfg = cfg_for_cv.clone();
            dialog.open(Some(&win), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        let entries = crate::cv_mode::load_cv_entries(&path);
                        ep.set_cv_entries(entries.clone());
                        cp.load_cv_entries(entries);
                        cp.set_cv_filename(path.file_name().and_then(|n| n.to_str()));
                        cfg.borrow_mut().cv_elements_path = Some(path);
                        let _ = cfg.borrow().save();
                    }
                }
            });
        });
    }

    // ── Reference manager: insert citation / jump to broken citation ──────

    let editor_for_ref = ctx.editor_pane.clone();
    ctx.ref_manager.set_on_insert(move |citation| {
        editor_for_ref.insert_at_cursor(&citation);
    });

    {
        let ep = ctx.editor_pane.clone();
        ctx.ref_manager.set_on_jump_citation(move |key| {
            ep.jump_to_text(&format!("@{key}"));
        });
    }

    // ── Reference manager: project-wide citation-key rename ───────────────
    {
        let ep = ctx.editor_pane.clone();
        let rm = ctx.ref_manager.clone();
        let cp = ctx.citation_panel.clone();
        let win = ctx.window.clone();
        let project_root_for_rename = ctx.project_root.clone();
        ctx.ref_manager.set_on_rename(move |old_key, new_key| {
            let Some(bib_path) = rm.bib_path() else { return };
            let is_bibtex = bib_path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("bib"));
            if !is_bibtex {
                let dlg = adw::MessageDialog::new(
                    Some(&win),
                    Some("Only BibTeX rename is supported"),
                    Some("Renaming keys is only available for .bib bibliographies."),
                );
                dlg.add_response("ok", "OK");
                dlg.present();
                return;
            }

            let typ_files = crate::project::collect_typ_files(&project_root_for_rename);
            let open_tab_texts: std::collections::HashMap<PathBuf, String> =
                ep.all_tab_texts().into_iter().collect();

            let mut affected_files = 0usize;
            for path in &typ_files {
                let changed = if let Some(text) = open_tab_texts.get(path) {
                    bibliography::rename_key_in_text(text, &old_key, &new_key).1
                } else {
                    std::fs::read_to_string(path).is_ok_and(|content| {
                        bibliography::rename_key_in_text(&content, &old_key, &new_key).1
                    })
                };
                if changed {
                    affected_files += 1;
                }
            }

            let dlg = adw::MessageDialog::new(
                Some(&win),
                Some("Rename citation key?"),
                Some(&format!(
                    "Rename @{old_key} to @{new_key} in the bibliography and {affected_files} document(s)?"
                )),
            );
            dlg.add_response("cancel", "Cancel");
            dlg.add_response("rename", "Rename");
            dlg.set_response_appearance("rename", adw::ResponseAppearance::Suggested);

            let bib_path2 = bib_path.clone();
            let old_key2 = old_key.clone();
            let new_key2 = new_key.clone();
            let ep2 = ep.clone();
            let rm2 = rm.clone();
            let cp2 = cp.clone();
            let win2 = win.clone();
            let typ_files2 = typ_files.clone();
            dlg.connect_response(None, move |dlg, response| {
                dlg.close();
                if response != "rename" {
                    return;
                }

                if let Err(e) = bibliography::rename_key_in_bib_file(&bib_path2, &old_key2, &new_key2) {
                    let err_dlg = adw::MessageDialog::new(
                        Some(&win2),
                        Some("Rename failed"),
                        Some(&format!("Could not update the bibliography file: {e}")),
                    );
                    err_dlg.add_response("ok", "OK");
                    err_dlg.present();
                    return;
                }

                ep2.replace_citation_key_in_open_tabs(&old_key2, &new_key2);

                let open_paths: std::collections::HashSet<PathBuf> =
                    ep2.open_tab_paths().into_iter().collect();
                for path in &typ_files2 {
                    if open_paths.contains(path) {
                        continue;
                    }
                    if let Ok(content) = std::fs::read_to_string(path) {
                        let (new_content, changed) =
                            bibliography::rename_key_in_text(&content, &old_key2, &new_key2);
                        if changed {
                            let _ = std::fs::write(path, new_content);
                        }
                    }
                }

                let entries = bibliography::load_bib(&bib_path2);
                ep2.set_bib_entries(entries.clone());
                cp2.load_bib(entries.clone());
                rm2.load_bib(&bib_path2);
            });
            dlg.present();
        });
    }


    auto_detected_bib
}

/// Opens a save dialog for a brand-new, empty `.bib` file, then wires it up
/// exactly like picking an existing one: loads it (empty) into the editor,
/// citation panel and reference manager, and remembers it in config.
fn open_create_bib_dialog(
    win: &adw::ApplicationWindow,
    ep: &EditorPane,
    cp: &CitationPanel,
    cfg: &Rc<RefCell<Config>>,
    rm: &RefManager,
) {
    let dialog = gtk4::FileDialog::new();
    dialog.set_title("Create Bibliography File");
    dialog.set_initial_name(Some("references.bib"));
    let win = win.clone();
    let ep = ep.clone();
    let cp = cp.clone();
    let cfg = cfg.clone();
    let rm = rm.clone();
    dialog.save(Some(&win), None::<&gtk4::gio::Cancellable>, move |result| {
        if let Ok(file) = result {
            if let Some(path) = file.path() {
                if std::fs::write(&path, "").is_ok() {
                    let entries = bibliography::load_bib(&path);
                    ep.set_bib_entries(entries.clone());
                    cp.load_bib(entries);
                    cp.set_bib_filename(path.file_name().and_then(|n| n.to_str()));
                    rm.load_bib(&path);
                    update_active_document_bib_path(&ep, &path);
                    cfg.borrow_mut().bib_path = Some(path);
                    let _ = cfg.borrow().save();
                }
            }
        }
    });
}
