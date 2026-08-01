use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use gtk4::prelude::*;
use gtk4::{
    AlertDialog, Align, Box as GtkBox, Button, Entry, Label, MenuButton,
    Notebook, Orientation, Paned, Popover, ScrolledWindow, Separator, Stack, ToggleButton,
};
use libadwaita as adw;
use adw::prelude::*;

use crate::bibliography;
use crate::config::{CompileProfile, Config, Theme};
use crate::writing_log::{WritingLog, count_words, new_file_start_words, FileStartWords};
use crate::git_sync;
use crate::keybindings::{matches_binding, Keybindings};
use crate::lsp::{DiagSeverity, LspClient};
use crate::session::Session;
use super::command_palette::{CommandPalette, default_commands, heading_items};
use super::dep_graph::DepGraph;
use super::docs_browser::DocsBrowser;
use super::editor_pane::{EditorPane, strip_typst_markup, strip_zerkalo_blocks};
use super::file_tree::FileTree;
use super::font_manager::FontManager;
use super::error_panel::{enrich_error_message, parse_typst_errors, CompileError, ErrorPanel, Severity};
use super::export_dialog::ExportDialog;
use super::help_window::HelpWindow;
use super::citation_panel::CitationPanel;
use super::outline_panel::OutlinePanel;
use super::package_browser::PackageBrowser;
use super::preview_pane::PreviewPane;
use super::ref_manager::RefManager;
use super::settings_dialog::SettingsDialog;
use super::sync_dialog::SyncDialog;
use super::template_dialog::TemplateDialog;
use super::notes_panel::NotesPanel;
use super::plan_panel::PlanPanel;
use super::snapshot_dialog::{SnapshotDialog, save_snapshot};
use super::library_window::LibraryWindow;
use crate::library::Library;

use crate::cv_mode::CV_HELPERS_TYPST;

pub struct AppWindow {
    window: adw::ApplicationWindow,
    editor_pane: EditorPane,
    preview_pane: PreviewPane,
    #[allow(dead_code)]
    error_panel: ErrorPanel,
    #[allow(dead_code)]
    outline_panel: OutlinePanel,
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
            let (sender, receiver) =
                glib::MainContext::channel::<Library>(glib::Priority::LOW);
            std::thread::spawn(move || {
                let mut lib = Library::open().unwrap_or_else(|e| {
                    tracing::warn!("Failed to open library DB: {e}");
                    Library::open_in_memory()
                });
                lib.import_directory(&work_dir_bg).ok();
                lib.fix_created_dates_from_fs();
                sender.send(lib).ok();
            });
            receiver.attach(None, move |lib| {
                *library_bg.borrow_mut() = lib;
                tracing::info!("Library DB ready");
                glib::ControlFlow::Break
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
        let effective_output_dir = proj_cfg.output_dir.clone();
        let extra_compiler_args = proj_cfg.compiler_args.clone();

        // ── Runtime-configurable values ─────────────────────────────────────

        let debounce_ms: Rc<RefCell<u64>> = Rc::new(RefCell::new(config.debounce_ms));
        let auto_compile: Rc<RefCell<bool>> = Rc::new(RefCell::new(config.auto_compile));
        let compile_on_save: Rc<RefCell<bool>> = Rc::new(RefCell::new(config.compile_on_save));
        let manual_compile_only: Rc<RefCell<bool>> = Rc::new(RefCell::new(config.manual_compile_only));
        let auto_save_idle_ms: Rc<RefCell<u64>> = Rc::new(RefCell::new(config.auto_save_idle_ms));
        let current_config: Rc<RefCell<Config>> = Rc::new(RefCell::new(config.clone()));
        let last_edit_instant: Rc<RefCell<Option<std::time::Instant>>> = Rc::new(RefCell::new(None));
        let has_compile_errors: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        // ── Header bar ──────────────────────────────────────────────────────

        let header = adw::HeaderBar::new();

        // Start: sidebar toggle + insert panel toggle (flat, left side)
        let sidebar_btn = Button::from_icon_name("sidebar-show-symbolic");
        sidebar_btn.set_tooltip_text(Some("Toggle sidebar"));
        sidebar_btn.add_css_class("flat");
        sidebar_btn.update_property(&[gtk4::accessible::Property::Label("Toggle sidebar")]);
        header.pack_start(&sidebar_btn);

        let library_btn = Button::with_label("Library");
        library_btn.add_css_class("flat");
        library_btn.set_tooltip_text(Some("Open document library (Ctrl+L)"));
        header.pack_start(&library_btn);

        // Style switcher dropdown — placed in header start, beside the title
        let style_names = crate::styles::STYLES.iter().map(|(n, _, _, _, _)| *n).collect::<Vec<_>>();
        let style_box = GtkBox::new(Orientation::Vertical, 0);
        style_box.set_margin_top(4);
        style_box.set_margin_bottom(4);
        let style_popover = Popover::new();
        style_popover.set_child(Some(&style_box));
        let style_btn = Button::with_label("Style");
        style_btn.add_css_class("flat");
        style_btn.add_css_class("caption");
        style_btn.set_tooltip_text(Some("Apply a formatting style to the document"));
        {
            let sp = style_popover.clone();
            let sb = style_btn.clone();
            style_btn.connect_clicked(move |_| {
                sp.set_parent(&sb);
                if sp.is_visible() { sp.popdown(); } else { sp.popup(); }
            });
        }
        for name in &style_names {
            let row = Button::new();
            row.set_label(name);
            row.set_halign(Align::Start);
            row.add_css_class("flat");
            row.set_size_request(160, -1);
            style_box.append(&row);
        }
        // Wire style buttons after editor_pane is available (done below)

        let todo_btn = ToggleButton::new();
        todo_btn.set_icon_name("view-list-symbolic");
        todo_btn.set_tooltip_text(Some("Toggle plan panel"));
        todo_btn.add_css_class("flat");
        todo_btn.set_active(false);
        todo_btn.update_property(&[gtk4::accessible::Property::Label("Toggle plan panel")]);

        // ── Compilation profile toggle (status bar) ──────────────────────────
        let draft_label = gtk4::Label::new(Some("Final"));
        draft_label.add_css_class("caption");
        let draft_toggle = ToggleButton::new();
        draft_toggle.set_child(Some(&draft_label));
        draft_toggle.add_css_class("flat");
        draft_toggle.set_tooltip_text(Some("Toggle Draft (fast preview) / Final (full quality)"));

        // ── Primary header buttons (packed together at end of section) ────────
        let preview_label = Label::new(Some("Preview"));
        preview_label.set_use_markup(true);
        let compile_btn = Button::new();
        compile_btn.set_child(Some(&preview_label));
        compile_btn.set_tooltip_text(Some("Toggle Preview (Ctrl+Shift+P)"));
        compile_btn.add_css_class("flat");

        let recompile_header_btn = Button::from_icon_name("view-refresh-symbolic");
        recompile_header_btn.set_tooltip_text(Some("Compile now (Ctrl+Shift+P)"));
        recompile_header_btn.add_css_class("flat");
        recompile_header_btn.update_property(&[gtk4::accessible::Property::Label("Compile now")]);

        let sync_btn = Button::from_icon_name("vcs-push-symbolic");
        sync_btn.set_tooltip_text(Some("Commit & Push to Git (Ctrl+Shift+G)"));
        sync_btn.add_css_class("flat");
        sync_btn.update_property(&[gtk4::accessible::Property::Label("Commit and push to Git")]);

        // ── Hamburger menu items (using make_menu_item for left+shortcut layout) ──
        let HamburgerItems {
            menu_new_template_item,
            menu_reapply_template_item,
            menu_repair_markers_item,
            menu_new_item,
            menu_open_item,
            menu_save_item,
            menu_save_as_item,
            menu_snapshots_item,
            menu_export_item,
            menu_export_web_item,
            menu_print_item,
            menu_import_item,
            menu_docs_item,
            menu_fonts_item,
            menu_settings_item,
            menu_setup_item,
            menu_backup_remote_item,
            menu_help_item,
            menu_writing_stats_item,
            menu_about_item,
            menu_import_pdf_item,
        } = build_hamburger_menu_items();

        // ── Popover layout ────────────────────────────────────────────────────
        let menu_popover_box = GtkBox::new(Orientation::Vertical, 0);
        menu_popover_box.set_margin_top(4);
        menu_popover_box.set_margin_bottom(4);
        menu_popover_box.set_width_request(260);

        // New / Open
        menu_popover_box.append(&menu_new_template_item);
        menu_popover_box.append(&menu_new_item);
        menu_popover_box.append(&Separator::new(Orientation::Horizontal));
        menu_popover_box.append(&menu_open_item);
        menu_popover_box.append(&menu_docs_item);
        menu_popover_box.append(&Separator::new(Orientation::Horizontal));
        // Current document
        menu_popover_box.append(&menu_reapply_template_item);
        menu_popover_box.append(&menu_repair_markers_item);
        menu_popover_box.append(&Separator::new(Orientation::Horizontal));
        // Save / version
        menu_popover_box.append(&menu_save_item);
        menu_popover_box.append(&menu_save_as_item);
        menu_popover_box.append(&menu_snapshots_item);
        menu_popover_box.append(&Separator::new(Orientation::Horizontal));
        // Export / share
        menu_popover_box.append(&menu_export_item);
        menu_popover_box.append(&menu_export_web_item);
        menu_popover_box.append(&menu_print_item);
        menu_popover_box.append(&menu_import_item);
        menu_popover_box.append(&Separator::new(Orientation::Horizontal));
        // App settings
        menu_popover_box.append(&menu_fonts_item);
        // Filled once editor_pane exists — it owns the button and its state.
        let gost_menu_slot = GtkBox::new(Orientation::Vertical, 0);
        menu_popover_box.append(&gost_menu_slot);
        menu_popover_box.append(&menu_settings_item);
        menu_popover_box.append(&menu_setup_item);
        menu_popover_box.append(&menu_backup_remote_item);
        menu_popover_box.append(&menu_writing_stats_item);
        menu_popover_box.append(&Separator::new(Orientation::Horizontal));
        menu_popover_box.append(&menu_help_item);
        menu_popover_box.append(&menu_about_item);

        let menu_popover = Popover::new();
        menu_popover.set_child(Some(&menu_popover_box));
        let menu_btn = MenuButton::new();
        menu_btn.set_icon_name("open-menu-symbolic");
        menu_btn.add_css_class("flat");
        menu_btn.set_popover(Some(&menu_popover));

        // Header end section layout (left → right):
        //   sync | todo | ⟳ compile now | compile mode | Preview | ≡
        // In GTK4 pack_end the last-packed widget is leftmost in the end section.
        // `compile_mode_slot` is packed empty here and filled further down, once
        // the config-backed compile-mode button exists — packing it late would
        // otherwise land it at the far left of the section, away from the
        // compile buttons it belongs with.
        let compile_mode_slot = GtkBox::new(Orientation::Horizontal, 0);
        header.pack_end(&menu_btn);
        header.pack_end(&compile_btn);
        header.pack_end(&compile_mode_slot);
        header.pack_end(&recompile_header_btn);
        header.pack_end(&todo_btn);
        header.pack_end(&sync_btn);

        // ── Setzer-style open dropdown ───────────────────────────────────────
        let open_search = Entry::new();
        open_search.set_placeholder_text(Some("Search documents…"));
        open_search.set_hexpand(true);
        open_search.set_margin_start(8);
        open_search.set_margin_end(8);
        open_search.set_margin_top(8);
        open_search.set_margin_bottom(4);

        let open_list_box = GtkBox::new(Orientation::Vertical, 0);

        let open_scroll = ScrolledWindow::new();
        open_scroll.set_child(Some(&open_list_box));
        open_scroll.set_min_content_height(80);
        open_scroll.set_max_content_height(360);
        open_scroll.set_propagate_natural_height(true);
        open_scroll.set_margin_start(4);
        open_scroll.set_margin_end(4);
        open_scroll.set_margin_bottom(4);

        let open_popover_box = GtkBox::new(Orientation::Vertical, 0);
        open_popover_box.set_width_request(280);
        open_popover_box.append(&open_search);
        open_popover_box.append(&open_scroll);

        let recent_popover = Popover::new();
        recent_popover.set_child(Some(&open_popover_box));

        let file_title_widget = adw::WindowTitle::new("untitled", "");

        let file_selector = MenuButton::new();
        file_selector.add_css_class("flat");
        file_selector.set_child(Some(&file_title_widget));
        file_selector.set_popover(Some(&recent_popover));

        // Root-file controls sit immediately right of the document title, where
        // they read as being about *this* document. Filled in further down.
        let title_extras = GtkBox::new(Orientation::Horizontal, 4);
        let title_box = GtkBox::new(Orientation::Horizontal, 6);
        title_box.append(&file_selector);
        title_box.append(&title_extras);
        header.set_title_widget(Some(&title_box));

        // ── Panels ──────────────────────────────────────────────────────────

        let editor_pane = EditorPane::new();

        let library_window = LibraryWindow::new(app, library.clone(), config.work_dir.clone());
        {
            let ep = editor_pane.clone();
            let win_for_open = window.clone();
            let lib_for_open = library.clone();
            library_window.set_on_open(move |path| {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    ep.open_file(path.clone(), &content);
                }
                lib_for_open.borrow_mut().touch_opened(&path).ok();
                win_for_open.present();
            });
        }
        {
            let lw = library_window.clone();
            library_btn.connect_clicked(move |_| lw.toggle());
        }

        let outline_panel = OutlinePanel::new();
        let citation_panel = CitationPanel::new();
        let ref_manager = RefManager::new();
        let dep_graph = DepGraph::new(project_root.clone());
        let package_browser = PackageBrowser::new();
        let todo_panel = PlanPanel::new(config.work_dir.clone());
        let notes_panel = NotesPanel::new();

        let writing_log: Rc<RefCell<WritingLog>> = Rc::new(RefCell::new(WritingLog::load()));
        let file_start_words = new_file_start_words();
        let session_start: Rc<RefCell<std::time::Instant>> =
            Rc::new(RefCell::new(std::time::Instant::now()));

        // Wire style buttons → editor; update style_btn label to current style name
        {
            let mut child_opt = style_box.first_child();
            for (name, code, bib_style, bib_title, style_key) in crate::styles::STYLES {
                let Some(child) = child_opt else { break };
                let next = child.next_sibling();
                let Some(btn) = child.downcast::<Button>().ok() else {
                    child_opt = next;
                    continue;
                };
                let ep = editor_pane.clone();
                let pop = style_popover.clone();
                let code_s = code.to_string();
                let bib_s = bib_style.to_string();
                let title_s = bib_title.to_string();
                let key_s = style_key.to_string();
                let sbtn = style_btn.clone();
                let name_s = name.to_string();
                let cfg_for_style = current_config.clone();
                let win_for_style = window.clone();
                btn.connect_clicked(move |_| {
                    pop.popdown();
                    if bib_s == crate::styles::CUSTOM_STYLE_PLACEHOLDER {
                        let custom_path = cfg_for_style.borrow().custom_csl_path.clone();
                        match custom_path {
                            Some(path) => {
                                ep.apply_style(&code_s, &path.to_string_lossy(), &title_s, &key_s);
                                sbtn.set_label(&name_s);
                            }
                            None => {
                                let dlg = adw::MessageDialog::new(
                                    Some(&win_for_style),
                                    Some("No custom CSL file configured"),
                                    Some("Choose a .csl file in Settings before using the Custom style."),
                                );
                                dlg.add_response("ok", "OK");
                                dlg.present();
                            }
                        }
                        return;
                    }
                    ep.apply_style(&code_s, &bib_s, &title_s, &key_s);
                    sbtn.set_label(&name_s);
                });
                child_opt = btn.next_sibling();
            }
        }

        // Wire outline symbol insert → editor
        {
            let ep = editor_pane.clone();
            outline_panel.set_on_symbol_insert(move |ch| ep.insert_at_cursor(&ch));
        }

        // Wire outline heading click → jump to line in editor.
        // Defer jump_to_line to idle so all open_file callbacks (page-switch, LSP, etc.)
        // finish before we try to scroll, preventing reentrancy crashes.
        {
            let ep = editor_pane.clone();
            outline_panel.set_on_jump(move |path, line| {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    ep.open_file(path.clone(), &content);
                }
                let ep_idle = ep.clone();
                let path_idle = path.clone();
                glib::idle_add_local_once(move || {
                    ep_idle.jump_to_line(&path_idle, line);
                });
            });
        }

        // Wire cursor movement → outline auto-select.
        // Preview scrolling is intentionally NOT driven by cursor movement — the
        // preview should only move via its own scrollbar or page-nav buttons.
        {
            let op = outline_panel.clone();
            editor_pane.set_on_cursor_heading(move |path, heading_line| {
                op.select_for_line(&path, heading_line);
            });
        }

        // Set project root for project-wide word count tooltip
        editor_pane.set_project_root(project_root.clone());

