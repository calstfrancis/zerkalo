use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Label,
    Notebook, Orientation, Paned, Separator, Stack, ToggleButton,
};
use libadwaita as adw;
use adw::prelude::*;

use crate::config::{CompileProfile, Config, Theme};
use crate::writing_log::{WritingLog, count_words, FileStartWords};
use crate::keybindings::{matches_binding, Keybindings};
use crate::lsp::LspClient;
use crate::session::Session;
use super::command_palette::{CommandPalette, default_commands, heading_items};
use super::editor_pane::EditorPane;
use super::file_tree::FileTree;
use super::error_panel::{parse_typst_errors, ErrorPanel, Severity};
use super::help_window::HelpWindow;
use super::outline_panel::OutlinePanel;
use super::preview_pane::PreviewPane;
use super::snapshot_dialog::save_snapshot;
use super::library_window::LibraryWindow;
use crate::library::Library;

use crate::cv_mode::CV_HELPERS_TYPST;

mod citations;
mod dialogs;
mod editor_extras;
use editor_extras::{EditorExtrasCtx, SidebarToolbarCtx, wire_editor_extras, wire_sidebar_toolbar};
mod file_tree_wiring;
use file_tree_wiring::{FileTreeCtx, wire_file_tree};
use citations::{CitationCtx, wire_citations};
mod header;
mod lifecycle;
mod menus;
use lifecycle::{LifecycleCtx, wire_startup};
use menus::{MenuCtx, wire_app_menus, wire_document_menus};
mod panels;
use panels::{Panels, build_panels};
use header::{HeaderWidgets, build_header};
mod import;
pub use import::prune_import_staging;
mod startup;
mod sync;
use startup::{PanePersistCtx, WatcherCtx, wire_file_watcher, wire_pane_persistence};
use dialogs::{show_changelog, show_doc_stats};
use import::{
    IMPORT_FORMATS, import_folder_via_pandoc, import_via_pandoc, paste_as_document,
    show_import_history_dialog,
};

/// Menu buttons the command palette forwards to, so every palette entry runs
/// exactly the handler its hamburger row runs.
#[derive(Clone)]
struct PaletteTargets {
    new_file: Button,
    open_file: Button,
    export: Button,
    settings: Button,
    template: Button,
    save: Button,
    sidebar: Button,
}

pub struct AppWindow {
    window: adw::ApplicationWindow,
    editor_pane: EditorPane,
    preview_pane: PreviewPane,
    #[allow(dead_code)]
    error_panel: ErrorPanel,
    #[allow(dead_code)]
    outline_panel: OutlinePanel,
    help_overlay: Rc<super::help_overlay::HelpOverlay>,
    project_root: PathBuf,
    sync_btn: Button,
    search_panel: super::search_panel::SearchPanel,
    #[allow(dead_code)]
    toast_overlay: adw::ToastOverlay,
    file_tree: FileTree,
    writing_log: Rc<RefCell<WritingLog>>,
    file_start_words: FileStartWords,
    session_start: Rc<RefCell<std::time::Instant>>,
    #[allow(dead_code)]
    compile_on_save: Rc<RefCell<bool>>,
    #[allow(dead_code)]
    manual_compile_only: Rc<RefCell<bool>>,
    #[allow(dead_code)]
    file_watcher: Option<notify::RecommendedWatcher>,
    compile_btn: Button,
    #[allow(dead_code)]
    library: Rc<RefCell<Library>>,
    library_window: LibraryWindow,
    menu_import_item: Button,
    /// The hamburger rows the command palette and keyboard shortcuts dispatch
    /// through. Routing to the real menu button rather than duplicating each
    /// action keeps the two surfaces from drifting — half the palette's
    /// commands used to be unhandled and silently did nothing.
    menu_actions: PaletteTargets,
    /// Shared with every handler that persists a preference. Held on the
    /// window so keyboard shortcuts, which are wired in a separate pass from
    /// the menu items, reach the same instance rather than a stale clone.
    config: Rc<RefCell<Config>>,
}

