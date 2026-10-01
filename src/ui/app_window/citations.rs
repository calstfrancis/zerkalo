//! Bibliography and CV-entry loading (with file watches), the citation panel's
//! insert/choose actions, and the reference manager's insert, jump and
//! project-wide citation-key rename. Split out of `AppWindow::new`.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use adw::prelude::*;
use gtk4::prelude::*;
use libadwaita as adw;

use super::super::citation_panel::CitationPanel;
use super::super::editor_pane::EditorPane;
use super::super::ref_manager::RefManager;
use super::bib_manager::BibManager;
use crate::bibliography;
use crate::config::Config;

/// What the citation/bibliography wiring needs from `AppWindow::new`.
pub(super) struct CitationCtx {
    pub(super) window: adw::ApplicationWindow,
    pub(super) editor_pane: EditorPane,
    pub(super) citation_panel: CitationPanel,
    pub(super) ref_manager: RefManager,
    pub(super) current_config: Rc<RefCell<Config>>,
    pub(super) project_root: PathBuf,
    pub(super) bib_manager: BibManager,
    pub(super) effective_cv_elements: Option<PathBuf>,
}

/// Returns the auto-detected `.bib` slot, which later sections read.
pub(super) fn wire_citations(ctx: &CitationCtx) -> Rc<RefCell<Option<PathBuf>>> {
    // ── Bibliography: one manager decides, loads and watches ────────────
    // The open document's own `#bibliography(...)` line wins; before any
    // document is open this is the project / Settings choice, else the one
    // bibliography found in the project folder.
    ctx.bib_manager.refresh(None);
    let auto_detected_bib: Rc<RefCell<Option<std::path::PathBuf>>> =
        Rc::new(RefCell::new(ctx.bib_manager.found_in_folder()));

    // ── CV entries loading & watch ───────────────────────────────────────

    if let Some(ref cvp) = ctx.effective_cv_elements {
        let entries = crate::cv_mode::load_cv_entries(cvp);
        if !entries.is_empty() {
            tracing::info!("Loaded {} CV entries from {}", entries.len(), cvp.display());
        }
        ctx.editor_pane.set_cv_entries(entries.clone());
        ctx.citation_panel.load_cv_entries(entries);
        ctx.citation_panel
            .set_cv_filename(cvp.file_name().and_then(|n| n.to_str()));

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
        ctx.citation_panel
            .set_on_insert(move |text| ep.insert_at_cursor(&text));
    }

    // ── Citation panel: choose bib file button ────────────────────────────

    {
        let win_for_bib = ctx.window.clone();
        let manager = ctx.bib_manager.clone();
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
            let manager = manager.clone();
            dialog.open(
                Some(&win_for_bib),
                None::<&gtk4::gio::Cancellable>,
                move |result| {
                    if let Some(path) = result.ok().and_then(|f| f.path()) {
                        manager.choose(path);
                    }
                },
            );
        });
    }

    // ── Citation panel + reference manager: start a new bibliography ──────
    // Both surfaces hit the same dead end without this: a first-time user has
    // no `.bib` file yet, and neither panel could previously create one.

    {
        let win = ctx.window.clone();
        let manager = ctx.bib_manager.clone();
        ctx.citation_panel.set_on_new_bib(move || {
            open_create_bib_dialog(&win, &manager);
        });
    }
    {
        let win = ctx.window.clone();
        let manager = ctx.bib_manager.clone();
        ctx.ref_manager.set_on_create_bib(move || {
            open_create_bib_dialog(&win, &manager);
        });
    }

    // ── Citation panel: choose Kartoteka vault folder button ──────────────

    {
        let win_for_vault = ctx.window.clone();
        let manager = ctx.bib_manager.clone();
        ctx.citation_panel.set_on_choose_vault(move || {
            let dialog = gtk4::FileDialog::new();
            dialog.set_title("Choose Kartoteka Vault Folder");
            let manager = manager.clone();
            dialog.select_folder(
                Some(&win_for_vault),
                None::<&gtk4::gio::Cancellable>,
                move |result| {
                    if let Some(path) = result.ok().and_then(|f| f.path()) {
                        manager.choose(path);
                    }
                },
            );
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
        let manager = ctx.bib_manager.clone();
        let win = ctx.window.clone();
        let project_root_for_rename = ctx.project_root.clone();
        ctx.ref_manager.set_on_rename(move |old_key, new_key| {
            let Some(bib_path) = manager.current().map(|r| r.path) else {
                return;
            };
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
            let manager2 = manager.clone();
            let win2 = win.clone();
            let typ_files2 = typ_files.clone();
            dlg.connect_response(None, move |dlg, response| {
                dlg.close();
                if response != "rename" {
                    return;
                }

                // Files that aren't open are changed on disk, together with the
                // bibliography, as one all-or-nothing step. Open tabs are
                // edited in the editor (undoable) once that has succeeded.
                let open_paths: std::collections::HashSet<PathBuf> =
                    ep2.open_tab_paths().into_iter().collect();
                let edits: Vec<bibliography::RenameEdit> = typ_files2
                    .iter()
                    .filter(|p| !open_paths.contains(*p))
                    .filter_map(|path| {
                        let before = std::fs::read_to_string(path).ok()?;
                        let (after, changed) =
                            bibliography::rename_key_in_text(&before, &old_key2, &new_key2);
                        changed.then(|| bibliography::RenameEdit {
                            path: path.clone(),
                            before,
                            after,
                        })
                    })
                    .collect();
                if let Err(e) =
                    bibliography::apply_rename(&bib_path2, &old_key2, &new_key2, &edits)
                {
                    let err_dlg = adw::MessageDialog::new(
                        Some(&win2),
                        Some("Rename failed"),
                        Some(&format!(
                            "Nothing was changed. Could not update the files: {e}"
                        )),
                    );
                    err_dlg.add_response("ok", "OK");
                    err_dlg.present();
                    return;
                }

                ep2.replace_citation_key_in_open_tabs(&old_key2, &new_key2);

                manager2.reload();
            });
            dlg.present();
        });
    }

    auto_detected_bib
}

/// Opens a save dialog for a brand-new, empty `.bib` file, then makes it the
/// bibliography exactly as if an existing one had been picked.
fn open_create_bib_dialog(win: &adw::ApplicationWindow, manager: &BibManager) {
    let dialog = gtk4::FileDialog::new();
    dialog.set_title("Create Bibliography File");
    dialog.set_initial_name(Some("references.bib"));
    let manager = manager.clone();
    dialog.save(Some(win), None::<&gtk4::gio::Cancellable>, move |result| {
        if let Some(path) = result.ok().and_then(|f| f.path()) {
            // Never write over a file that's already there: the save dialog
            // asks about overwriting, but an empty file written over a real
            // bibliography would lose every entry in it, so keep what exists.
            if path.exists() || std::fs::write(&path, "").is_ok() {
                manager.choose(path);
            }
        }
    });
}