        // Wire dep_graph → open file in editor
        {
            let ep = editor_pane.clone();
            dep_graph.set_on_open(move |path| {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    ep.open_file(path, &content);
                }
            });
        }

        // Wire package_browser → insert import at cursor
        {
            let ep = editor_pane.clone();
            package_browser.set_on_insert(move |import| ep.insert_at_cursor(&import));
        }

        // Pop-out preview state
        let popout_window: Rc<RefCell<Option<adw::Window>>> = Rc::new(RefCell::new(None));
        let popout_pane: Rc<RefCell<Option<PreviewPane>>> = Rc::new(RefCell::new(None));

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
                    let name_del = name.clone();
                    let outer_for_del = outer_row.clone();
                    let cfg_del = config_for_open.clone();
                    let ep_del = editor_for_open.clone();
                    del_btn.connect_clicked(move |_| {
                        let alert = AlertDialog::builder()
                            .modal(true)
                            .message("Move to trash?")
                            .detail(&format!("'{}' will be moved to the system trash.", name_del))
                            .buttons(["Cancel", "Move to Trash"])
                            .cancel_button(0)
                            .default_button(0)
                            .build();
                        let path_c = path_del.clone();
                        let outer_c = outer_for_del.clone();
                        let cfg_c = cfg_del.clone();
                        let ep_c = ep_del.clone();
                        alert.choose(
                            None::<&gtk4::Window>,
                            None::<&gtk4::gio::Cancellable>,
                            move |result| {
                                if result == Ok(1) {
                                    let _ = gtk4::gio::File::for_path(&path_c)
                                        .trash(None::<&gtk4::gio::Cancellable>);
                                    cfg_c.borrow_mut().recent_files.retain(|p| p != &path_c);
                                    let _ = cfg_c.borrow().save();
                                    ep_c.close_file_if_open(&path_c);
                                    if let Some(parent) = outer_c.parent() {
                                        if let Ok(p) = parent.downcast::<GtkBox>() {
                                            p.remove(&outer_c);
                                        }
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

        // ── Apply initial settings ──────────────────────────────────────────

        editor_pane.apply_font_size(config.editor_font_size);
        editor_pane.apply_font_family(&config.editor_font_family);
        editor_pane.apply_word_wrap(config.editor_word_wrap);
        editor_pane.set_word_wrap_btn(config.editor_word_wrap);
        editor_pane.apply_show_whitespace(config.editor_show_whitespace);
        editor_pane.apply_tab_width(config.editor_tab_width);
        editor_pane.apply_line_spacing(config.editor_line_spacing);
        editor_pane.apply_typewriter_scroll(config.typewriter_scrolling);
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
        // ── Doc font/size callbacks — update sidecar and regenerate template ──
        {
            let ep = editor_pane.clone();
            let preview_for_font = preview_pane.clone();
            editor_pane.set_on_doc_font(move |font_name| {
                let Some(path) = ep.get_active_path() else { return };
                let mut sc = super::template_dialog::load_sidecar(&path)
                    .unwrap_or_default();
                sc.font = font_name;
                super::template_dialog::save_sidecar(&path, &sc);
                let settings = super::template_dialog::sidecar_to_settings(&sc);
                let fresh = super::template_dialog::generate_typst_template(&settings);
                if let Some(content) = ep.get_active_content() {
                    let updated = super::template_dialog::apply_body_splice(&content, &fresh);
                    if let Err(e) = std::fs::write(&path, &updated) {
                        tracing::error!("Failed to write font change: {e}");
                    } else {
                        ep.splice_preamble(path.clone(), &updated);
                        preview_for_font.trigger_compile();
                    }
                }
            });
        }
        {
            let ep = editor_pane.clone();
            let preview_for_size = preview_pane.clone();
            editor_pane.set_on_doc_font_size(move |size| {
                let Some(path) = ep.get_active_path() else { return };
                let mut sc = super::template_dialog::load_sidecar(&path)
                    .unwrap_or_default();
                sc.font_size = size;
                super::template_dialog::save_sidecar(&path, &sc);
                let settings = super::template_dialog::sidecar_to_settings(&sc);
                let fresh = super::template_dialog::generate_typst_template(&settings);
                if let Some(content) = ep.get_active_content() {
                    let updated = super::template_dialog::apply_body_splice(&content, &fresh);
                    if let Err(e) = std::fs::write(&path, &updated) {
                        tracing::error!("Failed to write size change: {e}");
                    } else {
                        ep.splice_preamble(path.clone(), &updated);
                        preview_for_size.trigger_compile();
                    }
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

        // Document-view controls belong together in the header rather than
        // scattered along the status bar; pack_start puts them right of Library.
        header.pack_start(&editor_pane.simple_mode_button_for_header());
        header.pack_start(&editor_pane.focus_button_for_header());
        gost_menu_slot.append(&editor_pane.gost_button_for_menu());

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
        menu_import_item.set_visible(config.developer_mode);

        let editor_for_dark = editor_pane.clone();
        adw::StyleManager::default().connect_dark_notify(move |mgr| {
            editor_for_dark.apply_style_scheme(mgr.is_dark());
        });

        // ── Bibliography loading & watch ────────────────────────────────────

        if let Some(ref bp) = effective_bib {
            let entries = bibliography::load_bib(bp);
            if !entries.is_empty() {
                tracing::info!("Loaded {} bib entries from {}", entries.len(), bp.display());
            }
            editor_pane.set_bib_entries(entries.clone());
            citation_panel.load_bib(entries);
            citation_panel.set_bib_filename(bp.file_name().and_then(|n| n.to_str()));
            ref_manager.load_bib(bp);

            let editor_for_bib = editor_pane.clone();
            let citation_for_bib = citation_panel.clone();
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

        // ── Auto-detect .bib when no bib is configured ─────────────────────────
        let auto_detected_bib: Rc<RefCell<Option<std::path::PathBuf>>> = Rc::new(RefCell::new(None));
        if effective_bib.is_none() {
            if let Ok(mut entries) = std::fs::read_dir(&project_root) {
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
                    editor_pane.set_bib_entries(entries.clone());
                    citation_panel.load_bib(entries);
                    citation_panel.set_bib_filename(bib_path.file_name().and_then(|n| n.to_str()));
                    *auto_detected_bib.borrow_mut() = Some(bib_path);
                }
            }
        }

        // ── CV entries loading & watch ───────────────────────────────────────

        if let Some(ref cvp) = effective_cv_elements {
            let entries = crate::cv_mode::load_cv_entries(cvp);
            if !entries.is_empty() {
                tracing::info!("Loaded {} CV entries from {}", entries.len(), cvp.display());
            }
            editor_pane.set_cv_entries(entries.clone());
            citation_panel.load_cv_entries(entries);
            citation_panel.set_cv_filename(cvp.file_name().and_then(|n| n.to_str()));

            let editor_for_cv = editor_pane.clone();
            let citation_for_cv = citation_panel.clone();
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
            let ep = editor_pane.clone();
            citation_panel.set_on_insert(move |text| ep.insert_at_cursor(&text));
        }

        // ── Citation panel: choose bib file button ────────────────────────────

        {
            let win_for_bib = window.clone();
            let ep_for_bib = editor_pane.clone();
            let cp_for_bib = citation_panel.clone();
            let cfg_for_bib = current_config.clone();
            let rm_for_bib = ref_manager.clone();
            citation_panel.set_on_choose_bib(move || {
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
                            cfg.borrow_mut().bib_path = Some(path);
                            let _ = cfg.borrow().save();
                        }
                    }
                });
            });
        }

        // ── Citation panel: choose Skrizhal CV element file button ────────────

        {
            let win_for_cv = window.clone();
            let ep_for_cv = editor_pane.clone();
            let cp_for_cv = citation_panel.clone();
            let cfg_for_cv = current_config.clone();
            citation_panel.set_on_choose_cv(move || {
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

        let editor_for_ref = editor_pane.clone();
        ref_manager.set_on_insert(move |citation| {
            editor_for_ref.insert_at_cursor(&citation);
        });

        {
            let ep = editor_pane.clone();
            ref_manager.set_on_jump_citation(move |key| {
                ep.jump_to_text(&format!("@{key}"));
            });
        }

        // ── Reference manager: project-wide citation-key rename ───────────────
        {
            let ep = editor_pane.clone();
            let rm = ref_manager.clone();
            let cp = citation_panel.clone();
            let win = window.clone();
            let project_root_for_rename = project_root.clone();
            ref_manager.set_on_rename(move |old_key, new_key| {
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

        // ── Sidebar toggle (item 1) ─────────────────────────────────────────
        // (left_paned is set up in the layout section below; we capture it via Rc)
        let focus_active: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let preview_vis_holder: Rc<RefCell<Option<GtkBox>>> = Rc::new(RefCell::new(None));
        let sidebar_visible: Rc<RefCell<bool>> = Rc::new(RefCell::new(true));
        let sidebar_visible_c = sidebar_visible.clone();
        // left_paned_ref set after layout — closure reads it through the Rc
        let left_paned_holder: Rc<RefCell<Option<GtkBox>>> = Rc::new(RefCell::new(None));
        let right_sidebar_holder: Rc<RefCell<Option<GtkBox>>> = Rc::new(RefCell::new(None));
        let lpane_for_btn = left_paned_holder.clone();
        sidebar_btn.connect_clicked(move |_| {
            let mut v = sidebar_visible_c.borrow_mut();
            *v = !*v;
            if let Some(lp) = lpane_for_btn.borrow().as_ref() {
                lp.set_visible(*v);
            }
        });

        // ── Plan sidebar toggle ─────────────────────────────────────────────
        let rsh_for_todo = right_sidebar_holder.clone();
        todo_btn.connect_toggled(move |btn| {
            if let Some(rs) = rsh_for_todo.borrow().as_ref() {
                rs.set_visible(btn.is_active());
            }
        });

        // ── Focus mode toggle — status bar button, dims sidebar, hides preview
        {
            let focus_active_c = focus_active.clone();
            let preview_vis_for_focus = preview_vis_holder.clone();
            let rsh_for_focus = right_sidebar_holder.clone();
            let todo_btn_for_focus = todo_btn.clone();
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
                if let Some(rs) = rsh_for_focus.borrow().as_ref() {
                    rs.set_visible(!focused && todo_btn_for_focus.is_active());
                }
            });
        }

        // ── Menu: Browse Documents ──────────────────────────────────────────
        let window_for_docs = window.clone();
        let editor_for_docs = editor_pane.clone();
        let root_for_docs = project_root.clone();
        let menu_popover_for_docs = menu_popover.clone();
        menu_docs_item.connect_clicked(move |_| {
            menu_popover_for_docs.popdown();
            let browser = DocsBrowser::new(&window_for_docs, root_for_docs.clone());
            let ep = editor_for_docs.clone();
            browser.set_on_open(move |path| {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    ep.open_file(path, &content);
                }
            });
            browser.present();
        });

        // ── Compile/Preview toggle button ───────────────────────────────────
        // Wired after preview_outer is created (see below, search "preview_vis_holder.borrow_mut")

        // ── Menu: Settings ──────────────────────────────────────────────────

        let window_for_settings = window.clone();
        let editor_for_settings = editor_pane.clone();
        let debounce_for_settings = debounce_ms.clone();
        let auto_compile_for_settings = auto_compile.clone();
        let compile_on_save_for_settings = compile_on_save.clone();
        let manual_compile_only_for_settings = manual_compile_only.clone();
        let current_config_for_settings = current_config.clone();
        let menu_popover_for_settings = menu_popover.clone();
        let import_item_for_settings = menu_import_item.clone();
        let compile_mode_btn_for_settings = compile_mode_btn.clone();
        let compile_mode_label_for_settings = compile_mode_label.clone();
        menu_settings_item.connect_clicked(move |_| {
            menu_popover_for_settings.popdown();
            let dialog = SettingsDialog::new(
                &window_for_settings,
                &current_config_for_settings.borrow(),
            );
            let editor = editor_for_settings.clone();
            let debounce = debounce_for_settings.clone();
            let auto_flag = auto_compile_for_settings.clone();
            let cos_flag = compile_on_save_for_settings.clone();
            let mco_flag = manual_compile_only_for_settings.clone();
            let cfg_rc = current_config_for_settings.clone();
            let window_for_save = window_for_settings.clone();
            let import_item_save = import_item_for_settings.clone();
            let cm_btn_save = compile_mode_btn_for_settings.clone();
            let cm_lbl_save = compile_mode_label_for_settings.clone();

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
                apply_compile_mode_css(&cm_btn_save, new_cfg.auto_compile, new_cfg.compile_on_save, new_cfg.manual_compile_only);
                editor.apply_font_size(new_cfg.editor_font_size);
                editor.apply_font_family(&new_cfg.editor_font_family);
                editor.apply_word_wrap(new_cfg.editor_word_wrap);
                editor.set_word_wrap_btn(new_cfg.editor_word_wrap);
                editor.apply_show_whitespace(new_cfg.editor_show_whitespace);
                editor.apply_tab_width(new_cfg.editor_tab_width);
                editor.apply_line_spacing(new_cfg.editor_line_spacing);
                editor.apply_typewriter_scroll(new_cfg.typewriter_scrolling);
                editor.set_spell_enabled(new_cfg.spell_enabled);
                editor.set_spell_autocorrect(new_cfg.spell_autocorrect);
                editor.set_spell_languages(new_cfg.spell_languages.clone());
                apply_theme(&new_cfg.theme);
                editor.apply_style_scheme(adw::StyleManager::default().is_dark());
                // High contrast CSS class on the window
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
                }
                import_item_save.set_visible(new_cfg.developer_mode);
                let work_dir_changed = new_cfg.work_dir != cfg_rc.borrow().work_dir;
                *cfg_rc.borrow_mut() = new_cfg;
                if work_dir_changed {
                    let alert = AlertDialog::builder()
                        .modal(true)
                        .message("Restart required")
                        .detail("The work folder change takes effect after restarting Zerkalo.")
                        .buttons(["OK"])
                        .default_button(0)
                        .build();
                    alert.choose(
                        Some(&window_for_save),
                        None::<&gtk4::gio::Cancellable>,
                        |_| {},
                    );
                }
            });
            dialog.present();
        });

        // ── Menu: Help ──────────────────────────────────────────────────────

        let window_for_help = window.clone();
        let menu_popover_for_help = menu_popover.clone();
        let editor_for_help = editor_pane.clone();
        menu_help_item.connect_clicked(move |_| {
            menu_popover_for_help.popdown();
            HelpWindow::new(&window_for_help, editor_for_help.is_cv_mode()).present();
        });

        // ── Menu: Setup & Onboarding ────────────────────────────────────────

        let window_for_setup = window.clone();
        let root_for_setup = project_root.clone();
        let menu_popover_for_setup = menu_popover.clone();
        let cfg_for_setup = current_config.clone();
        menu_setup_item.connect_clicked(move |_| {
            menu_popover_for_setup.popdown();
            let (sans, serif) = font_defaults(&cfg_for_setup);
            super::setup_wizard::SetupWizard::new(
                &window_for_setup, &root_for_setup, &sans, &serif,
                make_font_save_cb(cfg_for_setup.clone()),
            ).present();
        });

        // ── Menu: Backup Remotes ────────────────────────────────────────────

        let window_for_backup = window.clone();
        let root_for_backup = project_root.clone();
        let menu_popover_for_backup = menu_popover.clone();
        menu_backup_remote_item.connect_clicked(move |_| {
            menu_popover_for_backup.popdown();
            show_backup_remote_dialog(&window_for_backup, &root_for_backup);
        });

        // ── Menu: About ─────────────────────────────────────────────────────

        let window_for_about = window.clone();
        let menu_popover_for_about = menu_popover.clone();
        menu_about_item.connect_clicked(move |_| {
            menu_popover_for_about.popdown();
            let dlg = adw::MessageDialog::new(
                Some(&window_for_about),
                Some(concat!("Zerkalo ", env!("CARGO_PKG_VERSION"))),
                Some(
                    "A contemplative Typst editor.\n\n\
                     Built with Rust · GTK4 · libadwaita · sourceview5\n\
                     Embedded Typst compiler — no binary required\n\n\
                     https://github.com/calstfrancis/zerkalo"
                ),
            );
            dlg.add_response("ok", "OK");
            dlg.present();
        });

        // ── Menu: Writing Stats ─────────────────────────────────────────────

        let window_for_stats = window.clone();
        let writing_log_for_stats = writing_log.clone();
        let menu_popover_for_stats = menu_popover.clone();
        menu_writing_stats_item.connect_clicked(move |_| {
            menu_popover_for_stats.popdown();
            let log = writing_log_for_stats.borrow();
            let today = log.total_today();
            let week = log.total_this_week();
            let streak = log.streak_days();
            let total = log.sessions.len();
            let body = format!(
                "Today: {:+} words\nThis week: {:+} words\nStreak: {} day{}\nTotal sessions: {}",
                today, week, streak,
                if streak == 1 { "" } else { "s" },
                total,
            );
            let dlg = adw::MessageDialog::new(
                Some(&window_for_stats),
                Some("Writing Stats"),
                Some(&body),
            );
            dlg.add_response("ok", "OK");
            dlg.present();
        });

        // ── Menu: Export ────────────────────────────────────────────────────

        let preview_for_export = preview_pane.clone();
        let window_for_export = window.clone();
        let menu_popover_for_export = menu_popover.clone();
        let current_config_for_export = current_config.clone();
        let project_root_for_export = project_root.clone();
        let cv_elements_for_export = effective_cv_elements.clone();
        menu_export_item.connect_clicked(move |_| {
            menu_popover_for_export.popdown();
            let initial_fmt = current_config_for_export.borrow().last_export_format;
            let cfg_for_save = current_config_for_export.clone();
            ExportDialog::new(
                &window_for_export,
                preview_for_export.root_file_path(),
                preview_for_export.output_dir(),
                project_root_for_export.clone(),
                cv_elements_for_export.clone(),
                initial_fmt,
                move |fmt| {
                    let mut cfg = cfg_for_save.borrow_mut();
                    cfg.last_export_format = fmt;
                    let _ = cfg.save();
                },
            )
            .present();
        });

        // ── Menu: Print PDF ─────────────────────────────────────────────────

        {
            let preview_for_print = preview_pane.clone();
            let menu_popover_for_print = menu_popover.clone();
            menu_print_item.connect_clicked(move |_| {
                menu_popover_for_print.popdown();
                print_pdf_from_preview(&preview_for_print);
            });
        }

        // ── Menu: Font Management ───────────────────────────────────────────

        let window_for_fonts = window.clone();
        let menu_popover_for_fonts = menu_popover.clone();
        let cfg_for_fonts = current_config.clone();
        menu_fonts_item.connect_clicked(move |_| {
            menu_popover_for_fonts.popdown();
            let cfg = cfg_for_fonts.borrow();
            FontManager::new(&window_for_fonts, &cfg.default_sans_font, &cfg.default_serif_font).present();
        });

        // ── Menu: Import (picker dialog) ───────────────────────────────────

        // Hoisted above its other uses (further down, around the sync button)
        // so the import machinery below — which shows in-progress/result toasts —
        // can capture it too.
        let toast_overlay = adw::ToastOverlay::new();

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
        let pdf_item_for_dlg = menu_import_pdf_item.clone();
        menu_import_item.connect_clicked(move |_| {
            menu_popover_for_import.popdown();

            let dlg = adw::Window::new();
            dlg.set_title(Some("Import"));
            dlg.set_default_width(280);
            dlg.set_modal(true);
            dlg.set_transient_for(Some(&window_for_import));
            dlg.set_deletable(true);

            let header_dlg = adw::HeaderBar::new();
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

        // ── Menu: Import PDF ───────────────────────────────────────────────

        let window_for_pdf = window.clone();
        let editor_for_pdf = editor_pane.clone();
        let menu_popover_for_pdf = menu_popover.clone();
        let work_dir_for_pdf = project_root.clone();
        menu_import_pdf_item.connect_clicked(move |_| {
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
            dialog.open(Some(&win_ref), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(input_path) = file.path() {
                        run_pdf_import(&win2, &ep2, input_path);
                    }
                }
            });
        });

        // ── Menu: New from Template ─────────────────────────────────────────

        let window_for_template = window.clone();
        let editor_for_template = editor_pane.clone();
        let menu_popover_for_template = menu_popover.clone();
        let project_root_for_template = project_root.clone();
        let cfg_for_template = current_config.clone();
        menu_new_template_item.connect_clicked(move |_| {
            menu_popover_for_template.popdown();
            let last_advanced = cfg_for_template.borrow().last_used_advanced;
            let dlg = TemplateDialog::new(&window_for_template, &project_root_for_template, last_advanced);
            {
                let cfg = cfg_for_template.borrow();
                dlg.set_bib_path(cfg.bib_path.clone());
                dlg.preselect_locked_identity(&cfg.locked_author.clone(), &cfg.locked_affiliation.clone());
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

        // ── Menu: Update Template Settings ─────────────────────────────────

        let window_for_reapply = window.clone();
        let editor_for_reapply = editor_pane.clone();
        let menu_popover_for_reapply = menu_popover.clone();
        let project_root_for_reapply = project_root.clone();
        let cfg_for_reapply = current_config.clone();
        let preview_for_reapply = preview_pane.clone();
        menu_reapply_template_item.connect_clicked(move |_| {
            menu_popover_for_reapply.popdown();
            let Some(current_path) = editor_for_reapply.get_active_path() else {
                let t = adw::Toast::new("Open a document first");
                t.set_timeout(3);
                // toast_overlay captured below; use a window dialog as fallback
                show_alert(&window_for_reapply, "No document open", "Open a .typ file first, then use Update Template Settings.");
                return;
            };
            let current_content = editor_for_reapply.get_active_content().unwrap_or_default();
            let last_advanced_reapply = cfg_for_reapply.borrow().last_used_advanced;
            let dlg = TemplateDialog::new(&window_for_reapply, &project_root_for_reapply, last_advanced_reapply);
            {
                let cfg_adv = cfg_for_reapply.clone();
                dlg.set_on_advanced_toggle(move |expanded| {
                    let mut c = cfg_adv.borrow_mut();
                    c.last_used_advanced = expanded;
                    let _ = c.save();
                });
            }
            {
                let cfg = cfg_for_reapply.borrow();
                dlg.set_bib_path(cfg.bib_path.clone());
                dlg.preselect_locked_identity(&cfg.locked_author.clone(), &cfg.locked_affiliation.clone());
                dlg.set_cv_elements_path(cfg.cv_elements_path.clone());
            }
            {
                let cfg = cfg_for_reapply.clone();
                dlg.set_on_lock_identity(move |author, affiliation| {
                    let mut c = cfg.borrow_mut();
                    c.locked_author = author;
                    c.locked_affiliation = affiliation;
                    let _ = c.save();
                });
            }
            {
                let cfg = cfg_for_reapply.clone();
                dlg.set_on_cv_elements_change(move |path| {
                    let mut c = cfg.borrow_mut();
                    c.cv_elements_path = Some(path);
                    let _ = c.save();
                });
            }

            if let Some(sidecar) = super::template_dialog::load_sidecar(&current_path) {
                dlg.preselect_from_sidecar(&sidecar);
            } else {
                let doc_kind = super::template_dialog::parse_doc_kind(&current_content);
                dlg.preselect_cv_mode(doc_kind.as_deref() == Some("cv"));
                dlg.preselect_body_kind(super::template_dialog::body_kind_from_key(
                    doc_kind.as_deref().unwrap_or(""),
                ));
                dlg.preselect_style(
                    &super::template_dialog::parse_style_key(&current_content)
                        .unwrap_or_default(),
                );
                // A CV document's @zerkalo-style marker is just the literal "cv"
                // (see generate_cv_template), so preselect_style above can't
                // recover the actual CV style (Modern/Academic/Classic/
                // Two-Column) from it — that's tracked separately via
                // @zerkalo-cv-style.
                if let Some(cv_style) = super::template_dialog::parse_cv_style(&current_content) {
                    if let Some(idx) = super::template_dialog::cv_style_index(&cv_style) {
                        dlg.preselect_cv_style_index(idx);
                    }
                }
                if let Some(f) = super::template_dialog::parse_font(&current_content) {
                    dlg.preselect_font(&f);
                }
                if let Some(p) = super::template_dialog::parse_paper(&current_content) {
                    dlg.preselect_paper(&p, "", "");
                }
                if let Some(s) = super::template_dialog::parse_spacing(&current_content) {
                    dlg.preselect_spacing(&s);
                }
                dlg.preselect_margin(super::template_dialog::parse_margin(&current_content), "");
                dlg.preselect_toc(
                    super::template_dialog::parse_has_toc(&current_content),
                    super::template_dialog::parse_toc_depth(&current_content),
                );
                dlg.preselect_abstract(
                    super::template_dialog::parse_has_abstract(&current_content),
                    &super::template_dialog::parse_abstract_text(&current_content),
                );
                dlg.preselect_keywords(
                    super::template_dialog::parse_has_keywords(&current_content),
                    &super::template_dialog::parse_keywords_text(&current_content),
                );
                if let Some(f) = super::template_dialog::parse_dropcap_font(&current_content) {
                    dlg.preselect_dropcap_font(&f);
                }
                if let Some(c) = super::template_dialog::parse_dropcap_color(&current_content) {
                    dlg.preselect_dropcap_color(&c);
                }
            }
            // The body is ground truth for CV-ness: if the sidecar/marker path above
            // disagrees with what the document's body actually calls (#cv-section, an
            // import of cv-helpers.typ), trust the body — see body_looks_like_cv's doc
            // comment. Without this, a document whose sidecar drifted to a non-CV kind
            // would keep regenerating a non-CV preamble onto its still-CV body forever,
            // producing a document that fails to compile ("unknown function: section").
            if super::template_dialog::body_looks_like_cv(&current_content) {
                dlg.preselect_cv_mode(true);
                dlg.preselect_body_kind(super::template_dialog::body_kind_from_key("cv"));
                // The sidecar/marker path above may have left the Style row on a
                // stale or non-CV-meaningful selection (e.g. a sidecar that drifted
                // to a non-CV body_kind never calls preselect_cv_style_index at
                // all). Re-derive it from the body's actual @zerkalo-cv-style marker
                // now that we know this is really a CV.
                if let Some(cv_style) = super::template_dialog::parse_cv_style(&current_content) {
                    if let Some(idx) = super::template_dialog::cv_style_index(&cv_style) {
                        dlg.preselect_cv_style_index(idx);
                    }
                }
            }
            if let Some(doc_abstract) = super::template_dialog::parse_abstract_from_doc(&current_content) {
                dlg.override_abstract_text(&doc_abstract);
            }
            // Always read metadata from the document — the user may have edited the
            // #let doc-* variables directly, and the sidecar won't reflect those changes.
            dlg.preselect_metadata(
                &super::template_dialog::parse_meta(&current_content, "title"),
                &super::template_dialog::parse_meta(&current_content, "subtitle"),
                &super::template_dialog::parse_meta(&current_content, "author"),
                &super::template_dialog::parse_meta(&current_content, "affiliation"),
                &super::template_dialog::parse_meta(&current_content, "course"),
                &super::template_dialog::parse_meta(&current_content, "professor"),
                &super::template_dialog::parse_meta(&current_content, "date"),
            );

            let ep = editor_for_reapply.clone();
            let win_for_apply = window_for_reapply.clone();
            let preview_apply = preview_for_reapply.clone();
            let current_content_for_apply = current_content.clone();
            let current_path_for_apply = current_path.clone();
            dlg.set_on_apply(move |new_content, sidecar| {
                apply_template_result(
                    &win_for_apply,
                    &ep,
                    &preview_apply,
                    current_path_for_apply.clone(),
                    current_content_for_apply.clone(),
                    new_content,
                    sidecar,
                );
            });
            dlg.present();
        });

        // ── Menu: Repair Template Markers ───────────────────────────────────

        let editor_for_repair = editor_pane.clone();
        let window_for_repair = window.clone();
        let menu_popover_for_repair = menu_popover.clone();
        menu_repair_markers_item.connect_clicked(move |_| {
            menu_popover_for_repair.popdown();
            let Some(path) = editor_for_repair.get_active_path() else { return };
            let (title, body) = match super::template_dialog::repair_template_markers(&path) {
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
            let dlg = adw::MessageDialog::new(
                Some(&window_for_repair),
                Some(title),
                Some(&body),
            );
            dlg.add_response("ok", "OK");
            dlg.set_default_response(Some("ok"));
            dlg.present();
        });

        // ── Menu: New Document ──────────────────────────────────────────────

        let window_for_new = window.clone();
        let editor_for_new = editor_pane.clone();
        let work_dir_for_new = project_root.clone();
        let menu_popover_for_new = menu_popover.clone();
        menu_new_item.connect_clicked(move |_| {
            menu_popover_for_new.popdown();
            let dialog = gtk4::FileDialog::new();
            dialog.set_title("New Document");
            dialog.set_initial_folder(Some(&gtk4::gio::File::for_path(&work_dir_for_new)));
            dialog.set_initial_name(Some("untitled.typ"));
            let win_c = window_for_new.clone();
            let ep_c = editor_for_new.clone();
            dialog.save(Some(&win_c), None::<&gtk4::gio::Cancellable>, move |result| {
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
            });
        });

        // ── Menu: Open File ─────────────────────────────────────────────────

        let window_for_open = window.clone();
        let editor_for_open_file = editor_pane.clone();
        let menu_popover_for_open = menu_popover.clone();
        menu_open_item.connect_clicked(move |_| {
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
            dialog.open(Some(&win_c), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            ep_c.open_file(path, &content);
                        }
                    }
                }
            });
        });

        // ── Menu: Save ──────────────────────────────────────────────────────

        let editor_for_menu_save = editor_pane.clone();
        let preview_for_menu_save = preview_pane.clone();
        let menu_popover_for_save = menu_popover.clone();
        let root_for_menu_save = project_root.clone();
        menu_save_item.connect_clicked(move |_| {
            menu_popover_for_save.popdown();
            if let Some(path) = editor_for_menu_save.save_current() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    save_snapshot(&root_for_menu_save, &path, &content);
                }
                preview_for_menu_save.trigger_compile();
            }
        });

        // ── Menu: Save As ───────────────────────────────────────────────────

        let window_for_save_as = window.clone();
        let editor_for_save_as = editor_pane.clone();
        let preview_for_save_as = preview_pane.clone();
        let menu_popover_for_save_as = menu_popover.clone();
        menu_save_as_item.connect_clicked(move |_| {
            menu_popover_for_save_as.popdown();
            let Some(content) = editor_for_save_as.get_active_content() else { return };
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
            dialog.save(Some(&win_c), None::<&gtk4::gio::Cancellable>, move |result| {
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
            });
        });

        // ── Menu: Browse Snapshots ──────────────────────────────────────────

        let window_for_snap = window.clone();
        let editor_for_snap = editor_pane.clone();
        let root_for_snap = project_root.clone();
        let menu_popover_for_snap = menu_popover.clone();
        menu_snapshots_item.connect_clicked(move |_| {
            menu_popover_for_snap.popdown();
            let Some(path) = editor_for_snap.get_active_path() else { return };
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

        // ── Sync button ─────────────────────────────────────────────────────

        let window_for_sync = window.clone();
        let sync_btn_ref = sync_btn.clone();
        let editor_for_sync = editor_pane.clone();
        let toast_for_sync_btn = toast_overlay.clone();
        let toast_for_sync_closure = toast_overlay.clone();

        if let Some(ref bib_path) = *auto_detected_bib.borrow() {
            let name = bib_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("refs.bib")
                .to_string();
            let t = adw::Toast::new(&format!("Loaded bibliography: {name}"));
            t.set_timeout(4);
            toast_overlay.add_toast(t);
        }

        // ── Menu: Export for Web ────────────────────────────────────────────
        {
            let ep = editor_pane.clone();
            let win = window.clone();
            let pop = menu_popover.clone();
            let toast = toast_for_sync_btn.clone();
            menu_export_web_item.connect_clicked(move |_| {
                pop.popdown();
                let Some(input_path) = ep.get_active_path() else { return };
                let dialog = gtk4::FileDialog::builder()
                    .title("Export for Web")
                    .modal(true)
                    .initial_name(
                        input_path.with_extension("html")
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("output.html"),
                    )
                    .build();
                let win_c = win.clone();
                let toast_c = toast.clone();
                dialog.save(Some(&win_c), None::<&gtk4::gio::Cancellable>, move |result| {
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
                });
            });
        }
        let config_for_sync = current_config.clone();
        let project_root_for_sync_fallback = project_root.clone();
        sync_btn.connect_clicked(move |_| {
            editor_for_sync.save_all_modified();
            let root = editor_for_sync.get_active_path()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .and_then(|dir| git_sync::git_repo_root(&dir))
                .unwrap_or_else(|| project_root_for_sync_fallback.clone());
            let win = window_for_sync.clone();
            let btn = sync_btn_ref.clone();
            let toasts = toast_for_sync_closure.clone();
            let token = crate::secret_store::load_github_token();
            let cfg_rc = config_for_sync.clone();

            if !git_sync::has_remote(&root) {
                let dialog = SyncDialog::new(&win);
                let root2 = root.clone();
                let win2 = win.clone();
                let btn2 = btn.clone();
                let toasts2 = toasts.clone();
                let token2 = token.clone();
                let cfg_rc2 = cfg_rc.clone();

                let confirmed = Rc::new(RefCell::new(false));
                let confirmed_set = confirmed.clone();
                dialog.set_on_confirm(move |url| {
                    *confirmed_set.borrow_mut() = true;
                    match git_sync::add_remote(&root2, &url) {
                        Ok(()) => do_sync(root2.clone(), win2.clone(), toasts2.clone(), btn2.clone(), token2.clone(), cfg_rc2.clone()),
                        Err(e) => {
                            show_alert(&win2, "Remote Setup Failed", &e);
                            btn2.set_sensitive(true);
                        }
                    }
                });

                let btn_cancel = btn.clone();
                dialog.window.connect_destroy(move |_| {
                    if !*confirmed.borrow() {
                        btn_cancel.set_sensitive(true);
                    }
                });

                btn.set_sensitive(false);
                dialog.present();
                return;
            }

            do_sync(root, win, toasts, btn, token, cfg_rc);
        });

        // ── Debounced compile + outline update + LSP ────────────────────────

        let preview_for_change = preview_pane.clone();
        let editor_for_change = editor_pane.clone();
        let debounce_for_change = debounce_ms.clone();
        let auto_compile_for_change = auto_compile.clone();
        let compile_on_save_for_change = compile_on_save.clone();
        let manual_compile_only_for_change = manual_compile_only.clone();
        let outline_for_change = outline_panel.clone();
        let notes_for_change = notes_panel.clone();
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
            let notes = notes_for_change.clone();
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
                            notes.update(&content, &path);
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
        let todo_panel_for_switch = todo_panel.clone();
        let notes_panel_for_switch = notes_panel.clone();
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
            notes_panel_for_switch.update(&content, &path);
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
            todo_panel_for_switch.set_current_file(Some(&path));
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
        let todo_panel_for_open = todo_panel.clone();
        let notes_panel_for_open = notes_panel.clone();
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
            todo_panel_for_open.set_current_file(Some(&path));
            notes_panel_for_open.update(&content, &path);
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
        preview_pane.set_on_compile_done(move |result| {
            compile_btn_for_done.remove_css_class("compiling-pulse");
            compile_rev_for_done.set_reveal_child(false);
            match &result {
                None => {
                    let had_errors = *has_errors_for_compile.borrow();
                    *has_errors_for_compile.borrow_mut() = false;
                    error_panel_for_compile.clear();
                    error_panel_for_compile.widget().set_visible(false);
                    editor_for_diag.clear_diagnostic_marks();
                    editor_for_diag.clear_error_marks();
                    editor_for_diag.set_diag_summary(0, 0);
                    error_banner_for_compile.set_visible(false);
                    error_banner_lbl_for_compile.set_visible(false);
                    window_for_compile.set_title(Some("Zerkalo"));
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

        // ── Startup: warn if required tools are missing ──────────────────────

        // ── Startup: combined missing-tool check (single alert, not stacked) ───
        let win_for_check = window.clone();
        glib::timeout_add_local(Duration::from_millis(900), move || {
            let in_flatpak = std::path::Path::new("/.flatpak-info").exists();
            let git_ok = if in_flatpak {
                std::process::Command::new("flatpak-spawn")
                    .args(["--host", "git", "--version"]).output().is_ok()
            } else {
                std::process::Command::new("git").arg("--version").output().is_ok()
            };
            let hunspell_ok = std::process::Command::new("hunspell")
                .arg("--version").output().is_ok();
            let pandoc_ok = crate::git_sync::host_command("pandoc")
                .arg("--version").output().is_ok();
            let tinymist_ok = ["/app/lib/zerkalo/tinymist", "/usr/lib/zerkalo/tinymist"]
                .iter()
                .find(|p| std::path::Path::new(p).exists())
                .map(|p| std::process::Command::new(p).arg("--version").output().is_ok())
                .unwrap_or_else(|| std::process::Command::new("tinymist").arg("--version").output().is_ok());

            let mut missing: Vec<String> = Vec::new();
            if !git_ok {
                tracing::warn!("git not found in PATH");
                missing.push(
                    "git — required for Git sync\n\
                     \n  zypper install git  |  apt install git  |  dnf install git".to_string()
                );
            }
            if !hunspell_ok {
                tracing::warn!("hunspell not found in PATH — spell check disabled");
                missing.push(
                    "hunspell — required for spell checking\n\
                     \n  zypper install hunspell hunspell-en\
                     \n  apt install hunspell hunspell-en-us\
                     \n  dnf install hunspell hunspell-en".to_string()
                );
            }
            if !pandoc_ok {
                tracing::info!("pandoc not found — LaTeX/DOCX import disabled");
            }
            if !tinymist_ok {
                tracing::info!("tinymist not found — LSP completions disabled");
                missing.push(
                    "tinymist (optional) — enables LSP completions and diagnostics\n\
                     \n  cargo install tinymist  |  https://github.com/Myriad-Dreamin/tinymist/releases".to_string()
                );
            }
            if !missing.is_empty() {
                let body = missing.join("\n\n");
                show_alert(&win_for_check, "Some tools are missing", &body);
            }
            glib::ControlFlow::Break
        });

        // ── Welcome window + chained setup wizard ────────────────────────────
        // Simple-mode intro is now part of the welcome window, so we no longer
        // need a separate dialog for it. Mark shown_simple_intro if not already set.
        if !config.shown_simple_intro {
            let cfg_for_intro = current_config.clone();
            cfg_for_intro.borrow_mut().shown_simple_intro = true;
            let _ = cfg_for_intro.borrow().save();
        }

        let win_for_welcome = window.clone();
        let root_for_welcome = project_root.clone();
        let cfg_for_welcome = current_config.clone();
        glib::timeout_add_local(Duration::from_millis(1200), move || {
            if super::welcome_window::WelcomeWindow::should_show() {
                let is_first_run = super::welcome_window::WelcomeWindow::is_first_run();
                super::welcome_window::WelcomeWindow::mark_shown();
                let ww = super::welcome_window::WelcomeWindow::new(&win_for_welcome, is_first_run);
                // Chain: after "Get Started", check if setup wizard is needed.
                let win_chain = win_for_welcome.clone();
                let root_chain = root_for_welcome.clone();
                let cfg_chain = cfg_for_welcome.clone();
                ww.set_on_dismissed(move || {
                    if super::setup_wizard::SetupWizard::should_show(&root_chain) {
                        let (sans, serif) = font_defaults(&cfg_chain);
                        super::setup_wizard::SetupWizard::new(
                            &win_chain, &root_chain, &sans, &serif,
                            make_font_save_cb(cfg_chain.clone()),
                        ).present();
                    }
                });
                ww.present();
            } else if super::setup_wizard::SetupWizard::should_show(&root_for_welcome) {
                let (sans, serif) = font_defaults(&cfg_for_welcome);
                super::setup_wizard::SetupWizard::new(
                    &win_for_welcome, &root_for_welcome, &sans, &serif,
                    make_font_save_cb(cfg_for_welcome.clone()),
                ).present();
            }
            glib::ControlFlow::Break
        });

        // ── Auto-backup on idle: write modified buffers after idle for auto_save_idle_ms ──

        let editor_for_autosave = editor_pane.clone();
        let toast_for_autosave = toast_overlay.clone();
        let last_edit_for_autosave = last_edit_instant.clone();
        let idle_ms_for_autosave = auto_save_idle_ms.clone();
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

        // ── LSP: initialise 500 ms after startup ────────────────────────────

        let lsp_init = lsp_client.clone();
        let root_for_lsp = project_root.clone();
        let editor_for_lsp_init = editor_pane.clone();
        glib::timeout_add_local(Duration::from_millis(500), move || {
            *lsp_init.borrow_mut() = LspClient::new(&root_for_lsp);
            if lsp_init.borrow().is_some() {
                tracing::info!("tinymist LSP active");
                editor_for_lsp_init.set_lsp_status("LSP ●");
            } else {
                tracing::info!("tinymist not found — LSP disabled");
                editor_for_lsp_init.set_lsp_status("");
            }
            glib::ControlFlow::Break
        });

        // ── LSP: poll for diagnostics + completions + auto-restart ──────────

        let lsp_poll = lsp_client.clone();
        let error_panel_for_lsp = error_panel.clone();
        let editor_for_comp_poll = editor_pane.clone();
        let editor_for_lsp_diag = editor_pane.clone();
        let editor_for_lsp_status = editor_pane.clone();
        let last_req_poll = last_completion_request.clone();
        let lsp_diags_for_poll = lsp_has_diags.clone();
        // Grace-period counter: only clear lsp_has_diags after 3 consecutive
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
            // callbacks may try to borrow lsp_client, causing a BorrowError
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
                            let message = if matches!(severity, Severity::Error) {
                                enrich_error_message(&d.message)
                            } else {
                                d.message
                            };
                            CompileError { file: d.file, line: d.line, col: d.col, message, severity }
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

        // ── Periodic auto-save every 30 s ───────────────────────────────────

        let editor_for_autosave = editor_pane.clone();
        glib::timeout_add_local(Duration::from_secs(30), move || {
            editor_for_autosave.save_all_modified();
            glib::ControlFlow::Continue
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
        preview_toolbar.append(&fit_width_btn);
        preview_toolbar.append(&fit_page_btn);
        preview_toolbar.append(&zoom_box);
        preview_toolbar.append(&zoom_label);
        preview_toolbar.append(&compile_time_label);
        let preview_spacer = GtkBox::new(Orientation::Horizontal, 0);
        preview_spacer.set_hexpand(true);
        preview_toolbar.append(&preview_spacer);
        // Page nav group
        let page_nav_box = GtkBox::new(Orientation::Horizontal, 0);
        page_nav_box.add_css_class("linked");
        page_nav_box.append(&page_prev_btn);
        page_nav_box.append(&page_next_btn);
        preview_toolbar.append(&page_nav_box);
        preview_toolbar.append(&page_label);
        preview_toolbar.append(&ref_toggle_btn);
        preview_toolbar.append(&popout_btn);

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
            print_btn.set_tooltip_text(Some("Print PDF"));
            let print_pane = secondary.clone();
            print_btn.connect_clicked(move |_| {
                print_pdf_from_preview(&print_pane);
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

        // ── File tree ────────────────────────────────────────────────────────
        let file_tree = FileTree::new(project_root.clone());
        {
            let ep = editor_pane.clone();
            let lib = library.clone();
            file_tree.set_on_open(move |path| {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    ep.open_file(path.clone(), &content);
                }
                lib.borrow_mut().touch_opened(&path).ok();
            });
        }
        {
            let root = project_root.clone();
            let ft = file_tree.clone();
            let ep = editor_pane.clone();
            file_tree.set_on_new_file(move |name| {
                let path = root.join(&name);
                if !path.exists() {
                    let _ = std::fs::write(&path, "");
                }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    ep.open_file(path, &content);
                }
                ft.refresh();
            });
        }
        {
            let ft = file_tree.clone();
            let win_for_ft_del = window.clone();
            file_tree.set_on_delete(move |path| {
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("this file")
                    .to_string();
                let alert = AlertDialog::builder()
                    .modal(true)
                    .message("Move to trash?")
                    .detail(&format!("'{}' will be moved to the system trash.", name))
                    .buttons(["Cancel", "Move to Trash"])
                    .cancel_button(0)
                    .default_button(0)
                    .build();
                let ft2 = ft.clone();
                alert.choose(
                    Some(&win_for_ft_del),
                    None::<&gtk4::gio::Cancellable>,
                    move |result| {
                        if result == Ok(1) {
                            let _ = gtk4::gio::File::for_path(&path)
                                .trash(None::<&gtk4::gio::Cancellable>);
                            ft2.refresh();
                        }
                    },
                );
            });
        }
        {
            let root = project_root.clone();
            let ft = file_tree.clone();
            file_tree.set_on_new_folder(move |name| {
                let _ = std::fs::create_dir_all(root.join(&name));
                ft.refresh();
            });
        }
        {
            let root = project_root.clone();
            let ft = file_tree.clone();
            let ep = editor_pane.clone();
            file_tree.set_on_new_chapter(move |name| {
                let slug = crate::templates::slugify(&name);
                if slug.is_empty() { return; }
                let filename = format!("{slug}.typ");
                let file_path = root.join(&filename);
                if file_path.exists() { return; }
                let _ = std::fs::write(&file_path, format!("= {name}\n\n"));
                // Insert #include before #bibliography (or at end) in main.typ
                let main_path = root.join("main.typ");
                if main_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&main_path) {
                        let include_line = format!("#include \"{filename}\"");
                        let new_content = if let Some(pos) = content.find("\n#bibliography(") {
                            format!("{}\n{}{}", &content[..pos], include_line, &content[pos..])
                        } else {
                            format!("{}\n{}\n", content.trim_end(), include_line)
                        };
                        let _ = std::fs::write(&main_path, new_content);
                    }
                }
                ft.refresh();
                // Open the new chapter file
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    ep.open_file(file_path, &content);
                }
            });
        }
        {
            let ep = editor_pane.clone();
            let preview = preview_pane.clone();
            file_tree.set_on_insert_include(move |abs_path| {
                let rel = compute_include_path(&preview, &abs_path);
                ep.insert_at_cursor(&format!("#include \"{rel}\"\n"));
            });
        }
        {
            let ep = editor_pane.clone();
            let preview = preview_pane.clone();
            file_tree.set_on_insert_import(move |abs_path| {
                let rel = compute_include_path(&preview, &abs_path);
                let stem = abs_path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("*");
                ep.insert_at_cursor(&format!("#import \"{rel}\": {stem}\n"));
            });
        }
        // ── Set / Clear root file via context menu ────────────────────────────
        {
            let preview = preview_pane.clone();
            let root_ref = configured_root.clone();
            let root_dir = project_root.clone();
            let title_w = file_title_widget.clone();
            let ep_for_root = editor_pane.clone();
            file_tree.set_on_set_root(move |path| {
                preview.set_root_file(path.clone());
                *root_ref.borrow_mut() = Some(path.clone());
                // Update breadcrumb if there's an active file
                if let Some(active) = ep_for_root.get_active_path() {
                    if path != active {
                        let root_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("root");
                        let active_name = active.file_name().and_then(|n| n.to_str()).unwrap_or("file");
                        title_w.set_subtitle(&format!("{root_name} › {active_name}"));
                    } else {
                        title_w.set_subtitle("");
                    }
                }
                // Save to project config
                let rel = path.strip_prefix(&root_dir).unwrap_or(&path).to_path_buf();
                let mut pcfg = crate::config::ProjectConfig::load(&root_dir).unwrap_or_default();
                pcfg.root_file = Some(rel);
                let _ = pcfg.save(&root_dir);
                preview.trigger_compile();
            });
        }
        {
            let preview = preview_pane.clone();
            let root_ref = configured_root.clone();
            let root_dir = project_root.clone();
            let title_w = file_title_widget.clone();
            file_tree.set_on_clear_root(move |()| {
                preview.clear_root_file();
                *root_ref.borrow_mut() = None;
                title_w.set_subtitle("");
                // Save to project config
                let mut pcfg = crate::config::ProjectConfig::load(&root_dir).unwrap_or_default();
                pcfg.root_file = None;
                let _ = pcfg.save(&root_dir);
            });
        }
        {
            let editor_for_tab_out = editor_pane.clone();
            file_tree.set_on_tab_out(move || {
                editor_for_tab_out.grab_focus();
            });
        }

        // ── Project toggle in status bar ─────────────────────────────────────
        //
        // A ToggleButton labelled "project" (default OFF). When toggled ON,
        // inline root-file controls become visible in the status bar.
        {
            let proj_toggle = ToggleButton::new();
            proj_toggle.add_css_class("flat");
            proj_toggle.add_css_class("status-toggle");
            proj_toggle.set_tooltip_text(Some("Toggle project controls (root file)"));
            proj_toggle.update_property(&[gtk4::accessible::Property::Label("Toggle project controls")]);
            proj_toggle.set_active(false);

            let proj_btn_label = Label::new(Some("project"));
            proj_btn_label.set_use_markup(true);
            proj_btn_label.add_css_class("caption");
            proj_btn_label.set_margin_top(3);
            proj_btn_label.set_margin_bottom(3);
            proj_toggle.set_child(Some(&proj_btn_label));

            // ── Inline controls (hidden until toggle is ON) ───────────────────
            let proj_controls = GtkBox::new(Orientation::Horizontal, 4);
            proj_controls.set_visible(false);
            proj_controls.set_margin_start(4);

            let root_value_lbl = Label::new(Some("no root"));
            root_value_lbl.add_css_class("caption");
            root_value_lbl.add_css_class("dim-label");
            root_value_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
            root_value_lbl.set_max_width_chars(22);
            proj_controls.append(&root_value_lbl);

            let set_root_btn = Button::with_label("Set\u{2026}");
            set_root_btn.add_css_class("flat");
            set_root_btn.add_css_class("caption");
            proj_controls.append(&set_root_btn);

            // Distinct icons: this one clears the chosen root, the next one
            // puts the whole control away. Two bare ✕ glyphs side by side read
            // as the same button twice.
            let clear_root_btn = Button::from_icon_name("edit-clear-symbolic");
            clear_root_btn.add_css_class("flat");
            clear_root_btn.set_tooltip_text(Some("Clear root file"));
            clear_root_btn.update_property(&[gtk4::accessible::Property::Label("Clear root file")]);
            proj_controls.append(&clear_root_btn);

            // Dismiss: for a one-file document there's no root to pick, and the
            // controls plus the main.typ banner are pure clutter. Shuts them for
            // this project and remembers it; the "project" toggle stays, so one
            // click brings them back.
            let dismiss_root_btn = Button::from_icon_name("window-close-symbolic");
            dismiss_root_btn.add_css_class("flat");
            dismiss_root_btn.set_tooltip_text(Some(
                "Hide project controls for this document (click \"project\" to bring them back)",
            ));
            dismiss_root_btn.update_property(&[
                gtk4::accessible::Property::Label("Hide project controls"),
            ]);
            proj_controls.append(&dismiss_root_btn);

            // Initialise from current root state
            {
                let root_name = configured_root.borrow().as_ref()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string());
                if let Some(name) = root_name {
                    root_value_lbl.set_text(&name);
                    proj_btn_label.set_markup("<b>project</b>");
                    clear_root_btn.set_sensitive(true);
                } else {
                    clear_root_btn.set_sensitive(false);
                }
            }

            {
                let ctrls = proj_controls.clone();
                let toggle_c = proj_toggle.clone();
                let banner_rc = root_banner.clone();
                let root_dir_c = project_root.clone();
                let toast_c = toast_overlay.clone();
                let title_c = file_title_widget.clone();
                dismiss_root_btn.connect_clicked(move |_| {
                    toggle_c.set_active(false);
                    ctrls.set_visible(false);
                    // The "root › file" breadcrumb is part of the same story.
                    title_c.set_subtitle("");
                    if let Some(b) = banner_rc.borrow().as_ref() {
                        b.set_revealed(false);
                    }
                    let mut pcfg =
                        crate::config::ProjectConfig::load(&root_dir_c).unwrap_or_default();
                    pcfg.root_controls_dismissed = true;
                    let _ = pcfg.save(&root_dir_c);
                    toast_c.add_toast(adw::Toast::new(
                        "Project controls hidden — click \"project\" to show them again",
                    ));
                });
            }

            // Toggle → show/hide inline controls and root banner; update proj_mode_active
            {
                let ctrls = proj_controls.clone();
                let banner_rc = root_banner.clone();
                let proj_mode_c = proj_mode_active.clone();
                proj_toggle.connect_toggled(move |btn| {
                    let on = btn.is_active();
                    proj_mode_c.set(on);
                    ctrls.set_visible(on);
                    if let Some(b) = banner_rc.borrow().as_ref() {
                        b.set_revealed(on);
                    }
                });
            }

            let root_value_lbl_rc = Rc::new(root_value_lbl);
            let proj_btn_label_rc = Rc::new(proj_btn_label);
            let clear_root_btn_rc = Rc::new(clear_root_btn);

            // "Set…" button
            {
                let win_c = window.clone();
                let root_dir_c = project_root.clone();
                let root_ref_c = configured_root.clone();
                let preview_c = preview_pane.clone();
                let title_c = file_title_widget.clone();
                let ep_c = editor_pane.clone();
                let rvl = root_value_lbl_rc.clone();
                let bll = proj_btn_label_rc.clone();
                let clr = clear_root_btn_rc.clone();
                set_root_btn.connect_clicked(move |_| {
                    let dialog = gtk4::FileDialog::new();
                    dialog.set_title("Set Root File");
                    let filter = gtk4::FileFilter::new();
                    filter.set_name(Some("Typst files (*.typ)"));
                    filter.add_pattern("*.typ");
                    let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
                    filters.append(&filter);
                    dialog.set_filters(Some(&filters));
                    dialog.set_initial_folder(Some(&gtk4::gio::File::for_path(&root_dir_c)));
                    let root_dir2 = root_dir_c.clone();
                    let root_ref2 = root_ref_c.clone();
                    let preview2 = preview_c.clone();
                    let title2 = title_c.clone();
                    let ep2 = ep_c.clone();
                    let rvl2 = rvl.clone();
                    let bll2 = bll.clone();
                    let clr2 = clr.clone();
                    dialog.open(Some(&win_c), None::<&gtk4::gio::Cancellable>, move |result| {
                        if let Ok(file) = result {
                            if let Some(path) = file.path() {
                                preview2.set_root_file(path.clone());
                                *root_ref2.borrow_mut() = Some(path.clone());
                                if let Some(active) = ep2.get_active_path() {
                                    if path != active {
                                        let rn = path.file_name().and_then(|n| n.to_str()).unwrap_or("root");
                                        let an = active.file_name().and_then(|n| n.to_str()).unwrap_or("file");
                                        title2.set_subtitle(&format!("{rn} › {an}"));
                                    } else {
                                        title2.set_subtitle("");
                                    }
                                }
                                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                                rvl2.set_text(name);
                                bll2.set_markup("<b>project</b>");
                                clr2.set_sensitive(true);
                                let rel = path.strip_prefix(&root_dir2).unwrap_or(&path).to_path_buf();
                                let mut pcfg = crate::config::ProjectConfig::load(&root_dir2).unwrap_or_default();
                                pcfg.root_file = Some(rel);
                                let _ = pcfg.save(&root_dir2);
                                preview2.trigger_compile();
                            }
                        }
                    });
                });
            }

            // "✕" clear button
            {
                let root_ref_c = configured_root.clone();
                let root_dir_c = project_root.clone();
                let preview_c = preview_pane.clone();
                let title_c = file_title_widget.clone();
                let rvl = root_value_lbl_rc.clone();
                let bll = proj_btn_label_rc.clone();
                let clr = clear_root_btn_rc.clone();
                clear_root_btn_rc.connect_clicked(move |_| {
                    preview_c.clear_root_file();
                    *root_ref_c.borrow_mut() = None;
                    title_c.set_subtitle("");
                    rvl.set_text("no root");
                    bll.set_markup("project");
                    clr.set_sensitive(false);
                    let mut pcfg = crate::config::ProjectConfig::load(&root_dir_c).unwrap_or_default();
                    pcfg.root_file = None;
                    let _ = pcfg.save(&root_dir_c);
                });
            }

            // Insert before SIMPLE: toggle first (ends up just left of SIMPLE),
            // then controls (ends up just left of toggle, so: [controls | toggle | SIMPLE]).
            title_extras.append(&proj_toggle);
            title_extras.append(&proj_controls);
        }

        // Wire file_tree into the compile-done holder
        *file_tree_holder.borrow_mut() = Some(file_tree.clone());

        // ── Unsaved-file indicator in file tree ─────────────────────────────
        {
            let ft = file_tree.clone();
            editor_pane.set_on_file_dirty(move |path, dirty| {
                ft.set_file_modified(&path, dirty);
            });
        }

        // ── Delete file from tab context menu ───────────────────────────────────
        {
            let ft = file_tree.clone();
            editor_pane.set_on_delete_file(move |_path| {
                ft.refresh();
            });
        }

        // ── Image drag-and-drop handler ──────────────────────────────────────────
        {
            let root = project_root.clone();
            let ep = editor_pane.clone();
            let ft = file_tree.clone();
            editor_pane.set_on_image_drop(move |src_path| {
                let fname = src_path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("image.png")
                    .to_string();
                let dest = root.join(&fname);
                if dest != src_path {
                    if let Err(e) = std::fs::copy(&src_path, &dest) {
                        tracing::warn!("Failed to copy image: {e}");
                        return;
                    }
                }
                ft.refresh();
                ep.insert_at_cursor(&format!(
                    "\n#figure(\n  image(\"{fname}\"),\n  caption: [],\n)\n"
                ));
            });
        }

        // ── Document drag-and-drop handler ────────────────────────────────────────
        {
            let win = window.clone();
            let ep = editor_pane.clone();
            let cfg = current_config.clone();
            let toast = toast_overlay.clone();
            editor_pane.set_on_document_drop(move |src_path| {
                let ext = src_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                if ext == "pdf" {
                    run_pdf_import(&win, &ep, src_path);
                    return;
                }
                if let Some(fmt) = IMPORT_FORMATS.iter().find(|f| f.extensions.contains(&ext.as_str())) {
                    let work_dir = ep.get_active_path()
                        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                        .unwrap_or_else(|| src_path.parent().map(|d| d.to_path_buf()).unwrap_or_default());
                    run_pandoc_import(&win, &ep, &cfg, &toast, &work_dir, src_path, fmt);
                }
            });
        }

        // (Refs and Files panels removed — refs/file-tree callbacks kept for
        //  compile-error marking, dirty indicators, and image-drop insertion)

        // ── GOST font toggle (status bar button wired here) ───────────────────
        let current_config_for_gost = current_config.clone();
        let ui_font_provider = gtk4::CssProvider::new();
        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &ui_font_provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
            );
        }
        {
            let ui_prov = ui_font_provider.clone();
            editor_pane.set_on_gost_toggle(move |enabled| {
                if enabled {
                    let cfg = current_config_for_gost.borrow();
                    let editor_font = cfg.editor_font_family.clone();
                    let size_clause = if cfg.editor_font_size > 0 {
                        format!("font-size: {}pt; ", cfg.editor_font_size)
                    } else {
                        String::new()
                    };
                    ui_prov.load_from_data(&format!(
                        "* {{ font-family: 'GOST type B'; }} \
                         textview {{ font-family: '{editor_font}'; {size_clause}}}",
                    ));
                } else {
                    ui_prov.load_from_data("* {}");
                }
            });
        }

        // ── Sidebar toolbar: Update Template button ───────────────────────────
        let update_template_btn = Button::new();
        update_template_btn.set_label("Update Template…");
        update_template_btn.add_css_class("flat");
        update_template_btn.set_hexpand(true);
        update_template_btn.set_tooltip_text(Some(
            "Change formatting style, margins, fonts for this document",
        ));
        let sidebar_toolbar = GtkBox::new(Orientation::Horizontal, 0);
        sidebar_toolbar.set_margin_start(6);
        sidebar_toolbar.set_margin_end(6);
        sidebar_toolbar.set_margin_top(4);
        sidebar_toolbar.set_margin_bottom(4);
        sidebar_toolbar.append(&update_template_btn);

        {
            let win_ut = window.clone();
            let ep_ut = editor_pane.clone();
            let root_ut = project_root.clone();
            let current_config_for_ut = current_config.clone();
            let preview_ut = preview_pane.clone();
            update_template_btn.connect_clicked(move |_| {
                let Some(current_path) = ep_ut.get_active_path() else { return };
                let current_content = ep_ut.get_active_content().unwrap_or_default();
                let dlg = TemplateDialog::new(&win_ut, &root_ut, false);

                dlg.set_cv_elements_path(current_config_for_ut.borrow().cv_elements_path.clone());
                {
                    let cfg = current_config_for_ut.clone();
                    dlg.set_on_cv_elements_change(move |path| {
                        let mut c = cfg.borrow_mut();
                        c.cv_elements_path = Some(path);
                        let _ = c.save();
                    });
                }

                if let Some(sidecar) = super::template_dialog::load_sidecar(&current_path) {
                    dlg.preselect_from_sidecar(&sidecar);
                } else {
                    let doc_kind = super::template_dialog::parse_doc_kind(&current_content);
                    dlg.preselect_cv_mode(doc_kind.as_deref() == Some("cv"));
                    dlg.preselect_body_kind(super::template_dialog::body_kind_from_key(
                        doc_kind.as_deref().unwrap_or(""),
                    ));
                    dlg.preselect_style(
                        &super::template_dialog::parse_style_key(&current_content)
                            .unwrap_or_default(),
                    );
                    // A CV document's @zerkalo-style marker is just the literal "cv"
                    // (see generate_cv_template), so preselect_style above can't
                    // recover the actual CV style (Modern/Academic/Classic/
                    // Two-Column) from it — that's tracked separately via
                    // @zerkalo-cv-style.
                    if let Some(cv_style) = super::template_dialog::parse_cv_style(&current_content) {
                        if let Some(idx) = super::template_dialog::cv_style_index(&cv_style) {
                            dlg.preselect_cv_style_index(idx);
                        }
                    }
                    if let Some(f) = super::template_dialog::parse_font(&current_content) {
                        dlg.preselect_font(&f);
                    }
                    if let Some(p) = super::template_dialog::parse_paper(&current_content) {
                        dlg.preselect_paper(&p, "", "");
                    }
                    if let Some(s) = super::template_dialog::parse_spacing(&current_content) {
                        dlg.preselect_spacing(&s);
                    }
                    dlg.preselect_margin(super::template_dialog::parse_margin(&current_content), "");
                    dlg.preselect_toc(
                        super::template_dialog::parse_has_toc(&current_content),
                        super::template_dialog::parse_toc_depth(&current_content),
                    );
                    dlg.preselect_abstract(
                        super::template_dialog::parse_has_abstract(&current_content),
                        &super::template_dialog::parse_abstract_text(&current_content),
                    );
                    dlg.preselect_keywords(
                        super::template_dialog::parse_has_keywords(&current_content),
                        &super::template_dialog::parse_keywords_text(&current_content),
                    );
                    if let Some(f) = super::template_dialog::parse_dropcap_font(&current_content) {
                        dlg.preselect_dropcap_font(&f);
                    }
                    if let Some(c) = super::template_dialog::parse_dropcap_color(&current_content) {
                        dlg.preselect_dropcap_color(&c);
                    }
                }
                // The body is ground truth for CV-ness: if the sidecar/marker path above
                // disagrees with what the document's body actually calls (#cv-section, an
                // import of cv-helpers.typ), trust the body — see body_looks_like_cv's doc
                // comment. Without this, a document whose sidecar drifted to a non-CV kind
                // would keep regenerating a non-CV preamble onto its still-CV body forever,
                // producing a document that fails to compile ("unknown function: section").
                if super::template_dialog::body_looks_like_cv(&current_content) {
                    dlg.preselect_cv_mode(true);
                    dlg.preselect_body_kind(super::template_dialog::body_kind_from_key("cv"));
                    // See the identical fallback earlier in this file (the
                    // read-only "current document" path) for why this is needed:
                    // the sidecar/marker path above may have left Style on a
                    // stale or non-CV-meaningful selection.
                    if let Some(cv_style) = super::template_dialog::parse_cv_style(&current_content) {
                        if let Some(idx) = super::template_dialog::cv_style_index(&cv_style) {
                            dlg.preselect_cv_style_index(idx);
                        }
                    }
                }
                // If the user edited the abstract directly in the .typ file, that wins
                // over what the sidecar recorded last time. Override with doc's text.
                if let Some(doc_abstract) = super::template_dialog::parse_abstract_from_doc(&current_content) {
                    dlg.override_abstract_text(&doc_abstract);
                }
                // Always read metadata from the document — the user may have edited the
                // #let doc-* variables directly, and the sidecar won't reflect those changes.
                dlg.preselect_metadata(
                    &super::template_dialog::parse_meta(&current_content, "title"),
                    &super::template_dialog::parse_meta(&current_content, "subtitle"),
                    &super::template_dialog::parse_meta(&current_content, "author"),
                    &super::template_dialog::parse_meta(&current_content, "affiliation"),
                    &super::template_dialog::parse_meta(&current_content, "course"),
                    &super::template_dialog::parse_meta(&current_content, "professor"),
                    &super::template_dialog::parse_meta(&current_content, "date"),
                );

                let ep2 = ep_ut.clone();
                let win_ut2 = win_ut.clone();
                let preview_ut2 = preview_ut.clone();
                let current_content_for_apply = current_content.clone();
                let current_path_for_apply = current_path.clone();
                dlg.set_on_apply(move |new_content, sidecar| {
                    apply_template_result(
                        &win_ut2,
                        &ep2,
                        &preview_ut2,
                        current_path_for_apply.clone(),
                        current_content_for_apply.clone(),
                        new_content,
                        sidecar,
                    );
                });
                dlg.present();
            });
        }

        let left_box = GtkBox::new(Orientation::Vertical, 0);
        left_box.set_hexpand(false);
        left_box.set_vexpand(true);
        left_box.set_overflow(gtk4::Overflow::Hidden);
        left_box.add_css_class("zerkalo-sidebar");
        left_box.append(&sidebar_toolbar);
        left_box.append(&Separator::new(Orientation::Horizontal));
        left_box.append(outline_panel.widget());
        left_box.append(&Separator::new(Orientation::Horizontal));
        left_box.append(citation_panel.widget());
        *left_paned_holder.borrow_mut() = Some(left_box.clone());

        // ── Right sidebar (Plan + Notes tabs) ────────────────────────────────
        let right_sidebar = GtkBox::new(Orientation::Vertical, 0);
        right_sidebar.set_width_request(260);
        right_sidebar.set_vexpand(true);

        let right_notebook = gtk4::Notebook::new();
        right_notebook.set_vexpand(true);
        right_notebook.set_tab_pos(gtk4::PositionType::Top);

        todo_panel.widget().set_vexpand(true);
        right_notebook.append_page(
            todo_panel.widget(),
            Some(&Label::new(Some("Plan"))),
        );

        notes_panel.widget().set_vexpand(true);
        right_notebook.append_page(
            notes_panel.widget(),
            Some(&Label::new(Some("Notes"))),
        );

        right_sidebar.append(&right_notebook);
        *right_sidebar_holder.borrow_mut() = Some(right_sidebar.clone());
        right_sidebar.set_visible(todo_btn.is_active());

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
        content_paned.set_end_child(Some(&right_sidebar));

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

        // ── Persist pane positions (debounced, 400 ms after last drag) ────────
        // Use a flag so we ignore position-notify during initial GTK layout.
        {
            let cfg = current_config.clone();
            let ready = Rc::new(std::cell::Cell::new(false));
            let ready2 = ready.clone();
            outer_paned.connect_realize(move |_| {
                let r = ready2.clone();
                glib::idle_add_local_once(move || { r.set(true); });
            });
            let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
            outer_paned.connect_position_notify(move |p| {
                if !ready.get() { return; }
                let pos = p.position();
                let cfg2 = cfg.clone();
                let pending_for_cb = pending.clone();
                let mut slot = pending.borrow_mut();
                if let Some(id) = slot.take() { id.remove(); }
                *slot = Some(glib::timeout_add_local_once(
                    std::time::Duration::from_millis(400),
                    move || {
                        *pending_for_cb.borrow_mut() = None;
                        let mut c = cfg2.borrow_mut();
                        c.sidebar_width = pos;
                        let _ = c.save();
                    },
                ));
            });
        }
        {
            let cfg = current_config.clone();
            let ready = Rc::new(std::cell::Cell::new(false));
            let ready2 = ready.clone();
            inner_paned.connect_realize(move |_| {
                let r = ready2.clone();
                glib::idle_add_local_once(move || { r.set(true); });
            });
            let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
            inner_paned.connect_position_notify(move |p| {
                if !ready.get() { return; }
                let pos = p.position();
                let cfg2 = cfg.clone();
                let pending_for_cb = pending.clone();
                let mut slot = pending.borrow_mut();
                if let Some(id) = slot.take() { id.remove(); }
                *slot = Some(glib::timeout_add_local_once(
                    std::time::Duration::from_millis(400),
                    move || {
                        *pending_for_cb.borrow_mut() = None;
                        let mut c = cfg2.borrow_mut();
                        c.preview_split = pos;
                        let _ = c.save();
                    },
                ));
            });
        }

        // The status bar spans the whole window, under the sidebar as well as
        // the editor — it reports on the document, not on one pane. (It used to
        // live inside the editor column, so it stopped at the sidebar edge.)
        let main_content = GtkBox::new(Orientation::Vertical, 0);
        main_content.set_hexpand(true);
        main_content.set_vexpand(true);
        main_content.append(&outer_paned);
        main_content.append(&Separator::new(Orientation::Horizontal));
        main_content.append(editor_pane.status_bar_widget());

        toast_for_sync_btn.set_child(Some(&main_content));

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.add_bottom_bar(&compile_rev);
        toolbar_view.set_content(Some(&toast_for_sync_btn));

        window.set_content(Some(&toolbar_view));

        // ── File-system watcher for external .typ changes ───────────────────
        // Fires when a .typ file in the project is written by an external tool
        // (e.g., a sync agent, another editor) so the preview stays current.
        let preview_for_watch = preview_pane.clone();
        let editor_for_watch = editor_pane.clone();
        let mco_for_watch = manual_compile_only.clone();
        let library_for_watch = library.clone();
        let lw_for_watch = library_window.clone();
        let file_watcher = crate::file_watcher::start(
            project_root.clone(),
            move |changed_path| {
                library_for_watch
                    .borrow_mut()
                    .upsert_document(&changed_path)
                    .ok();
                if lw_for_watch.window().is_visible() {
                    lw_for_watch.refresh();
                }
                // Only react to files we don't have open — those are handled by
                // the editor's own save path.
                let is_open = editor_for_watch.is_file_open(&changed_path);
                if !is_open && !*mco_for_watch.borrow() {
                    preview_for_watch.trigger_compile();
                }
            },
        );

        Self {
            window,
            editor_pane,
            preview_pane,
            error_panel,
            outline_panel,
            project_root,
            sync_btn,
            search_panel,
            toast_overlay: toast_for_sync_btn,
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
            menu_import_item,
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
        let compile_btn_for_key = self.compile_btn.clone();
        let library_window_for_key = self.library_window.clone();
        let library_for_key = self.library.clone();
        let controller = gtk4::EventControllerKey::new();

        // ── Command palette (Ctrl+P) ────────────────────────────────────────
        let palette = Rc::new(CommandPalette::new(&self.window));
        {
            let editor_for_pal = self.editor_pane.clone();
            let window_for_pal = self.window.clone();
            let search_for_pal = self.search_panel.clone();
            let preview_for_pal = self.preview_pane.clone();
            let root_for_pal = self.project_root.clone();
            palette.set_on_activate(move |id| {
                let w = window_for_pal.clone();
                if id.starts_with("heading:") {
                    let rest = &id["heading:".len()..];
                    if let Some(colon) = rest.find(':') {
                        let line_str = &rest[..colon];
                        let path_str = &rest[colon + 1..];
                        if let Ok(line) = line_str.parse::<u32>() {
                            let path = std::path::PathBuf::from(path_str);
                            editor_for_pal.jump_to_line(&path, line);
                        }
                    }
                } else if id.starts_with("file:") {
                    let path = std::path::PathBuf::from(&id["file:".len()..]);
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        editor_for_pal.open_file(path, &content);
                    }
                } else {
                    match id {
                        "toggle_find"    => editor_for_pal.toggle_find(),
                        "save"           => { editor_for_pal.save_all_modified(); }
                        "help"           => { HelpWindow::new(&w, editor_for_pal.is_cv_mode()).present(); }
                        "find_in_files"  => { search_for_pal.toggle(); }
                        "project_outline" => {
                            if let (Some(content), Some(path)) = (
                                editor_for_pal.get_active_content(),
                                editor_for_pal.get_active_path(),
                            ) {
                                let items = super::command_palette::heading_items(&content, &path);
                                // Return early — caller will set items; here we can't re-open
                                // the palette from inside its own callback, so we just no-op
                                // if already showing headings. The Ctrl+G shortcut covers this.
                                let _ = items;
                            }
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
        let window_for_paste_key = self.window.clone();
        let editor_for_paste_key = self.editor_pane.clone();
        let work_dir_for_paste_key = self.project_root.clone();
        let toast_overlay_for_paste_key = self.toast_overlay.clone();

        controller.connect_key_pressed(move |_, key, _, modifier| {
            use gtk4::gdk::ModifierType;
            let ctrl = modifier.contains(ModifierType::CONTROL_MASK);
            let shift = modifier.contains(ModifierType::SHIFT_MASK);
            let alt = modifier.contains(ModifierType::ALT_MASK);

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
            // Ctrl+Shift+I — open the Import picker
            {
                use gtk4::gdk::Key;
                if ctrl && shift && !alt && key == Key::i {
                    menu_import_item_for_key.emit_clicked();
                    return glib::Propagation::Stop;
                }
            }
            // Ctrl+Shift+V — Paste as Document
            {
                use gtk4::gdk::Key;
                if ctrl && shift && !alt && key == Key::v {
                    // AppWindow doesn't keep the shared `Rc<RefCell<Config>>` from
                    // `new()` around as a field, so load a fresh copy here — any
                    // setting relevant to import (e.g. bib_path) is written to
                    // disk immediately when changed, so this stays current.
                    let cfg = Rc::new(RefCell::new(Config::load().unwrap_or_default()));
                    paste_as_document(
                        &window_for_paste_key, &editor_for_paste_key, &work_dir_for_paste_key,
                        &cfg, &toast_overlay_for_paste_key,
                    );
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

            // Ctrl+P — print (compile PDF and open in viewer)
            {
                use gtk4::gdk::Key;
                if ctrl && !shift && !alt && key == Key::p {
                    print_pdf_from_preview(&preview);
                    return glib::Propagation::Stop;
                }
            }

            // Ctrl+Shift+E — export PDF to document directory (no dialog)
            {
                use gtk4::gdk::Key;
                if ctrl && shift && key == Key::e {
                    editor.save_all_modified();
                    if let Some(root_path) = preview.root_file_path() {
                        let dest = root_path.with_extension("pdf");
                        let t = adw::Toast::new("Exporting PDF…");
                        t.set_timeout(2);
                        toast_for_key.add_toast(t);
                        let (tx, rx) = std::sync::mpsc::sync_channel::<Result<Vec<u8>, String>>(1);
                        let root_for_thread = root_path.clone();
                        std::thread::spawn(move || {
                            let result = crate::compiler::compile_to_pdf_bytes(
                                &root_for_thread,
                                &std::collections::HashMap::new(),
                                &std::collections::HashMap::new(),
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

        let writing_log_for_close = self.writing_log.clone();
        let file_start_words_for_close = self.file_start_words.clone();
        let session_start_for_close = self.session_start.clone();

        self.window.connect_close_request(move |_| {
            // Second call after user confirmed — save session and proceed
            if *force_close.borrow() {
                record_writing_session(
                    &ep, &writing_log_for_close,
                    &file_start_words_for_close, &session_start_for_close,
                );
                let open_files = ep.get_open_paths_ordered();
                let active_file = ep.get_active_path();
                let cursor_positions = ep.get_cursor_positions();
                Session { open_files, active_file, cursor_positions }.save();
                return glib::Propagation::Proceed;
            }

            let unsaved = ep.modified_buffers();
            if unsaved.is_empty() {
                record_writing_session(
                    &ep, &writing_log_for_close,
                    &file_start_words_for_close, &session_start_for_close,
                );
                let open_files = ep.get_open_paths_ordered();
                let active_file = ep.get_active_path();
                let cursor_positions = ep.get_cursor_positions();
                Session { open_files, active_file, cursor_positions }.save();
                return glib::Propagation::Proceed;
            }

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

            glib::Propagation::Stop
        });

        self.window.present();
    }
}

fn font_defaults(cfg: &Rc<RefCell<Config>>) -> (String, String) {
    let c = cfg.borrow();
    (c.default_sans_font.clone(), c.default_serif_font.clone())
}

fn make_font_save_cb(cfg: Rc<RefCell<Config>>) -> impl Fn(String, String) + 'static {
    move |sans: String, serif: String| {
        let mut c = cfg.borrow_mut();
        c.default_sans_font = sans;
        c.default_serif_font = serif;
        let _ = c.save();
    }
}

fn compile_mode_label_str(auto: bool, cos: bool, mco: bool) -> &'static str {
    if mco { "manual" } else if auto { "auto" } else if cos { "on save" } else { "on save" }
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

fn do_sync(
    root: PathBuf,
    window: adw::ApplicationWindow,
    overlay: adw::ToastOverlay,
    btn: Button,
    token: Option<String>,
    current_config: Rc<RefCell<Config>>,
) {
    use std::sync::mpsc::TryRecvError;

    btn.set_sensitive(false);

    let root_for_thread = root.clone();
    let (tx, rx) = std::sync::mpsc::sync_channel::<git_sync::SyncResult>(1);
    std::thread::spawn(move || {
        tx.send(git_sync::sync(&root_for_thread, token.as_deref())).ok();
    });

    let rx = Rc::new(rx);
    glib::timeout_add_local(Duration::from_millis(100), move || match rx.try_recv() {
        Ok(result) => {
            btn.set_sensitive(true);
            show_sync_result(&window, &overlay, result, root.clone(), current_config.clone());
            glib::ControlFlow::Break
        }
        Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(TryRecvError::Disconnected) => {
            btn.set_sensitive(true);
            glib::ControlFlow::Break
        }
    });
}

fn show_sync_result(
    window: &adw::ApplicationWindow,
    overlay: &adw::ToastOverlay,
    result: git_sync::SyncResult,
    root: PathBuf,
    current_config: Rc<RefCell<Config>>,
) {
    if let Some(err) = result.error {
        show_alert(window, "Sync Failed", &err);
        return;
    }
    if !result.push_errors.is_empty() {
        let detail = result.push_errors.join("\n");
        if result.auth_failed {
            show_github_token_dialog(
                window,
                overlay,
                root,
                current_config,
                "GitHub authentication failed. Enter a Personal Access Token (PAT) to continue.\n\nGenerate one at github.com → Settings → Developer settings → Personal access tokens.",
            );
            return;
        }
        let is_conflict = detail.contains("CONFLICT") || detail.contains("Pull failed");
        if result.pushed {
            let summary = result.commit_message.lines().next().unwrap_or("Synced").to_string();
            overlay.add_toast(adw::Toast::new(&format!("Synced — {summary}")));
            show_alert(window, "Some remotes failed", &detail);
        } else if is_conflict {
            show_alert(
                window,
                "Merge conflict — sync aborted",
                "Remote changes conflict with your local edits. Your work is safe and unchanged.\n\nResolve the conflict by editing the file manually or force-pushing from the command line.",
            );
        } else {
            show_alert(window, "Push Failed", &detail);
        }
        return;
    }
    if result.pushed {
        let summary = result.commit_message.lines().next().unwrap_or("Synced").to_string();
        overlay.add_toast(adw::Toast::new(&format!("Synced — {summary}")));
    } else if result.committed {
        overlay.add_toast(adw::Toast::new("Committed locally — no remote push"));
    } else {
        overlay.add_toast(adw::Toast::new("Nothing to sync"));
    }
}

fn show_github_token_dialog(
    window: &adw::ApplicationWindow,
    overlay: &adw::ToastOverlay,
    root: PathBuf,
    current_config: Rc<RefCell<Config>>,
    message: &str,
) {
    let dialog = adw::Window::builder()
        .title("GitHub Login")
        .transient_for(window)
        .modal(true)
        .default_width(480)
        .default_height(300)
        .build();

    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);

    let label = gtk4::Label::new(Some(message));
    label.set_wrap(true);
    label.set_margin_top(12);
    label.set_margin_bottom(8);
    label.set_margin_start(16);
    label.set_margin_end(16);
    label.set_xalign(0.0);

    let entry = gtk4::Entry::new();
    entry.set_placeholder_text(Some("ghp_xxxxxxxxxxxxxxxxxxxx"));
    entry.set_visibility(false);
    entry.set_margin_start(16);
    entry.set_margin_end(16);
    entry.set_margin_bottom(12);

    let hint = gtk4::Label::new(Some("Your token is stored locally and never shared."));
    hint.add_css_class("caption");
    hint.add_css_class("dim-label");
    hint.set_margin_start(16);
    hint.set_margin_end(16);
    hint.set_margin_bottom(16);
    hint.set_xalign(0.0);

    let save_btn = Button::with_label("Save & Sync");
    save_btn.add_css_class("suggested-action");
    save_btn.set_margin_start(16);
    save_btn.set_margin_end(16);
    save_btn.set_margin_bottom(16);

    let cancel_btn = Button::with_label("Cancel");
    cancel_btn.add_css_class("flat");
    header.pack_start(&cancel_btn);

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    vbox.append(&header);
    vbox.append(&label);
    vbox.append(&entry);
    vbox.append(&hint);
    vbox.append(&save_btn);
    dialog.set_content(Some(&vbox));

    let dialog_cancel = dialog.clone();
    cancel_btn.connect_clicked(move |_| dialog_cancel.close());

    let dialog_save = dialog.clone();
    let entry_save = entry.clone();
    let overlay_retry = overlay.clone();
    let window_retry = window.clone();
    save_btn.connect_clicked(move |btn| {
        let tok = entry_save.text().to_string();
        if tok.is_empty() { return; }

        let _ = crate::secret_store::save_github_token(&tok);

        btn.set_sensitive(false);
        dialog_save.close();

        // Auto-retry the sync with the new token — no need to click again.
        let root_thread = root.clone();
        let root_result = root.clone();
        let win2 = window_retry.clone();
        let ov2 = overlay_retry.clone();
        let cfg2 = current_config.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel::<git_sync::SyncResult>(1);
        std::thread::spawn(move || { tx.send(git_sync::sync(&root_thread, Some(&tok))).ok(); });
        let rx = Rc::new(rx);
        glib::timeout_add_local(Duration::from_millis(100), move || {
            use std::sync::mpsc::TryRecvError;
            match rx.try_recv() {
                Ok(result) => {
                    show_sync_result(&win2, &ov2, result, root_result.clone(), cfg2.clone());
                    glib::ControlFlow::Break
                }
                Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(TryRecvError::Disconnected) => glib::ControlFlow::Break,
            }
        });
    });

    dialog.present();
}

fn show_backup_remote_dialog(window: &adw::ApplicationWindow, repo_path: &std::path::Path) {
    let dialog = adw::Window::builder()
        .title("Git Remotes")
        .transient_for(window)
        .modal(true)
        .default_width(520)
        .default_height(600)
        .build();

    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    let close_btn = Button::with_label("Close");
    close_btn.add_css_class("flat");
    header.pack_start(&close_btn);

    let page = adw::PreferencesPage::new();

    // ── Primary remote (origin / GitHub) ─────────────────────────────────────
    let origin_group = adw::PreferencesGroup::new();
    origin_group.set_title("Primary Remote");
    origin_group.set_description(Some(
        "Every sync pushes here first. Paste a GitHub HTTPS URL.",
    ));

    let origin_entry = adw::EntryRow::new();
    origin_entry.set_title("URL");
    if let Some(url) = git_sync::get_remote_url(repo_path, "origin") {
        origin_entry.set_text(&url);
    }

    let origin_status = Label::new(None);
    origin_status.set_xalign(0.0);
    origin_status.set_margin_top(4);
    origin_status.add_css_class("dim-label");

    let origin_apply = Button::with_label("Apply");
    origin_apply.add_css_class("suggested-action");
    origin_apply.set_halign(Align::End);
    {
        let entry = origin_entry.clone();
        let lbl = origin_status.clone();
        let root = repo_path.to_path_buf();
        origin_apply.connect_clicked(move |_| {
            let url = entry.text().to_string();
            if url.is_empty() {
                lbl.set_label("Enter a URL first.");
                return;
            }
            let _ = git_sync::remove_remote(&root, "origin");
            match git_sync::add_named_remote(&root, "origin", &url) {
                Ok(()) => {
                    lbl.set_label(&format!("✓ Origin set: {url}"));
                    lbl.remove_css_class("error");
                    lbl.add_css_class("success");
                }
                Err(e) => {
                    lbl.set_label(&format!("Error: {e}"));
                    lbl.remove_css_class("success");
                    lbl.add_css_class("error");
                }
            }
        });
    }

    let origin_suffix = GtkBox::new(Orientation::Vertical, 6);
    origin_suffix.set_margin_top(8);
    origin_suffix.set_margin_bottom(4);
    origin_suffix.append(&origin_status);
    origin_suffix.append(&origin_apply);

    origin_group.add(&origin_entry);
    origin_group.add(&{
        let row = adw::PreferencesRow::new();
        row.set_child(Some(&origin_suffix));
        row
    });
    page.add(&origin_group);

    // ── Additional remotes ────────────────────────────────────────────────────
    let current_group = adw::PreferencesGroup::new();
    current_group.set_title("Additional Remotes");

    let root_for_rebuild = repo_path.to_path_buf();
    // Track only the rows we explicitly added so we can safely remove them
    // without touching PreferencesGroup's internal header widgets.
    let tracked_rows: Rc<RefCell<Vec<adw::ActionRow>>> = Rc::new(RefCell::new(Vec::new()));

    let rebuild_current = {
        let group = current_group.clone();
        let root = root_for_rebuild.clone();
        let tracked = tracked_rows.clone();
        move || {
            for row in tracked.borrow().iter() {
                group.remove(row);
            }
            tracked.borrow_mut().clear();

            let remotes = git_sync::list_backup_remotes(&root);
            if remotes.is_empty() {
                let row = adw::ActionRow::new();
                row.set_title("No backup remotes configured");
                row.add_css_class("dim-label");
                group.add(&row);
                tracked.borrow_mut().push(row);
            } else {
                for (name, url) in remotes {
                    let row = adw::ActionRow::new();
                    row.set_title(&name);
                    row.set_subtitle(&url);
                    let rm_btn = Button::from_icon_name("user-trash-symbolic");
                    rm_btn.add_css_class("flat");
                    rm_btn.add_css_class("destructive-action");
                    rm_btn.set_valign(Align::Center);
                    rm_btn.set_tooltip_text(Some("Remove this backup remote"));
                    let root2 = root.clone();
                    let tracked2 = tracked.clone();
                    let group2 = group.clone();
                    rm_btn.connect_clicked(move |_| {
                        let _ = git_sync::remove_remote(&root2, &name);
                        for r in tracked2.borrow().iter() { group2.remove(r); }
                        tracked2.borrow_mut().clear();
                        let remotes2 = git_sync::list_backup_remotes(&root2);
                        if remotes2.is_empty() {
                            let ph = adw::ActionRow::new();
                            ph.set_title("No backup remotes configured");
                            ph.add_css_class("dim-label");
                            group2.add(&ph);
                            tracked2.borrow_mut().push(ph);
                        } else {
                            for (n, u) in remotes2 {
                                let r = adw::ActionRow::new();
                                r.set_title(&n);
                                r.set_subtitle(&u);
                                group2.add(&r);
                                tracked2.borrow_mut().push(r);
                            }
                        }
                    });
                    row.add_suffix(&rm_btn);
                    group.add(&row);
                    tracked.borrow_mut().push(row);
                }
            }
        }
    };
    let rebuild_current = Rc::new(rebuild_current);
    rebuild_current();
    page.add(&current_group);

    // ── Add a new backup remote ───────────────────────────────────────────────
    let add_group = adw::PreferencesGroup::new();
    add_group.set_title("Add a Backup Remote");
    add_group.set_description(Some(
        "Sync pushes here in addition to the primary remote. Enter a name and a URL or local path.",
    ));

    let name_row = adw::EntryRow::new();
    name_row.set_title("Remote name");
    name_row.set_text("backup");

    let url_row = adw::EntryRow::new();
    url_row.set_title("URL or path");

    // Folder-picker button
    let pick_btn = Button::from_icon_name("document-open-symbolic");
    pick_btn.set_valign(Align::Center);
    pick_btn.add_css_class("flat");
    pick_btn.set_tooltip_text(Some("Browse for a local folder"));
    {
        let row_c = url_row.clone();
        let win_c = window.clone();
        pick_btn.connect_clicked(move |_| {
            let fd = gtk4::FileDialog::new();
            let row2 = row_c.clone();
            fd.select_folder(Some(&win_c), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        row2.set_text(path.to_str().unwrap_or(""));
                    }
                }
            });
        });
    }
    url_row.add_suffix(&pick_btn);

    let status_lbl = Label::new(None);
    status_lbl.set_xalign(0.0);
    status_lbl.set_margin_top(4);
    status_lbl.add_css_class("dim-label");

    let add_btn = Button::with_label("Add Remote");
    add_btn.add_css_class("suggested-action");
    add_btn.set_halign(Align::End);

    let btn_box = gtk4::Box::new(Orientation::Vertical, 6);
    btn_box.set_margin_top(8);
    btn_box.set_margin_bottom(4);
    btn_box.append(&status_lbl);
    btn_box.append(&add_btn);
    let btn_wrapper = adw::ActionRow::new();
    btn_wrapper.set_activatable(false);
    btn_wrapper.add_suffix(&btn_box);

    add_group.add(&name_row);
    add_group.add(&url_row);
    add_group.add(&btn_wrapper);
    page.add(&add_group);

    {
        let root_c = repo_path.to_path_buf();
        let lbl_c = status_lbl.clone();
        let name_r = name_row.clone();
        let url_r = url_row.clone();
        let rebuild_c = rebuild_current.clone();
        add_btn.connect_clicked(move |_| {
            let name = name_r.text().trim().to_string();
            let url  = url_r.text().trim().to_string();
            if name.is_empty() || url.is_empty() {
                lbl_c.set_text("Enter both a name and a URL.");
                return;
            }
            if name == "origin" {
                lbl_c.set_text("\"origin\" is reserved for the primary remote.");
                return;
            }
            match git_sync::add_named_remote(&root_c, &name, &url) {
                Ok(()) => {
                    lbl_c.set_text(&format!("✓ Added «{name}»"));
                    url_r.set_text("");
                    rebuild_c();
                }
                Err(e) => lbl_c.set_text(&format!("Error: {e}")),
            }
        });
    }

    // ── Disroot: privacy-respecting git hosting ───────────────────────────────
    let disroot_group = adw::PreferencesGroup::new();
    disroot_group.set_title("Disroot (git.disroot.org)");
    disroot_group.set_description(Some(
        "Disroot is a non-profit, privacy-respecting community hosting Gitea at \
         git.disroot.org. Free to use. Good for a second off-site copy of your work.",
    ));
    for (title, subtitle) in [
        ("1. Create account", "Register at https://disroot.org/en/register"),
        ("2. Create repository", "Log in to git.disroot.org → New repository"),
        ("3. Copy the clone URL", "Use HTTPS or SSH — shown on the repo page"),
        ("4. Add it below", "Name it \"disroot\", paste the URL above, click Add"),
    ] {
        let row = adw::ActionRow::new();
        row.set_title(title);
        row.set_subtitle(subtitle);
        disroot_group.add(&row);
    }
    // Quick-fill button for Disroot
    let disroot_fill_btn = Button::with_label("Set name to \"disroot\"");
    disroot_fill_btn.add_css_class("flat");
    disroot_fill_btn.set_halign(Align::Start);
    disroot_fill_btn.set_margin_top(4);
    {
        let nr = name_row.clone();
        disroot_fill_btn.connect_clicked(move |_| nr.set_text("disroot"));
    }
    disroot_group.add(&adw::ActionRow::new()); // spacer
    // Can't add a plain Button to PreferencesGroup, so wrap in ActionRow suffix
    let fill_row = adw::ActionRow::new();
    fill_row.set_title("Quick-fill name");
    fill_row.set_activatable(true);
    let nr2 = name_row.clone();
    fill_row.connect_activated(move |_| nr2.set_text("disroot"));
    fill_row.add_suffix(&Button::from_icon_name("go-next-symbolic"));
    // Re-use disroot_fill_btn logic via action row activation
    disroot_group.add(&fill_row);
    page.add(&disroot_group);

    // ── Examples ─────────────────────────────────────────────────────────────
    let hint_group = adw::PreferencesGroup::new();
    hint_group.set_title("Other URL Examples");
    for (name, hint) in [
        ("Local / NAS", "/mnt/backup/my-project  or  /run/media/you/usb/project"),
        ("pCloud / Nextcloud", "Mount the drive, then use the mount path above"),
        ("Codeberg", "git@codeberg.org:username/project.git"),
        ("GitLab", "git@gitlab.com:username/project.git"),
        ("Self-hosted Gitea", "git@my-server.example.com:username/project.git"),
    ] {
        let row = adw::ActionRow::new();
        row.set_title(name);
        row.set_subtitle(hint);
        hint_group.add(&row);
    }
    page.add(&hint_group);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));
    dialog.set_content(Some(&toolbar));

    let dlg_close = dialog.clone();
    close_btn.connect_clicked(move |_| dlg_close.close());

    dialog.present();
}

fn show_alert(window: &adw::ApplicationWindow, title: &str, body: &str) {
    let dlg = adw::MessageDialog::new(Some(window), Some(title), Some(body));
    dlg.add_response("ok", "OK");
    dlg.present();
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
         Git & App\n\
         \u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\u{2014}\n\
         Git Sync            {git_sync}\n\
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

/// Post-process a pandoc-converted Typst file:
///   1. Insert `#pagebreak()` between the title block and the body
///      (just before the first top-level `= Heading`).
///   2. Insert `#pagebreak()` before the `#bibliography(...)` call.
///   3. Fix the bibliography path to the configured `.bib` file if supplied;
///      add a commented-out bibliography stub if none exists.
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
    if content.starts_with("---\n") {
        let rest = &content[4..];
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
                if after.starts_with('"') {
                    if let Some(end) = after[1..].find('"') {
                        let title = after[1..end + 1].to_string();
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
fn strip_pandoc_preamble(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let n = lines.len();
    let mut i = 0;
    while i < n {
        let t = lines[i].trim();
        if t.is_empty() || t.starts_with("//") {
            i += 1;
            continue;
        }
        // Strip #set rules (paren-depth aware for multi-line blocks)
        if t.starts_with("#set ") {
            let mut depth: i32 = 0;
            loop {
                for c in lines[i].chars() {
                    match c { '(' => depth += 1, ')' => depth -= 1, _ => {} }
                }
                i += 1;
                if depth <= 0 || i >= n { break; }
            }
            continue;
        }
        // Strip #show rules (bracket-depth aware) — pandoc emits #show heading: etc.
        if t.starts_with("#show ") {
            let mut depth: i32 = 0;
            loop {
                depth += lines[i].chars().filter(|&c| c == '[').count() as i32;
                depth -= lines[i].chars().filter(|&c| c == ']').count() as i32;
                i += 1;
                if depth <= 0 || i >= n { break; }
            }
            continue;
        }
        // Strip standalone #import / #let lines in the preamble region.
        if t.starts_with("#import ") || t.starts_with("#let ") {
            i += 1;
            continue;
        }
        break;
    }
    // Trim leading blank lines before actual content
    while i < n && lines[i].trim().is_empty() { i += 1; }
    if i >= n { return String::new(); }
    let result = lines[i..].join("\n");
    if result.ends_with('\n') { result } else { result + "\n" }
}

// ── Document import via pandoc (LaTeX, Word, Markdown, OpenDocument Text) ──────

struct ImportFormat {
    label: &'static str,
    icon: &'static str,
    /// File-glob patterns, e.g. `&["*.html", "*.htm"]`.
    patterns: &'static [&'static str],
    /// Bare extensions (no dot), used to match dropped files — kept separate
    /// from `patterns` since drop-matching compares against `Path::extension()`.
    extensions: &'static [&'static str],
    filter_name: &'static str,
    pandoc_from: &'static str,
}

const IMPORT_FORMATS: &[ImportFormat] = &[
    ImportFormat {
        label: "LaTeX (.tex)",
        icon: "text-x-generic-symbolic",
        patterns: &["*.tex"],
        extensions: &["tex"],
        filter_name: "LaTeX files (*.tex)",
        pandoc_from: "latex",
    },
    ImportFormat {
        label: "Word (.docx)",
        icon: "x-office-document-symbolic",
        patterns: &["*.docx"],
        extensions: &["docx"],
        filter_name: "Word documents (*.docx)",
        pandoc_from: "docx",
    },
    ImportFormat {
        label: "Markdown (.md)",
        icon: "text-x-generic-symbolic",
        patterns: &["*.md", "*.markdown"],
        extensions: &["md", "markdown"],
        filter_name: "Markdown files (*.md)",
        pandoc_from: "markdown",
    },
    ImportFormat {
        label: "OpenDocument Text (.odt)",
        icon: "x-office-document-symbolic",
        patterns: &["*.odt"],
        extensions: &["odt"],
        filter_name: "OpenDocument Text (*.odt)",
        pandoc_from: "odt",
    },
    ImportFormat {
        label: "HTML (.html)",
        icon: "text-x-generic-symbolic",
        patterns: &["*.html", "*.htm"],
        extensions: &["html", "htm"],
        filter_name: "HTML files (*.html)",
        pandoc_from: "html",
    },
    ImportFormat {
        label: "EPUB (.epub)",
        icon: "x-office-document-symbolic",
        patterns: &["*.epub"],
        extensions: &["epub"],
        filter_name: "EPUB files (*.epub)",
        pandoc_from: "epub",
    },
    ImportFormat {
        label: "Rich Text (.rtf)",
        icon: "x-office-document-symbolic",
        patterns: &["*.rtf"],
        extensions: &["rtf"],
        filter_name: "Rich Text files (*.rtf)",
        pandoc_from: "rtf",
    },
];

/// Read-only list of past import attempts (`ImportLog`), reached from the
/// history icon in the Import picker dialog.
/// Find the `ImportFormat` a history record's stored label refers to, so
/// "Retry" can re-run the same pipeline without the user re-picking a format.
fn find_import_format_by_label(label: &str) -> Option<&'static ImportFormat> {
    IMPORT_FORMATS.iter().find(|f| f.label == label)
}

#[allow(clippy::too_many_arguments)]
fn show_import_history_dialog(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    work_dir: &std::path::Path,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
) {
    show_import_history_dialog_filtered(window, editor, work_dir, cfg, toast_overlay, false);
}

#[allow(clippy::too_many_arguments)]
fn show_import_history_dialog_filtered(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    work_dir: &std::path::Path,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
    initial_failed_only: bool,
) {
    let log = crate::import_log::ImportLog::load();

    let dlg = adw::Window::new();
    dlg.set_title(Some("Import History"));
    dlg.set_default_size(480, 460);
    dlg.set_transient_for(Some(window));
    dlg.set_modal(true);

    let header = adw::HeaderBar::new();
    let title_lbl = gtk4::Label::new(Some("Import History"));
    title_lbl.add_css_class("heading");
    header.set_title_widget(Some(&title_lbl));

    if log.records.is_empty() {
        let empty = adw::StatusPage::new();
        empty.set_icon_name(Some("document-open-recent-symbolic"));
        empty.set_title("No Imports Yet");
        empty.set_description(Some("Documents you import will be listed here."));
        empty.set_vexpand(true);
        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&empty));
        dlg.set_content(Some(&toolbar_view));
        dlg.present();
        return;
    }

    let clear_btn = Button::from_icon_name("user-trash-symbolic");
    clear_btn.add_css_class("flat");
    clear_btn.set_tooltip_text(Some("Clear History"));
    header.pack_end(&clear_btn);
    {
        let win_c = window.clone();
        let ep_c = editor.clone();
        let work_dir_c = work_dir.to_path_buf();
        let cfg_c = cfg.clone();
        let toast_c = toast_overlay.clone();
        let dlg_c = dlg.clone();
        clear_btn.connect_clicked(move |_| {
            let mut log = crate::import_log::ImportLog::load();
            log.clear();
            dlg_c.close();
            show_import_history_dialog(&win_c, &ep_c, &work_dir_c, &cfg_c, &toast_c);
        });
    }

    let failed_only_btn = gtk4::ToggleButton::new();
    failed_only_btn.set_icon_name("dialog-warning-symbolic");
    failed_only_btn.set_tooltip_text(Some("Show only failed imports"));
    failed_only_btn.add_css_class("flat");
    failed_only_btn.set_active(initial_failed_only);
    header.pack_end(&failed_only_btn);

    let search_entry = gtk4::SearchEntry::new();
    search_entry.set_placeholder_text(Some("Filter by filename, format, or message…"));
    search_entry.set_margin_start(12);
    search_entry.set_margin_end(12);
    search_entry.set_margin_top(8);

    let outer_box = GtkBox::new(Orientation::Vertical, 0);
    outer_box.append(&search_entry);

    let list_box = gtk4::ListBox::new();
    list_box.add_css_class("boxed-list");
    list_box.set_selection_mode(gtk4::SelectionMode::None);

    let total = log.records.len();
    for (display_idx, record) in log.records.iter().rev().enumerate() {
        let record_idx = total - 1 - display_idx;
        let row = adw::ActionRow::new();
        let name = record.source.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        row.set_title(name);
        row.set_subtitle(&format!("{} · {} · {}", record.date, record.format, record.message));
        row.set_widget_name(&format!(
            "{}|{} {} {}",
            if record.success { "ok" } else { "fail" },
            name, record.format, record.message
        ).to_lowercase());

        let prefix = if record.success {
            let img = gtk4::Image::from_icon_name("emblem-ok-symbolic");
            img.add_css_class("success");
            img
        } else {
            let img = gtk4::Image::from_icon_name("dialog-warning-symbolic");
            img.add_css_class("warning");
            img
        };
        row.add_prefix(&prefix);

        if let Some(output) = &record.output {
            if output.exists() {
                let reveal_btn = Button::from_icon_name("folder-open-symbolic");
                reveal_btn.add_css_class("flat");
                reveal_btn.set_valign(Align::Center);
                reveal_btn.set_tooltip_text(Some("Show containing folder"));
                let output_dir = output.parent().map(|p| p.to_path_buf());
                reveal_btn.connect_clicked(move |_| {
                    if let Some(dir) = &output_dir {
                        let _ = crate::git_sync::host_command("xdg-open").arg(dir).spawn();
                    }
                });
                row.add_suffix(&reveal_btn);
            }
        }

        if !record.success && record.source.exists() {
            let retry_btn = Button::from_icon_name("view-refresh-symbolic");
            retry_btn.add_css_class("flat");
            retry_btn.set_valign(Align::Center);
            retry_btn.set_tooltip_text(Some("Retry"));
            let win_c = window.clone();
            let ep_c = editor.clone();
            let work_dir_c = work_dir.to_path_buf();
            let cfg_c = cfg.clone();
            let toast_c = toast_overlay.clone();
            let dlg_c = dlg.clone();
            let source = record.source.clone();
            let format_label = record.format.clone();
            retry_btn.connect_clicked(move |_| {
                dlg_c.close();
                if format_label == "PDF (.pdf)" {
                    run_pdf_import(&win_c, &ep_c, source.clone());
                } else if let Some(fmt) = find_import_format_by_label(&format_label) {
                    run_pandoc_import(&win_c, &ep_c, &cfg_c, &toast_c, &work_dir_c, source.clone(), fmt);
                }
            });
            row.add_suffix(&retry_btn);
        }

        let delete_btn = Button::from_icon_name("edit-delete-symbolic");
        delete_btn.add_css_class("flat");
        delete_btn.set_valign(Align::Center);
        delete_btn.set_tooltip_text(Some("Remove from history"));
        let win_c = window.clone();
        let ep_c = editor.clone();
        let work_dir_c = work_dir.to_path_buf();
        let cfg_c = cfg.clone();
        let toast_c = toast_overlay.clone();
        let dlg_c = dlg.clone();
        delete_btn.connect_clicked(move |_| {
            let mut log = crate::import_log::ImportLog::load();
            log.remove(record_idx);
            dlg_c.close();
            show_import_history_dialog(&win_c, &ep_c, &work_dir_c, &cfg_c, &toast_c);
        });
        row.add_suffix(&delete_btn);

        list_box.append(&row);
    }

    let search_text: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    {
        let search_text_c = search_text.clone();
        let failed_only_c = failed_only_btn.clone();
        list_box.set_filter_func(move |row| {
            let wn = row.widget_name().to_string();
            let Some((status, text)) = wn.split_once('|') else { return true };
            if failed_only_c.is_active() && status != "fail" {
                return false;
            }
            let query = search_text_c.borrow();
            query.is_empty() || text.contains(query.as_str())
        });
    }
    {
        let lb = list_box.clone();
        let search_text_c = search_text.clone();
        search_entry.connect_search_changed(move |e| {
            *search_text_c.borrow_mut() = e.text().to_lowercase();
            lb.invalidate_filter();
        });
    }
    {
        let lb = list_box.clone();
        failed_only_btn.connect_toggled(move |_| {
            lb.invalidate_filter();
        });
    }

    let scroll = ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_child(Some(&list_box));
    scroll.set_margin_start(12);
    scroll.set_margin_end(12);
    scroll.set_margin_top(8);
    scroll.set_margin_bottom(12);
    outer_box.append(&scroll);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&outer_box));
    dlg.set_content(Some(&toolbar_view));
    dlg.present();
}

/// If `path` already exists, find the next free "`stem` (N).typ" instead of
/// silently overwriting it — mirrors the "Untitled 2.typ" collision-avoidance
/// convention in `library_window.rs::create_new_from_template`.
fn unique_typ_path(path: std::path::PathBuf) -> std::path::PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("output").to_string();
    let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let mut n = 1;
    loop {
        let candidate = dir.join(format!("{stem} ({n}).typ"));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

/// Translate a couple of common pandoc failure signatures into a plain-language
/// message; anything else falls back to the raw stderr (first 5 lines).
fn describe_pandoc_failure(stderr: &str) -> String {
    let lower = stderr.to_lowercase();
    if lower.contains("unknown writer") || lower.contains("unrecognized output format")
        || lower.contains("unknown output format")
    {
        return "Your pandoc version doesn't support Typst output. Zerkalo needs \
                pandoc 3.1 or later — you have an older version installed."
            .to_string();
    }
    format!("pandoc error:\n{}", stderr.lines().take(5).collect::<Vec<_>>().join("\n"))
}

/// Best-effort detection of Zotero/Mendeley/EndNote field codes inside a
/// `.docx`'s `word/document.xml` — these citation managers store citations as
/// proprietary custom-XML field codes that pandoc's docx reader doesn't
/// understand, so such citations silently convert to nothing rather than a
/// Typst `@key`, unlike plain typed citations. Requires `unzip`; if it's
/// missing or the file can't be read, this just reports no signatures found
/// rather than blocking the import on a missing optional tool.
fn docx_has_citation_manager_fields(path: &std::path::Path) -> bool {
    let Ok(output) = crate::git_sync::host_command("unzip")
        .arg("-p").arg(path).arg("word/document.xml")
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let xml = String::from_utf8_lossy(&output.stdout).to_lowercase();
    xml.contains("zotero") || xml.contains("mendeley") || xml.contains("endnote")
}

/// Build the base pandoc invocation for converting `input_name` (a bare
/// filename, relative to `input_dir`) to Typst. `.current_dir()` on the outer
/// Command only moves `flatpak-spawn`'s own cwd inside the sandbox, not the
/// host pandoc process's — flatpak-spawn needs an explicit `--directory=`,
/// the same reason git_sync's `git_cmd` uses `-C <repo>` instead of relying
/// on `.current_dir()`.
fn build_pandoc_command(
    input_dir: &std::path::Path,
    input_name: &str,
    pandoc_from: &str,
    out_name: &str,
    media_name: &str,
) -> std::process::Command {
    let mut cmd = if crate::git_sync::in_flatpak() {
        let mut c = std::process::Command::new("flatpak-spawn");
        c.arg("--host").arg(format!("--directory={}", input_dir.display())).arg("pandoc");
        c
    } else {
        let mut c = std::process::Command::new("pandoc");
        c.current_dir(input_dir);
        c
    };
    cmd.arg(input_name)
        .arg("-f").arg(pandoc_from)
        .arg("-t").arg("typst")
        .arg("--standalone")
        .arg(format!("--extract-media={media_name}"))
        .arg("-o").arg(out_name)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    cmd
}

/// Recursively copy a directory (used to relocate pandoc's `--extract-media`
/// output when the user chooses a different destination than the source's
/// own folder in the import-preview dialog).
fn copy_dir_recursive(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

/// Recursively collect files under `dir` matching any of `extensions`, for
/// "Include subfolders" in batch import. Skips hidden directories (`.git`
/// and similar) and any `*_media` directory — pandoc's own `--extract-media`
/// output, not a source document folder.
fn scan_files_recursive(dir: &std::path::Path, extensions: &[&str], out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if name.starts_with('.') || name.ends_with("_media") {
                continue;
            }
            scan_files_recursive(&path, extensions, out);
        } else if path.extension().and_then(|e| e.to_str())
            .map(|ext| extensions.iter().any(|want| want.eq_ignore_ascii_case(ext)))
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
}

/// First `.bib`/`.yaml`/`.yml` file directly inside `dir`, if any — the same
/// matching rule as the project-root auto-detect at startup (`app_window.rs`,
/// "Auto-detect .bib when no bib is configured"), reused here to offer the
/// same convenience right after importing a document that likely cites one.
fn find_bib_like_file(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    std::fs::read_dir(dir).ok()?.find_map(|e| {
        let path = e.ok()?.path();
        let ext = path.extension().and_then(|x| x.to_str())?;
        if ext.eq_ignore_ascii_case("bib") || ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml") {
            Some(path)
        } else {
            None
        }
    })
}

/// If `cfg.bib_path` isn't already set, look for a bibliography file next to
/// the just-imported source and offer to use it via a toast action.
/// Returns `true` if a bibliography-like file was found next to the import
/// (whether or not a toast ended up being shown for it), so callers can
/// decide whether a *different* nudge (see `warn_if_citations_without_bib`)
/// still applies.
fn offer_bib_autodetect(
    toast_overlay: &adw::ToastOverlay,
    cfg: &Rc<RefCell<Config>>,
    input_dir: &std::path::Path,
) -> bool {
    let Some(bib_path) = find_bib_like_file(input_dir) else { return false };
    if cfg.borrow().bib_path.is_some() {
        return true;
    }
    let name = bib_path.file_name().and_then(|n| n.to_str()).unwrap_or("bibliography file").to_string();
    let toast = adw::Toast::new(&format!("Found {name} — use it as your bibliography?"));
    toast.set_button_label(Some("Set"));
    toast.set_timeout(6);
    let cfg_c = cfg.clone();
    toast.connect_button_clicked(move |_| {
        let mut c = cfg_c.borrow_mut();
        c.bib_path = Some(bib_path.clone());
        let _ = c.save();
    });
    toast_overlay.add_toast(toast);
    true
}

/// Shown after a successful pandoc conversion, before anything is written
/// permanently: a read-only preview of the generated Typst (matching the
/// "Preview Code" window in `template_dialog.rs`) plus a destination choice.
/// "Import" writes the chosen destination and opens it; "Discard" deletes the
/// temporary files pandoc already wrote and does nothing further.
#[allow(clippy::too_many_arguments)]
/// A rough, at-a-glance read of what pandoc produced, shown above the preview
/// text so a user can judge conversion fidelity before committing — especially
/// useful for math-heavy sources, where LaTeX-to-Typst equation syntax
/// sometimes needs manual cleanup that a silent word count wouldn't hint at.
/// Counts probable Typst `@key` citations — an `@` not preceded by a word
/// character and followed by an identifier-starting letter, which excludes
/// email addresses and other incidental `@` uses.
fn count_citations(text: &str) -> usize {
    let bytes = text.as_bytes();
    bytes.iter().enumerate().filter(|(i, &b)| {
        if b != b'@' { return false; }
        let prev_is_wordchar = *i > 0 && (bytes[*i - 1] as char).is_alphanumeric();
        let next_is_ident_start = text[*i + 1..].chars().next().map(|c| c.is_alphabetic()).unwrap_or(false);
        !prev_is_wordchar && next_is_ident_start
    }).count()
}

/// If the converted document cites sources but no bibliography is configured
/// and none was found next to it, nudge the user. This is deliberately just a
/// nudge, not extraction: DOCX/ODT documents with Zotero/Mendeley-managed
/// citations carry that data in proprietary field codes, not something
/// pandoc's CLI can export as a standalone `.bib` file.
fn warn_if_citations_without_bib(
    toast_overlay: &adw::ToastOverlay,
    cfg: &Rc<RefCell<Config>>,
    processed: &str,
    found_nearby_bib: bool,
) {
    if found_nearby_bib || cfg.borrow().bib_path.is_some() {
        return;
    }
    if count_citations(processed) == 0 {
        return;
    }
    let toast = adw::Toast::new(
        "This document cites sources but no bibliography is set. If it used \
         Zotero, Mendeley, or EndNote, export your library to a .bib file and \
         place it alongside this document.",
    );
    toast.set_timeout(8);
    toast_overlay.add_toast(toast);
}

fn summarize_import_content(text: &str) -> String {
    let words = crate::writing_log::count_words(text);
    let headings = text.lines().filter(|l| l.trim_start().starts_with('=')).count();
    let images = text.matches("image(").count();
    let citations = count_citations(text);

    // Rough: Typst inline/block math is `$...$`; count paired delimiters.
    let equations = text.matches('$').count() / 2;

    let mut parts = vec![format!("{words} word{}", if words == 1 { "" } else { "s" })];
    if headings > 0 { parts.push(format!("{headings} heading{}", if headings == 1 { "" } else { "s" })); }
    if images > 0 { parts.push(format!("{images} image{}", if images == 1 { "" } else { "s" })); }
    if citations > 0 { parts.push(format!("{citations} citation{}", if citations == 1 { "" } else { "s" })); }
    if equations > 0 { parts.push(format!("~{equations} equation{} — review math syntax", if equations == 1 { "" } else { "s" })); }
    parts.join(" · ")
}

fn show_import_preview_dialog(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
    input_path: std::path::PathBuf,
    fmt_label: &'static str,
    processed: String,
    temp_out_path: std::path::PathBuf,
    media_name: String,
    work_dir: std::path::PathBuf,
    pandoc_warnings: String,
) {
    let input_dir = input_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let out_name = temp_out_path.file_name().and_then(|s| s.to_str()).unwrap_or("output.typ").to_string();

    let dlg = adw::Window::new();
    dlg.set_title(Some("Import Preview"));
    dlg.set_default_size(680, 560);
    dlg.set_transient_for(Some(window));
    dlg.set_modal(false);

    let header = adw::HeaderBar::new();
    let discard_btn = Button::with_label("Discard");
    discard_btn.add_css_class("flat");
    header.pack_start(&discard_btn);
    let import_btn = Button::with_label("Import");
    import_btn.add_css_class("suggested-action");
    header.pack_end(&import_btn);

    let outer = GtkBox::new(Orientation::Vertical, 0);

    let dest_group = adw::PreferencesGroup::new();
    dest_group.set_margin_start(12);
    dest_group.set_margin_end(12);
    dest_group.set_margin_top(8);
    dest_group.set_margin_bottom(8);
    let dest_row = adw::ComboRow::new();
    dest_row.set_title("Save to");
    let same_as_project = input_dir == work_dir;
    dest_row.set_model(Some(&gtk4::StringList::new(&[
        "This project",
        "Same folder as source file",
    ])));
    dest_row.set_selected(if same_as_project { 0 } else { 1 });
    dest_group.add(&dest_row);
    outer.append(&dest_group);

    let summary_lbl = gtk4::Label::new(Some(&summarize_import_content(&processed)));
    summary_lbl.add_css_class("dim-label");
    summary_lbl.add_css_class("caption");
    summary_lbl.set_halign(Align::Start);
    summary_lbl.set_margin_start(16);
    summary_lbl.set_margin_bottom(6);
    summary_lbl.set_wrap(true);
    outer.append(&summary_lbl);

    let warning_lines: Vec<&str> = pandoc_warnings.lines().filter(|l| !l.trim().is_empty()).collect();
    if !warning_lines.is_empty() {
        let warn_lbl = gtk4::Label::new(Some(&format!(
            "pandoc reported {} warning{} during conversion:\n{}",
            warning_lines.len(),
            if warning_lines.len() == 1 { "" } else { "s" },
            warning_lines.iter().take(5).copied().collect::<Vec<&str>>().join("\n"),
        )));
        warn_lbl.add_css_class("warning");
        warn_lbl.add_css_class("caption");
        warn_lbl.set_halign(Align::Start);
        warn_lbl.set_xalign(0.0);
        warn_lbl.set_margin_start(16);
        warn_lbl.set_margin_end(16);
        warn_lbl.set_margin_bottom(6);
        warn_lbl.set_wrap(true);
        outer.append(&warn_lbl);
    }

    let is_docx = input_path.extension().and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("docx")).unwrap_or(false);
    if is_docx && count_citations(&processed) == 0 && docx_has_citation_manager_fields(&input_path) {
        let zotero_lbl = gtk4::Label::new(Some(
            "This document appears to use Zotero/Mendeley/EndNote-linked citations, \
             which pandoc can't read directly — that's likely why no citations \
             converted. In Word, use your citation manager's \"Unlink Citations\" \
             (or equivalent) first, then re-import.",
        ));
        zotero_lbl.add_css_class("warning");
        zotero_lbl.add_css_class("caption");
        zotero_lbl.set_halign(Align::Start);
        zotero_lbl.set_xalign(0.0);
        zotero_lbl.set_margin_start(16);
        zotero_lbl.set_margin_end(16);
        zotero_lbl.set_margin_bottom(6);
        zotero_lbl.set_wrap(true);
        outer.append(&zotero_lbl);
    }

    let tv = gtk4::TextView::new();
    tv.set_editable(false);
    tv.set_monospace(true);
    tv.set_left_margin(12);
    tv.set_right_margin(12);
    tv.set_top_margin(8);
    tv.set_bottom_margin(8);
    tv.buffer().set_text(&processed);
    let scroll = ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_hexpand(true);
    scroll.set_child(Some(&tv));
    outer.append(&scroll);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&outer));
    dlg.set_content(Some(&toolbar_view));

    {
        let dlg_c = dlg.clone();
        let temp_out = temp_out_path.clone();
        let temp_media = input_dir.join(&media_name);
        let input_path_c = input_path.clone();
        discard_btn.connect_clicked(move |_| {
            let _ = std::fs::remove_file(&temp_out);
            let _ = std::fs::remove_dir_all(&temp_media);
            let mut log = crate::import_log::ImportLog::load();
            log.record(input_path_c.clone(), fmt_label, None, false, "Discarded by user");
            dlg_c.close();
        });
    }

    {
        let dlg_c = dlg.clone();
        let editor_c = editor.clone();
        let cfg_c = cfg.clone();
        let toast_overlay_c = toast_overlay.clone();
        let input_path_c = input_path.clone();
        let input_dir_c = input_dir.clone();
        let temp_out = temp_out_path.clone();
        let out_name_c = out_name.clone();
        let media_name_c = media_name.clone();
        let dest_row_c = dest_row.clone();
        let processed_c = processed.clone();
        import_btn.connect_clicked(move |_| {
            let final_dir = if dest_row_c.selected() == 0 { work_dir.clone() } else { input_dir_c.clone() };
            let final_path = unique_typ_path(final_dir.join(&out_name_c));
            let _ = std::fs::write(&final_path, &processed_c);

            if final_dir != input_dir_c {
                let src_media = input_dir_c.join(&media_name_c);
                if src_media.is_dir() {
                    let dst_media = final_dir.join(&media_name_c);
                    let _ = copy_dir_recursive(&src_media, &dst_media);
                    let _ = std::fs::remove_dir_all(&src_media);
                }
                let _ = std::fs::remove_file(&temp_out);
            }

            editor_c.open_file(final_path.clone(), &processed_c);
            let found_bib = offer_bib_autodetect(&toast_overlay_c, &cfg_c, &input_dir_c);
            warn_if_citations_without_bib(&toast_overlay_c, &cfg_c, &processed_c, found_bib);

            let mut log = crate::import_log::ImportLog::load();
            log.record(input_path_c.clone(), fmt_label, Some(final_path.clone()), true, "Imported successfully");

            let name = final_path.file_name().and_then(|n| n.to_str()).unwrap_or("document").to_string();
            let imported_toast = adw::Toast::new(&format!("Imported {name}"));
            imported_toast.set_button_label(Some("Undo"));
            imported_toast.set_timeout(6);
            let ep_undo = editor_c.clone();
            let final_path_undo = final_path.clone();
            imported_toast.connect_button_clicked(move |_| {
                ep_undo.close_file_if_open(&final_path_undo);
                let _ = std::fs::remove_file(&final_path_undo);
                let mut log = crate::import_log::ImportLog::load();
                log.record(final_path_undo.clone(), fmt_label, None, false, "Undone by user");
            });
            toast_overlay_c.add_toast(imported_toast);

            dlg_c.close();
        });
    }

    dlg.present();
}

/// Shared entry point for all pandoc-based document import (LaTeX/DOCX/
/// Markdown/ODT/HTML/EPUB). Runs pandoc as a killable child process polled
/// from the main thread (no background thread/channel needed, since `Child`
/// isn't shared across threads), extracts embedded media instead of silently
/// dropping it, and never overwrites an existing `.typ` file.
fn import_via_pandoc(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    work_dir: &std::path::Path,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
    fmt: &'static ImportFormat,
) {
    let dialog = gtk4::FileDialog::new();
    dialog.set_title(&format!("Import {}", fmt.label));
    let filter = gtk4::FileFilter::new();
    filter.set_name(Some(fmt.filter_name));
    for p in fmt.patterns { filter.add_pattern(p); }
    let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&filter);
    dialog.set_filters(Some(&filters));
    dialog.set_initial_folder(Some(&gtk4::gio::File::for_path(work_dir)));

    let win = window.clone();
    let ep = editor.clone();
    let cfg = cfg.clone();
    let toast_overlay = toast_overlay.clone();
    let work_dir = work_dir.to_path_buf();
    let win_ref = win.clone();
    // Multi-select: a single file keeps the interactive preview flow below;
    // several files route through the same sequential batch queue folder
    // import uses (same-folder-as-source destination, no per-file preview —
    // reviewing N files individually would defeat the point of multi-select).
    dialog.open_multiple(Some(&win_ref), None::<&gtk4::gio::Cancellable>, move |result| {
        let Ok(list) = result else { return };
        let paths: Vec<std::path::PathBuf> = (0..list.n_items())
            .filter_map(|i| list.item(i))
            .filter_map(|obj| obj.downcast::<gtk4::gio::File>().ok())
            .filter_map(|f| f.path())
            .collect();
        match paths.len() {
            0 => {}
            1 => {
                run_pandoc_import(&win, &ep, &cfg, &toast_overlay, &work_dir, paths.into_iter().next().unwrap(), fmt);
            }
            n => {
                let queue: std::collections::VecDeque<std::path::PathBuf> = paths.into_iter().collect();
                run_batch_import_queue(win.clone(), ep.clone(), cfg.clone(), toast_overlay.clone(), work_dir.clone(), false, queue, fmt, n);
            }
        }
    });
}

/// Entry point for single-file pandoc import (from the picker, drag-drop,
/// multi-select, or Retry). Warns first if this exact source was already
/// imported successfully before, in case the user picked the wrong file or
/// forgot they'd already converted it; otherwise proceeds immediately.
fn run_pandoc_import(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
    work_dir: &std::path::Path,
    input_path: std::path::PathBuf,
    fmt: &'static ImportFormat,
) {
    let log = crate::import_log::ImportLog::load();
    let prior = log.records.iter().rev().find(|r| r.success && r.source == input_path).cloned();

    let Some(prior) = prior else {
        run_pandoc_import_confirmed(window, editor, cfg, toast_overlay, work_dir, input_path, fmt);
        return;
    };

    let dlg = adw::MessageDialog::new(
        Some(window),
        Some("Already Imported"),
        Some(&format!(
            "You already imported this file on {}. Import it again?",
            prior.date
        )),
    );
    dlg.add_response("cancel", "Cancel");
    dlg.add_response("ok", "Import Anyway");
    dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
    dlg.set_default_response(Some("cancel"));
    dlg.set_close_response("cancel");

    let win = window.clone();
    let ep = editor.clone();
    let cfg = cfg.clone();
    let toast_overlay = toast_overlay.clone();
    let work_dir = work_dir.to_path_buf();
    dlg.connect_response(None, move |_, resp| {
        if resp == "ok" {
            run_pandoc_import_confirmed(&win, &ep, &cfg, &toast_overlay, &work_dir, input_path.clone(), fmt);
        }
    });
    dlg.present();
}

/// Spawns pandoc for a single input file and wires up progress/cancel/result
/// handling. Split out from `import_via_pandoc` so batch/folder import (which
/// already has its file list, no picker dialog needed) can call it directly.
fn run_pandoc_import_confirmed(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
    work_dir: &std::path::Path,
    input_path: std::path::PathBuf,
    fmt: &'static ImportFormat,
) {
    let stem = input_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output").to_string();
    let out_path = unique_typ_path(input_path.with_file_name(format!("{stem}.typ")));
    // Typst resolves `/`-rooted paths against the project root, not the OS
    // filesystem — so pandoc must be run with cwd = the input's directory and
    // given bare relative names, or `--extract-media`/`-o` with absolute paths
    // makes it emit `#image("/abs/os/path...")`, which won't resolve as an
    // image path inside the document (verified against a real pandoc run).
    let out_stem = out_path.file_stem().and_then(|s| s.to_str()).unwrap_or(&stem).to_string();
    let out_name = out_path.file_name().and_then(|s| s.to_str()).unwrap_or("output.typ").to_string();
    let media_name = format!("{out_stem}_media");
    let input_dir = input_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let input_name = input_path.file_name().and_then(|s| s.to_str()).unwrap_or("input").to_string();

    let mut cmd = build_pandoc_command(&input_dir, &input_name, fmt.pandoc_from, &out_name, &media_name);
    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => {
            show_alert(window, "Import Failed", &format!(
                "pandoc was not found. Install it to use {} import:\n\
                 \n  zypper install pandoc\
                 \n  apt   install pandoc\
                 \n  brew  install pandoc\
                 \n  dnf   install pandoc\
                 \nVersion 3.1 or later is required.",
                fmt.label
            ));
            let mut log = crate::import_log::ImportLog::load();
            log.record(input_path, fmt.label, None, false, "pandoc not found");
            return;
        }
    };
    let child = Rc::new(RefCell::new(Some(child)));

    let toast = adw::Toast::new(&format!("Importing {}…", fmt.label));
    toast.set_priority(adw::ToastPriority::High);
    toast.set_timeout(0);
    toast.set_button_label(Some("Cancel"));
    {
        // Killing the local `flatpak-spawn` client process does not always
        // guarantee the host-side pandoc process it launched also terminates
        // immediately — best-effort, but this is the only cancellation lever
        // available without a portal-level process-tracking API.
        let child_for_cancel = child.clone();
        let toast_for_cancel = toast.clone();
        let toast_overlay_for_cancel = toast_overlay.clone();
        let input_path_for_cancel = input_path.clone();
        toast.connect_button_clicked(move |_| {
            if let Some(mut c) = child_for_cancel.borrow_mut().take() {
                let _ = c.kill();
            }
            toast_for_cancel.dismiss();
            let cancelled = adw::Toast::new("Import cancelled");
            cancelled.set_timeout(3);
            toast_overlay_for_cancel.add_toast(cancelled);
            let mut log = crate::import_log::ImportLog::load();
            log.record(input_path_for_cancel.clone(), fmt.label, None, false, "Cancelled by user");
        });
    }
    toast_overlay.add_toast(toast.clone());

    let started = std::time::Instant::now();
    let child_poll = child.clone();
    let win = window.clone();
    let ep = editor.clone();
    let cfg = cfg.clone();
    let toast_overlay = toast_overlay.clone();
    let work_dir = work_dir.to_path_buf();
    let out_path = out_path.clone();
    glib::timeout_add_local(Duration::from_millis(150), move || {
        let mut guard = child_poll.borrow_mut();
        let Some(c) = guard.as_mut() else {
            // Already taken (and killed) by the Cancel button above.
            return glib::ControlFlow::Break;
        };
        match c.try_wait() {
            Ok(Some(status)) => {
                let stdout = c.stdout.take();
                let stderr = c.stderr.take();
                drop(guard);
                toast.dismiss();
                use std::io::Read;
                let mut stderr_text = String::new();
                if let Some(mut s) = stderr { let _ = s.read_to_string(&mut stderr_text); }
                let _ = stdout;

                if status.success() {
                    if let Ok(raw) = std::fs::read_to_string(&out_path) {
                        let bib_path = cfg.borrow().bib_path.clone();
                        let processed = post_process_latex_import(&raw, bib_path.as_deref());
                        show_import_preview_dialog(
                            &win, &ep, &cfg, &toast_overlay,
                            input_path.clone(), fmt.label, processed,
                            out_path.clone(), media_name.clone(), work_dir.clone(),
                            stderr_text.clone(),
                        );
                    } else {
                        show_alert(&win, "Import Failed", "pandoc reported success but the output file could not be read.");
                        let mut log = crate::import_log::ImportLog::load();
                        log.record(input_path.clone(), fmt.label, None, false, "Output file unreadable");
                    }
                } else {
                    let description = describe_pandoc_failure(&stderr_text);
                    show_alert(&win, "Import Failed", &description);
                    let mut log = crate::import_log::ImportLog::load();
                    log.record(input_path.clone(), fmt.label, None, false, &description);
                }
                glib::ControlFlow::Break
            }
            Ok(None) => {
                let secs = started.elapsed().as_secs();
                if secs > 0 {
                    toast.set_title(&format!("Importing {}… ({secs}s)", fmt.label));
                }
                glib::ControlFlow::Continue
            }
            Err(_) => {
                drop(guard);
                toast.dismiss();
                show_alert(&win, "Import Failed", "Failed to check the import process's status.");
                glib::ControlFlow::Break
            }
        }
    });
}

/// Small picker for batch import: choose a format, a folder, and a destination,
/// then convert every matching file in that folder one at a time (not in
/// parallel — avoids launching many concurrent pandoc processes). Unlike the
/// single-file flow, batch import skips the per-file preview dialog; the
/// dialog says so up front.
/// "Paste as Document": reads plain text off the clipboard (not rich HTML —
/// that would need mime-type negotiation via `read_value_async`, out of scope
/// here) and runs it through the same markdown pandoc path as a file import,
/// via stdin instead of a saved file.
fn paste_as_document(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    work_dir: &std::path::Path,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
) {
    let clipboard = window.clipboard();
    let win = window.clone();
    let editor = editor.clone();
    let work_dir = work_dir.to_path_buf();
    let cfg = cfg.clone();
    let toast_overlay = toast_overlay.clone();
    clipboard.read_text_async(None::<&gtk4::gio::Cancellable>, move |result| {
        let Ok(Some(text)) = result else {
            show_alert(&win, "Nothing to Paste", "The clipboard doesn't contain any text.");
            return;
        };
        prompt_paste_filename(&win, &editor, &work_dir, &cfg, &toast_overlay, text.to_string());
    });
}

fn prompt_paste_filename(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    work_dir: &std::path::Path,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
    text: String,
) {
    let has_open_doc = editor.get_active_path().is_some();

    let dlg = adw::MessageDialog::new(Some(window), Some("Paste as Document"), None);
    dlg.add_response("cancel", "Cancel");
    dlg.add_response("ok", "Import");
    dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
    dlg.set_default_response(Some("ok"));
    dlg.set_close_response("cancel");

    let container = GtkBox::new(Orientation::Vertical, 10);

    let dest_row = adw::ComboRow::new();
    let entry = Entry::new();
    entry.set_placeholder_text(Some("Untitled"));

    if has_open_doc {
        dest_row.set_title("Destination");
        dest_row.set_model(Some(&gtk4::StringList::new(&[
            "Insert into the current document",
            "Create a new document",
        ])));
        dest_row.set_selected(0);
        container.append(&dest_row);
        entry.set_visible(false);
        {
            let entry_c = entry.clone();
            dest_row.connect_selected_notify(move |row| {
                entry_c.set_visible(row.selected() == 1);
            });
        }
    } else {
        let lbl = Label::new(Some("Name the new document:"));
        lbl.set_halign(Align::Start);
        container.append(&lbl);
    }
    container.append(&entry);
    dlg.set_extra_child(Some(&container));

    let win = window.clone();
    let editor = editor.clone();
    let work_dir_c = work_dir.to_path_buf();
    let cfg = cfg.clone();
    let toast_overlay = toast_overlay.clone();
    let entry_c = entry.clone();
    dlg.connect_response(None, move |_, resp| {
        if resp != "ok" { return; }
        let insert_at_cursor = has_open_doc && dest_row.selected() == 0;
        let name = entry_c.text().to_string();
        let stem = if name.trim().is_empty() { "Untitled".to_string() } else { name.trim().to_string() };
        run_pandoc_import_from_stdin(&win, &editor, &cfg, &toast_overlay, &work_dir_c, text.clone(), &stem, insert_at_cursor);
    });
    dlg.present();
}

/// Like `run_pandoc_import`, but for content that isn't a file on disk yet —
/// pandoc reads from stdin (`-` as input) instead of a named file.
fn run_pandoc_import_from_stdin(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
    work_dir: &std::path::Path,
    text: String,
    stem: &str,
    insert_at_cursor: bool,
) {
    let out_path = unique_typ_path(work_dir.join(format!("{stem}.typ")));
    let out_name = out_path.file_name().and_then(|s| s.to_str()).unwrap_or("output.typ").to_string();

    let mut cmd = if crate::git_sync::in_flatpak() {
        let mut c = std::process::Command::new("flatpak-spawn");
        c.arg("--host").arg(format!("--directory={}", work_dir.display())).arg("pandoc");
        c
    } else {
        let mut c = std::process::Command::new("pandoc");
        c.current_dir(work_dir);
        c
    };
    cmd.arg("-f").arg("markdown")
        .arg("-t").arg("typst")
        .arg("--standalone")
        .arg("-o").arg(&out_name)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => {
            show_alert(window, "Import Failed", "pandoc was not found. Install it to use Paste as Document.");
            return;
        }
    };
    {
        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
    }
    let child = Rc::new(RefCell::new(Some(child)));

    let toast = adw::Toast::new("Importing pasted text…");
    toast.set_priority(adw::ToastPriority::High);
    toast.set_timeout(0);
    toast_overlay.add_toast(toast.clone());

    let win = window.clone();
    let ep = editor.clone();
    let cfg = cfg.clone();
    let source_label = std::path::PathBuf::from(format!("Pasted text ({stem})"));
    glib::timeout_add_local(Duration::from_millis(150), move || {
        let mut guard = child.borrow_mut();
        let Some(c) = guard.as_mut() else { return glib::ControlFlow::Break };
        match c.try_wait() {
            Ok(Some(status)) => {
                let stderr = c.stderr.take();
                drop(guard);
                toast.dismiss();
                use std::io::Read;
                let mut stderr_text = String::new();
                if let Some(mut s) = stderr { let _ = s.read_to_string(&mut stderr_text); }

                let mut log = crate::import_log::ImportLog::load();
                if status.success() {
                    if let Ok(raw) = std::fs::read_to_string(&out_path) {
                        if insert_at_cursor {
                            // Body only — no Zerkalo preamble, since this is
                            // going into a document that (if templated) already
                            // has one.
                            let body = strip_pandoc_preamble(&raw);
                            let _ = std::fs::remove_file(&out_path);
                            ep.insert_at_cursor(&body);
                            log.record(source_label.clone(), "Paste as Document", None, true, "Inserted at cursor");
                        } else {
                            let bib_path = cfg.borrow().bib_path.clone();
                            let processed = post_process_latex_import(&raw, bib_path.as_deref());
                            let _ = std::fs::write(&out_path, &processed);
                            ep.open_file(out_path.clone(), &processed);
                            log.record(source_label.clone(), "Paste as Document", Some(out_path.clone()), true, "Imported successfully");
                        }
                    } else {
                        show_alert(&win, "Import Failed", "pandoc reported success but the output file could not be read.");
                        log.record(source_label.clone(), "Paste as Document", None, false, "Output file unreadable");
                    }
                } else {
                    let description = describe_pandoc_failure(&stderr_text);
                    show_alert(&win, "Import Failed", &description);
                    log.record(source_label.clone(), "Paste as Document", None, false, &description);
                }
                glib::ControlFlow::Break
            }
            Ok(None) => glib::ControlFlow::Continue,
            Err(_) => {
                drop(guard);
                toast.dismiss();
                glib::ControlFlow::Break
            }
        }
    });
}

fn import_folder_via_pandoc(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    work_dir: &std::path::Path,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
) {
    let dlg = adw::Window::new();
    dlg.set_title(Some("Import Folder"));
    dlg.set_default_width(340);
    dlg.set_modal(true);
    dlg.set_transient_for(Some(window));

    let header = adw::HeaderBar::new();
    let title_lbl = gtk4::Label::new(Some("Import Folder"));
    title_lbl.add_css_class("heading");
    header.set_title_widget(Some(&title_lbl));

    let group = adw::PreferencesGroup::new();
    group.set_margin_start(12);
    group.set_margin_end(12);
    group.set_margin_top(8);
    group.set_description(Some("Every matching file is converted one at a time; each is opened without an individual preview step."));

    let format_row = adw::ComboRow::new();
    format_row.set_title("Format");
    let labels: Vec<&str> = IMPORT_FORMATS.iter().map(|f| f.label).collect();
    format_row.set_model(Some(&gtk4::StringList::new(&labels)));
    group.add(&format_row);

    let folder_row = adw::ActionRow::new();
    folder_row.set_title("Folder");
    folder_row.set_subtitle("Not selected");
    folder_row.set_activatable(true);
    folder_row.add_suffix(&gtk4::Image::from_icon_name("folder-open-symbolic"));
    group.add(&folder_row);

    let dest_row = adw::ComboRow::new();
    dest_row.set_title("Save to");
    dest_row.set_model(Some(&gtk4::StringList::new(&["This project", "Same folder as each source file"])));
    group.add(&dest_row);

    let recursive_row = adw::SwitchRow::new();
    recursive_row.set_title("Include subfolders");
    recursive_row.set_active(false);
    group.add(&recursive_row);

    let selected_folder: Rc<RefCell<Option<std::path::PathBuf>>> = Rc::new(RefCell::new(None));
    {
        let win_c = window.clone();
        let folder_row_c = folder_row.clone();
        let selected_folder_c = selected_folder.clone();
        folder_row.connect_activated(move |_| {
            let fd = gtk4::FileDialog::new();
            let folder_row2 = folder_row_c.clone();
            let selected_folder2 = selected_folder_c.clone();
            fd.select_folder(Some(&win_c), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        folder_row2.set_subtitle(&path.display().to_string());
                        *selected_folder2.borrow_mut() = Some(path);
                    }
                }
            });
        });
    }

    let import_btn = Button::with_label("Import Folder");
    import_btn.add_css_class("suggested-action");
    import_btn.set_margin_start(12);
    import_btn.set_margin_end(12);
    import_btn.set_margin_top(12);
    import_btn.set_margin_bottom(12);
    import_btn.set_halign(Align::End);

    let vbox = GtkBox::new(Orientation::Vertical, 0);
    vbox.append(&header);
    vbox.append(&group);
    vbox.append(&import_btn);
    dlg.set_content(Some(&vbox));

    {
        let dlg_c = dlg.clone();
        let win_c = window.clone();
        let ep_c = editor.clone();
        let cfg_c = cfg.clone();
        let toast_overlay_c = toast_overlay.clone();
        let work_dir_c = work_dir.to_path_buf();
        let format_row_c = format_row.clone();
        let dest_row_c = dest_row.clone();
        let recursive_row_c = recursive_row.clone();
        let selected_folder_c = selected_folder.clone();
        import_btn.connect_clicked(move |_| {
            let Some(folder) = selected_folder_c.borrow().clone() else { return };
            let idx = format_row_c.selected() as usize;
            let Some(fmt) = IMPORT_FORMATS.get(idx) else { return };
            let dest_this_project = dest_row_c.selected() == 0;
            dlg_c.close();

            let mut files: Vec<std::path::PathBuf> = Vec::new();
            if recursive_row_c.is_active() {
                scan_files_recursive(&folder, fmt.extensions, &mut files);
            } else {
                files.extend(std::fs::read_dir(&folder)
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        p.is_file() && p.extension().and_then(|e| e.to_str())
                            .map(|ext| fmt.extensions.iter().any(|want| want.eq_ignore_ascii_case(ext)))
                            .unwrap_or(false)
                    }));
            }
            files.sort();

            if files.is_empty() {
                show_alert(&win_c, "Nothing to Import", &format!("No {} files were found in that folder.", fmt.label));
                return;
            }

            let total = files.len();
            let queue: std::collections::VecDeque<std::path::PathBuf> = files.into_iter().collect();
            run_batch_import_queue(
                win_c.clone(), ep_c.clone(), cfg_c.clone(), toast_overlay_c.clone(),
                work_dir_c.clone(), dest_this_project, queue, fmt, total,
            );
        });
    }

    dlg.present();
}