impl AppWindow {
    pub fn new(app: &adw::Application, config: Config) -> Self {
        let project_root = config.work_dir.clone();

        // Start with an in-memory placeholder so the window can open immediately.
        // The real DB is opened and scanned on a background thread; the RefCell
        // contents are swapped in when the thread finishes.
        let library = Rc::new(RefCell::new(Library::open_in_memory()));
        {
            let library_bg = library.clone();
            let work_dir_bg = config.work_dir.clone();
            // Plain mpsc polled from the main loop, matching every other
            // worker handoff in this file. `MainContext::channel` was
            // deprecated in favour of an async channel, and this codebase has
            // no async runtime on the GTK side to host one.
            let (sender, receiver) = std::sync::mpsc::sync_channel::<Library>(1);
            std::thread::spawn(move || {
                let mut lib = Library::open().unwrap_or_else(|e| {
                    tracing::warn!("Failed to open library DB: {e}");
                    Library::open_in_memory()
                });
                lib.import_directory(&work_dir_bg).ok();
                lib.fix_created_dates_from_fs();
                sender.send(lib).ok();
            });
            glib::timeout_add_local(Duration::from_millis(100), move || {
                match receiver.try_recv() {
                    Ok(lib) => {
                        *library_bg.borrow_mut() = lib;
                        tracing::info!("Library DB ready");
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                }
            });
        }

        let window = adw::ApplicationWindow::new(app);
        window.set_title(Some("Zerkalo"));
        window.set_default_width(1600);
        window.set_default_height(1000);
        window.maximize();

        // ── Application-wide accent CSS ─────────────────────────────────────
        load_app_css();

        // ── Per-project config ──────────────────────────────────────────────

        let proj_cfg = crate::config::ProjectConfig::load(&project_root).unwrap_or_default();
        let effective_bib = proj_cfg.bib_path.clone().or_else(|| config.bib_path.clone());
        // CV mode: resolved cv_elements_path wins over bib_path for this document
        // (a CV isn't also a cited academic paper) — see cv_helpers.rs.
        let effective_cv_elements = proj_cfg
            .cv_elements_path
            .clone()
            .or_else(|| config.cv_elements_path.clone());
        // Project config wins, but the global Settings → Folders value is the
        // fallback — without the `or_else` that setting saved and did nothing.
        let effective_output_dir = proj_cfg
            .output_dir
            .clone()
            .or_else(|| config.output_dir.clone());
        let extra_compiler_args = proj_cfg.compiler_args.clone();

        // ── Runtime-configurable values ─────────────────────────────────────

        let debounce_ms: Rc<RefCell<u64>> = Rc::new(RefCell::new(config.debounce_ms));
        let auto_compile: Rc<RefCell<bool>> = Rc::new(RefCell::new(config.auto_compile));
        let compile_on_save: Rc<RefCell<bool>> = Rc::new(RefCell::new(config.compile_on_save));
        let manual_compile_only: Rc<RefCell<bool>> = Rc::new(RefCell::new(config.manual_compile_only));
        let auto_save_idle_ms: Rc<RefCell<u64>> = Rc::new(RefCell::new(config.auto_save_idle_ms));
        // The process-wide instance, not a copy — dialogs that change settings
        // mutate this same one, so nothing silently reverts anyone else's edit.
        let current_config: Rc<RefCell<Config>> = crate::config::shared();
        let last_edit_instant: Rc<RefCell<Option<std::time::Instant>>> = Rc::new(RefCell::new(None));
        let has_compile_errors: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        let HeaderWidgets {
            menus,
            compile_btn,
            compile_mode_slot,
            draft_toggle,
            file_title_widget,
            gost_menu_slot,
            header,
            library_btn,
            menu_btn,
            menu_popover,
            open_list_box,
            open_search,
            preview_label,
            print_header_btn,
            recent_popover,
            recompile_header_btn,
            save_btn,
            sidebar_btn,
            style_box,
            style_btn,
            style_popover,
            sync_btn,
            title_extras,
        } = build_header();
        let Panels {
            citation_panel,
            dep_graph,
            editor_pane,
            file_start_words,
            library_window,
            outline_panel,
            popout_pane,
            popout_window,
            ref_manager,
            session_start,
            writing_log,
        } = build_panels(
            app,
            &window,
            &config,
            &current_config,
            &library,
            &project_root,
            &library_btn,
            &style_btn,
            &style_box,
            &style_popover,
        );
        // ── Open dropdown wiring ─────────────────────────────────────────────
        {
            let open_list_rc = open_list_box.clone();
            let work_dir_open = project_root.clone();
            let editor_for_open = editor_pane.clone();
            let pop_for_open = recent_popover.clone();
            let config_for_open = current_config.clone();
            let library_for_open = library.clone();

            let rebuild: Rc<dyn Fn(&str)> = Rc::new(move |query: &str| {
                while let Some(child) = open_list_rc.first_child() {
                    open_list_rc.remove(&child);
                }
                // Recent files first, then scanned files (deduplicated)
                let mut files: Vec<(std::path::PathBuf, std::time::SystemTime)> = {
                    let cfg = config_for_open.borrow();
                    cfg.recent_files.iter()
                        .filter(|p| p.exists())
                        .map(|p| {
                            let mtime = std::fs::metadata(p)
                                .and_then(|m| m.modified())
                                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                            (p.clone(), mtime)
                        })
                        .collect()
                };
                for (path, mtime) in super::docs_browser::scan_typ_files(&work_dir_open, 2) {
                    if !files.iter().any(|(p, _)| p == &path) {
                        files.push((path, mtime));
                    }
                }
                let q = query.to_lowercase();
                let filtered: Vec<_> = files.into_iter()
                    .filter(|(path, _)| {
                        if q.is_empty() { return true; }
                        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        name.to_lowercase().contains(&q)
                    })
                    .take(30)
                    .collect();

                // Group by date bucket: Today / This week / Older
                let now = std::time::SystemTime::now();
                let day_secs = 86_400u64;
                let week_secs = 7 * day_secs;
                let mut last_group = "";
                let add_group_header = |list: &GtkBox, title: &str| {
                    let lbl = Label::new(Some(title));
                    lbl.set_halign(Align::Start);
                    lbl.set_margin_start(10);
                    lbl.set_margin_top(8);
                    lbl.set_margin_bottom(2);
                    lbl.add_css_class("dim-label");
                    lbl.add_css_class("caption");
                    list.append(&lbl);
                };

                for (path, mtime) in filtered {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                    let age = now.duration_since(mtime).map(|d| d.as_secs()).unwrap_or(u64::MAX);
                    let group = if age < day_secs { "Today" } else if age < week_secs { "This week" } else { "Older" };
                    if group != last_group {
                        add_group_header(&open_list_rc, group);
                        last_group = group;
                    }
                    let date_str = format_file_mtime(mtime);

                    // Row: open button (left, expands) + trash button (right)
                    let outer_row = GtkBox::new(Orientation::Horizontal, 0);
                    outer_row.set_hexpand(true);

                    let btn = Button::new();
                    btn.add_css_class("flat");
                    btn.set_hexpand(true);
                    let row_box = GtkBox::new(Orientation::Vertical, 2);
                    row_box.set_margin_start(10);
                    row_box.set_margin_end(4);
                    row_box.set_margin_top(5);
                    row_box.set_margin_bottom(5);
                    let name_lbl = Label::new(Some(&name));
                    name_lbl.set_xalign(0.0);
                    name_lbl.set_halign(Align::Start);
                    name_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                    let date_lbl = Label::new(Some(&date_str));
                    date_lbl.set_xalign(0.0);
                    date_lbl.set_halign(Align::Start);
                    date_lbl.add_css_class("caption");
                    date_lbl.add_css_class("dim-label");
                    row_box.append(&name_lbl);
                    row_box.append(&date_lbl);
                    btn.set_child(Some(&row_box));
                    let ep = editor_for_open.clone();
                    let pop = pop_for_open.clone();
                    let p = path.clone();
                    let lib = library_for_open.clone();
                    btn.connect_clicked(move |_| {
                        if let Ok(content) = std::fs::read_to_string(&p) {
                            ep.open_file(p.clone(), &content);
                        }
                        lib.borrow_mut().touch_opened(&p).ok();
                        pop.popdown();
                    });

                    let del_btn = Button::from_icon_name("user-trash-symbolic");
                    del_btn.add_css_class("flat");
                    del_btn.set_valign(gtk4::Align::Center);
                    del_btn.set_margin_end(4);
                    del_btn.set_tooltip_text(Some("Delete file"));

                    let path_del = path.clone();
                    let outer_for_del = outer_row.clone();
                    let cfg_del = config_for_open.clone();
                    let ep_del = editor_for_open.clone();
                    del_btn.connect_clicked(move |_| {
                        let outer_c = outer_for_del.clone();
                        let cfg_c = cfg_del.clone();
                        let ep_c = ep_del.clone();
                        super::confirm::confirm_trash(
                            None,
                            path_del.clone(),
                            move |path_c| {
                                cfg_c.borrow_mut().recent_files.retain(|p| p != path_c);
                                let _ = cfg_c.borrow().save();
                                ep_c.close_file_if_open(&path_c.to_path_buf());
                                if let Some(parent) = outer_c.parent() {
                                    if let Ok(p) = parent.downcast::<GtkBox>() {
                                        p.remove(&outer_c);
                                    }
                                }
                            },
                        );
                    });

                    outer_row.append(&btn);
                    outer_row.append(&del_btn);
                    open_list_rc.append(&outer_row);
                }
            });

            let rbl_show = rebuild.clone();
            let search_for_show = open_search.clone();
            recent_popover.connect_show(move |_| {
                search_for_show.set_text("");
                rbl_show("");
            });

            let rbl_search = rebuild.clone();
            open_search.connect_changed(move |entry| {
                let q = entry.text().to_string();
                rbl_search(&q);
            });
        }

        let preview_pane = PreviewPane::new(
            None,
            effective_output_dir,
            extra_compiler_args,
        );
        // CV mode: make #cv-entry/#cv-section available at compile time.
        // cv-helpers.typ's content is static (embedded), so a one-time
        // virtual-file override is correct; the actual data path is stored
        // and re-read fresh on every compile (see set_cv_elements_path) so
        // edits made in Skrizhal while Zerkalo is open aren't stale. Injected
        // unconditionally (not gated on a Skrizhal file being configured)
        // since CV templates now unconditionally `#import` it — cv-data
        // degrades to an empty dict, and cv-section shows "No entries yet."
        preview_pane.set_buffer_snapshot(
            project_root.join("cv-helpers.typ"),
            CV_HELPERS_TYPST.to_string(),
        );
        preview_pane.set_cv_elements_path(effective_cv_elements.clone());
        let error_panel = ErrorPanel::new();
        error_panel.widget().set_visible(false);

        // ── LSP client ──────────────────────────────────────────────────────

        let lsp_client: Rc<RefCell<Option<LspClient>>> = Rc::new(RefCell::new(None));

        // Created here rather than inside the Print menu section: the doc
        // font/size wiring below needs it, the menu wiring does too, and so do
        // several later sections.
        let toast_overlay = adw::ToastOverlay::new();

        // ── Apply initial settings ──────────────────────────────────────────

        editor_pane.apply_font_size(config.editor_font_size);
        editor_pane.apply_font_family(&config.editor_font_family);
        editor_pane.apply_word_wrap(config.editor_word_wrap);
        editor_pane.set_word_wrap_btn(config.editor_word_wrap);
        editor_pane.apply_show_whitespace(config.editor_show_whitespace);
        editor_pane.apply_tab_width(config.editor_tab_width);
        editor_pane.apply_line_spacing(config.editor_line_spacing);
        editor_pane.apply_typewriter_scroll(config.typewriter_scrolling);
        editor_pane.apply_word_count_goal(config.word_count_goal);
        editor_pane.set_spell_enabled(config.spell_enabled);
        editor_pane.set_spell_autocorrect(config.spell_autocorrect);
        editor_pane.set_spell_languages(config.spell_languages.clone());
        {
            let cfg = current_config.clone();
            editor_pane.set_on_autocorrect_toggle(move |enabled| {
                cfg.borrow_mut().spell_autocorrect = enabled;
                let _ = cfg.borrow().save();
            });
        }
        editor_pane.set_format_bar_visible(config.format_bar_visible);
        {
            let cfg = current_config.clone();
            editor_pane.set_on_format_bar_toggle(move |visible| {
                cfg.borrow_mut().format_bar_visible = visible;
                let _ = cfg.borrow().save();
            });
        }
        // ── Doc font/size callbacks — edit the one line that holds the value ──
        {
            let ep = editor_pane.clone();
            let preview_for_font = preview_pane.clone();
            let toast_for_font = toast_overlay.clone();
            editor_pane.set_on_doc_font(move |font_name| {
                let edited = super::template_dialog::set_template_font(
                    &ep.get_active_content().unwrap_or_default(),
                    &font_name,
                );
                let applied = apply_doc_font_edit(
                    &ep,
                    &preview_for_font,
                    &toast_for_font,
                    edited,
                    |sc| sc.font = font_name.clone(),
                );
                if applied {
                    ep.set_doc_font_label(&font_name);
                }
            });
        }
        {
            let ep = editor_pane.clone();
            let preview_for_size = preview_pane.clone();
            let toast_for_size = toast_overlay.clone();
            editor_pane.set_on_doc_font_size(move |size| {
                let edited = super::template_dialog::set_template_font_size(
                    &ep.get_active_content().unwrap_or_default(),
                    &size,
                );
                let applied = apply_doc_font_edit(
                    &ep,
                    &preview_for_size,
                    &toast_for_size,
                    edited,
                    |sc| sc.font_size = size.clone(),
                );
                if applied {
                    ep.set_doc_size_label(&size);
                }
            });
        }
        {
            let win = window.clone();
            editor_pane.set_on_version_click(move || {
                show_changelog(&win);
            });
        }
        {
            let win = window.clone();
            let ep = editor_pane.clone();
            editor_pane.set_on_word_count_click(move || {
                if let Some(text) = ep.active_text() {
                    let session_start = ep.session_start_words();
                    let project_root = ep.project_root();
                    show_doc_stats(&win, &text, session_start, project_root.as_deref());
                }
            });
        }
        preview_pane.set_zoom(config.preview_zoom);

        // ── Compilation profile wiring ──────────────────────────────────────
        {
            let initial_draft = config.active_profile == CompileProfile::Draft;
            preview_pane.set_draft_mode(initial_draft);
            draft_toggle.set_active(initial_draft);
            update_draft_toggle_label(&draft_toggle, initial_draft);
        }
        {
            let pp = preview_pane.clone();
            let cfg = current_config.clone();
            draft_toggle.connect_toggled(move |btn| {
                let is_draft = btn.is_active();
                pp.set_draft_mode(is_draft);
                update_draft_toggle_label(btn, is_draft);
                cfg.borrow_mut().active_profile = if is_draft {
                    CompileProfile::Draft
                } else {
                    CompileProfile::Final
                };
                let _ = cfg.borrow().save();
            });
        }

        // Style dropdown goes in the breadcrumb/toolbar bar on the right side
        editor_pane.breadcrumb_bar_append(&style_btn);
        // Draft toggle hidden for now — functionality preserved but not shown
        draft_toggle.set_visible(false);
        editor_pane.status_bar_insert_after_goal(&draft_toggle);

        editor_pane.set_completion_picks(proj_cfg.completion_picks.clone());

        // The header held twelve controls, mixing six text buttons with six
        // icons. What reports a mode — Simple, focus, Library, notes — moves to
        // the status bar, which is a line of plain words and already carries the
        // other mode toggles. What acts occasionally goes to the menu, and
        // compiling moves next to the editor it compiles. The header keeps the
        // sidebar toggle, the title, Save, Preview and the menu.
        //
        // status_bar_append_left inserts directly after the first toggle, so the
        // last one added ends up leftmost: add them in reverse reading order.
        gost_menu_slot.append(&editor_pane.gost_button_for_menu());
        gost_menu_slot.append(&editor_pane.autocorrect_button_for_menu());

        header.remove(&library_btn);
        library_btn.add_css_class("status-toggle");
        // The other status words are caption-sized; a default label beside them
        // reads as a heading rather than as one of the row.
        if let Some(l) = library_btn.child().and_downcast::<Label>() {
            l.add_css_class("caption");
            l.add_css_class("dim-label");
        }
        editor_pane.status_bar_append_left(&library_btn);

        editor_pane.status_bar_append_left(&editor_pane.focus_button_for_header());
        editor_pane.status_bar_append_left(&editor_pane.simple_mode_button_for_header());

        // Git as a word, beside the other status-bar words, rather than an icon
        // in a header that has too many already.
        header.remove(&sync_btn);
        let git_label = Label::new(Some("Git"));
        git_label.add_css_class("caption");
        git_label.add_css_class("dim-label");
        sync_btn.set_child(Some(&git_label));
        sync_btn.add_css_class("status-toggle");
        editor_pane.status_bar_append_right(&sync_btn);

        header.remove(&print_header_btn);
        header.remove(&recompile_header_btn);
        header.remove(&compile_mode_slot);
        editor_pane.breadcrumb_bar_append(&compile_mode_slot);
        editor_pane.breadcrumb_bar_append(&recompile_header_btn);

        compile_btn.add_css_class("fond-pill");
        compile_btn.set_valign(gtk4::Align::Center);

        // ── Simple mode wiring ──────────────────────────────────────────────
        {
            let cfg = current_config.clone();
            let initial_simple = config.simple_mode;
            editor_pane.apply_simple_mode(initial_simple);
            editor_pane.set_on_simple_mode_toggle(move |on| {
                cfg.borrow_mut().simple_mode = on;
                let _ = cfg.borrow().save();
            });
        }

        // ── Compile mode status bar toggle ──────────────────────────────────
        let compile_mode_label = Label::new(None);
        compile_mode_label.add_css_class("caption");
        compile_mode_label.set_text(compile_mode_label_str(
            config.auto_compile,
            config.compile_on_save,
            config.manual_compile_only,
        ));
        let compile_mode_btn = {
            let btn = Button::new();
            btn.set_child(Some(&compile_mode_label));
            btn.add_css_class("flat");
            btn.add_css_class("status-toggle");
            btn.set_tooltip_text(Some("Cycle compile mode: auto → on save → manual"));
            apply_compile_mode_css(&btn, config.auto_compile, config.compile_on_save, config.manual_compile_only);

            // Beside the compile buttons in the header — it says what those
            // buttons will do, so it belongs with them rather than at the far
            // end of the status bar.
            compile_mode_slot.append(&btn);

            let auto_cm = auto_compile.clone();
            let cos_cm = compile_on_save.clone();
            let mco_cm = manual_compile_only.clone();
            let cfg_cm = current_config.clone();
            let cm_lbl = compile_mode_label.clone();
            btn.connect_clicked(move |b| {
                let auto = *auto_cm.borrow();
                let mco = *mco_cm.borrow();
                let (new_auto, new_cos, new_mco) = if auto {
                    (false, true, false)
                } else if mco {
                    (true, false, false)
                } else {
                    (false, false, true)
                };
                *auto_cm.borrow_mut() = new_auto;
                *cos_cm.borrow_mut() = new_cos;
                *mco_cm.borrow_mut() = new_mco;
                cm_lbl.set_text(compile_mode_label_str(new_auto, new_cos, new_mco));
                apply_compile_mode_css(b, new_auto, new_cos, new_mco);
                let mut cfg = cfg_cm.borrow_mut();
                cfg.auto_compile = new_auto;
                cfg.compile_on_save = new_cos;
                cfg.manual_compile_only = new_mco;
                let _ = cfg.save();
            });
            btn
        };

        apply_theme(&config.theme);
        if config.high_contrast {
            window.add_css_class("high-contrast");
        }
        // Import is experimental — only visible in developer mode
        menus.menu_import_item.set_visible(config.developer_mode);

        let editor_for_dark = editor_pane.clone();
        adw::StyleManager::default().connect_dark_notify(move |mgr| {
            editor_for_dark.apply_style_scheme(mgr.is_dark());
        });

        let auto_detected_bib = wire_citations(&CitationCtx {
            window: window.clone(),
            editor_pane: editor_pane.clone(),
            citation_panel: citation_panel.clone(),
            ref_manager: ref_manager.clone(),
            current_config: current_config.clone(),
            project_root: project_root.clone(),
            effective_bib: effective_bib.clone(),
            effective_cv_elements: effective_cv_elements.clone(),
        });
        // ── Sidebar toggle (item 1) ─────────────────────────────────────────
        // (left_paned is set up in the layout section below; we capture it via Rc)
        let focus_active: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let preview_vis_holder: Rc<RefCell<Option<GtkBox>>> = Rc::new(RefCell::new(None));
        let sidebar_visible: Rc<RefCell<bool>> = Rc::new(RefCell::new(true));
        let sidebar_visible_c = sidebar_visible.clone();
        // left_paned_ref set after layout — closure reads it through the Rc
        let left_paned_holder: Rc<RefCell<Option<GtkBox>>> = Rc::new(RefCell::new(None));
        let lpane_for_btn = left_paned_holder.clone();
        sidebar_btn.connect_clicked(move |_| {
            let mut v = sidebar_visible_c.borrow_mut();
            *v = !*v;
            if let Some(lp) = lpane_for_btn.borrow().as_ref() {
                lp.set_visible(*v);
            }
        });

        // ── Focus mode toggle — status bar button, dims sidebar, hides preview
        {
            let focus_active_c = focus_active.clone();
            let preview_vis_for_focus = preview_vis_holder.clone();
            let window_for_focus = window.clone();
            let editor_for_focus = editor_pane.clone();
            editor_pane.set_on_focus_toggle(move |focused| {
                *focus_active_c.borrow_mut() = focused;
                if focused {
                    window_for_focus.add_css_class("zen-writing");
                } else {
                    window_for_focus.remove_css_class("zen-writing");
                }
                editor_for_focus.set_zen_width(focused);
                if let Some(pc) = preview_vis_for_focus.borrow().as_ref() {
                    pc.set_visible(!focused);
                }
            });
        }

        let menu_ctx = MenuCtx {
            window: window.clone(),
            editor_pane: editor_pane.clone(),
            preview_pane: preview_pane.clone(),
            error_panel: error_panel.clone(),
            citation_panel: citation_panel.clone(),
            toast_overlay: toast_overlay.clone(),
            current_config: current_config.clone(),
            project_root: project_root.clone(),
            writing_log: writing_log.clone(),
            menu_popover: menu_popover.clone(),
            auto_compile: auto_compile.clone(),
            compile_on_save: compile_on_save.clone(),
            manual_compile_only: manual_compile_only.clone(),
            debounce_ms: debounce_ms.clone(),
            compile_mode_btn: compile_mode_btn.clone(),
            compile_mode_label: compile_mode_label.clone(),
            effective_cv_elements: effective_cv_elements.clone(),
            auto_detected_bib: auto_detected_bib.clone(),
            print_header_btn: print_header_btn.clone(),
            save_btn: save_btn.clone(),
            sync_btn: sync_btn.clone(),
        };

        wire_app_menus(&menu_ctx, &menus);
        // ── Menu: Import (picker dialog) ───────────────────────────────────


        // ── Citation panel: "Skrizhal" button launches the actual app ────────
        {
            let toast_for_skrizhal = toast_overlay.clone();
            citation_panel.set_on_open_skrizhal(move || {
                let installed = crate::git_sync::host_command("flatpak")
                    .args(["info", "io.github.calstfrancis.Skrizhal"])
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                if !installed {
                    toast_for_skrizhal.add_toast(adw::Toast::new(
                        "Skrizhal isn't installed — see calstfrancis.github.io/flatpak",
                    ));
                    return;
                }
                let result = crate::git_sync::host_command("flatpak")
                    .args(["run", "io.github.calstfrancis.Skrizhal"])
                    .spawn();
                if let Err(e) = result {
                    tracing::warn!("Couldn't launch Skrizhal: {e}");
                    toast_for_skrizhal.add_toast(adw::Toast::new("Couldn't open Skrizhal"));
                }
            });
        }

        let window_for_import = window.clone();
        let editor_for_import = editor_pane.clone();
        let menu_popover_for_import = menu_popover.clone();
        let work_dir_for_import = project_root.clone();
        let config_for_import = current_config.clone();
        let toast_overlay_for_import = toast_overlay.clone();
        let pdf_item_for_dlg = menus.menu_import_pdf_item.clone();
        menus.menu_import_item.connect_clicked(move |_| {
            menu_popover_for_import.popdown();

            let dlg = adw::Window::new();
            dlg.set_title(Some("Import"));
            dlg.set_default_width(280);
            dlg.set_modal(true);
            dlg.set_transient_for(Some(&window_for_import));
            dlg.set_deletable(true);

            let header_dlg = adw::HeaderBar::new();
            header_dlg.add_css_class("fond-chrome");
            let title_lbl = gtk4::Label::new(Some("Import"));
            title_lbl.add_css_class("heading");
            header_dlg.set_title_widget(Some(&title_lbl));

            let row_box = GtkBox::new(Orientation::Vertical, 0);
            row_box.set_margin_top(8);
            row_box.set_margin_bottom(8);

            let make_row = |icon: &str, label: &str| -> adw::ActionRow {
                let row = adw::ActionRow::new();
                row.set_title(label);
                row.set_activatable(true);
                row.add_prefix(&gtk4::Image::from_icon_name(icon));
                row.add_suffix(&gtk4::Image::from_icon_name("go-next-symbolic"));
                row
            };

            let group = adw::PreferencesGroup::new();
            group.set_margin_start(12);
            group.set_margin_end(12);

            // One row per pandoc-based format, all routed through import_via_pandoc.
            for fmt in IMPORT_FORMATS {
                let row = make_row(fmt.icon, fmt.label);
                group.add(&row);
                let dlg_c = dlg.clone();
                let win_c = window_for_import.clone();
                let ep_c = editor_for_import.clone();
                let work_dir_c = work_dir_for_import.clone();
                let cfg_c = config_for_import.clone();
                let toast_c = toast_overlay_for_import.clone();
                row.connect_activated(move |_| {
                    dlg_c.close();
                    import_via_pandoc(&win_c, &ep_c, &work_dir_c, &cfg_c, &toast_c, fmt);
                });
            }

            let pdf_row = make_row("application-pdf-symbolic", "PDF (.pdf)");
            group.add(&pdf_row);
            row_box.append(&group);

            let folder_group = adw::PreferencesGroup::new();
            folder_group.set_margin_start(12);
            folder_group.set_margin_end(12);
            folder_group.set_margin_top(4);
            let folder_row = make_row("folder-symbolic", "Import Folder…");
            folder_group.add(&folder_row);
            let paste_row = make_row("edit-paste-symbolic", "Paste as Document…");
            folder_group.add(&paste_row);
            row_box.append(&folder_group);
            {
                let dlg_c = dlg.clone();
                let win_c = window_for_import.clone();
                let ep_c = editor_for_import.clone();
                let work_dir_c = work_dir_for_import.clone();
                let cfg_c = config_for_import.clone();
                let toast_c = toast_overlay_for_import.clone();
                folder_row.connect_activated(move |_| {
                    dlg_c.close();
                    import_folder_via_pandoc(&win_c, &ep_c, &work_dir_c, &cfg_c, &toast_c);
                });
            }
            {
                let dlg_c = dlg.clone();
                let win_c = window_for_import.clone();
                let ep_c = editor_for_import.clone();
                let work_dir_c = work_dir_for_import.clone();
                let cfg_c = config_for_import.clone();
                let toast_c = toast_overlay_for_import.clone();
                paste_row.connect_activated(move |_| {
                    dlg_c.close();
                    paste_as_document(&win_c, &ep_c, &work_dir_c, &cfg_c, &toast_c);
                });
            }

            // History icon button in the header, opening a read-only list of
            // past import attempts.
            let history_btn = Button::from_icon_name("document-open-recent-symbolic");
            history_btn.add_css_class("flat");
            history_btn.set_tooltip_text(Some("Import History"));
            header_dlg.pack_end(&history_btn);
            {
                let win_c = window_for_import.clone();
                let ep_c = editor_for_import.clone();
                let work_dir_c = work_dir_for_import.clone();
                let cfg_c = config_for_import.clone();
                let toast_c = toast_overlay_for_import.clone();
                let dlg_c = dlg.clone();
                history_btn.connect_clicked(move |_| {
                    dlg_c.close();
                    show_import_history_dialog(&win_c, &ep_c, &work_dir_c, &cfg_c, &toast_c);
                });
            }

            let vbox = GtkBox::new(Orientation::Vertical, 0);
            vbox.append(&header_dlg);
            vbox.append(&row_box);
            dlg.set_content(Some(&vbox));

            // PDF import uses a different pipeline (pdftotext, no structure to
            // preserve), so it stays as its own forward-clicked hidden button.
            let pdf_trigger = pdf_item_for_dlg.clone();
            let dlg_c = dlg.clone();
            pdf_row.connect_activated(move |_| {
                dlg_c.close();
                pdf_trigger.emit_clicked();
            });

            dlg.present();
        });

        wire_document_menus(&menu_ctx, &menus);

        // Rows that act on the active document go insensitive when there isn't
        // one, rather than being clickable and silently doing nothing. Computed
        // when the popover opens so it needs no per-tab signal plumbing.
        {
            let document_rows = [
                menus.menu_reapply_template_item.clone(),
                menus.menu_repair_markers_item.clone(),
                menus.menu_save_item.clone(),
                menus.menu_save_as_item.clone(),
                menus.menu_snapshots_item.clone(),
                menus.menu_history_item.clone(),
                menus.menu_export_web_item.clone(),
            ];
            let editor_for_rows = editor_pane.clone();
            menu_popover.connect_show(move |_| {
                let has_doc = editor_for_rows.get_active_path().is_some();
                for row in &document_rows {
                    row.set_sensitive(has_doc);
                }
            });
        }
        // ── Debounced compile + outline update + LSP ────────────────────────

        let preview_for_change = preview_pane.clone();
        let editor_for_change = editor_pane.clone();
        let debounce_for_change = debounce_ms.clone();
        let auto_compile_for_change = auto_compile.clone();
        let compile_on_save_for_change = compile_on_save.clone();
        let manual_compile_only_for_change = manual_compile_only.clone();
        let outline_for_change = outline_panel.clone();
        let refs_for_change = ref_manager.clone();
        let lsp_for_change = lsp_client.clone();
        let last_edit_for_change = last_edit_instant.clone();
        let gen: Rc<RefCell<u64>> = Rc::new(RefCell::new(0));
        let gen2 = gen.clone();
        let editor_pane_for_delta = editor_pane.clone();
        let editor_pane_for_bib = editor_pane.clone();
        editor_pane.set_on_change(move || {
            // While the citation popup is open, suppress compile and LSP updates —
            // partial @keys cause spurious errors and make typing difficult.
            if editor_pane_for_bib.is_bib_active() {
                return;
            }
            *last_edit_for_change.borrow_mut() = Some(std::time::Instant::now());
            *gen2.borrow_mut() += 1;
            let my_gen = *gen2.borrow();
            let preview = preview_for_change.clone();
            let editor = editor_for_change.clone();
            let gen3 = gen2.clone();
            let auto = auto_compile_for_change.clone();
            let cos = compile_on_save_for_change.clone();
            let mco = manual_compile_only_for_change.clone();
            let outline = outline_for_change.clone();
            let refs = refs_for_change.clone();
            let lsp = lsp_for_change.clone();
            let delay = Duration::from_millis(*debounce_for_change.borrow());
            let delta = editor_pane_for_delta.get_active_session_delta();
            editor_pane_for_delta.set_session_delta(delta);
            glib::timeout_add_local(delay, move || {
                if *gen3.borrow() == my_gen {
                    let should_compile = *auto.borrow()
                        && !*cos.borrow()
                        && !*mco.borrow();
                    if should_compile {
                        if let Some(path) = editor.get_active_path() {
                            if let Some(content) = editor.get_active_content() {
                                preview.set_buffer_snapshot(path.clone(), content);
                            }
                            preview.set_root_file(path);
                        }
                        preview.trigger_compile();
                    }
                    if let Some(path) = editor.get_active_path() {
                        if let Some(content) = editor.get_active_content() {
                            outline.update(&content, &path);
                            refs.update_used_keys(&content);
                        }
                    }
                    if let Some(client) = lsp.borrow_mut().as_mut() {
                        if let (Some(path), Some(content)) =
                            (editor.get_active_path(), editor.get_active_content())
                        {
                            let version = SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as i64;
                            client.did_change(&path, &content, version);
                        }
                    }
                }
                glib::ControlFlow::Break
            });
        });

        // ── Multi-file root: configured root persists across tab switches ─────
        let configured_root: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(
            proj_cfg.root_file.as_ref()
                .and_then(|r| crate::project_model::resolve_root_file(&project_root, r))
        ));
        // Project mode is OFF by default; only use configured_root when it is ON.
        // Shared with the project toggle below.
        let proj_mode_active: Rc<std::cell::Cell<bool>> = Rc::new(std::cell::Cell::new(false));

        // ── Outline + title: update on tab switch ──────────────────────────

        let outline_for_switch = outline_panel.clone();
        let refs_for_switch = ref_manager.clone();
        let dep_graph_for_switch = dep_graph.clone();
        let title_widget_for_switch = file_title_widget.clone();
        let preview_for_switch = preview_pane.clone();
        let style_btn_for_switch = style_btn.clone();
        let editor_pane_for_switch_delta = editor_pane.clone();
        let cs_stack = gtk4::Stack::new();
        cs_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        let cs_stack_for_switch = cs_stack.clone();
        let cs_stack_for_open = cs_stack.clone();
        let style_btn_for_cv_switch = style_btn.clone();
        let style_btn_for_cv_open = style_btn.clone();

        let editor_pane_cv_switch = editor_pane.clone();
        let citation_panel_for_switch = citation_panel.clone();
        let configured_root_for_switch = configured_root.clone();
        let proj_mode_for_switch = proj_mode_active.clone();
        // Track per-file content hashes so tab switches don't recompile unchanged files.
        let switch_hash_map: Rc<RefCell<std::collections::HashMap<std::path::PathBuf, u64>>> =
            Rc::new(RefCell::new(std::collections::HashMap::new()));
        editor_pane.set_on_page_switch(move |content, path| {
            let delta = editor_pane_for_switch_delta.get_active_session_delta();
            editor_pane_for_switch_delta.set_session_delta(delta);
            outline_for_switch.update(&content, &path);
            refs_for_switch.update_used_keys(&content);
            dep_graph_for_switch.refresh(Some(&path));
            // Only recompile if the content has changed since the last compile for this file.
            let content_hash = {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                content.hash(&mut h);
                h.finish()
            };
            let prev_hash = switch_hash_map.borrow().get(&path).copied();
            let needs_compile = prev_hash != Some(content_hash);
            switch_hash_map.borrow_mut().insert(path.clone(), content_hash);
            preview_for_switch.set_buffer_snapshot(path.clone(), content.clone());
            // Only use configured root when project mode is actively ON; otherwise
            // always compile the active file so the preview matches the editor.
            let root_for_compile = if proj_mode_for_switch.get() {
                configured_root_for_switch.borrow().clone().unwrap_or_else(|| path.clone())
            } else {
                path.clone()
            };
            preview_for_switch.set_root_file(root_for_compile.clone());
            if needs_compile {
                preview_for_switch.trigger_compile();
            }
            let title = extract_doc_title(&content).or_else(|| {
                path.file_name().and_then(|n| n.to_str())
                    .map(|n| n.strip_suffix(".typ").unwrap_or(n).to_string())
            }).unwrap_or_default();
            title_widget_for_switch.set_title(&title);
            // Show root breadcrumb only when project mode is on and root differs
            if proj_mode_for_switch.get() {
                if let Some(ref root_path) = *configured_root_for_switch.borrow() {
                    if root_path != &path {
                        let root_name = root_path.file_name().and_then(|n| n.to_str()).unwrap_or("root");
                        let active_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
                        title_widget_for_switch.set_subtitle(&format!("{root_name} › {active_name}"));
                    } else {
                        title_widget_for_switch.set_subtitle("");
                    }
                } else {
                    title_widget_for_switch.set_subtitle("");
                }
            } else {
                title_widget_for_switch.set_subtitle("");
            }
            let is_cv = super::template_dialog::parse_doc_kind(&content)
                .map(|k| k == "cv")
                .unwrap_or(false);
            editor_pane_cv_switch.set_cv_mode(is_cv);
            citation_panel_for_switch.set_cv_mode(is_cv);
            if is_cv {
                editor_pane_cv_switch.update_cv_style_label(&content);
            }
            style_btn_for_cv_switch.set_visible(!is_cv);
            cs_stack_for_switch.set_visible_child_name(if is_cv { "cv" } else { "normal" });
            let style_name = super::template_dialog::parse_style_key(&content)
                .and_then(|key| super::template_dialog::style_name_for_key(&key))
                .unwrap_or("Style");
            style_btn_for_switch.set_label(style_name);
            // Same refresh the open handler does: without it the format bar
            // keeps showing the previous tab's font and size, so the picker
            // reads as if this document were already set that way.
            editor_pane_cv_switch.set_doc_font_label(
                &super::template_dialog::parse_font(&content).unwrap_or_else(|| "font".into())
            );
            editor_pane_cv_switch.set_doc_size_label(
                &super::template_dialog::parse_font_size(&content).unwrap_or_else(|| "size".into())
            );
        });

        // ── Modified / autosave indicator ──────────────────────────────────────
        {
            let title_for_modified = file_title_widget.clone();
            editor_pane.set_on_modified_changed(move |modified| {
                if modified {
                    title_for_modified.set_subtitle("Modified");
                } else {
                    title_for_modified.set_subtitle("Saved");
                    let tw = title_for_modified.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_secs(2), move || {
                        tw.set_subtitle("");
                    });
                }
            });
        }

