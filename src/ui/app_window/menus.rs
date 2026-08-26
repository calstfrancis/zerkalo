//! Wiring for the hamburger menu's rows, split out of `AppWindow::new`.
//!
//! Two runs rather than one because the menu sections are not contiguous in the
//! original function: the import picker and the citation panel's Skrizhal
//! button sit between them and stay in `new()`.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk4::prelude::*;
use gtk4::{Button, Label, Popover};
use libadwaita as adw;

use super::super::editor_pane::EditorPane;
use super::super::error_panel::ErrorPanel;
use super::super::export_dialog::ExportDialog;
use super::super::font_manager::FontManager;
use super::super::help_window::HelpWindow;
use super::super::preview_pane::PreviewPane;
use super::super::settings_dialog::SettingsDialog;
use super::super::snapshot_dialog::{save_snapshot, SnapshotDialog};
use super::super::table_dialog::TableDialog;
use super::super::template_dialog::TemplateDialog;
use super::header::Menus;
use super::import::run_pdf_import;
use super::open_template_for_active_document;
use super::sync::{do_sync, show_backup_remote_dialog};
use super::{
    apply_compile_mode_css, apply_theme, compile_mode_label_str, print_from_preview,
    restore_snapshot_with_confirm, show_alert, show_dep_graph_window, show_file_history_window,
    show_ref_manager_window,
};
use crate::bibliography;
use crate::config::Config;
use crate::git_sync;
use crate::writing_log::WritingLog;

/// The shared state the menu handlers close over. One value instead of the 30
/// and 21 separate captures the two runs would otherwise need.
pub(super) struct MenuCtx {
    pub(super) window: adw::ApplicationWindow,
    pub(super) editor_pane: EditorPane,
    pub(super) preview_pane: PreviewPane,
    pub(super) error_panel: ErrorPanel,
    pub(super) citation_panel: super::super::citation_panel::CitationPanel,
    pub(super) dep_graph: super::super::dep_graph::DepGraph,
    pub(super) ref_manager: super::super::ref_manager::RefManager,
    pub(super) toast_overlay: adw::ToastOverlay,
    pub(super) current_config: Rc<RefCell<Config>>,
    pub(super) project_root: std::path::PathBuf,
    pub(super) writing_log: Rc<RefCell<WritingLog>>,
    pub(super) menu_popover: Popover,
    pub(super) auto_compile: Rc<RefCell<bool>>,
    pub(super) compile_on_save: Rc<RefCell<bool>>,
    pub(super) manual_compile_only: Rc<RefCell<bool>>,
    pub(super) debounce_ms: Rc<RefCell<u64>>,
    pub(super) compile_mode_btn: Button,
    pub(super) compile_mode_label: Label,
    pub(super) effective_cv_elements: Option<std::path::PathBuf>,
    pub(super) effective_bib: Option<std::path::PathBuf>,
    pub(super) auto_detected_bib: Rc<RefCell<Option<std::path::PathBuf>>>,
    pub(super) print_header_btn: Button,
    pub(super) sync_btn: Button,
    pub(super) sync_badge: Label,
}