/// Processes one file from the batch queue, then recurses for the next once
/// pandoc exits — sequential by design, to avoid many concurrent pandoc
/// processes and many simultaneous "Importing…" toasts.
/// Entry point for batch import: starts up to `cfg.batch_import_concurrency`
/// workers pulling from a shared queue, each recursing into the next file on
/// its own completion — bounded parallelism rather than strictly one-at-a-time,
/// with one shared progress toast updated as files finish.
#[allow(clippy::too_many_arguments)]
fn run_batch_import_queue(
    window: adw::ApplicationWindow,
    editor: EditorPane,
    cfg: Rc<RefCell<Config>>,
    toast_overlay: adw::ToastOverlay,
    work_dir: std::path::PathBuf,
    dest_this_project: bool,
    queue: std::collections::VecDeque<std::path::PathBuf>,
    fmt: &'static ImportFormat,
    total: usize,
) {
    let queue = Rc::new(RefCell::new(queue));
    let done = Rc::new(std::cell::Cell::new(0usize));
    let failed = Rc::new(std::cell::Cell::new(0usize));
    let active = Rc::new(std::cell::Cell::new(0usize));
    let written: Rc<RefCell<Vec<std::path::PathBuf>>> = Rc::new(RefCell::new(Vec::new()));

    let progress = adw::Toast::new(&format!("Importing… (0 of {total} done)"));
    progress.set_priority(adw::ToastPriority::High);
    progress.set_timeout(0);
    toast_overlay.add_toast(progress.clone());
    let progress = Rc::new(progress);

    let concurrency = cfg.borrow().batch_import_concurrency.max(1) as usize;
    let n_workers = concurrency.min(total.max(1));
    for _ in 0..n_workers {
        run_next_batch_worker(
            window.clone(), editor.clone(), cfg.clone(), toast_overlay.clone(),
            work_dir.clone(), dest_this_project, queue.clone(), fmt,
            done.clone(), failed.clone(), active.clone(), total, progress.clone(), written.clone(),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn run_next_batch_worker(
    window: adw::ApplicationWindow,
    editor: EditorPane,
    cfg: Rc<RefCell<Config>>,
    toast_overlay: adw::ToastOverlay,
    work_dir: std::path::PathBuf,
    dest_this_project: bool,
    queue: Rc<RefCell<std::collections::VecDeque<std::path::PathBuf>>>,
    fmt: &'static ImportFormat,
    done: Rc<std::cell::Cell<usize>>,
    failed: Rc<std::cell::Cell<usize>>,
    active: Rc<std::cell::Cell<usize>>,
    total: usize,
    progress: Rc<adw::Toast>,
    written: Rc<RefCell<Vec<std::path::PathBuf>>>,
) {
    let Some(input_path) = queue.borrow_mut().pop_front() else {
        // No more work for this worker slot. Once every worker has reached
        // this point (none still active), the batch is finished.
        if active.get() == 0 {
            progress.dismiss();
            let has_failures = failed.get() > 0;
            let has_successes = done.get() > 0;
            let summary = if has_failures {
                format!("Imported {} of {} files ({} failed)", done.get(), total, failed.get())
            } else {
                format!("Imported {} of {} files", done.get(), total)
            };
            let toast = adw::Toast::new(&summary);
            toast.set_timeout(5);
            if has_successes {
                // Only one action button fits on a toast — undoing the batch is
                // the more time-sensitive action when there's something to undo.
                toast.set_button_label(Some("Undo All"));
                let editor_c = editor.clone();
                let written_c = written.clone();
                let fmt_label = fmt.label;
                toast.connect_button_clicked(move |_| {
                    let mut log = crate::import_log::ImportLog::load();
                    for path in written_c.borrow().iter() {
                        editor_c.close_file_if_open(path);
                        let _ = std::fs::remove_file(path);
                        log.record(path.clone(), fmt_label, None, false, "Undone by user (batch)");
                    }
                });
            } else if has_failures {
                toast.set_button_label(Some("View Failures"));
                let win_c = window.clone();
                let ep_c = editor.clone();
                let work_dir_c = work_dir.clone();
                let cfg_c = cfg.clone();
                let toast_overlay_c = toast_overlay.clone();
                toast.connect_button_clicked(move |_| {
                    show_import_history_dialog_filtered(&win_c, &ep_c, &work_dir_c, &cfg_c, &toast_overlay_c, true);
                });
            }
            toast_overlay.add_toast(toast);
        }
        return;
    };
    active.set(active.get() + 1);

    let stem = input_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output").to_string();
    let out_path = unique_typ_path(input_path.with_file_name(format!("{stem}.typ")));
    let out_stem = out_path.file_stem().and_then(|s| s.to_str()).unwrap_or(&stem).to_string();
    let out_name = out_path.file_name().and_then(|s| s.to_str()).unwrap_or("output.typ").to_string();
    let media_name = format!("{out_stem}_media");
    let input_dir = input_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let input_name = input_path.file_name().and_then(|s| s.to_str()).unwrap_or("input").to_string();

    let mut cmd = build_pandoc_command(&input_dir, &input_name, fmt.pandoc_from, &out_name, &media_name);
    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => {
            let mut log = crate::import_log::ImportLog::load();
            log.record(input_path, fmt.label, None, false, "pandoc not found");
            failed.set(failed.get() + 1);
            active.set(active.get() - 1);
            show_alert(&window, "Import Failed", "pandoc was not found. Install it to use folder import.");
            run_next_batch_worker(window, editor, cfg, toast_overlay, work_dir, dest_this_project, queue, fmt, done, failed, active, total, progress, written);
            return;
        }
    };
    let child = Rc::new(RefCell::new(Some(child)));

    glib::timeout_add_local(Duration::from_millis(150), move || {
        let mut guard = child.borrow_mut();
        let Some(c) = guard.as_mut() else { return glib::ControlFlow::Break };
        match c.try_wait() {
            Ok(Some(status)) => {
                let stderr = c.stderr.take();
                drop(guard);
                use std::io::Read;
                let mut stderr_text = String::new();
                if let Some(mut s) = stderr { let _ = s.read_to_string(&mut stderr_text); }

                if status.success() {
                    if let Ok(raw) = std::fs::read_to_string(&out_path) {
                        let bib_path = cfg.borrow().bib_path.clone();
                        let processed = post_process_latex_import(&raw, bib_path.as_deref());
                        let final_dir = if dest_this_project { work_dir.clone() } else { input_dir.clone() };
                        let final_path = unique_typ_path(final_dir.join(&out_name));
                        let _ = std::fs::write(&final_path, &processed);
                        if final_dir != input_dir {
                            let src_media = input_dir.join(&media_name);
                            if src_media.is_dir() {
                                let dst_media = final_dir.join(&media_name);
                                let _ = copy_dir_recursive(&src_media, &dst_media);
                                let _ = std::fs::remove_dir_all(&src_media);
                            }
                            let _ = std::fs::remove_file(&out_path);
                        }
                        let mut log = crate::import_log::ImportLog::load();
                        log.record(input_path.clone(), fmt.label, Some(final_path.clone()), true, "Imported successfully (batch)");
                        written.borrow_mut().push(final_path);
                        done.set(done.get() + 1);
                    } else {
                        let mut log = crate::import_log::ImportLog::load();
                        log.record(input_path.clone(), fmt.label, None, false, "Output file unreadable");
                        failed.set(failed.get() + 1);
                    }
                } else {
                    let description = describe_pandoc_failure(&stderr_text);
                    let mut log = crate::import_log::ImportLog::load();
                    log.record(input_path.clone(), fmt.label, None, false, &description);
                    failed.set(failed.get() + 1);
                }

                progress.set_title(&format!("Importing… ({} of {} done)", done.get() + failed.get(), total));
                active.set(active.get() - 1);
                run_next_batch_worker(
                    window.clone(), editor.clone(), cfg.clone(), toast_overlay.clone(),
                    work_dir.clone(), dest_this_project, queue.clone(), fmt,
                    done.clone(), failed.clone(), active.clone(), total, progress.clone(), written.clone(),
                );
                glib::ControlFlow::Break
            }
            Ok(None) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        }
    });
}

fn post_process_latex_import(content: &str, bib_path: Option<&std::path::Path>) -> String {
    // ── Phase 1: single-pass classifier ───────────────────────────────────────
    //
    // Every line in the pandoc-converted content falls into one of three buckets:
    //
    //  DISCARDED  — formatting rules that Zerkalo's template block controls:
    //               #set page(...)  #set text(...)  #set par(...)
    //               #show heading*  #set heading(...)
    //
    //  MACROS     — definitions the body may depend on; placed after the template:
    //               #import "..."   #let name = ...
    //
    //  BODY       — all actual document content (headings, paragraphs, citations,
    //               #page(...) content blocks, #figure, #footnote, etc.)
    //
    // This approach handles content scattered throughout the file, not just at the
    // top, which is what pandoc produces for complex LaTeX sources.

    enum Scan { Body, SkipSet(i32), SkipShow(i32), CollectLet(i32) }

    let lines: Vec<&str> = content.lines().collect();
    let mut macro_defs: Vec<String> = Vec::new();
    let mut body: Vec<String> = Vec::new();
    let mut scan = Scan::Body;
    let mut let_buf = String::new();

    // Combined depth counting for all delimiter types
    let paren_depth = |s: &str| -> i32 {
        s.chars().fold(0i32, |d, c| match c {
            '(' => d + 1,
            ')' => d - 1,
            _ => d,
        })
    };
    // For #show heading blocks, which use block(...)[\n...\n] syntax, we must
    // track ALL delimiters together: the `(` opens before the `[` does.
    let total_depth = |s: &str| -> i32 {
        s.chars().fold(0i32, |d, c| match c {
            '(' | '[' | '{' => d + 1,
            ')' | ']' | '}' => d - 1,
            _ => d,
        })
    };
    for &line in &lines {
        let t = line.trim();
        scan = match scan {
            // ── Continuation: discarding a multi-line #set block ────────────────
            Scan::SkipSet(d) => {
                let d = d + paren_depth(t);
                if d > 0 { Scan::SkipSet(d) } else { Scan::Body }
            }

            // ── Continuation: discarding a multi-line #show heading block ────────
            // Uses total_depth (all delimiters) because show rules use block(...)[\n...\n]
            // where the `(` opens before the `[` does.
            Scan::SkipShow(d) => {
                let d = d + total_depth(t);
                if d > 0 { Scan::SkipShow(d) } else { Scan::Body }
            }

            // ── Continuation: collecting a multi-line #let definition ────────────
            Scan::CollectLet(d) => {
                let_buf.push('\n');
                let_buf.push_str(line);
                let d = d + total_depth(t);
                if d <= 0 {
                    macro_defs.push(std::mem::take(&mut let_buf));
                    Scan::Body
                } else {
                    Scan::CollectLet(d)
                }
            }

            // ── Normal body scan ─────────────────────────────────────────────────
            Scan::Body => {
                if t.starts_with("#set ") {
                    // Strip all #set rules pandoc generates (page, text, par, heading,
                    // list, table, math.equation, etc.); track depth for multi-line blocks.
                    let d = paren_depth(t);
                    if d > 0 { Scan::SkipSet(d) } else { Scan::Body }
                } else if t.starts_with("#show") {
                    // Strip all #show rules (#show heading:, #show:, #show terms:, etc.).
                    // Uses total_depth because show rules mix (), [], {} delimiters.
                    let d = total_depth(t);
                    if d > 0 { Scan::SkipShow(d) } else { Scan::Body }
                } else if t.starts_with("#import ") {
                    macro_defs.push(line.to_string());
                    Scan::Body
                } else if t.starts_with("#let ") {
                    // Use total_depth: pandoc's #let conf(...) = {...} uses () for
                    // function params before {} for the body.
                    let d = total_depth(t);
                    let_buf = line.to_string();
                    if d > 0 {
                        Scan::CollectLet(d)
                    } else {
                        macro_defs.push(std::mem::take(&mut let_buf));
                        Scan::Body
                    }
                } else {
                    body.push(line.to_string());
                    Scan::Body
                }
            }
        };
    }

    // ── Phase 2: process body — insert pagebreaks, fix bibliography ───────────

    // Trim leading blank lines from the body
    let skip = body.iter().position(|l| !l.trim().is_empty()).unwrap_or(body.len());
    let body = body[skip..].to_vec();

    let first_heading = body.iter().position(|l| {
        let t = l.trim();
        t.starts_with("= ") && !t.starts_with("==")
    });

    let bib_idx = body.iter().position(|l| l.trim().starts_with("#bibliography"));

    let bib_style = bib_idx
        .and_then(|bi| {
            let s = body[bi].trim();
            let start = s.find("style:")? + 6;
            let after = s[start..].trim_start().trim_start_matches('"');
            let end = after.find('"')?;
            Some(after[..end].to_string())
        })
        .unwrap_or_else(|| "chicago-author-date".to_string());

    let bib_call = match bib_path {
        Some(bp) => format!("#bibliography(\"{}\", style: \"{}\")", bp.display(), bib_style),
        None if bib_idx.is_some() => body[bib_idx.unwrap()].trim().to_string(),
        None => format!("// #bibliography(\"refs.bib\", style: \"{}\")", bib_style),
    };

    let trim_trailing = |v: &mut Vec<String>| {
        while v.last().map(|l: &String| l.trim().is_empty()).unwrap_or(false) {
            v.pop();
        }
    };

    let mut processed: Vec<String> = Vec::with_capacity(body.len() + 8);
    let mut pb_done = false;

    for (i, line) in body.iter().enumerate() {
        // Pagebreak before first top-level heading (separates title block from body)
        if Some(i) == first_heading && !pb_done && i > 0 {
            trim_trailing(&mut processed);
            processed.push(String::new());
            processed.push("#pagebreak()".to_string());
            processed.push(String::new());
            pb_done = true;
        }

        // Replace bibliography line with a clean, properly-placed version
        if Some(i) == bib_idx {
            trim_trailing(&mut processed);
            processed.push(String::new());
            processed.push("#pagebreak()".to_string());
            processed.push(String::new());
            processed.push(bib_call.clone());
            continue;
        }

        processed.push(line.clone());
    }

    if bib_idx.is_none() {
        processed.push(String::new());
        processed.push(
            "// ── Bibliography ────────────────────────────────────────────────────"
                .to_string(),
        );
        processed.push(bib_call);
    }

    // ── Phase 3: assemble a well-formed Zerkalo document ─────────────────────

    let preamble = super::template_dialog::default_import_preamble();
    let mut out = preamble;
    out.push('\n');

    if !macro_defs.is_empty() {
        out.push_str(
            "// ── Imported macros ─────────────────────────────────────────────────────\n",
        );
        for def in &macro_defs {
            out.push_str(def);
            out.push('\n');
        }
        out.push('\n');
    }

    out.push_str(
        "// ── Document body ───────────────────────────────────────────────────────\n\n",
    );
    out.push_str(&processed.join("\n"));
    if !out.ends_with('\n') {
        out.push('\n');
    }

    out
}

/// Wrap plain text extracted from a PDF into a Typst document managed by Zerkalo's template system.
/// A line extracted from a PDF is treated as a probable section heading (and
/// promoted to `== Heading`) when it's short, isn't sentence-ending
/// punctuation, and sits alone between blank lines — the closest signal
/// `pdftotext`'s plain-text output gives us to the source PDF's actual
/// heading styling, which is lost entirely once text is extracted.
fn is_probable_pdf_heading(line: &str, prev_blank: bool, next_blank: bool) -> bool {
    let t = line.trim();
    if t.is_empty() || !prev_blank || !next_blank {
        return false;
    }
    if t.chars().count() > 60 {
        return false;
    }
    !matches!(t.chars().last(), Some('.' | ',' | ';' | ':'))
}

/// Reflow pdftotext output, promoting probable headings (see
/// `is_probable_pdf_heading`) to `== Heading` lines.
fn format_pdf_body(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = String::new();
    let mut prev_blank = true;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            out.push('\n');
            prev_blank = true;
            continue;
        }
        let next_blank = lines.get(i + 1).map(|l| l.trim().is_empty()).unwrap_or(true);
        if is_probable_pdf_heading(trimmed, prev_blank, next_blank) {
            out.push_str("== ");
        }
        out.push_str(trimmed);
        out.push('\n');
        prev_blank = false;
    }
    out
}