        // ── LSP: did_open + recent tracking + auto-save recovery ────────────

        let lsp_for_open = lsp_client.clone();
        let current_config_for_open = current_config.clone();
        // Recovery queue: at most one dialog on screen at a time. Each dialog's
        // response handler calls show_next to pop and show the next item.
        let recovery_queue: Rc<RefCell<std::collections::VecDeque<(PathBuf, String, String)>>> =
            Rc::new(RefCell::new(std::collections::VecDeque::new()));
        let show_next_recovery: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        {
            let queue = recovery_queue.clone();
            let show_next_weak = Rc::downgrade(&show_next_recovery);
            let win = window.clone();
            let ep = editor_pane.clone();
            *show_next_recovery.borrow_mut() = Some(Box::new(move || {
                let next = queue.borrow_mut().pop_front();
                if let Some((path, content, ts)) = next {
                    let dlg = adw::MessageDialog::new(
                        Some(&win),
                        Some("Unsaved changes detected"),
                        Some(&format!(
                            "An auto-save from {ts} is newer than the last saved version.\n\
                             Restore the auto-saved content?"
                        )),
                    );
                    dlg.add_response("discard", "Discard");
                    dlg.add_response("restore", "Restore");
                    dlg.set_response_appearance("restore", adw::ResponseAppearance::Suggested);
                    dlg.set_default_response(Some("restore"));
                    let ep_c = ep.clone();
                    let path_c = path.clone();
                    let sn = show_next_weak.clone();
                    dlg.connect_response(None, move |_, resp| {
                        if resp == "restore" {
                            ep_c.set_content(&path_c, &content);
                        }
                        crate::auto_save::clear(&path_c);
                        if let Some(f) = sn.upgrade() {
                            if let Some(cb) = f.borrow().as_ref() { cb(); }
                        }
                    });
                    dlg.present();
                }
            }));
        }
        let recovery_queue_for_open = recovery_queue.clone();
        let show_next_for_open = show_next_recovery.clone();
        let style_btn_for_open = style_btn.clone();
        let file_start_words_for_open = file_start_words.clone();
        let title_widget_for_open = file_title_widget.clone();
        let ep_for_open = editor_pane.clone();
        let ep_cv_for_open = editor_pane.clone();
        let citation_panel_for_open = citation_panel.clone();
        editor_pane.set_on_file_opened(move |path, content| {
            // Track initial word count for this file (first open only)
            let mut starts = file_start_words_for_open.borrow_mut();
            if !starts.contains_key(&path) {
                starts.insert(path.clone(), count_words(&content));
            }
            drop(starts);
            if let Some(client) = lsp_for_open.borrow_mut().as_mut() {
                client.did_open(&path, &content);
            }
            let is_cv = super::template_dialog::parse_doc_kind(&content)
                .map(|k| k == "cv")
                .unwrap_or(false);
            ep_cv_for_open.set_cv_mode(is_cv);
            citation_panel_for_open.set_cv_mode(is_cv);
            if is_cv { ep_cv_for_open.update_cv_style_label(&content); }
            style_btn_for_cv_open.set_visible(!is_cv);
            cs_stack_for_open.set_visible_child_name(if is_cv { "cv" } else { "normal" });
            let style_name = super::template_dialog::parse_style_key(&content)
                .and_then(|key| super::template_dialog::style_name_for_key(&key))
                .unwrap_or("Style");
            style_btn_for_open.set_label(style_name);
            ep_for_open.set_doc_font_label(
                &super::template_dialog::parse_font(&content).unwrap_or_else(|| "font".into())
            );
            ep_for_open.set_doc_size_label(
                &super::template_dialog::parse_font_size(&content).unwrap_or_else(|| "size".into())
            );
            let title = extract_doc_title(&content).or_else(|| {
                path.file_name().and_then(|n| n.to_str())
                    .map(|n| n.strip_suffix(".typ").unwrap_or(n).to_string())
            }).unwrap_or_default();
            title_widget_for_open.set_title(&title);
            let mut cfg = current_config_for_open.borrow_mut();
            cfg.push_recent(path.clone());
            let _ = cfg.save();

            // Auto-save recovery check — queue to avoid stacking dialogs during session restore
            if let Some((recovered, save_time)) = crate::auto_save::find_recovery(&path) {
                let ts = chrono::DateTime::<chrono::Local>::from(save_time)
                    .format("%Y-%m-%d %H:%M")
                    .to_string();
                let was_empty = recovery_queue_for_open.borrow().is_empty();
                recovery_queue_for_open.borrow_mut().push_back((path.clone(), recovered, ts));
                if was_empty {
                    if let Some(f) = show_next_for_open.borrow().as_ref() { f(); }
                }
            }
        });

        // ── LSP: request completions when # trigger fires ────────────────────

        let last_completion_request: Rc<RefCell<Option<u64>>> = Rc::new(RefCell::new(None));

        let lsp_for_comp = lsp_client.clone();
        let last_req_for_comp = last_completion_request.clone();
        editor_pane.set_on_completion_needed(move |path, line, col| {
            if let Some(client) = lsp_for_comp.borrow_mut().as_mut() {
                let id = client.request_completion(&path, line, col);
                *last_req_for_comp.borrow_mut() = Some(id);
            }
        });

        // ── Compile done callback ────────────────────────────────────────────

        // Inline compile-error banner widgets created here so the compile callback can capture them
        let error_banner = Label::new(None);
        error_banner.add_css_class("error");
        error_banner.set_wrap(true);
        error_banner.set_xalign(0.0);
        error_banner.set_margin_start(8);
        error_banner.set_margin_end(8);
        error_banner.set_margin_top(4);
        error_banner.set_margin_bottom(4);
        error_banner.set_visible(false);
        let error_banner_scroll = gtk4::ScrolledWindow::new();
        error_banner_scroll.set_child(Some(&error_banner));
        error_banner_scroll.set_max_content_height(72);
        error_banner_scroll.set_propagate_natural_height(true);
        error_banner_scroll.set_visible(false);
        // file_tree holder: filled after FileTree is constructed below
        let file_tree_holder: Rc<RefCell<Option<super::file_tree::FileTree>>> = Rc::new(RefCell::new(None));

        // LSP dedup: when LSP has live diagnostics, suppress compile-stderr errors
        let lsp_has_diags: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        let compile_progress = gtk4::ProgressBar::new();
        compile_progress.add_css_class("compile-progress");
        compile_progress.set_pulse_step(0.08);
        let compile_rev = gtk4::Revealer::new();
        compile_rev.set_transition_type(gtk4::RevealerTransitionType::SlideDown);
        compile_rev.set_transition_duration(120);
        compile_rev.set_child(Some(&compile_progress));
        compile_rev.set_reveal_child(false);

        let error_panel_for_compile = error_panel.clone();
        let editor_for_diag = editor_pane.clone();
        let root_for_compile = project_root.clone();
        let popout_pane_for_compile = popout_pane.clone();
        let dep_graph_for_compile = dep_graph.clone();
        let lsp_diags_for_compile = lsp_has_diags.clone();
        let error_banner_for_compile = error_banner_scroll.clone();
        let error_banner_lbl_for_compile = error_banner.clone();
        let _file_tree_holder_for_compile = file_tree_holder.clone();
        let toast_for_compile = toast_overlay.clone();
        let has_errors_for_compile = has_compile_errors.clone();
        let window_for_compile = window.clone();

        let compile_rev_for_start = compile_rev.clone();
        let compile_bar_for_start = compile_progress.clone();
        let compile_btn_for_start = compile_btn.clone();
        // Holds the active pulse timer so we can cancel it before starting a new one.
        let pulse_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        preview_pane.set_on_compile_start(move || {
            compile_btn_for_start.add_css_class("compiling-pulse");
            compile_rev_for_start.set_reveal_child(true);
            let bar = compile_bar_for_start.clone();
            let rev = compile_rev_for_start.clone();
            let timer_slot = pulse_timer.clone();
            // Cancel any previous pulse timer before spawning a new one.
            if let Some(id) = pulse_timer.borrow_mut().take() { id.remove(); }
            let id = glib::timeout_add_local(Duration::from_millis(80), move || {
                if rev.reveals_child() {
                    bar.pulse();
                    glib::ControlFlow::Continue
                } else {
                    *timer_slot.borrow_mut() = None;
                    glib::ControlFlow::Break
                }
            });
            *pulse_timer.borrow_mut() = Some(id);
        });

        let compile_rev_for_cancel = compile_rev.clone();
        let compile_btn_for_cancel = compile_btn.clone();
        preview_pane.set_on_compile_cancelled(move || {
            compile_btn_for_cancel.remove_css_class("compiling-pulse");
            compile_rev_for_cancel.set_reveal_child(false);
        });

        let compile_rev_for_done = compile_rev.clone();
        let compile_btn_for_done = compile_btn.clone();
        preview_pane.set_on_compile_done(move |result, warnings| {
            compile_btn_for_done.remove_css_class("compiling-pulse");
            compile_rev_for_done.set_reveal_child(false);
            match &result {
                None => {
                    let had_errors = *has_errors_for_compile.borrow();
                    *has_errors_for_compile.borrow_mut() = false;
                    error_panel_for_compile.clear();
                    editor_for_diag.clear_diagnostic_marks();
                    editor_for_diag.clear_error_marks();
                    error_banner_for_compile.set_visible(false);
                    error_banner_lbl_for_compile.set_visible(false);
                    // A clean compile can still have warnings — deprecations,
                    // unused imports. They go through the same panel as errors
                    // rather than being dropped, but never raise a banner or a
                    // toast, since nothing is broken.
                    let warns = if warnings.is_empty() {
                        Vec::new()
                    } else {
                        parse_typst_errors(&warnings, &root_for_compile)
                    };
                    if warns.is_empty() {
                        error_panel_for_compile.widget().set_visible(false);
                        editor_for_diag.set_diag_summary(0, 0);
                        window_for_compile.set_title(Some("Zerkalo"));
                    } else {
                        let diags: Vec<(std::path::PathBuf, u32, bool, String)> = warns
                            .iter()
                            .map(|w| (w.file.clone(), w.line, false, w.message.clone()))
                            .collect();
                        editor_for_diag.mark_diagnostics(&diags);
                        editor_for_diag.set_diag_summary(0, warns.len() as u32);
                        window_for_compile.set_title(Some(&format!(
                            "Zerkalo ({} warning{})",
                            warns.len(),
                            if warns.len() == 1 { "" } else { "s" }
                        )));
                        error_panel_for_compile.show_compile_errors(warns);
                        error_panel_for_compile.set_build_log(&warnings);
                        error_panel_for_compile.widget().set_visible(true);
                    }
                    // Only show success toast when recovering from errors
                    if had_errors {
                        let t = adw::Toast::new("Compiled successfully");
                        t.set_timeout(2);
                        toast_for_compile.add_toast(t);
                    }
                }
                Some(stderr) => {
                    *has_errors_for_compile.borrow_mut() = true;
                    let already_visible = error_banner_for_compile.is_visible();
                    let first_line = stderr.lines().next().unwrap_or("Compile error").to_string();
                    error_banner_lbl_for_compile.set_text(&first_line);
                    error_banner_lbl_for_compile.set_visible(true);
                    error_banner_for_compile.set_visible(true);
                    if already_visible {
                        error_banner_lbl_for_compile.add_css_class("shake-banner");
                        let lbl_shake = error_banner_lbl_for_compile.clone();
                        glib::timeout_add_local_once(Duration::from_millis(600), move || {
                            lbl_shake.remove_css_class("shake-banner");
                        });
                    }
                    let t = adw::Toast::new("Compile error — see panel");
                    t.set_timeout(3);
                    toast_for_compile.add_toast(t);
                    // If LSP is providing diagnostics, skip showing compile stderr
                    if *lsp_diags_for_compile.borrow() {
                        dep_graph_for_compile.refresh(None);
                        if let Some(pane) = popout_pane_for_compile.borrow().as_ref() {
                            pane.refresh_display();
                        }
                        return;
                    }
                    let errors = parse_typst_errors(stderr, &root_for_compile);
                    let err_count = errors.iter().filter(|e| matches!(e.severity, Severity::Error)).count();
                    let warn_count = errors.len() - err_count;
                    let diags: Vec<(std::path::PathBuf, u32, bool, String)> = errors
                        .iter()
                        .map(|e| (e.file.clone(), e.line, matches!(e.severity, Severity::Error), e.message.clone()))
                        .collect();
                    editor_for_diag.mark_diagnostics(&diags);
                    let error_lines: Vec<(std::path::PathBuf, u32)> = errors.iter()
                        .filter(|e| matches!(e.severity, Severity::Error))
                        .map(|e| (e.file.clone(), e.line))
                        .collect();
                    editor_for_diag.mark_error_lines(&error_lines);
                    editor_for_diag.set_diag_summary(err_count as u32, warn_count as u32);
                    // Update window title with error count
                    let title = match (err_count, warn_count) {
                        (e, 0) => format!("Zerkalo ({e} error{})", if e == 1 { "" } else { "s" }),
                        (0, w) => format!("Zerkalo ({w} warning{})", if w == 1 { "" } else { "s" }),
                        (e, w) => format!("Zerkalo ({e} error{}, {w} warning{})",
                            if e == 1 { "" } else { "s" },
                            if w == 1 { "" } else { "s" }),
                    };
                    window_for_compile.set_title(Some(&title));
                    error_panel_for_compile.show_compile_errors(errors);
                    error_panel_for_compile.set_build_log(stderr);
                    error_panel_for_compile.widget().set_visible(true);
                }
            }
            dep_graph_for_compile.refresh(None);
            if let Some(pane) = popout_pane_for_compile.borrow().as_ref() {
                pane.refresh_display();
            }
        });