/// Application-level rows: Settings, Help, Setup, Backup Remotes, About,
/// Writing Stats, Export, Print, Font Management.
pub(super) fn wire_app_menus(ctx: &MenuCtx, menus: &Menus) {
    // ── Compile/Preview toggle button ───────────────────────────────────
    // Wired after preview_outer is created (see below, search "ctx.preview_vis_holder.borrow_mut")

    // ── Menu: Settings ──────────────────────────────────────────────────

    let window_for_settings = ctx.window.clone();
    let editor_for_settings = ctx.editor_pane.clone();
    let debounce_for_settings = ctx.debounce_ms.clone();
    let auto_compile_for_settings = ctx.auto_compile.clone();
    let compile_on_save_for_settings = ctx.compile_on_save.clone();
    let manual_compile_only_for_settings = ctx.manual_compile_only.clone();
    let current_config_for_settings = ctx.current_config.clone();
    let menu_popover_for_settings = ctx.menu_popover.clone();
    let compile_mode_btn_for_settings = ctx.compile_mode_btn.clone();
    let compile_mode_label_for_settings = ctx.compile_mode_label.clone();
    let preview_for_settings = ctx.preview_pane.clone();
    let citation_for_settings = ctx.citation_panel.clone();
    let root_for_settings = ctx.project_root.clone();
    menus.menu_settings_item.connect_clicked(move |_| {
        menu_popover_for_settings.popdown();
        let dialog =
            SettingsDialog::new(&window_for_settings, &current_config_for_settings.borrow());

        // These three used to be their own hamburger rows; the dialog itself
        // doesn't know how to construct them (FontManager needs the
        // adw::ApplicationWindow, the other two need project_root), so the
        // caller supplies "open it" the same way it already supplies
        // on_save/on_preview.
        {
            let win = window_for_settings.clone();
            let cfg = current_config_for_settings.clone();
            dialog.set_on_open_font_manager(move || {
                let c = cfg.borrow();
                FontManager::new(&win, &c.default_sans_font, &c.default_serif_font).present();
            });
        }
        {
            let win = window_for_settings.clone();
            let root = root_for_settings.clone();
            dialog.set_on_open_setup_wizard(move || {
                super::super::setup_wizard::SetupWizard::new(&win, &root).present();
            });
        }
        {
            let win = window_for_settings.clone();
            let root = root_for_settings.clone();
            dialog.set_on_open_backup_locations(move || {
                show_backup_remote_dialog(&win, &root);
            });
        }
        let editor = editor_for_settings.clone();
        let debounce = debounce_for_settings.clone();
        let auto_flag = auto_compile_for_settings.clone();
        let cos_flag = compile_on_save_for_settings.clone();
        let mco_flag = manual_compile_only_for_settings.clone();
        let cfg_rc = current_config_for_settings.clone();
        let window_for_save = window_for_settings.clone();
        let cm_btn_save = compile_mode_btn_for_settings.clone();
        let cm_lbl_save = compile_mode_label_for_settings.clone();
        let preview_for_save = preview_for_settings.clone();
        let citation_for_save = citation_for_settings.clone();

        // Live preview — apply appearance changes immediately while dialog is open
        {
            let editor_p = editor.clone();
            let win_p = window_for_save.clone();
            dialog.set_on_preview(move |cfg| {
                editor_p.apply_font_size(cfg.editor_font_size);
                editor_p.apply_font_family(&cfg.editor_font_family);
                editor_p.apply_word_wrap(cfg.editor_word_wrap);
                editor_p.set_word_wrap_btn(cfg.editor_word_wrap);
                editor_p.apply_show_whitespace(cfg.editor_show_whitespace);
                editor_p.apply_tab_width(cfg.editor_tab_width);
                editor_p.apply_line_spacing(cfg.editor_line_spacing);
                editor_p.apply_typewriter_scroll(cfg.typewriter_scrolling);
                editor_p.apply_word_count_goal(cfg.word_count_goal);
                apply_theme(&cfg.theme);
                editor_p.apply_style_scheme(adw::StyleManager::default().is_dark());
                if cfg.high_contrast {
                    win_p.add_css_class("high-contrast");
                } else {
                    win_p.remove_css_class("high-contrast");
                }
            });
        }

        dialog.set_on_save(move |new_cfg| {
            *debounce.borrow_mut() = new_cfg.debounce_ms;
            *auto_flag.borrow_mut() = new_cfg.auto_compile;
            *cos_flag.borrow_mut() = new_cfg.compile_on_save;
            *mco_flag.borrow_mut() = new_cfg.manual_compile_only;
            cm_lbl_save.set_text(compile_mode_label_str(
                new_cfg.auto_compile,
                new_cfg.compile_on_save,
                new_cfg.manual_compile_only,
            ));
            apply_compile_mode_css(
                &cm_btn_save,
                new_cfg.auto_compile,
                new_cfg.compile_on_save,
                new_cfg.manual_compile_only,
            );
            editor.apply_font_size(new_cfg.editor_font_size);
            editor.apply_font_family(&new_cfg.editor_font_family);
            editor.apply_word_wrap(new_cfg.editor_word_wrap);
            editor.set_word_wrap_btn(new_cfg.editor_word_wrap);
            editor.apply_show_whitespace(new_cfg.editor_show_whitespace);
            editor.apply_tab_width(new_cfg.editor_tab_width);
            editor.apply_line_spacing(new_cfg.editor_line_spacing);
            editor.apply_typewriter_scroll(new_cfg.typewriter_scrolling);
            editor.apply_word_count_goal(new_cfg.word_count_goal);
            editor.set_spell_enabled(new_cfg.spell_enabled);
            editor.set_spell_autocorrect(new_cfg.spell_autocorrect);
            editor.set_spell_languages(new_cfg.spell_languages.clone());
            apply_theme(&new_cfg.theme);
            editor.apply_style_scheme(adw::StyleManager::default().is_dark());
            // High contrast CSS class on the ctx.window
            if new_cfg.high_contrast {
                window_for_save.add_css_class("high-contrast");
            } else {
                window_for_save.remove_css_class("high-contrast");
            }
            let old_bib = cfg_rc.borrow().bib_path.clone();
            if old_bib != new_cfg.bib_path {
                match new_cfg.bib_path.as_ref() {
                    Some(bp) => editor.set_bib_entries(bibliography::load_bib(bp)),
                    None => editor.set_bib_entries(Vec::new()),
                }
                preview_for_save.set_bib_path(new_cfg.bib_path.clone());
            }
            // CV elements were resolved once at startup, so changing this path
            // used to do nothing until the next launch, silently. It can be
            // applied live, so it is.
            let old_cv = cfg_rc.borrow().cv_elements_path.clone();
            if old_cv != new_cfg.cv_elements_path {
                preview_for_save.set_cv_elements_path(new_cfg.cv_elements_path.clone());
                match new_cfg.cv_elements_path.as_ref() {
                    Some(p) => {
                        let entries = crate::cv_mode::load_cv_entries(p);
                        editor.set_cv_entries(entries.clone());
                        citation_for_save.load_cv_entries(entries);
                    }
                    None => {
                        editor.set_cv_entries(Vec::new());
                        citation_for_save.load_cv_entries(Vec::new());
                    }
                }
                preview_for_save.trigger_compile();
            }

            // Everything else is live; these two are read once when the window
            // is built, so say so rather than appearing to have applied.
            let mut needs_restart: Vec<&str> = Vec::new();
            if new_cfg.work_dir != cfg_rc.borrow().work_dir {
                needs_restart.push("work folder");
            }
            if new_cfg.output_dir != cfg_rc.borrow().output_dir {
                needs_restart.push("output folder");
            }
            *cfg_rc.borrow_mut() = new_cfg;
            if !needs_restart.is_empty() {
                show_alert(
                    &window_for_save,
                    "Restart required",
                    &format!(
                        "The {} change takes effect after restarting Zerkalo.",
                        needs_restart.join(" and "),
                    ),
                );
            }
        });
        dialog.present();
    });

    // ── Menu: Help ──────────────────────────────────────────────────────

    let window_for_help = ctx.window.clone();
    let menu_popover_for_help = ctx.menu_popover.clone();
    let editor_for_help = ctx.editor_pane.clone();
    menus.menu_help_item.connect_clicked(move |_| {
        menu_popover_for_help.popdown();
        HelpWindow::new(&window_for_help, editor_for_help.is_cv_mode()).present();
    });

    // ── Menu: Keyboard Shortcuts ────────────────────────────────────────
    // Reads keybindings.toml live, so it's the accurate list even after a
    // rebind — previously only reachable via its own shortcut.

    let window_for_keys = ctx.window.clone();
    let menu_popover_for_keys = ctx.menu_popover.clone();
    menus.menu_shortcuts_item.connect_clicked(move |_| {
        menu_popover_for_keys.popdown();
        super::show_dynamic_shortcuts_window(
            &window_for_keys,
            &crate::keybindings::Keybindings::load(),
        );
    });

    // ── Menu: What's New ────────────────────────────────────────────────
    // The release-name window only ever appeared on first run and after an
    // upgrade; nothing let a user open it again.

    let window_for_whats_new = ctx.window.clone();
    let menu_popover_for_whats_new = ctx.menu_popover.clone();
    menus.menu_whats_new_item.connect_clicked(move |_| {
        menu_popover_for_whats_new.popdown();
        super::super::welcome_window::WelcomeWindow::new(&window_for_whats_new, false).present();
    });

    // ── Menu: About ─────────────────────────────────────────────────────

    let window_for_about = ctx.window.clone();
    let menu_popover_for_about = ctx.menu_popover.clone();
    menus.menu_about_item.connect_clicked(move |_| {
        menu_popover_for_about.popdown();
        // An AdwAboutWindow rather than a plain message dialog: the repo link
        // is now clickable, the licence is stated, and the release name the
        // release process assigns is actually visible in the app.
        let about = adw::AboutWindow::builder()
            .transient_for(&window_for_about)
            .modal(true)
            .application_name("Zerkalo")
            .application_icon("io.github.calstfrancis.Zerkalo")
            .version(format!(
                "{} \u{201c}{}\u{201d}",
                env!("CARGO_PKG_VERSION"),
                super::super::welcome_window::RELEASE_NAME,
            ))
            .comments(
                "A contemplative Typst editor.\n\n\
                 Built with Rust · GTK4 · libadwaita · sourceview5.\n\
                 Embedded Typst compiler — no external binary required.",
            )
            .website("https://github.com/calstfrancis/zerkalo")
            .issue_url("https://github.com/calstfrancis/zerkalo/issues")
            .developer_name("Cal St Francis")
            .license_type(gtk4::License::MitX11)
            .build();
        about.present();
    });

    // ── Menu: Writing Stats ─────────────────────────────────────────────

    let window_for_stats = ctx.window.clone();
    let writing_log_for_stats = ctx.writing_log.clone();
    let menu_popover_for_stats = ctx.menu_popover.clone();
    menus.menu_writing_stats_item.connect_clicked(move |_| {
        menu_popover_for_stats.popdown();
        let log = writing_log_for_stats.borrow();
        let today = log.total_today();
        let week = log.total_this_week();
        let streak = log.streak_days();
        let total = log.sessions.len();
        let body = format!(
            "Today: {:+} words\nThis week: {:+} words\nStreak: {} day{}\nTotal sessions: {}",
            today,
            week,
            streak,
            if streak == 1 { "" } else { "s" },
            total,
        );
        let dlg =
            adw::MessageDialog::new(Some(&window_for_stats), Some("Writing Stats"), Some(&body));
        dlg.add_response("ok", "OK");
        dlg.present();
    });

    // ── Menu: Export ────────────────────────────────────────────────────

    let preview_for_export = ctx.preview_pane.clone();
    let window_for_export = ctx.window.clone();
    let menu_popover_for_export = ctx.menu_popover.clone();
    let current_config_for_export = ctx.current_config.clone();
    let project_root_for_export = ctx.project_root.clone();
    let cv_elements_for_export = ctx.effective_cv_elements.clone();
    let bib_for_export = ctx.effective_bib.clone();
    menus.menu_export_item.connect_clicked(move |_| {
        menu_popover_for_export.popdown();
        let initial_fmt = current_config_for_export.borrow().last_export_format;
        let cfg_for_save = current_config_for_export.clone();
        ExportDialog::new(
            &window_for_export,
            preview_for_export.root_file_path(),
            preview_for_export.output_dir(),
            project_root_for_export.clone(),
            cv_elements_for_export.clone(),
            bib_for_export.clone(),
            initial_fmt,
            move |fmt| {
                let mut cfg = cfg_for_save.borrow_mut();
                cfg.last_export_format = fmt;
                let _ = cfg.save();
            },
        )
        .present();
    });

    // Hoisted above its uses (the print handlers just below, and the import
    // machinery further down around the sync button) so all of them — which
    // show in-progress and result toasts — can capture it.

    // ── Menu: Print ─────────────────────────────────────────────────────

    {
        let window_for_print = ctx.window.clone();
        let editor_for_print = ctx.editor_pane.clone();
        let preview_for_print = ctx.preview_pane.clone();
        let toast_for_print = ctx.toast_overlay.clone();
        let panel_for_print = ctx.error_panel.clone();
        let root_for_print = ctx.project_root.clone();
        let menu_popover_for_print = ctx.menu_popover.clone();
        let config_for_print = ctx.current_config.clone();

        // The hamburger item and the ctx.header button do the same thing, so
        // they share one closure rather than two that can drift.
        let open_print_sheet: Rc<dyn Fn()> = Rc::new(move || {
            print_from_preview(
                &window_for_print,
                &editor_for_print,
                &preview_for_print,
                &toast_for_print,
                &panel_for_print,
                &root_for_print,
                &config_for_print,
            );
        });

        let from_menu = open_print_sheet.clone();
        menus.menu_print_item.connect_clicked(move |_| {
            menu_popover_for_print.popdown();
            from_menu();
        });
        ctx.print_header_btn
            .connect_clicked(move |_| open_print_sheet());
    }
}