/// Runs the pdftotext-based PDF import pipeline for `input_path`, shared by
/// the ☰ → Import → PDF file picker and drag-and-drop.
fn run_pdf_import(window: &adw::ApplicationWindow, editor: &EditorPane, input_path: std::path::PathBuf) {
    let stem = input_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output").to_string();
    let out_path = unique_typ_path(input_path.with_file_name(format!("{stem}.typ")));
    let output = crate::git_sync::host_command("pdftotext")
        .arg("-layout")
        .arg(&input_path)
        .arg("-")
        .output();
    let mut log = crate::import_log::ImportLog::load();
    match output {
        Ok(o) if o.status.success() => {
            let extracted = String::from_utf8_lossy(&o.stdout).to_string();
            let typst_doc = post_process_pdf_import(&extracted, stem.as_str());
            let _ = std::fs::write(&out_path, &typst_doc);
            editor.open_file(out_path.clone(), &typst_doc);
            log.record(input_path, "PDF (.pdf)", Some(out_path), true, "Imported successfully");
        }
        Ok(_) => {
            show_alert(window, "Import Failed", "pdftotext could not extract text from this PDF.");
            log.record(input_path, "PDF (.pdf)", None, false, "pdftotext could not extract text");
        }
        Err(_) => {
            show_alert(window, "Import Failed",
                "pdftotext was not found. Install poppler-utils to use PDF import:\n\
                 \n  zypper install poppler-tools\
                 \n  apt   install poppler-utils\
                 \n  brew  install poppler\
                 \n  dnf   install poppler-utils");
            log.record(input_path, "PDF (.pdf)", None, false, "pdftotext not found");
        }
    }
}