        let editor_for_jump = editor_pane.clone();
        error_panel.set_on_jump(move |path, line| {
            if let Ok(content) = std::fs::read_to_string(&path) {
                editor_for_jump.open_file(path.clone(), &content);
            }
            editor_for_jump.jump_to_line(&path, line);
        });

        // Export log: toast with saved path
        {
            let toast_for_export = toast_overlay.clone();
            error_panel.set_on_export_done(move |path| {
                let t = adw::Toast::new(&format!("Error log saved to {path}"));
                t.set_timeout(4);
                toast_for_export.add_toast(t);
            });
        }

        // Try-Fix: look up the matching pattern for this error's message and apply
        // its fix at the reported line, via an undoable buffer replace.
        {
            let editor_for_fix = editor_pane.clone();
            {
                let editor_for_src = editor_pane.clone();
                error_panel.set_source_line_provider(move |path, line| {
                    editor_for_src.line_text(path, line)
                });
            }
            error_panel.set_on_try_fix(move |path, line, message| {
                let Some(content) = editor_for_fix.get_active_content() else { return };
                let Some(fix) = crate::error_patterns::match_fix(&message) else { return };
                let Some(fix_fn) = fix.fix_fn else { return };
                let line_idx = (line as usize).saturating_sub(1);
                if let Some(patched) = fix_fn(&content, line_idx) {
                    editor_for_fix.set_active_content_undoable(&patched);
                }
                let _ = path;
            });
        }

        wire_startup(&LifecycleCtx {
            window: window.clone(),
            editor_pane: editor_pane.clone(),
            error_panel: error_panel.clone(),
            toast_overlay: toast_overlay.clone(),
            current_config: current_config.clone(),
            project_root: project_root.clone(),
            auto_save_idle_ms: auto_save_idle_ms.clone(),
            sync_btn: sync_btn.clone(),
            lsp_client: lsp_client.clone(),
            lsp_has_diags: lsp_has_diags.clone(),
            last_completion_request: last_completion_request.clone(),
            last_edit_instant: last_edit_instant.clone(),
            shown_simple_intro: config.shown_simple_intro,
        });
        // ── Layout ──────────────────────────────────────────────────────────

        // Preview toolbar: linked zoom group + pop-out button (rubric style)
        let zoom_out_btn = Button::from_icon_name("zoom-out-symbolic");
        zoom_out_btn.set_tooltip_text(Some("Zoom out"));
        zoom_out_btn.update_property(&[gtk4::accessible::Property::Label("Zoom out preview")]);

        let zoom_in_btn = Button::from_icon_name("zoom-in-symbolic");
        zoom_in_btn.set_tooltip_text(Some("Zoom in"));
        zoom_in_btn.update_property(&[gtk4::accessible::Property::Label("Zoom in preview")]);

        let fit_width_btn = Button::from_icon_name("zoom-fit-best-symbolic");
        fit_width_btn.set_tooltip_text(Some("Fit page width"));
        fit_width_btn.add_css_class("flat");
        fit_width_btn.update_property(&[gtk4::accessible::Property::Label("Fit page width")]);

        let fit_page_btn = Button::from_icon_name("view-fullscreen-symbolic");
        fit_page_btn.set_tooltip_text(Some("Fit page to window"));
        fit_page_btn.add_css_class("flat");
        fit_page_btn.update_property(&[gtk4::accessible::Property::Label("Fit page to window")]);

        // Zoom buttons as a linked pill (rubric pattern)
        let zoom_box = GtkBox::new(Orientation::Horizontal, 0);
        zoom_box.add_css_class("linked");
        zoom_box.append(&zoom_out_btn);
        zoom_box.append(&zoom_in_btn);

        let zoom_pct = (config.preview_zoom * 100.0).round() as u32;
        let zoom_label = Label::new(Some(&format!("{zoom_pct}%")));
        zoom_label.set_width_chars(5);
        zoom_label.set_xalign(0.5);
        zoom_label.add_css_class("caption");
        zoom_label.add_css_class("dim-label");

        let popout_btn = Button::from_icon_name("window-new-symbolic");
        popout_btn.add_css_class("flat");
        popout_btn.update_property(&[gtk4::accessible::Property::Label("Pop out preview window")]);
        popout_btn.set_tooltip_text(Some("Pop out preview"));

        let ref_toggle_btn = ToggleButton::with_label("Help");
        ref_toggle_btn.add_css_class("flat");
        ref_toggle_btn.set_tooltip_text(Some("Toggle Cheatsheet & Help"));
        ref_toggle_btn.update_property(&[gtk4::accessible::Property::Label("Toggle cheatsheet and help panel")]);

        // Page navigation
        let page_prev_btn = Button::from_icon_name("go-previous-symbolic");
        page_prev_btn.add_css_class("flat");
        page_prev_btn.set_tooltip_text(Some("Previous page"));
        page_prev_btn.update_property(&[gtk4::accessible::Property::Label("Previous page")]);
        let page_next_btn = Button::from_icon_name("go-next-symbolic");
        page_next_btn.add_css_class("flat");
        page_next_btn.set_tooltip_text(Some("Next page"));
        page_next_btn.update_property(&[gtk4::accessible::Property::Label("Next page")]);
        let page_label = Label::new(Some(""));
        page_label.add_css_class("caption");
        page_label.add_css_class("dim-label");
        page_label.set_width_chars(8);
        page_label.set_xalign(0.5);

        let compile_time_label = Label::new(None);
        compile_time_label.add_css_class("caption");
        compile_time_label.add_css_class("dim-label");
        compile_time_label.set_tooltip_text(Some("Last compile time"));

        let preview_toolbar = GtkBox::new(Orientation::Horizontal, 4);
        preview_toolbar.set_margin_start(8);
        preview_toolbar.set_margin_end(8);
        preview_toolbar.set_margin_top(4);
        preview_toolbar.set_margin_bottom(4);
        // The bar reads: where you are, how big, how the last compile went —
        // then everything occasional behind one overflow. It carried ten
        // controls, which is more chrome than the page it sits under.
        preview_toolbar.add_css_class("fond-chrome");
        preview_toolbar.add_css_class("fond-edge-top");

        let page_nav_box = GtkBox::new(Orientation::Horizontal, 0);
        page_nav_box.add_css_class("linked");
        page_nav_box.append(&page_prev_btn);
        page_nav_box.append(&page_next_btn);
        preview_toolbar.append(&page_nav_box);
        preview_toolbar.append(&page_label);
        preview_toolbar.append(&zoom_box);
        preview_toolbar.append(&zoom_label);

        let preview_spacer = GtkBox::new(Orientation::Horizontal, 0);
        preview_spacer.set_hexpand(true);
        preview_toolbar.append(&preview_spacer);
        preview_toolbar.append(&compile_time_label);

        // Fit, Help and pop-out are reached from here rather than sitting in the
        // bar. They are re-parented, not duplicated, so their existing handlers
        // and toggle state carry over untouched.
        let pv_more_box = GtkBox::new(Orientation::Vertical, 4);
        pv_more_box.set_margin_top(8);
        pv_more_box.set_margin_bottom(8);
        pv_more_box.set_margin_start(8);
        pv_more_box.set_margin_end(8);
        for (label, w) in [
            ("Fit width", fit_width_btn.clone().upcast::<gtk4::Widget>()),
            ("Fit page", fit_page_btn.clone().upcast::<gtk4::Widget>()),
            ("Open in a window", popout_btn.clone().upcast::<gtk4::Widget>()),
        ] {
            let row = GtkBox::new(Orientation::Horizontal, 8);
            let lab = Label::new(Some(label));
            lab.set_xalign(0.0);
            lab.set_hexpand(true);
            row.append(&lab);
            w.set_valign(gtk4::Align::Center);
            row.append(&w);
            pv_more_box.append(&row);
        }
        let pv_more_popover = gtk4::Popover::new();
        pv_more_popover.set_child(Some(&pv_more_box));
        let pv_more_btn = gtk4::MenuButton::new();
        pv_more_btn.set_icon_name("view-more-symbolic");
        pv_more_btn.add_css_class("flat");
        pv_more_btn.set_tooltip_text(Some("Preview options"));
        pv_more_btn.set_popover(Some(&pv_more_popover));
        preview_toolbar.append(&ref_toggle_btn);
        preview_toolbar.append(&pv_more_btn);

        // on_zoom_changed wires all zoom changes (including auto-fit) to the label
        {
            let zoom_lbl_auto = zoom_label.clone();
            preview_pane.set_on_zoom_changed(move |z| {
                zoom_lbl_auto.set_text(&format!("{}%", (z * 100.0).round() as u32));
            });
        }

        // Compile time display
        {
            let lbl = compile_time_label.clone();
            preview_pane.set_on_compile_time(move |ms, pages| {
                crate::compile_stats::record(ms);
                let secs = ms as f64 / 1000.0;
                if let Some(n) = pages {
                    lbl.set_text(&format!("✓ {n} page{} · {secs:.1}s", if n == 1 { "" } else { "s" }));
                } else {
                    lbl.set_text(&format!("✗ {secs:.1}s"));
                }
                if ms >= 3000 {
                    lbl.add_css_class("warning");
                    lbl.set_tooltip_text(Some(
                        "Compilation took over 3 s — tips:\n\
                         \u{2022} Use Draft profile (header bar) for faster preview\n\
                         \u{2022} Move large images out of the main body\n\
                         \u{2022} Split the document into included files"
                    ));
                } else {
                    lbl.remove_css_class("warning");
                    lbl.set_tooltip_text(Some("Last compile time"));
                }
            });
        }

        // Zoom button wiring
        let preview_for_zoom_out = preview_pane.clone();
        zoom_out_btn.connect_clicked(move |_| {
            let new_z = (preview_for_zoom_out.zoom() - 0.25).max(0.25);
            preview_for_zoom_out.set_zoom(new_z);
            preview_for_zoom_out.show_zoom_osd(new_z);
        });

        let preview_for_zoom_in = preview_pane.clone();
        zoom_in_btn.connect_clicked(move |_| {
            let new_z = (preview_for_zoom_in.zoom() + 0.25).min(4.0);
            preview_for_zoom_in.set_zoom(new_z);
            preview_for_zoom_in.show_zoom_osd(new_z);
        });

        // Fit width / fit page buttons
        let preview_for_fw = preview_pane.clone();
        fit_width_btn.connect_clicked(move |_| {
            preview_for_fw.fit_width();
            preview_for_fw.show_zoom_osd(preview_for_fw.zoom());
        });

        let preview_for_fp = preview_pane.clone();
        fit_page_btn.connect_clicked(move |_| {
            preview_for_fp.fit_page();
            preview_for_fp.show_zoom_osd(preview_for_fp.zoom());
        });

        // Page navigation wiring
        {
            let lbl = page_label.clone();
            preview_pane.set_on_page_changed(move |current, total| {
                lbl.set_text(&format!("{} / {}", current + 1, total));
            });
        }
        {
            let p = preview_pane.clone();
            page_prev_btn.connect_clicked(move |_| {
                let cur = p.current_page_idx();
                if cur > 0 { p.scroll_to_page(cur - 1); }
            });
        }
        {
            let p = preview_pane.clone();
            page_next_btn.connect_clicked(move |_| {
                let cur = p.current_page_idx();
                let total = p.page_count();
                if total > 0 && cur < total - 1 { p.scroll_to_page(cur + 1); }
            });
        }

        // ── Preview click-to-jump wiring ─────────────────────────────────────
        {
            let editor_for_jump = editor_pane.clone();
            let window_for_jump = window.clone();
            let preview_for_jump = preview_pane.clone();
            preview_pane.set_on_click_jump(move |page, rel_y| {
                handle_preview_click_jump(
                    &preview_for_jump,
                    &editor_for_jump,
                    &window_for_jump,
                    page,
                    rel_y,
                );
            });
        }

        // ── Preview double-click-word-to-jump wiring ─────────────────────────
        {
            let editor_for_word_jump = editor_pane.clone();
            let window_for_word_jump = window.clone();
            let preview_for_word_jump = preview_pane.clone();
            preview_pane.set_on_word_click_jump(move |page, rel_x, rel_y| {
                handle_preview_word_jump(
                    &preview_for_word_jump,
                    &editor_for_word_jump,
                    &window_for_word_jump,
                    page,
                    rel_x,
                    rel_y,
                );
            });
        }

        let preview_container = GtkBox::new(Orientation::Vertical, 0);
        preview_container.set_hexpand(true);
        preview_container.set_vexpand(true);
        preview_container.append(&Separator::new(Orientation::Horizontal));
        preview_container.append(preview_pane.widget());
        preview_container.append(&error_banner_scroll);
        preview_container.append(&Separator::new(Orientation::Horizontal));
        preview_container.append(&preview_toolbar);

        // ── Reference panel (Cheatsheet + Help + FAQ) with back bar ─────────
        let ref_notebook = Notebook::new();
        ref_notebook.set_vexpand(true);
        ref_notebook.set_hexpand(true);
        {
            let cs_lbl = Label::new(Some("Cheatsheet"));
            let normal_cs = super::help_window::cheatsheet_scroll();
            let cv_cs = super::help_window::cv_cheatsheet_scroll();
            cs_stack.add_named(&normal_cs, Some("normal"));
            cs_stack.add_named(&cv_cs, Some("cv"));
            cs_stack.set_visible_child_name("normal");
            cs_stack.set_hexpand(true);
            cs_stack.set_vexpand(true);
            ref_notebook.append_page(&cs_stack, Some(&cs_lbl));
            let help_lbl = Label::new(Some("Help"));
            let help_scroll = super::help_window::overview_scroll();
            ref_notebook.append_page(&help_scroll, Some(&help_lbl));
            let faq_lbl = Label::new(Some("FAQ"));
            let faq_scroll = super::help_window::faq_scroll();
            ref_notebook.append_page(&faq_scroll, Some(&faq_lbl));
        }

        // Back-to-preview bar at the bottom of the reference panel
        let back_btn = Button::new();
        back_btn.set_label("← Back to Preview");
        back_btn.add_css_class("flat");
        back_btn.set_hexpand(true);
        let back_bar = GtkBox::new(Orientation::Horizontal, 0);
        back_bar.set_margin_start(8);
        back_bar.set_margin_end(8);
        back_bar.set_margin_top(4);
        back_bar.set_margin_bottom(4);
        back_bar.append(&back_btn);

        let ref_panel = GtkBox::new(Orientation::Vertical, 0);
        ref_panel.set_hexpand(true);
        ref_panel.set_vexpand(true);
        ref_panel.append(&ref_notebook);
        ref_panel.append(&Separator::new(Orientation::Horizontal));
        ref_panel.append(&back_bar);

        // Stack: "preview" (live output) ↔ "reference" (cheatsheet/help)
        let preview_stack = Stack::new();
        preview_stack.set_hexpand(true);
        preview_stack.set_vexpand(true);
        preview_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        preview_stack.add_named(&preview_container, Some("preview"));
        preview_stack.add_named(&ref_panel, Some("reference"));

        // Wrapper box captured by visibility toggles (focus mode, sidebar toggle)
        let preview_outer = GtkBox::new(Orientation::Vertical, 0);
        preview_outer.set_hexpand(true);
        preview_outer.set_vexpand(true);
        preview_outer.append(&preview_stack);

        // Wire reference toggle button
        {
            let stack = preview_stack.clone();
            ref_toggle_btn.connect_toggled(move |btn| {
                if btn.is_active() {
                    stack.set_visible_child_name("reference");
                } else {
                    stack.set_visible_child_name("preview");
                }
            });
        }