/// Document-level rows: Import PDF, templates, New, Open, Save, Save As,
/// Snapshots, Export for Web.
pub(super) fn wire_document_menus(ctx: &MenuCtx, menus: &Menus) {
    // ── Menu: Import PDF ───────────────────────────────────────────────

    let window_for_pdf = ctx.window.clone();
    let editor_for_pdf = ctx.editor_pane.clone();
    let menu_popover_for_pdf = ctx.menu_popover.clone();
    let work_dir_for_pdf = ctx.project_root.clone();
    menus.menu_import_pdf_item.connect_clicked(move |_| {
        menu_popover_for_pdf.popdown();
        let dialog = gtk4::FileDialog::new();
        dialog.set_title("Import PDF File");
        let filter = gtk4::FileFilter::new();
        filter.set_name(Some("PDF files (*.pdf)"));
        filter.add_pattern("*.pdf");
        let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));
        dialog.set_initial_folder(Some(&gtk4::gio::File::for_path(&work_dir_for_pdf)));
        let win2 = window_for_pdf.clone();
        let ep2 = editor_for_pdf.clone();
        let win_ref = win2.clone();
        dialog.open(
            Some(&win_ref),
            None::<&gtk4::gio::Cancellable>,
            move |result| {
                if let Ok(file) = result {
                    if let Some(input_path) = file.path() {
                        run_pdf_import(&win2, &ep2, input_path);
                    }
                }
            },
        );
    });

    // ── Menu: New from Template ─────────────────────────────────────────

    let window_for_template = ctx.window.clone();
    let editor_for_template = ctx.editor_pane.clone();
    let menu_popover_for_template = ctx.menu_popover.clone();
    let project_root_for_template = ctx.project_root.clone();
    let cfg_for_template = ctx.current_config.clone();
    menus.menu_new_template_item.connect_clicked(move |_| {
        menu_popover_for_template.popdown();
        let last_advanced = cfg_for_template.borrow().last_used_advanced;
        let dlg = TemplateDialog::new(
            &window_for_template,
            &project_root_for_template,
            last_advanced,
        );
        {
            let cfg = cfg_for_template.borrow();
            dlg.set_bib_path(cfg.bib_path.clone());
            dlg.preselect_locked_identity(
                &cfg.locked_author.clone(),
                &cfg.locked_affiliation.clone(),
            );
        }
        {
            let cfg2 = cfg_for_template.clone();
            dlg.set_on_advanced_toggle(move |expanded| {
                let mut c = cfg2.borrow_mut();
                c.last_used_advanced = expanded;
                let _ = c.save();
            });
        }
        {
            let cfg = cfg_for_template.clone();
            dlg.set_on_lock_identity(move |author, affiliation| {
                let mut c = cfg.borrow_mut();
                c.locked_author = author;
                c.locked_affiliation = affiliation;
                let _ = c.save();
            });
        }
        let ep = editor_for_template.clone();
        dlg.set_on_create(move |path| {
            if let Ok(content) = std::fs::read_to_string(&path) {
                ep.open_file(path, &content);
            }
        });
        dlg.present();
    });

    // ── Menu: Change Document Style ──────────────────────────────────────

    let window_for_reapply = ctx.window.clone();
    let editor_for_reapply = ctx.editor_pane.clone();
    let menu_popover_for_reapply = ctx.menu_popover.clone();
    let project_root_for_reapply = ctx.project_root.clone();
    let cfg_for_reapply = ctx.current_config.clone();
    let preview_for_reapply = ctx.preview_pane.clone();
    let toast_for_reapply = ctx.toast_overlay.clone();
    menus.menu_reapply_template_item.connect_clicked(move |_| {
        menu_popover_for_reapply.popdown();
        open_template_for_active_document(
            &window_for_reapply,
            &editor_for_reapply,
            &preview_for_reapply,
            &toast_for_reapply,
            &project_root_for_reapply,
            &cfg_for_reapply,
        );
    });

    // ── Menu: Repair Template Markers ───────────────────────────────────

    let editor_for_repair = ctx.editor_pane.clone();
    let window_for_repair = ctx.window.clone();
    let menu_popover_for_repair = ctx.menu_popover.clone();
    menus.menu_repair_markers_item.connect_clicked(move |_| {
        menu_popover_for_repair.popdown();
        let Some(path) = editor_for_repair.get_active_path() else {
            return;
        };
        let (title, body) = match super::super::template_dialog::repair_template_markers(&path) {
            Ok(true) => {
                if let Ok(new_content) = std::fs::read_to_string(&path) {
                    editor_for_repair.reload_file(path, &new_content);
                }
                (
                    "Marker repaired",
                    "The body marker was re-inserted. A backup was saved as .typ.bak.".to_string(),
                )
            }
            Ok(false) => (
                "Marker already present",
                "The file already contains a valid body marker. No changes were made.".to_string(),
            ),
            Err(e) => ("Repair failed", e),
        };
        let dlg = adw::MessageDialog::new(Some(&window_for_repair), Some(title), Some(&body));
        dlg.add_response("ok", "OK");
        dlg.set_default_response(Some("ok"));
        dlg.present();
    });

    // ── Menu: New Document ──────────────────────────────────────────────

    let window_for_new = ctx.window.clone();
    let editor_for_new = ctx.editor_pane.clone();
    let work_dir_for_new = ctx.project_root.clone();
    let menu_popover_for_new = ctx.menu_popover.clone();
    menus.menu_new_item.connect_clicked(move |_| {
        menu_popover_for_new.popdown();
        let dialog = gtk4::FileDialog::new();
        dialog.set_title("New Document");
        dialog.set_initial_folder(Some(&gtk4::gio::File::for_path(&work_dir_for_new)));
        dialog.set_initial_name(Some("untitled.typ"));
        let win_c = window_for_new.clone();
        let ep_c = editor_for_new.clone();
        dialog.save(
            Some(&win_c),
            None::<&gtk4::gio::Cancellable>,
            move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        if !path.exists() {
                            let _ = std::fs::write(&path, "= Title\n\n");
                        }
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            ep_c.open_file(path, &content);
                        }
                    }
                }
            },
        );
    });

    // ── Menu: Open File ─────────────────────────────────────────────────

    let window_for_open = ctx.window.clone();
    let editor_for_open_file = ctx.editor_pane.clone();
    let menu_popover_for_open = ctx.menu_popover.clone();
    menus.menu_open_item.connect_clicked(move |_| {
        menu_popover_for_open.popdown();
        let dialog = gtk4::FileDialog::new();
        dialog.set_title("Open File");
        let filter = gtk4::FileFilter::new();
        filter.set_name(Some("Typst files (*.typ)"));
        filter.add_pattern("*.typ");
        let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));
        let win_c = window_for_open.clone();
        let ep_c = editor_for_open_file.clone();
        dialog.open(
            Some(&win_c),
            None::<&gtk4::gio::Cancellable>,
            move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            ep_c.open_file(path, &content);
                        }
                    }
                }
            },
        );
    });

    // ── Menu: Save ──────────────────────────────────────────────────────

    let editor_for_menu_save = ctx.editor_pane.clone();
    let preview_for_menu_save = ctx.preview_pane.clone();
    let menu_popover_for_save = ctx.menu_popover.clone();
    let root_for_menu_save = ctx.project_root.clone();
    let toast_for_menu_save = ctx.toast_overlay.clone();
    menus.menu_save_item.connect_clicked(move |_| {
        menu_popover_for_save.popdown();
        match editor_for_menu_save.save_current() {
            Ok(Some(path)) => {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    save_snapshot(&root_for_menu_save, &path, &content);
                    // The debounced on-change compile is deliberately suppressed
                    // in Compile-on-Save/Manual modes (see mod.rs's on_change
                    // wiring), so the preview's buffer_snapshot override can be
                    // stale from whenever this tab was last switched to. Refresh
                    // it here or Save silently recompiles old content.
                    preview_for_menu_save.set_buffer_snapshot(path.clone(), content);
                }
                preview_for_menu_save.trigger_compile();
            }
            Ok(None) => {}
            Err(e) => {
                let t = adw::Toast::new(&format!("Save failed: {e}"));
                t.set_timeout(6);
                toast_for_menu_save.add_toast(t);
            }
        }
    });

    // ── Menu: Save As ───────────────────────────────────────────────────

    let window_for_save_as = ctx.window.clone();
    let editor_for_save_as = ctx.editor_pane.clone();
    let preview_for_save_as = ctx.preview_pane.clone();
    let menu_popover_for_save_as = ctx.menu_popover.clone();
    menus.menu_save_as_item.connect_clicked(move |_| {
        menu_popover_for_save_as.popdown();
        let Some(content) = editor_for_save_as.get_active_content() else {
            return;
        };
        let dialog = gtk4::FileDialog::new();
        dialog.set_title("Save As");
        let filter = gtk4::FileFilter::new();
        filter.set_name(Some("Typst files (*.typ)"));
        filter.add_pattern("*.typ");
        let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));
        dialog.set_initial_name(Some("untitled.typ"));
        let win_c = window_for_save_as.clone();
        let ep_c = editor_for_save_as.clone();
        let pv_c = preview_for_save_as.clone();
        dialog.save(
            Some(&win_c),
            None::<&gtk4::gio::Cancellable>,
            move |result| {
                if let Ok(file) = result {
                    if let Some(mut path) = file.path() {
                        if path.extension().is_none() {
                            path.set_extension("typ");
                        }
                        if std::fs::write(&path, content.as_bytes()).is_ok() {
                            ep_c.open_file(path.clone(), &content);
                            pv_c.set_root_file(path);
                            pv_c.trigger_compile();
                        }
                    }
                }
            },
        );
    });

    // ── Menu: Browse Snapshots ──────────────────────────────────────────

    let window_for_snap = ctx.window.clone();
    let editor_for_snap = ctx.editor_pane.clone();
    let root_for_snap = ctx.project_root.clone();
    let menu_popover_for_snap = ctx.menu_popover.clone();
    menus.menu_snapshots_item.connect_clicked(move |_| {
        menu_popover_for_snap.popdown();
        let Some(path) = editor_for_snap.get_active_path() else {
            return;
        };
        let content = editor_for_snap.get_active_content().unwrap_or_default();
        let dialog = SnapshotDialog::new(&window_for_snap, &root_for_snap, &path, &content);
        let ep = editor_for_snap.clone();
        let pp_path = path.clone();
        let win_for_restore = window_for_snap.clone();
        dialog.set_on_restore(move |text| {
            restore_snapshot_with_confirm(&win_for_restore, &ep, &pp_path, text);
        });
        dialog.present();
    });

    // ── Menu: File History ────────────────────────────────────────────────

    let window_for_history = ctx.window.clone();
    let editor_for_history = ctx.editor_pane.clone();
    let root_for_history = ctx.project_root.clone();
    let menu_popover_for_history = ctx.menu_popover.clone();
    menus.menu_history_item.connect_clicked(move |_| {
        menu_popover_for_history.popdown();
        let Some(path) = editor_for_history.get_active_path() else {
            return;
        };
        show_file_history_window(&window_for_history, &root_for_history, &path);
    });

    // ── Menu: Reference Manager ──────────────────────────────────────────

    let window_for_refs = ctx.window.clone();
    let ref_manager_for_menu = ctx.ref_manager.clone();
    let menu_popover_for_refs = ctx.menu_popover.clone();
    menus.menu_refs_item.connect_clicked(move |_| {
        menu_popover_for_refs.popdown();
        show_ref_manager_window(&window_for_refs, &ref_manager_for_menu);
    });

    // ── Menu: Dependency Graph ───────────────────────────────────────────

    let window_for_depgraph = ctx.window.clone();
    let dep_graph_for_menu = ctx.dep_graph.clone();
    let menu_popover_for_depgraph = ctx.menu_popover.clone();
    menus.menu_depgraph_item.connect_clicked(move |_| {
        menu_popover_for_depgraph.popdown();
        show_dep_graph_window(&window_for_depgraph, &dep_graph_for_menu);
    });

    // ── Menu: Insert Table ───────────────────────────────────────────────

    let window_for_table = ctx.window.clone();
    let editor_for_table = ctx.editor_pane.clone();
    let menu_popover_for_table = ctx.menu_popover.clone();
    menus.menu_table_item.connect_clicked(move |_| {
        menu_popover_for_table.popdown();
        let dialog = TableDialog::new(&window_for_table);
        let ep = editor_for_table.clone();
        dialog.set_on_insert(move |code| {
            ep.insert_at_cursor(&code);
        });
        dialog.present();
    });

    // ── Sync button ─────────────────────────────────────────────────────

    let window_for_sync = ctx.window.clone();
    let sync_btn_ref = ctx.sync_btn.clone();
    let sync_badge_ref = ctx.sync_badge.clone();
    let editor_for_sync = ctx.editor_pane.clone();
    let toast_for_sync_closure = ctx.toast_overlay.clone();

    if let Some(ref bib_path) = *ctx.auto_detected_bib.borrow() {
        let name = bib_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("refs.bib")
            .to_string();
        let t = adw::Toast::new(&format!("Loaded bibliography: {name}"));
        t.set_timeout(4);
        ctx.toast_overlay.add_toast(t);
    }

    // ── Menu: Export for Web ────────────────────────────────────────────
    {
        let ep = ctx.editor_pane.clone();
        let win = ctx.window.clone();
        let pop = ctx.menu_popover.clone();
        let toast = ctx.toast_overlay.clone();
        menus.menu_export_web_item.connect_clicked(move |_| {
            pop.popdown();
            let Some(input_path) = ep.get_active_path() else {
                return;
            };
            let dialog = gtk4::FileDialog::builder()
                .title("Export for Web")
                .modal(true)
                .initial_name(
                    input_path
                        .with_extension("html")
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("output.html"),
                )
                .build();
            let win_c = win.clone();
            let toast_c = toast.clone();
            dialog.save(
                Some(&win_c),
                None::<&gtk4::gio::Cancellable>,
                move |result| {
                    let Ok(gfile) = result else { return };
                    let Some(out_path) = gfile.path() else { return };
                    match crate::web_export::export_for_web(&input_path, &out_path) {
                        Ok(()) => {
                            let t = adw::Toast::new("Exported for web");
                            t.set_timeout(3);
                            toast_c.add_toast(t);
                        }
                        Err(e) => {
                            let t = adw::Toast::new(&format!("Export failed: {e}"));
                            t.set_timeout(6);
                            toast_c.add_toast(t);
                        }
                    }
                },
            );
        });
    }
    let config_for_sync = ctx.current_config.clone();
    let project_root_for_sync_fallback = ctx.project_root.clone();
    ctx.sync_btn.connect_clicked(move |_| {
        editor_for_sync.save_all_modified();
        let root = editor_for_sync
            .get_active_path()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .and_then(|dir| git_sync::git_repo_root(&dir))
            .unwrap_or_else(|| project_root_for_sync_fallback.clone());
        let win = window_for_sync.clone();
        let btn = sync_btn_ref.clone();
        let badge = sync_badge_ref.clone();
        let toasts = toast_for_sync_closure.clone();
        let token = crate::secret_store::load_github_token();
        let cfg_rc = config_for_sync.clone();

        if !git_sync::has_remote(&root) {
            // No remote configured yet — send the user through the full
            // Setup Wizard (GitHub sign-in, folder backup, etc.) rather than
            // the bare "paste a git URL" dialog, which has no sign-in path
            // and assumes the user already has a URL to paste. The wizard
            // does its own push once finished, so there's nothing to chain
            // into afterward — just restore the button.
            let wizard = super::super::setup_wizard::SetupWizard::new(&win, &root);
            let btn_reenable = btn.clone();
            wizard.window().connect_destroy(move |_| {
                btn_reenable.set_sensitive(true);
            });

            btn.set_sensitive(false);
            wizard.present();
            return;
        }

        do_sync(root, win, toasts, btn, badge, token, cfg_rc);
    });
}