fn post_process_pdf_import(text: &str, title: &str) -> String {
    let escaped_title = title.replace('"', "\\\"");
    let preamble = super::template_dialog::default_import_preamble();
    let mut out = format!(
        "{preamble}\n\
         // ── Document body ───────────────────────────────────────────────────────\n\
         // Imported from PDF — plain text only. Section headings are guessed from\n\
         // short, isolated lines; review them, and other formatting (tables, math,\n\
         // images) is not preserved at all.\n\
         \n\
         = {escaped_title}\n\
         \n"
    );

    out.push_str(&format_pdf_body(text));

    // Bibliography stub so Zerkalo can locate it
    out.push_str(
        "\n// ── Bibliography ────────────────────────────────────────────────────\n\
         // #bibliography(\"refs.bib\", style: \"chicago-author-date\")\n",
    );

    out
}

fn load_app_css() {
    crate::ui::styles::load_global_css();

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
    menu_export_item: Button,
    menu_export_web_item: Button,
    menu_print_item: Button,
    menu_import_item: Button,
    menu_docs_item: Button,
    menu_fonts_item: Button,
    menu_settings_item: Button,
    menu_setup_item: Button,
    menu_backup_remote_item: Button,
    menu_help_item: Button,
    menu_writing_stats_item: Button,
    menu_about_item: Button,
    menu_import_pdf_item: Button,
}