        // Wire back button → deactivate toggle → returns to preview
        {
            let rtb = ref_toggle_btn.clone();
            back_btn.connect_clicked(move |_| {
                rtb.set_active(false);
            });
        }

        // ── main.typ heuristic banner ────────────────────────────────────────
        // Banner starts hidden; revealed only when the project toggle is ON.
        let root_banner: Rc<RefCell<Option<adw::Banner>>> = Rc::new(RefCell::new(None));
        {
            let main_path = project_root.join("main.typ");
            let no_root_configured = configured_root.borrow().is_none();
            // Once dismissed, the suggestion stays dismissed — it's advice about
            // a project shape the user has already declined.
            let dismissed = crate::config::ProjectConfig::load(&project_root)
                .map(|c| c.root_controls_dismissed)
                .unwrap_or(false);
            if no_root_configured && main_path.exists() && !dismissed {
                let banner = adw::Banner::new("main.typ detected — set it as root?");
                banner.set_button_label(Some("Set as Root"));
                banner.set_revealed(false); // revealed by project toggle
                let preview_for_banner = preview_pane.clone();
                let root_ref_banner = configured_root.clone();
                let root_dir_banner = project_root.clone();
                let title_w_banner = file_title_widget.clone();
                let ep_for_banner = editor_pane.clone();
                banner.connect_button_clicked(move |b| {
                    b.set_revealed(false);
                    preview_for_banner.set_root_file(main_path.clone());
                    *root_ref_banner.borrow_mut() = Some(main_path.clone());
                    if let Some(active) = ep_for_banner.get_active_path() {
                        if main_path != active {
                            let root_name = main_path.file_name().and_then(|n| n.to_str()).unwrap_or("root");
                            let active_name = active.file_name().and_then(|n| n.to_str()).unwrap_or("file");
                            title_w_banner.set_subtitle(&format!("{root_name} › {active_name}"));
                        }
                    }
                    let rel = main_path.strip_prefix(&root_dir_banner).unwrap_or(&main_path).to_path_buf();
                    let mut pcfg = crate::config::ProjectConfig::load(&root_dir_banner).unwrap_or_default();
                    pcfg.root_file = Some(rel);
                    let _ = pcfg.save(&root_dir_banner);
                    preview_for_banner.trigger_compile();
                });
                preview_outer.prepend(&banner);
                *root_banner.borrow_mut() = Some(banner);
            }
        }

        *preview_vis_holder.borrow_mut() = Some(preview_outer.clone());

        // ── Preview toggle button wiring (needs preview_outer) ───────────────
        {
            let preview_label_c = preview_label.clone();
            let preview_outer_c = preview_outer.clone();
            let preview_for_btn = preview_pane.clone();
            let editor_for_btn = editor_pane.clone();
            compile_btn.connect_clicked(move |_| {
                let now_visible = !preview_outer_c.is_visible();
                preview_outer_c.set_visible(now_visible);
                if now_visible {
                    preview_label_c.set_markup("<b>Preview</b>");
                    if let Some(path) = editor_for_btn.get_active_path() {
                        if let Some(content) = editor_for_btn.get_active_content() {
                            preview_for_btn.set_buffer_snapshot(path.clone(), content);
                        }
                        preview_for_btn.set_root_file(path);
                    }
                    preview_for_btn.trigger_compile();
                } else {
                    preview_label_c.set_text("Preview");
                }
            });
        }

        // Compile button wiring
        {
            let pv = preview_pane.clone();
            let editor_for_compile = editor_pane.clone();
            recompile_header_btn.connect_clicked(move |_| {
                if let Some(path) = editor_for_compile.get_active_path() {
                    if let Some(content) = editor_for_compile.get_active_content() {
                        pv.set_buffer_snapshot(path.clone(), content);
                    }
                    pv.set_root_file(path);
                }
                pv.trigger_compile();
            });
        }

        // Pop-out button wiring
        let preview_for_popout = preview_pane.clone();
        let popout_win_for_btn = popout_window.clone();
        let popout_pane_for_btn = popout_pane.clone();
        let window_for_popout = window.clone();
        let editor_for_popout_print = editor_pane.clone();
        let toast_for_popout_print = toast_overlay.clone();
        let panel_for_popout_print = error_panel.clone();
        let root_for_popout_print = project_root.clone();
        let config_for_popout_print = current_config.clone();
        popout_btn.connect_clicked(move |_| {
            // If already open, just raise it
            if let Some(win) = popout_win_for_btn.borrow().as_ref() {
                win.present();
                return;
            }
            // Create a new secondary preview pane reading from same output dir
            let secondary = PreviewPane::new(
                preview_for_popout.root_file_path(),
                Some(preview_for_popout.output_dir()),
                preview_for_popout.extra_args(),
            );
            secondary.refresh_display();

            let header_po = adw::HeaderBar::new();
            header_po.add_css_class("fond-chrome");

            let po_zoom_out = Button::from_icon_name("zoom-out-symbolic");
            po_zoom_out.add_css_class("flat");
            po_zoom_out.set_tooltip_text(Some("Zoom out"));
            let po_zoom_in = Button::from_icon_name("zoom-in-symbolic");
            po_zoom_in.add_css_class("flat");
            po_zoom_in.set_tooltip_text(Some("Zoom in"));
            let po_zoom_box = GtkBox::new(Orientation::Horizontal, 0);
            po_zoom_box.add_css_class("linked");
            po_zoom_box.append(&po_zoom_out);
            po_zoom_box.append(&po_zoom_in);
            let po_zoom_pct = (secondary.zoom() * 100.0).round() as u32;
            let po_zoom_label = Label::new(Some(&format!("{po_zoom_pct}%")));
            po_zoom_label.set_width_chars(5);
            po_zoom_label.set_xalign(0.5);
            po_zoom_label.add_css_class("caption");
            po_zoom_label.add_css_class("dim-label");

            let recompile_btn = Button::from_icon_name("media-playback-start-symbolic");
            recompile_btn.add_css_class("flat");
            recompile_btn.set_tooltip_text(Some("Recompile"));
            let sec_clone = secondary.clone();
            recompile_btn.connect_clicked(move |_| sec_clone.trigger_compile());

            let print_btn = Button::from_icon_name("printer-symbolic");
            print_btn.add_css_class("flat");
            print_btn.set_tooltip_text(Some("Print (Ctrl+P)"));
            // Prints via the *main* preview pane, not `secondary`: the popout is
            // constructed with only a root file and output dir, so it carries
            // neither the unsaved buffer contents nor the CV elements path.
            let print_pane = preview_for_popout.clone();
            let print_win = window_for_popout.clone();
            let print_editor = editor_for_popout_print.clone();
            let print_toast = toast_for_popout_print.clone();
            let print_panel = panel_for_popout_print.clone();
            let print_root = root_for_popout_print.clone();
            let print_config = config_for_popout_print.clone();
            print_btn.connect_clicked(move |_| {
                print_from_preview(
                    &print_win,
                    &print_editor,
                    &print_pane,
                    &print_toast,
                    &print_panel,
                    &print_root,
                    &print_config,
                );
            });

            let lbl_po_out = po_zoom_label.clone();
            let sec_po_out = secondary.clone();
            po_zoom_out.connect_clicked(move |_| {
                let new_z = (sec_po_out.zoom() - 0.25).max(0.25);
                sec_po_out.set_zoom(new_z);
                lbl_po_out.set_text(&format!("{}%", (new_z * 100.0).round() as u32));
            });
            let lbl_po_in = po_zoom_label.clone();
            let sec_po_in = secondary.clone();
            po_zoom_in.connect_clicked(move |_| {
                let new_z = (sec_po_in.zoom() + 0.25).min(4.0);
                sec_po_in.set_zoom(new_z);
                lbl_po_in.set_text(&format!("{}%", (new_z * 100.0).round() as u32));
            });

            header_po.pack_start(&po_zoom_box);
            header_po.pack_start(&po_zoom_label);
            header_po.pack_end(&recompile_btn);
            header_po.pack_end(&print_btn);

            let tv_po = adw::ToolbarView::new();
            tv_po.set_top_bar_style(adw::ToolbarStyle::RaisedBorder);
            tv_po.add_top_bar(&header_po);
            tv_po.set_content(Some(secondary.widget()));

            let win_po = adw::Window::new();
            win_po.set_title(Some("Preview — Zerkalo"));
            win_po.set_default_width(700);
            win_po.set_default_height(950);
            win_po.set_transient_for(Some(&window_for_popout));
            win_po.set_content(Some(&tv_po));

            let maximize_btn = Button::from_icon_name("window-maximize-symbolic");
            maximize_btn.add_css_class("flat");
            maximize_btn.set_tooltip_text(Some("Maximize window"));
            let win_for_max = win_po.clone();
            maximize_btn.connect_clicked(move |_| win_for_max.maximize());
            header_po.pack_end(&maximize_btn);

            let win_rc = popout_win_for_btn.clone();
            let pane_rc = popout_pane_for_btn.clone();
            win_po.connect_close_request(move |_| {
                *win_rc.borrow_mut() = None;
                *pane_rc.borrow_mut() = None;
                glib::Propagation::Proceed
            });

            win_po.present();
            *popout_win_for_btn.borrow_mut() = Some(win_po);
            *popout_pane_for_btn.borrow_mut() = Some(secondary);
        });

        let file_tree = wire_file_tree(&FileTreeCtx {
            window: window.clone(),
            editor_pane: editor_pane.clone(),
            preview_pane: preview_pane.clone(),
            toast_overlay: toast_overlay.clone(),
            project_root: project_root.clone(),
            library: library.clone(),
            file_title_widget: file_title_widget.clone(),
            title_extras: title_extras.clone(),
            file_tree_holder: file_tree_holder.clone(),
            configured_root: configured_root.clone(),
            proj_mode_active: proj_mode_active.clone(),
            root_banner: root_banner.clone(),
        });
        wire_editor_extras(&EditorExtrasCtx {
            window: window.clone(),
            editor_pane: editor_pane.clone(),
            file_tree: file_tree.clone(),
            toast_overlay: toast_overlay.clone(),
            current_config: current_config.clone(),
            project_root: project_root.clone(),
        });
        let (left_box, template_btn) = wire_sidebar_toolbar(&SidebarToolbarCtx {
            window: window.clone(),
            editor_pane: editor_pane.clone(),
            preview_pane: preview_pane.clone(),
            outline_panel: outline_panel.clone(),
            citation_panel: citation_panel.clone(),
            current_config: current_config.clone(),
            project_root: project_root.clone(),
            left_paned_holder: left_paned_holder.clone(),
            toast_overlay: toast_overlay.clone(),
        });
        // Restored only after the toggle callback above exists, so it applies
        // the CSS rather than just flipping a label.
        if config.gost_font {
            editor_pane.set_gost_enabled(true);
        }
        // Template is a document-level action, so it sits with the others in the
        // header rather than as the one non-panel row in the sidebar column.
        header.pack_start(&template_btn);

        let inner_paned = Paned::new(Orientation::Horizontal);
        inner_paned.set_position(config.preview_split);
        inner_paned.set_hexpand(true);
        inner_paned.set_vexpand(true);
        inner_paned.set_start_child(Some(editor_pane.widget()));
        inner_paned.set_end_child(Some(&preview_outer));

        // ── Global search panel (Ctrl+Shift+F) ───────────────────────────────
        let search_panel = super::search_panel::SearchPanel::new(project_root.clone());
        {
            let ep = editor_pane.clone();
            search_panel.set_on_result(move |path, line| {
                if !ep.state_has_file(&path) {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        ep.open_file(path.clone(), &content);
                    }
                }
                ep.jump_to_line(&path, line);
            });
        }
        {
            // Reload file in editor when replace_all modifies it
            let ep = editor_pane.clone();
            let toast_for_reload = toast_overlay.clone();
            search_panel.set_on_replace_done(move |path| {
                if ep.state_has_file(&path) {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        ep.reload_file(path, &content);
                        toast_for_reload.add_toast(
                            adw::Toast::new("File reloaded — undo history cleared")
                        );
                    }
                }
            });
        }
        {
            // Push searches to config history
            let cfg = current_config.clone();
            search_panel.set_on_search(move |query| {
                let mut c = cfg.borrow_mut();
                c.push_recent_search(query);
                let _ = c.save();
            });
        }
        // Seed recent searches from config
        search_panel.set_recent_searches(config.recent_searches.clone());

        // Search panel is hidden by default; Ctrl+Shift+F toggles it
        search_panel.widget().set_visible(false);

        let right_col = GtkBox::new(Orientation::Vertical, 0);
        right_col.set_hexpand(true);
        right_col.set_vexpand(true);
        right_col.append(&inner_paned);
        right_col.append(search_panel.widget());
        right_col.append(error_panel.widget());


        let content_paned = Paned::new(Orientation::Horizontal);
        content_paned.set_hexpand(true);
        content_paned.set_vexpand(true);
        content_paned.set_resize_start_child(true);
        content_paned.set_resize_end_child(false);
        content_paned.set_shrink_end_child(false);
        content_paned.set_start_child(Some(&right_col));

        let outer_paned = Paned::new(Orientation::Horizontal);
        outer_paned.set_position(config.sidebar_width);
        outer_paned.set_resize_start_child(false);
        outer_paned.set_resize_end_child(true);
        outer_paned.set_shrink_start_child(false);
        outer_paned.set_shrink_end_child(false);
        outer_paned.set_hexpand(true);
        outer_paned.set_vexpand(true);
        outer_paned.set_start_child(Some(&left_box));
        outer_paned.set_end_child(Some(&content_paned));

        wire_pane_persistence(&PanePersistCtx {
            current_config: current_config.clone(),
            outer_paned: outer_paned.clone(),
            inner_paned: inner_paned.clone(),
        });

        // The status bar spans the whole window, under the sidebar as well as
        // the editor — it reports on the document, not on one pane. (It used to
        // live inside the editor column, so it stopped at the sidebar edge.)
        let main_content = GtkBox::new(Orientation::Vertical, 0);
        main_content.set_hexpand(true);
        main_content.set_vexpand(true);
        main_content.append(&outer_paned);
        main_content.append(&Separator::new(Orientation::Horizontal));
        main_content.append(editor_pane.status_bar_widget());

        toast_overlay.set_child(Some(&main_content));

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.set_top_bar_style(adw::ToolbarStyle::RaisedBorder);
        toolbar_view.add_top_bar(&header);
        toolbar_view.add_bottom_bar(&compile_rev);
        toolbar_view.set_content(Some(&toast_overlay));

        // F1 labels everything on screen. Wrapping the toolbar view rather
        // than its content means the header bar's controls can be labelled
        // too — they're where most of the buttons are.
        let help_overlay = super::help_overlay::HelpOverlay::new(&toolbar_view);
        super::help_overlay::annotate_window(
            &help_overlay,
            &super::help_overlay::AnnotationTargets {
                sidebar_btn: &sidebar_btn,
                file_title_widget: &file_title_widget,
                style_btn: &style_btn,
                save_btn: &save_btn,
                sync_btn: &sync_btn,
                library_btn: &library_btn,
                preview_label: &preview_label,
                menu_btn: &menu_btn,
                compile_btn: &compile_btn,
                compile_mode_slot: &compile_mode_slot,
                outline: outline_panel.widget(),
                citations: citation_panel.widget(),
                editor: editor_pane.widget(),
                preview: preview_pane.widget(),
                status_bar: editor_pane.status_bar_widget(),
            },
        );
        window.set_content(Some(help_overlay.widget()));
        let file_watcher = wire_file_watcher(&WatcherCtx {
            editor_pane: editor_pane.clone(),
            preview_pane: preview_pane.clone(),
            project_root: project_root.clone(),
            library: library.clone(),
            library_window: library_window.clone(),
            manual_compile_only: manual_compile_only.clone(),
        });

        Self {
            window,
            editor_pane,
            preview_pane,
            error_panel,
            outline_panel,
            help_overlay,
            project_root,
            sync_btn,
            search_panel,
            toast_overlay,
            file_tree,
            writing_log,
            file_start_words,
            session_start,
            compile_on_save,
            manual_compile_only,
            file_watcher,
            compile_btn,
            library,
            library_window,
            menu_actions: PaletteTargets {
                new_file: menus.menu_new_item,
                open_file: menus.menu_open_item,
                export: menus.menu_export_item,
                settings: menus.menu_settings_item,
                template: menus.menu_new_template_item,
                save: menus.menu_save_item,
                sidebar: sidebar_btn,
            },
            menu_import_item: menus.menu_import_item,
            config: current_config,
        }
    }

    #[allow(dead_code)]
    pub fn show_toast(&self, msg: &str) {
        let toast = adw::Toast::new(msg);
        toast.set_timeout(3);
        self.toast_overlay.add_toast(toast);
    }

    pub fn setup_keybindings(&self) {
        Keybindings::write_default_if_missing();
        let kb = Keybindings::load();

        let editor = self.editor_pane.clone();
        let preview = self.preview_pane.clone();
        let window = self.window.clone();
        let sync = self.sync_btn.clone();
        let search = self.search_panel.clone();
        let file_tree = self.file_tree.clone();
        let kb_manual_only = self.manual_compile_only.clone();
        let snapshot_root = self.project_root.clone();
        let toast_for_key = self.toast_overlay.clone();
        let config_for_print_key = self.config.clone();
        let compile_btn_for_key = self.compile_btn.clone();
        let library_window_for_key = self.library_window.clone();
        let library_for_key = self.library.clone();
        let controller = gtk4::EventControllerKey::new();

        // ── Command palette (Ctrl+K) ────────────────────────────────────────
        let palette = Rc::new(CommandPalette::new(&self.window));
        {
            let editor_for_pal = self.editor_pane.clone();
            let window_for_pal = self.window.clone();
            let search_for_pal = self.search_panel.clone();
            let preview_for_pal = self.preview_pane.clone();
            let root_for_pal = self.project_root.clone();
            let toast_for_pal = self.toast_overlay.clone();
            let panel_for_pal = self.error_panel.clone();
            let config_for_pal = self.config.clone();
            let targets_for_pal = self.menu_actions.clone();
            let compile_btn_for_pal = self.compile_btn.clone();
            let sync_btn_for_pal = self.sync_btn.clone();
            let palette_for_outline = Rc::downgrade(&palette);
            palette.set_on_activate(move |id| {
                let w = window_for_pal.clone();
                if let Some(rest) = id.strip_prefix("heading:") {
                    if let Some(colon) = rest.find(':') {
                        let line_str = &rest[..colon];
                        let path_str = &rest[colon + 1..];
                        if let Ok(line) = line_str.parse::<u32>() {
                            let path = std::path::PathBuf::from(path_str);
                            editor_for_pal.jump_to_line(&path, line);
                        }
                    }
                } else if let Some(rest) = id.strip_prefix("file:") {
                    let path = std::path::PathBuf::from(rest);
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        editor_for_pal.open_file(path, &content);
                    }
                } else {
                    match id {
                        "toggle_find"    => editor_for_pal.toggle_find(),
                        // Forwarded to the menu row so this is the same save
                        // the ≡ menu and Ctrl+S do — snapshot and recompile
                        // included, which `save_all_modified` alone skips.
                        "save"           => targets_for_pal.save.emit_clicked(),
                        "new_file"       => targets_for_pal.new_file.emit_clicked(),
                        "open_file"      => targets_for_pal.open_file.emit_clicked(),
                        "export"         => targets_for_pal.export.emit_clicked(),
                        "settings"       => targets_for_pal.settings.emit_clicked(),
                        "template"       => targets_for_pal.template.emit_clicked(),
                        "toggle_sidebar" => targets_for_pal.sidebar.emit_clicked(),
                        "toggle_preview" => compile_btn_for_pal.emit_clicked(),
                        "git_sync"       => sync_btn_for_pal.emit_clicked(),
                        "focus_mode"     => editor_for_pal.focus_button_for_header().emit_clicked(),
                        "help"           => { HelpWindow::new(&w, editor_for_pal.is_cv_mode()).present(); }
                        "find_in_files"  => { search_for_pal.toggle(); }
                        "project_outline" => {
                            // Re-entering the palette from inside its own
                            // activate callback deadlocks on the items borrow,
                            // so defer the reopen to the next main-loop turn.
                            if let (Some(content), Some(path)) = (
                                editor_for_pal.get_active_content(),
                                editor_for_pal.get_active_path(),
                            ) {
                                let items = super::command_palette::heading_items(&content, &path);
                                if !items.is_empty() {
                                    let pal = palette_for_outline.clone();
                                    glib::idle_add_local_once(move || {
                                        if let Some(pal) = pal.upgrade() {
                                            pal.set_items(items);
                                            pal.show();
                                        }
                                    });
                                }
                            }
                        }
                        "print" => {
                            print_from_preview(
                                &w,
                                &editor_for_pal,
                                &preview_for_pal,
                                &toast_for_pal,
                                &panel_for_pal,
                                &root_for_pal,
                                &config_for_pal,
                            );
                        }
                        "toggle_profile" => {
                            let is_draft = preview_for_pal.is_draft_mode();
                            preview_for_pal.set_draft_mode(!is_draft);
                        }
                        "browse_snapshots" => {
                            if let Some(path) = editor_for_pal.get_active_path() {
                                let content = editor_for_pal.get_active_content().unwrap_or_default();
                                let dialog = super::snapshot_dialog::SnapshotDialog::new(
                                    &w, &root_for_pal, &path, &content,
                                );
                                let ep = editor_for_pal.clone();
                                let pp = path.clone();
                                let win_for_restore = w.clone();
                                dialog.set_on_restore(move |text| {
                                    restore_snapshot_with_confirm(&win_for_restore, &ep, &pp, text);
                                });
                                dialog.present();
                            }
                        }
                        "browse_history" => {
                            if let Some(path) = editor_for_pal.get_active_path() {
                                show_file_history_window(&w, &root_for_pal, &path);
                            }
                        }
                        _ => {}
                    }
                }
            });
        }
        {
            let editor_for_pal_close = editor.clone();
            palette.set_on_close(move || {
                editor_for_pal_close.grab_focus();
            });
        }
        let palette_for_key = palette.clone();
        let editor_for_palette_key = editor.clone();
        let error_panel_for_key = self.error_panel.clone();
        let menu_import_item_for_key = self.menu_import_item.clone();
        let settings_item_for_key = self.menu_actions.settings.clone();
        let config_for_experimental = self.config.clone();
        let window_for_paste_key = self.window.clone();
        let editor_for_paste_key = self.editor_pane.clone();
        let work_dir_for_paste_key = self.project_root.clone();
        let toast_overlay_for_paste_key = self.toast_overlay.clone();
        let help_overlay_for_key = self.help_overlay.clone();

        controller.connect_key_pressed(move |_, key, _, modifier| {
            use gtk4::gdk::ModifierType;
            let ctrl = modifier.contains(ModifierType::CONTROL_MASK);
            let shift = modifier.contains(ModifierType::SHIFT_MASK);
            let alt = modifier.contains(ModifierType::ALT_MASK);

            // Labels everything on screen; Escape takes the labels away.
            // Checked before anything else so the overlay can always be shut,
            // whatever else Escape might mean to the widget with focus.
            if matches_binding(&kb.help_overlay, ctrl, shift, alt, key) {
                help_overlay_for_key.toggle();
                return glib::Propagation::Stop;
            }
            if key == gtk4::gdk::Key::Escape && help_overlay_for_key.is_shown() {
                help_overlay_for_key.hide();
                return glib::Propagation::Stop;
            }

            if matches_binding(&kb.save, ctrl, shift, alt, key) {
                if let Some(path) = editor.get_active_path() {
                    if let Some(content) = editor.get_active_content() {
                        if std::fs::write(&path, &content).is_ok() {
                            editor.mark_saved(&path);
                            crate::auto_save::clear(&path);
                            library_for_key.borrow_mut().touch_saved(&path).ok();
                            save_snapshot(&snapshot_root, &path, &content);
                            if !*kb_manual_only.borrow() {
                                preview.set_buffer_snapshot(path.clone(), content);
                                preview.set_root_file(path);
                                preview.trigger_compile();
                            }
                        }
                    }
                }
                return glib::Propagation::Stop;
            }
            if matches_binding(&kb.compile, ctrl, shift, alt, key) {
                compile_btn_for_key.emit_clicked();
                return glib::Propagation::Stop;
            }
            // Ctrl+L — toggle the document library
            {
                use gtk4::gdk::Key;
                if ctrl && !shift && !alt && key == Key::l {
                    library_window_for_key.toggle();
                    return glib::Propagation::Stop;
                }
            }
            // Ctrl+E — focus first error row (shows the panel if hidden)
            {
                use gtk4::gdk::Key;
                if ctrl && !shift && !alt && key == Key::e {
                    error_panel_for_key.widget().set_visible(true);
                    if error_panel_for_key.grab_first_focus() {
                        return glib::Propagation::Stop;
                    }
                }
            }
            // Ctrl+, — Settings, the desktop-wide convention.
            {
                use gtk4::gdk::Key;
                if ctrl && !shift && !alt && key == Key::comma {
                    settings_item_for_key.emit_clicked();
                    return glib::Propagation::Stop;
                }
            }
            // Ctrl+Shift+I — open the Import picker. Import is experimental and
            // its menu row is hidden unless developer_mode is on; the shortcut
            // has to honour the same gate or the row hides nothing.
            {
                use gtk4::gdk::Key;
                if ctrl && shift && !alt && key == Key::i {
                    if config_for_experimental.borrow().developer_mode {
                        menu_import_item_for_key.emit_clicked();
                    }
                    return glib::Propagation::Stop;
                }
            }
            // Ctrl+Shift+V — Paste as Document, which lives inside that same
            // experimental Import dialog, so it's gated with it.
            {
                use gtk4::gdk::Key;
                if ctrl && shift && !alt && key == Key::v {
                    let cfg = crate::config::shared();
                    if cfg.borrow().developer_mode {
                        paste_as_document(
                            &window_for_paste_key, &editor_for_paste_key, &work_dir_for_paste_key,
                            &cfg, &toast_overlay_for_paste_key,
                        );
                    }
                    return glib::Propagation::Stop;
                }
            }
            if matches_binding(&kb.find, ctrl, shift, alt, key) {
                editor.toggle_find();
                return glib::Propagation::Stop;
            }
            if matches_binding(&kb.quit, ctrl, shift, alt, key) {
                window.close();
                return glib::Propagation::Stop;
            }
            if matches_binding(&kb.next_tab, ctrl, shift, alt, key) {
                editor.next_tab();
                return glib::Propagation::Stop;
            }
            if matches_binding(&kb.prev_tab, ctrl, shift, alt, key) {
                editor.prev_tab();
                return glib::Propagation::Stop;
            }
            if matches_binding(&kb.git_sync, ctrl, shift, alt, key) {
                sync.emit_clicked();
                return glib::Propagation::Stop;
            }
            // Ctrl+Shift+F — global project search
            {
                use gtk4::gdk::Key;
                if ctrl && shift && key == Key::f {
                    search.toggle();
                    return glib::Propagation::Stop;
                }
                // F6 — cycle pane focus: file tree → editor → (repeat)
                if !ctrl && !alt && key == Key::F6 {
                    if shift {
                        editor.grab_focus();
                    } else {
                        file_tree.grab_focus();
                    }
                    return glib::Propagation::Stop;
                }
            }
            // Ctrl+Shift+Tab also maps to ISO_Left_Tab on X11
            {
                use gtk4::gdk::Key;
                if ctrl && (key == Key::ISO_Left_Tab) {
                    editor.prev_tab();
                    return glib::Propagation::Stop;
                }
                // Ctrl+? / Ctrl+Shift+/ — keyboard shortcut help overlay
                if ctrl && (key == Key::question || (shift && key == Key::slash)) {
                    HelpWindow::new(&window, editor.is_cv_mode()).present();
                    return glib::Propagation::Stop;
                }
                // Command palette (default Ctrl+K, configurable via keybindings.toml)
                if matches_binding(&kb.command_palette, ctrl, shift, alt, key) {
                    let mut items = default_commands();
                    if let Some(content) = editor_for_palette_key.get_active_content() {
                        if let Some(path) = editor_for_palette_key.get_active_path() {
                            items.extend(heading_items(&content, &path));
                        }
                    }
                    palette_for_key.set_items(items);
                    palette_for_key.show();
                    return glib::Propagation::Stop;
                }
                // Ctrl+Shift+H (configurable) — dynamic keyboard shortcuts window
                if matches_binding(&kb.shortcuts_help, ctrl, shift, alt, key) {
                    show_dynamic_shortcuts_window(&window, &kb);
                    return glib::Propagation::Stop;
                }
                // Ctrl+G — go to heading
                if ctrl && !shift && key == Key::g {
                    if let Some(content) = editor_for_palette_key.get_active_content() {
                        if let Some(path) = editor_for_palette_key.get_active_path() {
                            let items = heading_items(&content, &path);
                            if !items.is_empty() {
                                palette_for_key.set_items(items);
                                palette_for_key.show();
                                return glib::Propagation::Stop;
                            }
                        }
                    }
                }
            }

            // Ctrl+P — open the print sheet
            {
                use gtk4::gdk::Key;
                if ctrl && !shift && !alt && key == Key::p {
                    print_from_preview(
                        &window,
                        &editor,
                        &preview,
                        &toast_for_key,
                        &error_panel_for_key,
                        &snapshot_root,
                        &config_for_print_key,
                    );
                    return glib::Propagation::Stop;
                }
            }

            // Ctrl+Shift+E — export PDF to document directory (no dialog)
            {
                use gtk4::gdk::Key;
                if ctrl && shift && key == Key::e {
                    editor.save_all_modified();
                    // Same inputs the preview compiles with — assembling them
                    // by hand here is what left CV documents exporting blank.
                    if let Some((root_path, overrides, sys_inputs)) = preview.compile_inputs() {
                        let dest = root_path.with_extension("pdf");
                        let t = adw::Toast::new("Exporting PDF…");
                        t.set_timeout(2);
                        toast_for_key.add_toast(t);
                        let (tx, rx) = std::sync::mpsc::sync_channel::<Result<Vec<u8>, String>>(1);
                        let root_for_thread = root_path.clone();
                        std::thread::spawn(move || {
                            let result = crate::compiler::compile_to_pdf_bytes(
                                &root_for_thread,
                                &overrides,
                                &sys_inputs,
                            ).map_err(|e| e.to_string());
                            let _ = tx.send(result);
                        });
                        let toast_ref = toast_for_key.clone();
                        glib::timeout_add_local(Duration::from_millis(100), move || {
                            use std::sync::mpsc::TryRecvError;
                            match rx.try_recv() {
                                Ok(Ok(bytes)) => {
                                    let msg = match std::fs::write(&dest, &bytes) {
                                        Ok(_) => format!(
                                            "Exported {}",
                                            dest.file_name().and_then(|n| n.to_str()).unwrap_or("PDF")
                                        ),
                                        Err(e) => format!("Write failed: {e}"),
                                    };
                                    let t = adw::Toast::new(&msg);
                                    t.set_timeout(4);
                                    toast_ref.add_toast(t);
                                    glib::ControlFlow::Break
                                }
                                Ok(Err(e)) => {
                                    let t = adw::Toast::new(&format!("Export failed: {e}"));
                                    t.set_timeout(4);
                                    toast_ref.add_toast(t);
                                    glib::ControlFlow::Break
                                }
                                Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
                                Err(_) => glib::ControlFlow::Break,
                            }
                        });
                    }
                    return glib::Propagation::Stop;
                }
            }

            glib::Propagation::Proceed
        });

        self.window.add_controller(controller);
    }

    pub fn open_initial_file(&self, initial: Option<PathBuf>) {
        if let Some(path) = initial {
            // Explicit file argument: open it, ignore session
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => {
                    let default = "// Welcome to Zerkalo\n\n= Introduction\n\nStart writing here...\n";
                    let _ = std::fs::write(&path, default);
                    default.to_string()
                }
            };
            self.editor_pane.open_file(path, &content);
            return;
        }

        let session = Session::load();

        // Only restore files that belong to the current project root — prevents
        // old-project files leaking in when the work_dir has changed.
        let session_files: Vec<&PathBuf> = session.open_files.iter()
            .filter(|p| p.starts_with(&self.project_root))
            .collect();

        if !session_files.is_empty() {
            for path in &session_files {
                if let Ok(content) = std::fs::read_to_string(path) {
                    self.editor_pane.open_file((*path).clone(), &content);
                }
            }
            // Switch to the previously active file
            if let Some(ref active) = session.active_file {
                self.editor_pane.switch_to_file(active);
            }
            // After the event loop starts, pin the preview to whichever file the
            // editor is actually showing.  This runs last, after all the
            // open_file / switch_to_file on_page_switch compiles, so it wins.
            let ep = self.editor_pane.clone();
            let pv = self.preview_pane.clone();
            let positions = session.cursor_positions.clone();
            glib::idle_add_local_once(move || {
                if let Some(path) = ep.get_active_path() {
                    if let Some(content) = ep.get_active_content() {
                        pv.set_buffer_snapshot(path.clone(), content);
                    }
                    pv.set_root_file(path);
                    pv.trigger_compile();
                }
                for (path, offset) in &positions {
                    ep.restore_cursor(path, *offset);
                }
            });
        } else {
            // No session: open or create main.typ
            let path = self.project_root.join("main.typ");
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => {
                    let default = "// Welcome to Zerkalo\n\n= Introduction\n\nStart writing here...\n";
                    let _ = std::fs::write(&path, default);
                    default.to_string()
                }
            };
            self.editor_pane.open_file(path, &content);
            // Pin preview to the active editor file after the event loop starts.
            let ep = self.editor_pane.clone();
            let pv = self.preview_pane.clone();
            glib::idle_add_local_once(move || {
                if let Some(path) = ep.get_active_path() {
                    if let Some(content) = ep.get_active_content() {
                        pv.set_buffer_snapshot(path.clone(), content);
                    }
                    pv.set_root_file(path);
                }
                pv.trigger_compile();
            });
        }
    }

    /// Open one or more external file paths (called by the GApplication::open handler
    /// when Nautilus or another file manager activates an already-running instance).
    pub fn open_external(&self, paths: &[PathBuf]) {
        for path in paths {
            if let Ok(content) = std::fs::read_to_string(path) {
                self.editor_pane.open_file(path.clone(), &content);
            }
        }
        self.window.present();
    }

    pub fn present(&self) {
        let ep = self.editor_pane.clone();
        let win = self.window.clone();
        let force_close: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let git_synced: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        let writing_log_for_close = self.writing_log.clone();
        let file_start_words_for_close = self.file_start_words.clone();
        let session_start_for_close = self.session_start.clone();
        let project_root_for_close = self.project_root.clone();

        self.window.connect_close_request(move |_| {
            // Stage 1: an unsaved-buffer dialog resolves into a second call
            // with force_close set — skip straight past it here.
            if !*force_close.borrow() {
                let unsaved = ep.modified_buffers();
                if unsaved.is_empty() {
                    *force_close.borrow_mut() = true;
                } else {
                    // Build file list for the dialog body
                    let names: Vec<String> = unsaved
                        .iter()
                        .map(|(p, _)| {
                            p.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("?")
                                .to_string()
                        })
                        .collect();
                    let body = format!(
                        "The following file{} {} unsaved changes:\n\n{}",
                        if names.len() == 1 { "" } else { "s" },
                        if names.len() == 1 { "has" } else { "have" },
                        names.join("\n"),
                    );

                    let dlg = adw::MessageDialog::new(
                        Some(&win),
                        Some("Save before closing?"),
                        Some(&body),
                    );
                    dlg.add_response("cancel", "Cancel");
                    dlg.add_response("discard", "Discard");
                    dlg.add_response("save", "Save All");
                    dlg.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
                    dlg.set_response_appearance("save", adw::ResponseAppearance::Suggested);
                    dlg.set_default_response(Some("save"));
                    dlg.set_close_response("cancel");

                    let ep2 = ep.clone();
                    let win2 = win.clone();
                    let fc = force_close.clone();
                    dlg.connect_response(None, move |_, resp| {
                        match resp {
                            "save" => {
                                ep2.save_all_modified();
                                *fc.borrow_mut() = true;
                                win2.close();
                            }
                            "discard" => {
                                *fc.borrow_mut() = true;
                                win2.close();
                            }
                            _ => {} // cancel — do nothing
                        }
                    });
                    dlg.present();

                    return glib::Propagation::Stop;
                }
            }

            // Stage 2: back up before closing, if a backup location is
            // configured and there's actually something to send — silent
            // and best-effort, capped so an offline connection can't hang
            // the app on quit.
            if !*git_synced.borrow() {
                *git_synced.borrow_mut() = true;
                let root = crate::git_sync::git_repo_root(&project_root_for_close)
                    .unwrap_or_else(|| project_root_for_close.clone());
                if crate::git_sync::has_remote(&root)
                    && !crate::git_sync::changed_files(&root).is_empty()
                {
                    let closed = Rc::new(RefCell::new(false));
                    let win_for_sync = win.clone();
                    let closed_a = closed.clone();
                    sync::auto_sync_quiet(
                        root,
                        None,
                        crate::secret_store::load_github_token(),
                        move || {
                            if !*closed_a.borrow() {
                                *closed_a.borrow_mut() = true;
                                win_for_sync.close();
                            }
                        },
                    );
                    let win_for_timeout = win.clone();
                    let closed_b = closed.clone();
                    glib::timeout_add_local_once(Duration::from_secs(6), move || {
                        if !*closed_b.borrow() {
                            *closed_b.borrow_mut() = true;
                            win_for_timeout.close();
                        }
                    });
                    return glib::Propagation::Stop;
                }
            }

            // Stage 3: finalize.
            record_writing_session(
                &ep, &writing_log_for_close,
                &file_start_words_for_close, &session_start_for_close,
            );
            let open_files = ep.get_open_paths_ordered();
            let active_file = ep.get_active_path();
            let cursor_positions = ep.get_cursor_positions();
            Session { open_files, active_file, cursor_positions }.save();
            glib::Propagation::Proceed
        });

        self.window.present();

        // A window only reachable by clicking cannot be captured headlessly:
        // on an Xvfb display with no window manager, neither synthetic clicks
        // nor key presses activate anything (verified — the pointer hovers and
        // nothing else lands). This opens the library at startup so a
        // screenshot script can see it. Debug builds only; it cannot fire for
        // a user.
        #[cfg(debug_assertions)]
        if std::env::var_os("ZERKALO_OPEN_LIBRARY").is_some() {
            // The library DB is loaded on a worker and swapped in later, so
            // opening now would refresh against the empty placeholder.
            let lw = self.library_window.clone();
            glib::timeout_add_local_once(Duration::from_secs(3), move || lw.toggle());
        }
    }
}