fn build_hamburger_menu_items() -> HamburgerItems {
    HamburgerItems {
        menu_new_template_item:    make_menu_item("New from Template…",         None),
        menu_reapply_template_item: make_menu_item("Update Template Settings…", None),
        menu_repair_markers_item:  make_menu_item("Repair Template Markers…",   None),
        menu_new_item:             make_menu_item("New Blank Document…",         None),
        menu_open_item:            make_menu_item("Open File…",                  None),
        menu_save_item:              make_menu_item("Save",                      Some("Ctrl+S")),
        menu_save_as_item:         make_menu_item("Save As…",                    None),
        menu_snapshots_item:       make_menu_item("Browse Snapshots…",           None),
        menu_export_item:          make_menu_item("Export…",                     None),
        menu_export_web_item:      make_menu_item("Export for Web…",             None),
        menu_print_item:           make_menu_item("Print PDF",                   Some("Ctrl+P")),
        menu_import_item:          make_menu_item("Import…",                     Some("Ctrl+Shift+I")),
        menu_docs_item:            make_menu_item("Browse Documents…",           None),
        menu_fonts_item:           make_menu_item("Font Management…",            None),
        menu_settings_item:        make_menu_item("Settings",                    None),
        menu_setup_item:           make_menu_item("Setup & Onboarding…",         None),
        menu_backup_remote_item:   make_menu_item("Git Remotes…",                 None),
        menu_help_item:            make_menu_item("Keyboard Shortcuts & Help",   Some("Ctrl+?")),
        menu_writing_stats_item:   make_menu_item("Writing Stats",               None),
        menu_about_item:           make_menu_item("About Zerkalo",               None),
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

fn show_doc_stats(
    parent: &impl IsA<gtk4::Window>,
    text: &str,
    session_start: u32,
    _project_root: Option<&std::path::Path>,
) {
    let content = strip_typst_markup(&strip_zerkalo_blocks(text));
    let words = content.split_whitespace().count();
    let chars = content.chars().filter(|c| !c.is_whitespace()).count();
    let chars_with_spaces = content.chars().count();
    let paragraphs = content.split("\n\n").filter(|s| !s.trim().is_empty()).count();
    let sentences = content
        .split(|c: char| matches!(c, '.' | '!' | '?'))
        .filter(|s| !s.trim().is_empty())
        .count();
    let reading_mins = if words < 200 { "<1".to_string() } else { format!("{}", words / 200) };
    let session_delta = if words as u32 > session_start {
        format!("+{}", words as u32 - session_start)
    } else if (words as u32) < session_start {
        format!("{}", words as i64 - session_start as i64)
    } else {
        "±0".to_string()
    };

    let win = adw::Window::new();
    win.set_title(Some("Document Statistics"));
    win.set_default_width(360);
    win.set_resizable(false);
    win.set_transient_for(Some(parent));
    win.set_modal(false);

    let make_row = |title: &str, value: &str| {
        let row = adw::ActionRow::new();
        row.set_title(title);
        let lbl = gtk4::Label::new(Some(value));
        lbl.add_css_class("dim-label");
        lbl.set_valign(gtk4::Align::Center);
        row.add_suffix(&lbl);
        row
    };

    let group = adw::PreferencesGroup::new();
    group.set_margin_start(12);
    group.set_margin_end(12);
    group.set_margin_top(12);
    group.set_margin_bottom(12);
    group.add(&make_row("Words", &format!("{words}  ({session_delta} this session)")));
    group.add(&make_row("Characters", &format!("{chars}  ({chars_with_spaces} with spaces)")));
    group.add(&make_row("Paragraphs", &paragraphs.to_string()));
    group.add(&make_row("Sentences", &sentences.to_string()));
    group.add(&make_row("Reading time", &format!("{reading_mins} min")));

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Never);
    scroll.set_child(Some(&group));

    let header = adw::HeaderBar::new();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scroll));
    win.set_content(Some(&toolbar));
    win.present();
}