fn compile_mode_label_str(auto: bool, _cos: bool, mco: bool) -> &'static str {
    // Anything that isn't manual or auto compiles on save, whether or not the
    // compile_on_save flag is explicitly set.
    if mco { "manual" } else if auto { "auto" } else { "on save" }
}

fn apply_compile_mode_css(btn: &Button, auto: bool, _cos: bool, mco: bool) {
    if mco {
        btn.add_css_class("compile-mode-manual");
        btn.remove_css_class("compile-mode-auto");
    } else if auto {
        btn.add_css_class("compile-mode-auto");
        btn.remove_css_class("compile-mode-manual");
    } else {
        btn.remove_css_class("compile-mode-manual");
        btn.remove_css_class("compile-mode-auto");
    }
}

// ── Writing session recorder ──────────────────────────────────────────────────

fn record_writing_session(
    ep: &super::editor_pane::EditorPane,
    writing_log: &Rc<RefCell<WritingLog>>,
    file_start_words: &crate::writing_log::FileStartWords,
    session_start: &Rc<RefCell<std::time::Instant>>,
) {
    if let (Some(path), Some(content)) = (ep.get_active_path(), ep.get_active_content()) {
        let current_words = count_words(&content);
        let start_words = file_start_words.borrow().get(&path).copied().unwrap_or(current_words);
        let words_added = current_words - start_words;
        let duration_secs = session_start.borrow().elapsed().as_secs();
        writing_log.borrow_mut().record(path, words_added, duration_secs);
    }
}

// ── Theme helper ──────────────────────────────────────────────────────────────

fn apply_theme(theme: &Theme) {
    let manager = adw::StyleManager::default();
    let scheme = match theme {
        Theme::System => adw::ColorScheme::Default,
        Theme::Light => adw::ColorScheme::ForceLight,
        Theme::Dark => adw::ColorScheme::ForceDark,
    };
    manager.set_color_scheme(scheme);
}

// ── Sync helpers ──────────────────────────────────────────────────────────────

fn show_alert(window: &adw::ApplicationWindow, title: &str, body: &str) {
    super::confirm::notice(Some(window.upcast_ref()), title, body);
}

fn show_dynamic_shortcuts_window(
    window: &adw::ApplicationWindow,
    kb: &crate::keybindings::Keybindings,
) {
    use gtk4::prelude::*;
    let body = format!(
        "Editing\n\
         \u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\n\
         Save                {save}\n\
         Find & Replace      {find}\n\
         Next tab            {next_tab}\n\
         Previous tab        {prev_tab}\n\
         Add reference       {add_ref}\n\n\
         Navigation\n\
         \u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\n\
         Command Palette     {palette}\n\
         Go to heading       Ctrl+G\n\
         Find in Files       Ctrl+Shift+F\n\n\
         Compile & Preview\n\
         \u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\n\
         Compile             {compile}\n\
         Export PDF          Ctrl+Shift+E\n\n\
         Backup & App\n\
         \u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\n\
         Save a version      {git_sync}\n\
         What things do      {help_overlay}\n\
         Keyboard Shortcuts  {shortcuts_help}\n\
         Quit                {quit}\n\n\
         Keybindings file: ~/.config/zerkalo/keybindings.toml",
        save = kb.save,
        find = kb.find,
        next_tab = kb.next_tab,
        prev_tab = kb.prev_tab,
        add_ref = kb.add_reference,
        palette = kb.command_palette,
        compile = kb.compile,
        git_sync = kb.git_sync,
        shortcuts_help = kb.shortcuts_help,
        help_overlay = kb.help_overlay,
        quit = kb.quit,
    );
    let dlg = adw::MessageDialog::new(
        Some(window),
        Some("Keyboard Shortcuts"),
        Some(&body),
    );
    dlg.add_response("ok", "OK");
    dlg.present();
}

fn handle_preview_click_jump(
    preview: &super::preview_pane::PreviewPane,
    editor: &super::editor_pane::EditorPane,
    window: &adw::ApplicationWindow,
    page: usize,
    rel_y: f64,
) {
    match super::preview_pane::extract_page_text_via_pdftotext(preview, page, 0.0, 1.0) {
        Some(text) => {
            let lines: Vec<&str> = text.lines().collect();
            if lines.is_empty() { return; }
            let target = ((lines.len() as f64 * rel_y) as usize).min(lines.len().saturating_sub(1));
            // Search a ±3 line window for a non-trivial snippet
            let start = target.saturating_sub(3);
            let end = (target + 3).min(lines.len().saturating_sub(1));
            let snippet = (start..=end)
                .filter_map(|i| {
                    let l = lines[i].trim();
                    if l.len() >= 6 { Some(l) } else { None }
                })
                .next()
                .unwrap_or("");
            if snippet.len() >= 6 {
                let phrase: String = snippet.chars().take(40).collect();
                editor.jump_to_text(&phrase);
            }
        }
        None => {
            show_alert(window, "Click-to-Jump",
                "Could not extract text from the preview. Make sure pdftotext \
                 (poppler-utils) is installed and the document has been compiled at least once.\
                 \n\n  apt install poppler-utils\
                 \n  dnf install poppler-utils\
                 \n  zypper install poppler-tools");
        }
    }
}

fn handle_preview_word_jump(
    preview: &super::preview_pane::PreviewPane,
    editor: &super::editor_pane::EditorPane,
    window: &adw::ApplicationWindow,
    page: usize,
    rel_x: f64,
    rel_y: f64,
) {
    match super::preview_pane::extract_word_at_position(preview, page, rel_x, rel_y) {
        Some(phrase) if !phrase.trim().is_empty() => {
            editor.jump_to_text(&phrase);
        }
        Some(_) => {}
        None => {
            show_alert(window, "Jump to Word",
                "Could not extract text from the preview. Make sure pdftotext \
                 (poppler-utils) is installed and the document has been compiled at least once.\
                 \n\n  apt install poppler-utils\
                 \n  dnf install poppler-utils\
                 \n  zypper install poppler-tools");
        }
    }
}

fn format_file_mtime(mtime: std::time::SystemTime) -> String {
    let Ok(dur) = std::time::SystemTime::now().duration_since(mtime) else {
        return "unknown".to_string();
    };
    let secs = dur.as_secs();
    if secs < 60 { "just now".to_string() }
    else if secs < 3600 { format!("{} min ago", secs / 60) }
    else if secs < 86400 { format!("{} h ago", secs / 3600) }
    else if secs < 86400 * 30 { format!("{} days ago", secs / 86400) }
    else { format!("{} months ago", secs / (86400 * 30)) }
}

/// Compute a path string for `#include`/`#import` relative to the compilation root's directory.
/// Falls back to the filename if no root is set or paths don't share a prefix.
fn compute_include_path(preview: &super::preview_pane::PreviewPane, abs_path: &std::path::Path) -> String {
    if let Some(root) = preview.root_file_path() {
        if let Some(root_dir) = root.parent() {
            if let Ok(rel) = abs_path.strip_prefix(root_dir) {
                return rel.to_string_lossy().replace('\\', "/");
            }
        }
    }
    abs_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file.typ")
        .to_string()
}

fn extract_doc_title(content: &str) -> Option<String> {
    // 1. TOML/YAML front-matter: ---\ntitle = "..." or title: ...\n---
    if let Some(rest) = content.strip_prefix("---\n") {
        let end = rest.find("\n---\n").or_else(|| rest.find("\n---"));
        if let Some(end) = end {
            for line in rest[..end].lines() {
                if let Some(val) = line.strip_prefix("title = ").or_else(|| line.strip_prefix("title: ")) {
                    let title = val.trim().trim_matches('"').to_string();
                    if !title.is_empty() {
                        return Some(title);
                    }
                }
            }
        }
    }
    // 2. Zerkalo template variable: #let doc-title = "..."
    for line in content.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("#let doc-title = ") {
            let title = rest.trim().trim_matches('"').to_string();
            if !title.is_empty() && title != "Untitled" {
                return Some(title);
            }
        }
    }
    // 3. #set document(title: "...")
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("#set document(") {
            if let Some(pos) = t.find("title:") {
                let after = t[pos + "title:".len()..].trim();
                if let Some(inner) = after.strip_prefix('"') {
                    if let Some(end) = inner.find('"') {
                        let title = inner[..end].to_string();
                        if !title.is_empty() {
                            return Some(title);
                        }
                    }
                }
            }
        }
    }
    // 4. First = Heading
    for line in content.lines() {
        if let Some(h) = line.strip_prefix("= ") {
            let title = h.trim().to_string();
            if !title.is_empty() {
                return Some(title);
            }
        }
    }
    None
}

/// Strip pandoc's generated `#set` preamble from a standalone Typst output so we can
/// replace it with a Zerkalo template section.
#[cfg_attr(not(test), allow(dead_code))]
fn load_app_css() {
    crate::ui::styles::load_global_css();
    crate::ui::styles::pin_icon_theme();

    // If GNOME "Reduce Animations" is enabled, strip transitions so vestibular
    // disorder users aren't affected by the error revealer slide and sidebar fade.
    let animations_enabled = gtk4::Settings::default()
        .map(|s| s.is_gtk_enable_animations())
        .unwrap_or(true);
    if !animations_enabled {
        let reduced = gtk4::CssProvider::new();
        reduced.load_from_data(
            "* { transition: none !important; animation: none !important; } \
             revealer > * { transition: none !important; }",
        );
        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &reduced,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
            );
        }
    }
}

struct HamburgerItems {
    menu_new_template_item: Button,
    menu_reapply_template_item: Button,
    menu_repair_markers_item: Button,
    menu_new_item: Button,
    menu_open_item: Button,
    menu_save_item: Button,
    menu_save_as_item: Button,
    menu_snapshots_item: Button,
    menu_history_item: Button,
    menu_export_item: Button,
    menu_export_web_item: Button,
    menu_print_item: Button,
    menu_import_item: Button,
    menu_docs_item: Button,
    menu_fonts_item: Button,
    menu_settings_item: Button,
    menu_setup_item: Button,
    menu_tools_item: Button,
    menu_backup_remote_item: Button,
    menu_help_item: Button,
    menu_shortcuts_item: Button,
    menu_writing_stats_item: Button,
    menu_about_item: Button,
    menu_whats_new_item: Button,
    menu_import_pdf_item: Button,
}

fn build_hamburger_menu_items() -> HamburgerItems {
    // Shortcut labels come from the user's bindings rather than string
    // literals, so rebinding in keybindings.toml relabels the menu too.
    let kb = crate::keybindings::Keybindings::load();
    let d = crate::keybindings::display_binding;

    HamburgerItems {
        menu_new_template_item:    make_menu_item("New from Template…",         None),
        menu_reapply_template_item: make_menu_item("Update Template Settings…", None),
        menu_repair_markers_item:  make_menu_item("Repair Template Markers…",   None),
        menu_new_item:             make_menu_item("New Blank Document…",         None),
        menu_open_item:            make_menu_item("Open File…",                  None),
        menu_save_item:            make_menu_item("Save",                      Some(&d(&kb.save))),
        menu_save_as_item:         make_menu_item("Save As…",                    None),
        menu_snapshots_item:       make_menu_item("Browse Snapshots…",           None),
        menu_history_item:         make_menu_item("File History…",               None),
        menu_export_item:          make_menu_item("Export…",                     None),
        menu_export_web_item:      make_menu_item("Export for Web…",             None),
        // Print and Import aren't in keybindings.toml — they're fixed in the
        // key handler, so a literal is the honest label here.
        menu_print_item:           make_menu_item("Print\u{2026}",               Some("Ctrl+P")),
        menu_import_item:          make_menu_item("Import…",                     Some("Ctrl+Shift+I")),
        menu_docs_item:            make_menu_item("Browse Documents…",           None),
        menu_fonts_item:           make_menu_item("Document Fonts…",             None),
        menu_settings_item:        make_menu_item("Settings",                    None),
        menu_setup_item:           make_menu_item("Set Up Zerkalo…",             None),
        menu_tools_item:           make_menu_item("Tools…",                      None),
        menu_backup_remote_item:   make_menu_item("Backup Locations…",            None),
        menu_help_item:            make_menu_item("Help",                      Some("Ctrl+?")),
        // The keybinding-aware shortcuts window was reachable only by its
        // shortcut; nothing in the menu opened it.
        menu_shortcuts_item:       make_menu_item("Keyboard Shortcuts", Some(&d(&kb.shortcuts_help))),
        menu_writing_stats_item:   make_menu_item("Writing Stats",               None),
        menu_about_item:           make_menu_item("About Zerkalo",               None),
        menu_whats_new_item:       make_menu_item("What's New",                  None),
        menu_import_pdf_item:      make_menu_item("Import PDF File…",            None),
    }
}

/// Build a hamburger-menu row: label flush-left, optional shortcut dim-right.
fn make_menu_item(label: &str, shortcut: Option<&str>) -> Button {
    let btn = Button::new();
    btn.add_css_class("flat");

    let row = GtkBox::new(Orientation::Horizontal, 0);
    row.set_margin_start(4);
    row.set_margin_end(6);

    let name_lbl = Label::new(Some(label));
    name_lbl.set_halign(Align::Start);
    name_lbl.set_hexpand(true);
    row.append(&name_lbl);

    if let Some(sc) = shortcut {
        let sc_lbl = Label::new(Some(sc));
        sc_lbl.set_halign(Align::End);
        sc_lbl.add_css_class("dim-label");
        sc_lbl.add_css_class("caption");
        sc_lbl.set_margin_start(16);
        row.append(&sc_lbl);
    }

    btn.set_child(Some(&row));
    btn
}

/// Restores snapshot `text` into `path`'s editor buffer, confirming first if
/// the buffer has unsaved edits that would otherwise be silently discarded.
fn restore_snapshot_with_confirm(
    window: &adw::ApplicationWindow,
    ep: &super::editor_pane::EditorPane,
    path: &std::path::Path,
    text: String,
) {
    if !ep.is_modified(path) {
        ep.set_content(path, &text);
        return;
    }
    let ep = ep.clone();
    let path = path.to_path_buf();
    super::confirm::confirm_destructive(
        Some(window.upcast_ref()),
        "Restore this snapshot?",
        "You have unsaved changes in this document. Restoring the snapshot will discard them.",
        "Restore",
        move || ep.set_content(&path, &text),
    );
}