fn show_changelog(parent: &impl IsA<gtk4::Window>) {
    const CHANGELOG: &str = include_str!("../../CHANGELOG.md");
    const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

    let win = adw::Window::new();
    win.set_title(Some("Changelog — Zerkalo"));
    win.set_default_width(720);
    win.set_default_height(680);
    win.set_transient_for(Some(parent));
    win.set_modal(false);

    let header = adw::HeaderBar::new();
    let title_widget = adw::WindowTitle::new(
        "Changelog",
        &format!("You're on v{CURRENT_VERSION}"),
    );
    header.set_title_widget(Some(&title_widget));

    let body = gtk4::Box::new(Orientation::Vertical, 4);
    body.set_margin_start(24);
    body.set_margin_end(24);
    body.set_margin_top(16);
    body.set_margin_bottom(24);

    let mut first_heading = true;
    for line in CHANGELOG.lines() {
        let trimmed = line.trim();
        if let Some(inner) = trimmed.strip_prefix("## [") {
            // "## [0.16.1-dev4] — Skrizhal CV element integration"
            // Version and title are split onto their own rows — a version
            // tag is always short (never needs eliding), and the title can
            // then wrap freely instead of being cut off with "…".
            let (version, rest) = match inner.split_once(']') {
                Some((v, r)) => (v, r.trim()),
                None => (inner.trim_end_matches(']'), ""),
            };
            let title = rest.strip_prefix("— ").unwrap_or(rest);

            let heading_row = gtk4::Box::new(Orientation::Horizontal, 8);
            heading_row.set_margin_top(if first_heading { 0 } else { 22 });
            first_heading = false;

            let ver_lbl = gtk4::Label::new(Some(version));
            ver_lbl.add_css_class("monospace");
            ver_lbl.add_css_class("dim-label");
            ver_lbl.add_css_class("caption-heading");
            ver_lbl.set_xalign(0.0);
            heading_row.append(&ver_lbl);

            if version == CURRENT_VERSION {
                let badge = gtk4::Label::new(Some("· Current"));
                badge.add_css_class("caption-heading");
                badge.add_css_class("accent");
                heading_row.append(&badge);
            }
            body.append(&heading_row);

            if !title.is_empty() {
                let title_lbl = gtk4::Label::new(Some(title));
                title_lbl.add_css_class("title-3");
                title_lbl.set_xalign(0.0);
                title_lbl.set_wrap(true);
                title_lbl.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
                title_lbl.set_margin_bottom(2);
                body.append(&title_lbl);
            }
        } else if let Some(text) = trimmed.strip_prefix("### ") {
            let lbl = gtk4::Label::new(Some(text));
            lbl.add_css_class("heading");
            lbl.set_xalign(0.0);
            lbl.set_margin_top(8);
            lbl.set_margin_start(4);
            lbl.set_margin_bottom(2);
            lbl.set_wrap(true);
            body.append(&lbl);
        } else if let Some(content) = trimmed.strip_prefix("- ") {
            body.append(&changelog_bullet(content));
        }
    }

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(700);
    clamp.set_child(Some(&body));
    scroll.set_child(Some(&clamp));

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scroll));
    win.set_content(Some(&toolbar));
    win.present();
}