/// Opens a small window showing `path`'s git commit history and diffs, for
/// both the hamburger's "File History…" row and the Ctrl+K palette.
pub(super) fn show_file_history_window(
    parent: &adw::ApplicationWindow,
    project_root: &Path,
    path: &Path,
) {
    let history_window = adw::Window::builder()
        .title("File History")
        .transient_for(parent)
        .modal(true)
        .default_width(760)
        .default_height(560)
        .build();
    let header = adw::HeaderBar::new();
    header.add_css_class("fond-chrome");
    let close_btn = Button::with_label("Close");
    header.pack_end(&close_btn);
    let content_box = GtkBox::new(Orientation::Vertical, 0);
    content_box.append(&header);

    let panel = super::history_panel::HistoryPanel::new(project_root.to_path_buf());
    panel.load_file_history(path);
    content_box.append(panel.widget());
    history_window.set_content(Some(&content_box));

    let win_close = history_window.clone();
    close_btn.connect_clicked(move |_| win_close.close());

    history_window.present();
}

/// Opens the template dialog preloaded from the active document, for both the
/// hamburger's "Update Template Settings…" and the header's "Template" button.
///
/// These were two ~110-line copies of the same preselection sequence, and had
/// already drifted: the header copy hardcoded the advanced-expander state,
/// never passed the bib path, dropped the locked author/affiliation entirely,
/// and silently did nothing with no document open.
pub(super) fn open_template_for_active_document(
    window: &adw::ApplicationWindow,
    editor: &super::editor_pane::EditorPane,
    preview: &super::preview_pane::PreviewPane,
    toast_overlay: &adw::ToastOverlay,
    project_root: &Path,
    config: &Rc<RefCell<Config>>,
) {
    use super::template_dialog as td;

    let Some(current_path) = editor.get_active_path() else {
        show_alert(
            window,
            "No document open",
            "Open a .typ file first, then use Update Template Settings.",
        );
        return;
    };
    let current_content = editor.get_active_content().unwrap_or_default();

    let last_advanced = config.borrow().last_used_advanced;
    let dlg = td::TemplateDialog::new(window, project_root, last_advanced);

    {
        let cfg = config.borrow();
        dlg.set_bib_path(cfg.bib_path.clone());
        dlg.preselect_locked_identity(&cfg.locked_author.clone(), &cfg.locked_affiliation.clone());
        dlg.set_cv_elements_path(cfg.cv_elements_path.clone());
    }
    {
        let cfg = config.clone();
        dlg.set_on_advanced_toggle(move |expanded| {
            let mut c = cfg.borrow_mut();
            c.last_used_advanced = expanded;
            let _ = c.save();
        });
    }
    {
        let cfg = config.clone();
        dlg.set_on_lock_identity(move |author, affiliation| {
            let mut c = cfg.borrow_mut();
            c.locked_author = author;
            c.locked_affiliation = affiliation;
            let _ = c.save();
        });
    }
    {
        let cfg = config.clone();
        dlg.set_on_cv_elements_change(move |path| {
            let mut c = cfg.borrow_mut();
            c.cv_elements_path = Some(path);
            let _ = c.save();
        });
    }

    if let Some(sidecar) = td::load_sidecar(&current_path) {
        dlg.preselect_from_sidecar(&sidecar);
    } else {
        let doc_kind = td::parse_doc_kind(&current_content);
        dlg.preselect_cv_mode(doc_kind.as_deref() == Some("cv"));
        dlg.preselect_body_kind(td::body_kind_from_key(doc_kind.as_deref().unwrap_or("")));
        dlg.preselect_style(&td::parse_style_key(&current_content).unwrap_or_default());
        // A CV document's @zerkalo-style marker is just the literal "cv" (see
        // generate_cv_template), so preselect_style above can't recover the
        // actual CV style (Modern/Academic/Classic/Two-Column) from it — that's
        // tracked separately via @zerkalo-cv-style.
        if let Some(cv_style) = td::parse_cv_style(&current_content) {
            if let Some(idx) = td::cv_style_index(&cv_style) {
                dlg.preselect_cv_style_index(idx);
            }
        }
        dlg.preselect_toc(
            td::parse_has_toc(&current_content),
            td::parse_toc_depth(&current_content),
        );
        dlg.preselect_abstract(
            td::parse_has_abstract(&current_content),
            &td::parse_abstract_text(&current_content),
        );
        dlg.preselect_keywords(
            td::parse_has_keywords(&current_content),
            &td::parse_keywords_text(&current_content),
        );
        if let Some(f) = td::parse_dropcap_font(&current_content) {
            dlg.preselect_dropcap_font(&f);
        }
        if let Some(c) = td::parse_dropcap_color(&current_content) {
            dlg.preselect_dropcap_color(&c);
        }
    }

    // Formatting the document *actually* carries, read back from the file
    // itself. Applied whether or not a sidecar exists, because the sidecar is a
    // cache of the last Apply and the document is what compiles: it can have
    // been edited by hand, restored from a backup, or copied from a file whose
    // sidecar belonged to a different version. Whatever isn't found in the
    // document leaves the sidecar's (or the form's) value alone.
    //
    // Font size in particular had no reader at all on the no-sidecar path,
    // so re-opening this dialog on a 14 pt document and pressing Apply reset
    // it to 12 pt — the parser existed, it was just never called.
    if let Some(f) = td::parse_font(&current_content) {
        dlg.preselect_font(&f);
    }
    if let Some(sz) = td::parse_font_size(&current_content) {
        dlg.preselect_font_size(&sz);
    }
    if let Some(p) = td::parse_paper(&current_content) {
        // A custom-sized page has to carry its dimensions back into the
        // Custom fields too, or Apply regenerates it at the 210×297 default.
        let (w, h) = td::parse_custom_paper(&current_content).unwrap_or_default();
        dlg.preselect_paper(&p, &w, &h);
    }
    if let Some(s) = td::parse_spacing(&current_content) {
        dlg.preselect_spacing(&s);
    }
    if td::has_page_margins(&current_content) {
        dlg.preselect_margin(
            td::parse_margin(&current_content),
            &td::parse_custom_margin(&current_content).unwrap_or_default(),
        );
    }
    // Only meaningful for documents Zerkalo generated: on anything else these
    // read as "off", which would turn a missing parse into a silent reset.
    if td::has_template_block(&current_content) {
        dlg.preselect_page_numbers(td::parse_page_numbers(&current_content));
        dlg.preselect_header(td::parse_header_style(&current_content));
        dlg.preselect_packages(&td::parse_packages(&current_content));
        dlg.preselect_languages(&td::parse_languages(&current_content));
        let (numbering_on, format) = td::parse_heading_numbering(&current_content);
        dlg.preselect_heading_numbering(numbering_on);
        if !format.is_empty() {
            dlg.preselect_heading_format(&format);
        }
    }

    // The body is ground truth for CV-ness: if the sidecar/marker path above
    // disagrees with what the document's body actually calls (#cv-section, an
    // import of cv-helpers.typ), trust the body — see body_looks_like_cv's doc
    // comment. Without this, a document whose sidecar drifted to a non-CV kind
    // would keep regenerating a non-CV preamble onto its still-CV body forever,
    // producing a document that fails to compile ("unknown function: section").
    if td::body_looks_like_cv(&current_content) {
        dlg.preselect_cv_mode(true);
        dlg.preselect_body_kind(td::body_kind_from_key("cv"));
        // The sidecar/marker path above may have left the Style row on a stale
        // or non-CV-meaningful selection, so re-derive it now.
        if let Some(cv_style) = td::parse_cv_style(&current_content) {
            if let Some(idx) = td::cv_style_index(&cv_style) {
                dlg.preselect_cv_style_index(idx);
            }
        }
    }
    // If the user edited the abstract directly in the .typ file, that wins over
    // what the sidecar recorded last time.
    if let Some(doc_abstract) = td::parse_abstract_from_doc(&current_content) {
        dlg.override_abstract_text(&doc_abstract);
    }
    // Always read metadata from the document — the user may have edited the
    // #let doc-* variables directly, and the sidecar won't reflect that.
    dlg.preselect_metadata(
        &td::parse_meta(&current_content, "title"),
        &td::parse_meta(&current_content, "subtitle"),
        &td::parse_meta(&current_content, "author"),
        &td::parse_meta(&current_content, "affiliation"),
        &td::parse_meta(&current_content, "course"),
        &td::parse_meta(&current_content, "professor"),
        &td::parse_meta(&current_content, "date"),
    );

    let ep = editor.clone();
    let win = window.clone();
    let pv = preview.clone();
    let toasts = toast_overlay.clone();
    let root = project_root.to_path_buf();
    dlg.set_on_apply(move |new_content, sidecar| {
        apply_template_result(
            &win,
            &ep,
            &pv,
            &toasts,
            &root,
            current_path.clone(),
            current_content.clone(),
            new_content,
            sidecar,
        );
    });
    dlg.present();
}

/// Writes back a one-value preamble edit from the format bar's font/size
/// pickers, keeps the sidecar in step, and recompiles.
///
/// `edited` is `None` when the document has no Zerkalo template block for the
/// edit to land in — that's said out loud rather than papered over by
/// regenerating a template onto a document that never had one.
///
/// The sidecar is only *updated*, never created here: an absent sidecar means
/// the document itself is the record of its settings, and writing a fresh one
/// from a single font pick would claim defaults for every setting it doesn't
/// know, which "Update Template Settings" would then trust over the file.
/// Returns whether the document actually changed, so the caller only relabels
/// the format bar for an edit that landed.
fn apply_doc_font_edit(
    editor: &super::editor_pane::EditorPane,
    preview: &super::preview_pane::PreviewPane,
    toast_overlay: &adw::ToastOverlay,
    edited: Option<String>,
    update_sidecar: impl FnOnce(&mut super::template_dialog::SidecarSettings),
) -> bool {
    let Some(path) = editor.get_active_path() else { return false };
    let Some(updated) = edited else {
        toast_overlay.add_toast(adw::Toast::new(
            "This document has no Zerkalo template block — nothing to change. \
             Use Update Template Settings… to give it one.",
        ));
        return false;
    };

    if let Err(e) = super::template_dialog::write_atomically(&path, &updated) {
        tracing::error!("Failed to write document font change: {e}");
        toast_overlay.add_toast(adw::Toast::new(&format!("Couldn't save the change: {e}")));
        return false;
    }
    if let Some(mut sc) = super::template_dialog::load_sidecar(&path) {
        update_sidecar(&mut sc);
        super::template_dialog::save_sidecar(&path, &sc);
    }
    editor.splice_preamble(path, &updated);
    preview.trigger_compile();
    true
}

/// Applies a template dialog's result to `path`, splicing the fresh template
/// onto the editor buffer's *current* content (read fresh here, not a
/// snapshot taken when the dialog was opened, so edits made while the
/// non-modal dialog was open aren't discarded). Confirms first if the
/// document has no body marker, since applying then replaces the whole file.
#[allow(clippy::too_many_arguments)]
fn apply_template_result(
    window: &adw::ApplicationWindow,
    editor: &super::editor_pane::EditorPane,
    preview: &super::preview_pane::PreviewPane,
    toast_overlay: &adw::ToastOverlay,
    project_root: &Path,
    path: PathBuf,
    current_content: String,
    new_content: String,
    sidecar: super::template_dialog::SidecarSettings,
) {
    use super::template_dialog::SpliceOutcome;

    let do_apply = {
        let editor = editor.clone();
        let preview = preview.clone();
        let toast_overlay = toast_overlay.clone();
        let path = path.clone();
        let project_root = project_root.to_path_buf();
        move || {
            let cc = editor.get_active_content().unwrap_or_default();
            let (updated, outcome) =
                super::template_dialog::apply_body_splice_reporting(&cc, &new_content);

            // Nothing survives a refusal, so say so — this used to close the
            // dialog and leave the document untouched with no explanation,
            // reading as a dead Apply button.
            if outcome == SpliceOutcome::RefusedIncompatible {
                toast_overlay.add_toast(adw::Toast::new(
                    "Settings not applied — these settings don't match this document's body.",
                ));
                return;
            }

            // Everything from ZERKALO-TEMPLATE-END down to the body marker —
            // title page, abstract, keywords, contents — is regenerated even
            // on a clean splice, so a title page the user had customised by
            // hand goes with it. A snapshot first makes every Apply
            // recoverable through Browse Snapshots…, which is cheaper than
            // trying to detect which of those lines the user had touched.
            save_snapshot(&project_root, &path, &cc);

            // Anything that doesn't preserve the body verbatim gets a backup
            // first too: those paths can discard the user's actual writing,
            // and want a copy sitting visibly next to the document.
            let mut backup_note = String::new();
            if outcome != SpliceOutcome::Preserved {
                match super::template_dialog::backup_document(&path) {
                    Ok(b) => {
                        backup_note = b
                            .file_name()
                            .map(|n| format!(" Backup saved as {}.", n.to_string_lossy()))
                            .unwrap_or_default();
                    }
                    Err(e) => {
                        tracing::error!("Failed to back up before template apply: {e}");
                        toast_overlay.add_toast(adw::Toast::new(&format!(
                            "Settings not applied — couldn't back up the document first: {e}"
                        )));
                        return;
                    }
                }
            }

            if let Err(e) = super::template_dialog::write_atomically(&path, &updated) {
                tracing::error!("Failed to write updated template: {e}");
                toast_overlay.add_toast(adw::Toast::new(&format!(
                    "Couldn't save the updated document: {e}"
                )));
                return;
            }
            super::template_dialog::save_sidecar(&path, &sidecar);
            editor.splice_preamble(path.clone(), &updated);
            preview.trigger_compile();

            match outcome {
                SpliceOutcome::BodyRegenerated => toast_overlay.add_toast(adw::Toast::new(
                    &format!("Layout changed, so the CV body was rebuilt.{backup_note}"),
                )),
                SpliceOutcome::WholeDocumentReplaced => toast_overlay.add_toast(
                    adw::Toast::new(&format!("Document replaced.{backup_note}")),
                ),
                _ => {}
            }
        }
    };

    if super::template_dialog::has_body_marker(&current_content) {
        do_apply();
    } else {
        let confirm = adw::MessageDialog::new(
            Some(window),
            Some("Replace entire document?"),
            Some("This document has no body marker, so the template \
                  will replace the whole file. Your current text will be \
                  moved to a .typ.bak backup alongside it.\n\n\
                  If you meant to keep this text, cancel and use \
                  Repair Template Markers first."),
        );
        confirm.add_response("cancel", "Cancel");
        confirm.add_response("replace", "Replace Document");
        confirm.set_response_appearance("replace", adw::ResponseAppearance::Destructive);
        confirm.set_default_response(Some("cancel"));
        confirm.set_close_response("cancel");
        confirm.connect_response(None, move |_, id| {
            if id == "replace" {
                do_apply();
            }
        });
        confirm.present();
    }
}

/// Open the print sheet for the current document.
///
/// This used to compile a PDF into `~/.cache/zerkalo/<stem>.pdf` and `xdg-open`
/// it, which meant "Print" actually exported a PDF to a path the user didn't
/// choose and handed it to whatever application owns PDFs. It also compiled with
/// no sys inputs, so a CV document — whose entries arrive through
/// `skrizhal-cv-data` — failed to compile, and the error was discarded, leaving
/// the button apparently dead.
#[allow(clippy::too_many_arguments)]
fn print_from_preview(
    parent: &adw::ApplicationWindow,
    editor: &EditorPane,
    preview: &super::preview_pane::PreviewPane,
    toast_overlay: &adw::ToastOverlay,
    error_panel: &ErrorPanel,
    project_root: &Path,
    config: &Rc<RefCell<Config>>,
) {
    // Print what's on screen, not the last saved state. `compile_inputs`
    // carries the unsaved buffer contents, but only the preview's own snapshot —
    // so flush every other modified tab to disk first, the same way the
    // Ctrl+Shift+E export does.
    editor.save_all_modified();

    let Some(request) = crate::ui::print_sheet::request_for(preview) else {
        toast_overlay.add_toast(adw::Toast::new("Nothing to print — no root file detected."));
        return;
    };

    let toast_for_errors = toast_overlay.clone();
    let toast_for_status = toast_overlay.clone();
    let panel = error_panel.clone();
    let root_for_errors = project_root.to_path_buf();
    let config_for_save = config.clone();

    crate::ui::print_sheet::PrintSheet::open(
        parent,
        request,
        &config.borrow(),
        move |prefs| {
            let mut cfg = config_for_save.borrow_mut();
            if cfg.print != prefs {
                cfg.print = prefs;
                let _ = cfg.save();
            }
        },
        move |msg| {
            let errors = parse_typst_errors(&msg, &root_for_errors);
            if errors.is_empty() {
                let t = adw::Toast::new(&format!("Couldn't print: {msg}"));
                t.set_timeout(5);
                toast_for_errors.add_toast(t);
            } else {
                panel.show_compile_errors(errors);
                panel.widget().set_visible(true);
                let t = adw::Toast::new("Couldn't print — see the error panel.");
                t.set_timeout(4);
                toast_for_errors.add_toast(t);
            }
        },
        move |status| {
            use crate::ui::print::PrintStatus;
            match status {
                PrintStatus::Failed(msg) => {
                    let t = adw::Toast::new(&format!("Couldn't print: {msg}"));
                    t.set_timeout(5);
                    toast_for_status.add_toast(t);
                }
                PrintStatus::Cancelled => {}
                PrintStatus::Sent => {
                    let t = adw::Toast::new("Sent to printer");
                    t.set_timeout(3);
                    toast_for_status.add_toast(t);
                }
            }
        },
    );
}

fn update_draft_toggle_label(btn: &gtk4::ToggleButton, is_draft: bool) {
    if let Some(lbl) = btn.child().and_downcast::<gtk4::Label>() {
        if is_draft {
            lbl.set_markup("<b>Draft</b>");
        } else {
            lbl.set_markup("Final");
        }
    }
}