fn changelog_bullet(text: &str) -> gtk4::Box {
    let row = gtk4::Box::new(Orientation::Horizontal, 8);
    row.set_margin_start(8);
    let dot = gtk4::Label::new(Some("•"));
    dot.set_valign(gtk4::Align::Start);
    dot.add_css_class("dim-label");
    dot.set_margin_top(1);

    let markup = md_inline_to_pango(text);
    let lbl = gtk4::Label::new(None);
    lbl.set_markup(&markup);
    lbl.set_xalign(0.0);
    lbl.set_wrap(true);
    lbl.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    lbl.set_hexpand(true);
    lbl.set_halign(gtk4::Align::Fill);

    row.append(&dot);
    row.append(&lbl);
    row
}

fn md_inline_to_pango(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                out.push_str("<b>");
                let mut inner = String::new();
                loop {
                    match chars.next() {
                        Some('*') if chars.peek() == Some(&'*') => { chars.next(); break; }
                        Some(ch) => inner.push(ch),
                        None => break,
                    }
                }
                out.push_str(&glib::markup_escape_text(&inner));
                out.push_str("</b>");
            }
            '`' => {
                out.push_str("<tt>");
                let mut inner = String::new();
                loop {
                    match chars.next() {
                        Some('`') => break,
                        Some(ch) => inner.push(ch),
                        None => break,
                    }
                }
                out.push_str(&glib::markup_escape_text(&inner));
                out.push_str("</tt>");
            }
            '[' => {
                // [text](url) → just text
                let mut link_text = String::new();
                loop {
                    match chars.next() {
                        Some(']') => break,
                        Some(ch) => link_text.push(ch),
                        None => break,
                    }
                }
                if chars.peek() == Some(&'(') {
                    // consume (url)
                    chars.next();
                    loop { match chars.next() { Some(')') | None => break, _ => {} } }
                }
                out.push_str(&glib::markup_escape_text(&link_text));
            }
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
    out
}

/// Compile the current root file to PDF and open it with xdg-open for printing.
/// Writes to a path in ~/.cache/zerkalo/ so the file is always accessible from
/// the host even when running inside a flatpak sandbox.
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
    let alert = AlertDialog::builder()
        .modal(true)
        .message("Restore this snapshot?")
        .detail("You have unsaved changes in this document. Restoring the snapshot will discard them.")
        .buttons(["Cancel", "Restore"])
        .cancel_button(0)
        .default_button(0)
        .build();
    let ep = ep.clone();
    let path = path.to_path_buf();
    alert.choose(Some(window), None::<&gtk4::gio::Cancellable>, move |result| {
        if result == Ok(1) {
            ep.set_content(&path, &text);
        }
    });
}

/// Applies a template dialog's result to `path`, splicing the fresh template
/// onto the editor buffer's *current* content (read fresh here, not a
/// snapshot taken when the dialog was opened, so edits made while the
/// non-modal dialog was open aren't discarded). Confirms first if the
/// document has no body marker, since applying then replaces the whole file.
fn apply_template_result(
    window: &adw::ApplicationWindow,
    editor: &super::editor_pane::EditorPane,
    preview: &super::preview_pane::PreviewPane,
    path: PathBuf,
    current_content: String,
    new_content: String,
    sidecar: super::template_dialog::SidecarSettings,
) {
    let do_apply = {
        let editor = editor.clone();
        let preview = preview.clone();
        let path = path.clone();
        move || {
            let cc = editor.get_active_content().unwrap_or_default();
            let updated = super::template_dialog::apply_body_splice(&cc, &new_content);
            super::template_dialog::save_sidecar(&path, &sidecar);
            if let Err(e) = std::fs::write(&path, &updated) {
                tracing::error!("Failed to write updated template: {e}");
            } else {
                editor.splice_preamble(path.clone(), &updated);
                preview.trigger_compile();
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
                  will replace the whole file. Your current text will \
                  be lost. Make sure you have a backup."),
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

fn print_pdf_from_preview(preview: &super::preview_pane::PreviewPane) {
    let Some(root) = preview.root_file_path() else { return };
    let stem = root
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document")
        .to_string();

    let cache_dir = PathBuf::from(shellexpand::tilde("~/.cache/zerkalo").as_ref());
    let _ = std::fs::create_dir_all(&cache_dir);
    let out_path = cache_dir.join(format!("{stem}.pdf"));

    std::thread::spawn(move || {
        let result = crate::compiler::compile_to_pdf_bytes(
            &root,
            &std::collections::HashMap::new(),
            &std::collections::HashMap::new(),
        );
        match result {
            Ok(bytes) => {
                if std::fs::write(&out_path, &bytes).is_ok() {
                    crate::git_sync::host_command("xdg-open")
                        .arg(&out_path)
                        .spawn()
                        .ok();
                }
            }
            Err(_) => {}
        }
    });
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

#[cfg(test)]
mod tests {
    use super::{
        describe_pandoc_failure, format_pdf_body, post_process_latex_import,
        scan_files_recursive, strip_pandoc_preamble, summarize_import_content, unique_typ_path,
    };

    // ── document import helpers ───────────────────────────────────────────────

    #[test]
    fn unique_typ_path_passes_through_when_free() {
        let dir = std::env::temp_dir().join(format!("zerkalo-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("nonexistent.typ");
        assert_eq!(unique_typ_path(path.clone()), path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unique_typ_path_suffixes_on_collision() {
        let dir = std::env::temp_dir().join(format!("zerkalo-test-collide-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let taken = dir.join("essay.typ");
        std::fs::write(&taken, "").unwrap();
        let result = unique_typ_path(taken.clone());
        assert_eq!(result, dir.join("essay (1).typ"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn describe_pandoc_failure_recognizes_unknown_writer() {
        let msg = describe_pandoc_failure("Error: Unknown writer: typst");
        assert!(msg.contains("pandoc 3.1 or later"), "got: {msg}");
    }

    #[test]
    fn describe_pandoc_failure_falls_back_to_raw_stderr() {
        let msg = describe_pandoc_failure("some other pandoc error\nline two");
        assert!(msg.starts_with("pandoc error:\n"), "got: {msg}");
        assert!(msg.contains("some other pandoc error"));
    }

    // ── format_pdf_body ────────────────────────────────────────────────────────

    #[test]
    fn format_pdf_body_promotes_isolated_short_line_to_heading() {
        let input = "\nIntroduction\n\nSome body text here that goes on.\n";
        let result = format_pdf_body(input);
        assert!(result.contains("== Introduction"), "got: {result}");
    }

    #[test]
    fn format_pdf_body_does_not_promote_long_lines() {
        let long = "This is a much longer line of text that runs well past sixty characters total.";
        let input = format!("\n{long}\n\nMore text.\n");
        let result = format_pdf_body(&input);
        assert!(!result.contains(&format!("== {long}")), "got: {result}");
        assert!(result.contains(long));
    }

    #[test]
    fn format_pdf_body_does_not_promote_sentence_ending_lines() {
        let input = "\nThis looks short.\n\nMore text.\n";
        let result = format_pdf_body(input);
        assert!(!result.contains("== This looks short."), "got: {result}");
    }

    #[test]
    fn format_pdf_body_does_not_promote_lines_without_blank_neighbors() {
        let input = "Some heading\nfollowed immediately by body text.\n";
        let result = format_pdf_body(input);
        assert!(!result.contains("== Some heading"), "got: {result}");
    }

    // ── scan_files_recursive ──────────────────────────────────────────────────

    #[test]
    fn scan_files_recursive_finds_nested_matches_and_skips_media_dirs() {
        let dir = std::env::temp_dir().join(format!("zerkalo-scan-test-{}", std::process::id()));
        let sub = dir.join("chapter1");
        let media = dir.join("essay_media");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::create_dir_all(&media).unwrap();
        std::fs::write(dir.join("essay.tex"), "").unwrap();
        std::fs::write(sub.join("notes.tex"), "").unwrap();
        std::fs::write(sub.join("readme.txt"), "").unwrap();
        std::fs::write(media.join("stray.tex"), "").unwrap();

        let mut found = Vec::new();
        scan_files_recursive(&dir, &["tex"], &mut found);
        found.sort();

        assert_eq!(found.len(), 2, "got: {found:?}");
        assert!(found.iter().any(|p| p.ends_with("essay.tex")));
        assert!(found.iter().any(|p| p.ends_with("chapter1/notes.tex")));
        assert!(!found.iter().any(|p| p.to_string_lossy().contains("essay_media")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── summarize_import_content ──────────────────────────────────────────────

    #[test]
    fn summarize_counts_words_headings_images_citations() {
        let text = "= Title\n\nSome words here today @smith2020 and more.\n\n#figure(image(\"a.png\"))\n";
        let summary = summarize_import_content(text);
        assert!(summary.contains("heading"), "got: {summary}");
        assert!(summary.contains("1 image"), "got: {summary}");
        assert!(summary.contains("1 citation"), "got: {summary}");
    }

    #[test]
    fn summarize_ignores_email_like_at_signs() {
        let text = "Contact me at name@example.com for details.";
        let summary = summarize_import_content(text);
        assert!(!summary.contains("citation"), "got: {summary}");
    }

    #[test]
    fn summarize_omits_zero_counts() {
        let text = "Just plain prose with nothing special in it at all.";
        let summary = summarize_import_content(text);
        assert!(!summary.contains("heading"));
        assert!(!summary.contains("image"));
        assert!(!summary.contains("citation"));
        assert!(!summary.contains("equation"));
        assert!(summary.contains("word"));
    }

    // ── post_process_latex_import ─────────────────────────────────────────────

    #[test]
    fn import_discards_formatting_rules() {
        // Simulates a complex pandoc output with set/show rules throughout the file
        let input = "\
#set page(paper: \"a4\", margin: 1in)\n\
#set text(font: \"Arial\", size: 12pt)\n\
#set par(leading: 1em)\n\
#set heading(numbering: \"1.1.\")\n\
#show heading: it => block[#it.body]\n\
\n\
= Introduction\n\
\n\
Some text.\n\
\n\
#bibliography(\"refs.bib\", style: \"apa\")\n";

        let result = post_process_latex_import(input, None);

        // Template block is present
        assert!(result.contains("// ZERKALO-TEMPLATE-BEGIN"), "template block present");
        assert!(result.contains("// ZERKALO-TEMPLATE-END"), "template block closed");

        // Check only the section AFTER the template markers — that's where the
        // user's formatting rules would appear if they weren't discarded.
        // (The template block itself legitimately contains these directives.)
        let after_template = result
            .split("// ZERKALO-TEMPLATE-END")
            .nth(1)
            .unwrap_or("");
        assert!(!after_template.contains("#set page("), "set page not in body");
        assert!(!after_template.contains("#set text("), "set text not in body");
        assert!(!after_template.contains("#set par("), "set par not in body");
        assert!(!after_template.contains("#set heading("), "set heading not in body");
        assert!(!after_template.contains("#show heading"), "show heading not in body");

        // Body content is preserved
        assert!(result.contains("= Introduction"), "heading preserved");
        assert!(result.contains("Some text."), "body text preserved");

        // Bibliography is present
        assert!(after_template.contains("#bibliography("), "bibliography present");
    }

    #[test]
    fn import_moves_macros_to_section() {
        let input = "\
#set text(font: \"Arial\")\n\
#import \"@preview/droplet:0.3.1\": dropcap\n\
#let essay-par(body) = block(width: 100%, body)\n\
\n\
= Heading\n\
\n\
#essay-par[Some text.]\n";

        let result = post_process_latex_import(input, None);

        // Macros are placed after the template block, not discarded
        assert!(result.contains("#import \"@preview/droplet:0.3.1\""), "import preserved");
        assert!(result.contains("#let essay-par"), "let definition preserved");

        // Macros come AFTER the template block
        let template_end = result.find("// ZERKALO-TEMPLATE-END").unwrap();
        let import_pos = result.find("#import").unwrap();
        assert!(import_pos > template_end, "import is after template block");

        // Body content is preserved
        assert!(result.contains("= Heading"), "heading preserved");
        assert!(result.contains("#essay-par[Some text.]"), "macro usage preserved");
    }

    #[test]
    fn import_multiline_show_heading_discarded() {
        let input = "\
#show heading.where(level: 1): it => block(\n\
  width: 100%,\n\
  above: 1em,\n\
)[\n\
  #align(center)[#it.body]\n\
]\n\
\n\
= Body\n";

        let result = post_process_latex_import(input, None);
        let after_template = result
            .split("// ZERKALO-TEMPLATE-END")
            .nth(1)
            .unwrap_or("");
        // The user's custom show rule should not appear in the body
        assert!(!after_template.contains("#show heading"), "multi-line show heading discarded from body");
        // The body inside the show rule should also be gone
        assert!(!after_template.contains("#align(center)[#it.body]"), "show heading body discarded");
        // Actual document content is kept
        assert!(result.contains("= Body"), "actual content kept");
    }

    #[test]
    fn import_inserts_pagebreak_before_first_heading() {
        // When there is content before the first heading (a title block), a
        // pagebreak must be inserted between them.
        let input = "\
#set text(font: \"Arial\")\n\
\n\
Title material here\n\
\n\
= Introduction\n\
\n\
Body.\n";

        let result = post_process_latex_import(input, None);
        let pb = result.find("#pagebreak()").unwrap();
        let h1 = result.find("= Introduction").unwrap();
        assert!(pb < h1, "pagebreak before first heading");
    }

    #[test]
    fn import_body_marker_present() {
        let input = "= Heading\n\nText.\n";
        let result = post_process_latex_import(input, None);
        assert!(result.contains("// ── Document body"), "body marker present");
    }

    #[test]
    fn strip_pandoc_empty_input() {
        assert_eq!(strip_pandoc_preamble(""), "");
    }

    #[test]
    fn strip_pandoc_only_set_rules() {
        let input = "#set text(font: \"Arial\")\n#set page(paper: \"a4\")\n";
        assert_eq!(strip_pandoc_preamble(input), "");
    }

    #[test]
    fn strip_pandoc_preserves_body() {
        let input = "#set text(font: \"Arial\")\n\n= Introduction\n\nBody text.\n";
        let result = strip_pandoc_preamble(input);
        assert_eq!(result, "= Introduction\n\nBody text.\n");
    }

    #[test]
    fn strip_pandoc_multiline_set_rule() {
        let input = "#set text(\n  font: \"Arial\",\n  size: 12pt,\n)\n\n= Heading\n";
        let result = strip_pandoc_preamble(input);
        assert_eq!(result, "= Heading\n");
    }

    #[test]
    fn strip_pandoc_skips_leading_comments() {
        let input = "// Generated by pandoc\n#set text(font: \"Arial\")\n\n= Body\n";
        let result = strip_pandoc_preamble(input);
        assert_eq!(result, "= Body\n");
    }

    #[test]
    fn strip_pandoc_no_preamble() {
        let input = "= Just a heading\n\nSome text.\n";
        let result = strip_pandoc_preamble(input);
        assert_eq!(result, "= Just a heading\n\nSome text.\n");
    }
}
