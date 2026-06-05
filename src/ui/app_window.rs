use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Entry, Label, MenuButton,
    Notebook, Orientation, Paned, Popover, ScrolledWindow, Separator, Stack, ToggleButton,
};
use libadwaita as adw;
use adw::prelude::*;

use crate::bibliography;
use crate::config::{Config, ProjectConfig, Theme};
use crate::writing_log::{WritingLog, count_words, new_file_start_words, FileStartWords};
use crate::git_sync;
use crate::keybindings::{matches_binding, Keybindings};
use crate::lsp::{DiagSeverity, LspClient};
use crate::project_model::ProjectModel;
use crate::session::Session;
use super::command_palette::{CommandPalette, default_commands, heading_items};
use super::dep_graph::DepGraph;
use super::docs_browser::DocsBrowser;
use super::editor_pane::EditorPane;
use super::file_tree::FileTree;
use super::font_manager::FontManager;
use super::error_panel::{parse_typst_errors, CompileError, ErrorPanel, Severity};
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
use super::plan_panel::PlanPanel;

pub struct AppWindow {
    window: adw::ApplicationWindow,
    editor_pane: EditorPane,
    preview_pane: PreviewPane,
    #[allow(dead_code)]
    error_panel: ErrorPanel,
    #[allow(dead_code)]
    outline_panel: OutlinePanel,
    project_root: PathBuf,
    #[allow(dead_code)]
    project_model: ProjectModel,
    sync_btn: Button,
    search_panel: super::search_panel::SearchPanel,
    #[allow(dead_code)]
    toast_overlay: adw::ToastOverlay,
    file_tree: FileTree,
    writing_log: Rc<RefCell<WritingLog>>,
    file_start_words: FileStartWords,
    session_start: Rc<RefCell<std::time::Instant>>,
}

impl AppWindow {
    pub fn new(app: &adw::Application, config: Config) -> Self {
        let project_root = config.work_dir.clone();

        let window = adw::ApplicationWindow::new(app);
        window.set_title(Some("Zerkalo"));
        window.set_default_width(1600);
        window.set_default_height(1000);

        // ── Application-wide accent CSS ─────────────────────────────────────
        load_app_css();

        // ── Per-project config ──────────────────────────────────────────────

        let proj_cfg = ProjectConfig::load(&project_root).unwrap_or_default();
        let effective_bib = proj_cfg.bib_path.clone().or_else(|| config.bib_path.clone());
        let effective_output_dir = proj_cfg.output_dir.clone();
        let extra_compiler_args = proj_cfg.compiler_args.clone();

        // ── Runtime-configurable values ─────────────────────────────────────

        let debounce_ms: Rc<RefCell<u64>> = Rc::new(RefCell::new(config.debounce_ms));
        let auto_compile: Rc<RefCell<bool>> = Rc::new(RefCell::new(config.auto_compile));
        let current_config: Rc<RefCell<Config>> = Rc::new(RefCell::new(config.clone()));

        // ── Header bar ──────────────────────────────────────────────────────

        let header = adw::HeaderBar::new();

        // Start: sidebar toggle + insert panel toggle (flat, left side)
        let sidebar_btn = Button::from_icon_name("sidebar-show-symbolic");
        sidebar_btn.set_tooltip_text(Some("Toggle sidebar"));
        sidebar_btn.add_css_class("flat");
        sidebar_btn.update_property(&[gtk4::accessible::Property::Label("Toggle sidebar")]);
        header.pack_start(&sidebar_btn);

        let focus_btn = ToggleButton::new();
        focus_btn.set_icon_name("view-fullscreen-symbolic");
        focus_btn.set_tooltip_text(Some("Focus mode — hide sidebar and preview"));
        focus_btn.add_css_class("flat");
        focus_btn.update_property(&[gtk4::accessible::Property::Label("Toggle focus mode")]);
        header.pack_start(&focus_btn);

        // Style switcher dropdown — placed in header start, beside the title
        let style_names = crate::styles::STYLES.iter().map(|(n, _, _, _, _)| *n).collect::<Vec<_>>();
        let style_box = GtkBox::new(Orientation::Vertical, 0);
        style_box.set_margin_top(4);
        style_box.set_margin_bottom(4);
        let style_popover = Popover::new();
        style_popover.set_child(Some(&style_box));
        let style_btn = MenuButton::new();
        style_btn.set_label("Style");
        style_btn.add_css_class("flat");
        style_btn.set_tooltip_text(Some("Apply a formatting style to the document"));
        style_btn.set_popover(Some(&style_popover));
        header.pack_start(&style_btn);
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
        todo_btn.set_icon_name("text-editor-symbolic");
        todo_btn.set_tooltip_text(Some("Toggle plan panel"));
        todo_btn.add_css_class("flat");
        todo_btn.set_active(false);
        todo_btn.update_property(&[gtk4::accessible::Property::Label("Toggle plan panel")]);

        // ── Primary header buttons (packed together at end of section) ────────
        let compile_btn = Button::with_label("Preview");
        compile_btn.set_tooltip_text(Some("Compile & Preview (Ctrl+Shift+P)"));
        compile_btn.add_css_class("suggested-action");
        compile_btn.add_css_class("pill");

        let sync_btn = Button::from_icon_name("vcs-push-symbolic");
        sync_btn.set_tooltip_text(Some("Commit & Push to Git (Ctrl+Shift+G)"));
        sync_btn.add_css_class("flat");
        sync_btn.update_property(&[gtk4::accessible::Property::Label("Commit and push to Git")]);

        // ── Hamburger menu items (using make_menu_item for left+shortcut layout) ──
        let HamburgerItems {
            menu_new_template_item,
            menu_reapply_template_item,
            menu_new_item,
            menu_open_item,
            menu_open_project_item,
            menu_recent_projects_item,
            menu_save_item,
            menu_save_as_item,
            menu_export_item,
            menu_import_item,
            menu_docs_item,
            menu_fonts_item,
            menu_settings_item,
            menu_setup_item,
            menu_backup_remote_item,
            menu_help_item,
            menu_writing_stats_item,
            menu_about_item,
            menu_import_latex_item,
            menu_import_docx_item,
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
        menu_popover_box.append(&menu_open_project_item);
        menu_popover_box.append(&menu_recent_projects_item);
        menu_popover_box.append(&Separator::new(Orientation::Horizontal));
        // Save
        menu_popover_box.append(&menu_save_item);
        menu_popover_box.append(&menu_save_as_item);
        menu_popover_box.append(&Separator::new(Orientation::Horizontal));
        // Convert / share
        menu_popover_box.append(&menu_export_item);
        menu_popover_box.append(&menu_import_item);
        menu_popover_box.append(&Separator::new(Orientation::Horizontal));
        // View
        menu_popover_box.append(&menu_docs_item);
        menu_popover_box.append(&Separator::new(Orientation::Horizontal));
        // App settings
        menu_popover_box.append(&menu_fonts_item);
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

        // Header end section layout (left → right): sync | todo | Preview | ≡
        // In GTK4 pack_end the last-packed widget is leftmost in the end section.
        header.pack_end(&menu_btn);
        header.pack_end(&compile_btn);
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
        header.set_title_widget(Some(&file_selector));

        // ── Panels ──────────────────────────────────────────────────────────

        let editor_pane = EditorPane::new();
        let project_model = ProjectModel::scan(project_root.clone());
        let outline_panel = OutlinePanel::new();
        let citation_panel = CitationPanel::new();
        let ref_manager = RefManager::new();
        let dep_graph = DepGraph::new(project_root.clone());
        let package_browser = PackageBrowser::new();
        let todo_panel = PlanPanel::new(config.work_dir.clone());

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
                btn.connect_clicked(move |_| {
                    pop.popdown();
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

        // Wire outline heading click → jump to line in editor
        {
            let ep = editor_pane.clone();
            outline_panel.set_on_jump(move |path, line| {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    ep.open_file(path.clone(), &content);
                }
                ep.jump_to_line(&path, line);
            });
        }

        // Wire cursor movement → outline auto-select and preview scroll
        // preview_pane_ref is populated after preview_pane is created below.
        let preview_pane_for_heading: Rc<RefCell<Option<PreviewPane>>> =
            Rc::new(RefCell::new(None));
        {
            let op = outline_panel.clone();
            let ep = editor_pane.clone();
            let pp_ref = preview_pane_for_heading.clone();
            editor_pane.set_on_cursor_heading(move |_path, heading_line| {
                op.select_for_line(heading_line);
                if let Some(ref pp) = *pp_ref.borrow() {
                    let total = ep.active_line_count().max(1);
                    let page_count = pp.page_count();
                    if page_count > 0 {
                        let page_idx = ((heading_line as f64 / total as f64)
                            * page_count as f64) as usize;
                        let page_idx = page_idx.min(page_count - 1);
                        pp.scroll_to_page(page_idx);
                    }
                }
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
                    let btn = Button::new();
                    btn.add_css_class("flat");
                    btn.set_hexpand(true);
                    let row_box = GtkBox::new(Orientation::Vertical, 2);
                    row_box.set_margin_start(10);
                    row_box.set_margin_end(10);
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
                    btn.connect_clicked(move |_| {
                        if let Ok(content) = std::fs::read_to_string(&p) {
                            ep.open_file(p.clone(), &content);
                        }
                        pop.popdown();
                    });
                    open_list_rc.append(&btn);
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

        if let Some(f) = &project_model.root_file {
            tracing::info!("Detected root file: {}", f.display());
        }

        let initial_root = if let Some(rel) = &proj_cfg.root_file {
            let abs = project_root.join(rel);
            if abs.exists() { Some(abs) } else { project_model.root_file.clone() }
        } else {
            project_model.root_file.clone()
        };
        let preview_pane = PreviewPane::new(
            initial_root,
            effective_output_dir,
            extra_compiler_args,
        );
        *preview_pane_for_heading.borrow_mut() = Some(preview_pane.clone());
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

        // ── Citation panel: insert @key at cursor ─────────────────────────────

        {
            let ep = editor_pane.clone();
            citation_panel.set_on_insert(move |key| ep.insert_at_cursor(&format!("@{key}")));
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

        // ── Focus mode toggle — dims sidebar, hides preview ────────────────
        let focus_active_c = focus_active.clone();
        let preview_vis_for_focus = preview_vis_holder.clone();
        let rsh_for_focus = right_sidebar_holder.clone();
        let todo_btn_for_focus = todo_btn.clone();
        let window_for_focus = window.clone();
        let editor_for_focus = editor_pane.clone();
        focus_btn.connect_toggled(move |btn| {
            let focused = btn.is_active();
            *focus_active_c.borrow_mut() = focused;
            // Toggle CSS class to dim the sidebar (opacity via CSS)
            if focused {
                window_for_focus.add_css_class("zen-writing");
            } else {
                window_for_focus.remove_css_class("zen-writing");
            }
            // Constrain editor to a comfortable reading width in zen mode
            editor_for_focus.set_zen_width(focused);
            if let Some(pc) = preview_vis_for_focus.borrow().as_ref() {
                pc.set_visible(!focused);
            }
            if let Some(rs) = rsh_for_focus.borrow().as_ref() {
                rs.set_visible(!focused && todo_btn_for_focus.is_active());
            }
        });

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

        // ── Compile button ──────────────────────────────────────────────────

        let preview_for_btn = preview_pane.clone();
        let editor_for_btn = editor_pane.clone();
        compile_btn.connect_clicked(move |_| {
            if let Some(path) = editor_for_btn.get_active_path() {
                if let Some(content) = editor_for_btn.get_active_content() {
                    preview_for_btn.set_buffer_snapshot(path.clone(), content);
                }
                preview_for_btn.set_root_file(path);
            }
            preview_for_btn.trigger_compile();
        });

        // ── Menu: Settings ──────────────────────────────────────────────────

        let window_for_settings = window.clone();
        let editor_for_settings = editor_pane.clone();
        let debounce_for_settings = debounce_ms.clone();
        let auto_compile_for_settings = auto_compile.clone();
        let current_config_for_settings = current_config.clone();
        let menu_popover_for_settings = menu_popover.clone();
        let import_item_for_settings = menu_import_item.clone();
        menu_settings_item.connect_clicked(move |_| {
            menu_popover_for_settings.popdown();
            let dialog = SettingsDialog::new(
                &window_for_settings,
                &current_config_for_settings.borrow(),
            );
            let editor = editor_for_settings.clone();
            let debounce = debounce_for_settings.clone();
            let auto_flag = auto_compile_for_settings.clone();
            let cfg_rc = current_config_for_settings.clone();
            let window_for_save = window_for_settings.clone();
            let import_item_save = import_item_for_settings.clone();

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
                *cfg_rc.borrow_mut() = new_cfg;
            });
            dialog.present();
        });

        // ── Menu: Help ──────────────────────────────────────────────────────

        let window_for_help = window.clone();
        let menu_popover_for_help = menu_popover.clone();
        menu_help_item.connect_clicked(move |_| {
            menu_popover_for_help.popdown();
            HelpWindow::new(&window_for_help).present();
        });

        // ── Menu: Open Project Folder ───────────────────────────────────────

        let window_for_open_proj = window.clone();
        let cfg_for_open_proj = current_config.clone();
        let menu_popover_for_open_proj = menu_popover.clone();
        menu_open_project_item.connect_clicked(move |_| {
            menu_popover_for_open_proj.popdown();
            let dlg = gtk4::FileDialog::builder()
                .title("Open Project Folder")
                .modal(true)
                .build();
            let cfg_c = cfg_for_open_proj.clone();
            dlg.select_folder(
                Some(&window_for_open_proj),
                None::<&gtk4::gio::Cancellable>,
                move |result| {
                    if let Ok(gfile) = result {
                        if let Some(folder) = gfile.path() {
                            let mut cfg = cfg_c.borrow_mut();
                            cfg.push_recent_project(folder.clone());
                            cfg.work_dir = folder;
                            let _ = cfg.save();
                            if let Ok(exe) = std::env::current_exe() {
                                let _ = std::process::Command::new(exe).spawn();
                            }
                            std::process::exit(0);
                        }
                    }
                },
            );
        });

        // ── Menu: Recent Projects ───────────────────────────────────────────

        let window_for_recent_proj = window.clone();
        let cfg_for_recent_proj = current_config.clone();
        let menu_popover_for_recent_proj = menu_popover.clone();
        menu_recent_projects_item.connect_clicked(move |_| {
            menu_popover_for_recent_proj.popdown();
            let projects = cfg_for_recent_proj.borrow().recent_projects.clone();
            if projects.is_empty() {
                let dlg = adw::MessageDialog::new(
                    Some(&window_for_recent_proj),
                    Some("No recent projects"),
                    Some("Open a project folder to add it to this list."),
                );
                dlg.add_response("ok", "OK");
                dlg.present();
                return;
            }
            let dialog = adw::Window::builder()
                .title("Recent Projects")
                .transient_for(&window_for_recent_proj)
                .modal(true)
                .default_width(420)
                .default_height(320)
                .build();
            let header = adw::HeaderBar::new();
            let list = gtk4::ListBox::new();
            list.add_css_class("boxed-list");
            list.set_margin_start(12);
            list.set_margin_end(12);
            list.set_margin_top(12);
            list.set_margin_bottom(12);
            for proj in &projects {
                let row = adw::ActionRow::new();
                let name = proj.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                row.set_title(name);
                row.set_subtitle(&proj.to_string_lossy());
                row.set_activatable(true);
                row.set_widget_name(&proj.to_string_lossy());
                list.append(&row);
            }
            let scroll = gtk4::ScrolledWindow::new();
            scroll.set_vexpand(true);
            scroll.set_child(Some(&list));
            let body = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
            body.append(&scroll);
            let tv = adw::ToolbarView::new();
            tv.add_top_bar(&header);
            tv.set_content(Some(&body));
            dialog.set_content(Some(&tv));
            let cfg_c2 = cfg_for_recent_proj.clone();
            let win_c = dialog.clone();
            list.connect_row_activated(move |_, row| {
                let path = std::path::PathBuf::from(row.widget_name().to_string());
                let mut cfg = cfg_c2.borrow_mut();
                cfg.push_recent_project(path.clone());
                cfg.work_dir = path;
                let _ = cfg.save();
                win_c.close();
                if let Ok(exe) = std::env::current_exe() {
                    let _ = std::process::Command::new(exe).spawn();
                }
                std::process::exit(0);
            });
            dialog.present();
        });

        // ── Menu: Setup & Onboarding ────────────────────────────────────────

        let window_for_setup = window.clone();
        let root_for_setup = project_root.clone();
        let menu_popover_for_setup = menu_popover.clone();
        menu_setup_item.connect_clicked(move |_| {
            menu_popover_for_setup.popdown();
            super::setup_wizard::SetupWizard::new(&window_for_setup, &root_for_setup).present();
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
        menu_export_item.connect_clicked(move |_| {
            menu_popover_for_export.popdown();
            let initial_fmt = current_config_for_export.borrow().last_export_format;
            let cfg_for_save = current_config_for_export.clone();
            ExportDialog::new(
                &window_for_export,
                preview_for_export.root_file_path(),
                preview_for_export.output_dir(),
                initial_fmt,
                move |fmt| {
                    let mut cfg = cfg_for_save.borrow_mut();
                    cfg.last_export_format = fmt;
                    let _ = cfg.save();
                },
            )
            .present();
        });

        // ── Menu: Font Management ───────────────────────────────────────────

        let window_for_fonts = window.clone();
        let menu_popover_for_fonts = menu_popover.clone();
        menu_fonts_item.connect_clicked(move |_| {
            menu_popover_for_fonts.popdown();
            FontManager::new(&window_for_fonts).present();
        });

        // ── Menu: Import (picker dialog) ───────────────────────────────────

        let window_for_import = window.clone();
        let menu_popover_for_import = menu_popover.clone();
        let latex_item_for_dlg = menu_import_latex_item.clone();
        let docx_item_for_dlg = menu_import_docx_item.clone();
        let pdf_item_for_dlg = menu_import_pdf_item.clone();
        menu_import_item.connect_clicked(move |_| {
            menu_popover_for_import.popdown();

            let dlg = adw::Window::new();
            dlg.set_title(Some("Import File"));
            dlg.set_default_width(280);
            dlg.set_modal(true);
            dlg.set_transient_for(Some(&window_for_import));
            dlg.set_deletable(true);

            let header_dlg = adw::HeaderBar::new();
            let title_lbl = gtk4::Label::new(Some("Import File"));
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
            let latex_row = make_row("text-x-generic-symbolic", "LaTeX (.tex)");
            let docx_row = make_row("x-office-document-symbolic", "Word (.docx)");
            let pdf_row = make_row("application-pdf-symbolic", "PDF (.pdf)");

            let group = adw::PreferencesGroup::new();
            group.add(&latex_row);
            group.add(&docx_row);
            group.add(&pdf_row);
            group.set_margin_start(12);
            group.set_margin_end(12);
            row_box.append(&group);

            let vbox = GtkBox::new(Orientation::Vertical, 0);
            vbox.append(&header_dlg);
            vbox.append(&row_box);
            dlg.set_content(Some(&vbox));

            // Wire each row to forward-click the hidden original import buttons
            let latex_trigger = latex_item_for_dlg.clone();
            let dlg_c = dlg.clone();
            latex_row.connect_activated(move |_| {
                dlg_c.close();
                latex_trigger.emit_clicked();
            });
            let docx_trigger = docx_item_for_dlg.clone();
            let dlg_c = dlg.clone();
            docx_row.connect_activated(move |_| {
                dlg_c.close();
                docx_trigger.emit_clicked();
            });
            let pdf_trigger = pdf_item_for_dlg.clone();
            let dlg_c = dlg.clone();
            pdf_row.connect_activated(move |_| {
                dlg_c.close();
                pdf_trigger.emit_clicked();
            });

            dlg.present();
        });

        // ── Menu: Import LaTeX ──────────────────────────────────────────────

        let window_for_latex = window.clone();
        let editor_for_latex = editor_pane.clone();
        let menu_popover_for_latex = menu_popover.clone();
        let work_dir_for_latex = project_root.clone();
        let config_for_latex = current_config.clone();
        menu_import_latex_item.connect_clicked(move |_| {
            menu_popover_for_latex.popdown();
            let dialog = gtk4::FileDialog::new();
            dialog.set_title("Import LaTeX File");
            let filter = gtk4::FileFilter::new();
            filter.set_name(Some("LaTeX files (*.tex)"));
            filter.add_pattern("*.tex");
            let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
            filters.append(&filter);
            dialog.set_filters(Some(&filters));
            dialog.set_initial_folder(Some(&gtk4::gio::File::for_path(&work_dir_for_latex)));
            let win2 = window_for_latex.clone();
            let ep2 = editor_for_latex.clone();
            let cfg2 = config_for_latex.clone();
            let win_ref = win2.clone();
            dialog.open(Some(&win_ref), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(input_path) = file.path() {
                        let stem = input_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output").to_string();
                        let out_path = input_path.with_file_name(format!("{stem}.typ"));
                        let output = std::process::Command::new("pandoc")
                            .arg(&input_path)
                            .arg("-f").arg("latex")
                            .arg("-t").arg("typst")
                            .arg("--standalone")
                            .arg("-o").arg(&out_path)
                            .output();
                        match output {
                            Ok(o) if o.status.success() => {
                                if let Ok(raw) = std::fs::read_to_string(&out_path) {
                                    let bib_path = cfg2.borrow().bib_path.clone();
                                    let processed = post_process_latex_import(&raw, bib_path.as_deref());
                                    let _ = std::fs::write(&out_path, &processed);
                                    ep2.open_file(out_path, &processed);
                                }
                            }
                            Ok(o) => {
                                let msg = String::from_utf8_lossy(&o.stderr);
                                show_alert(&win2, "Import Failed", &format!("pandoc error:\n{}", msg.lines().take(5).collect::<Vec<_>>().join("\n")));
                            }
                            Err(_) => {
                                show_alert(&win2, "Import Failed",
                                    "pandoc was not found. Install it to use LaTeX import:\n\
                                     \n  zypper install pandoc\
                                     \n  apt   install pandoc\
                                     \n  brew  install pandoc\
                                     \n  dnf   install pandoc\
                                     \nVersion 3.1 or later is required.");
                            }
                        }
                    }
                }
            });
        });

        // ── Menu: Import DOCX ──────────────────────────────────────────────

        let window_for_docx = window.clone();
        let editor_for_docx = editor_pane.clone();
        let menu_popover_for_docx = menu_popover.clone();
        let work_dir_for_docx = project_root.clone();
        let config_for_docx = current_config.clone();
        menu_import_docx_item.connect_clicked(move |_| {
            menu_popover_for_docx.popdown();
            let dialog = gtk4::FileDialog::new();
            dialog.set_title("Import DOCX File");
            let filter = gtk4::FileFilter::new();
            filter.set_name(Some("Word documents (*.docx)"));
            filter.add_pattern("*.docx");
            let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
            filters.append(&filter);
            dialog.set_filters(Some(&filters));
            dialog.set_initial_folder(Some(&gtk4::gio::File::for_path(&work_dir_for_docx)));
            let win2 = window_for_docx.clone();
            let ep2 = editor_for_docx.clone();
            let cfg2 = config_for_docx.clone();
            let win_ref = win2.clone();
            dialog.open(Some(&win_ref), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(input_path) = file.path() {
                        let stem = input_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output").to_string();
                        let out_path = input_path.with_file_name(format!("{stem}.typ"));
                        let output = std::process::Command::new("pandoc")
                            .arg(&input_path)
                            .arg("-f").arg("docx")
                            .arg("-t").arg("typst")
                            .arg("--standalone")
                            .arg("-o").arg(&out_path)
                            .output();
                        match output {
                            Ok(o) if o.status.success() => {
                                if let Ok(raw) = std::fs::read_to_string(&out_path) {
                                    let bib_path = cfg2.borrow().bib_path.clone();
                                    let processed = post_process_latex_import(&raw, bib_path.as_deref());
                                    let _ = std::fs::write(&out_path, &processed);
                                    ep2.open_file(out_path, &processed);
                                }
                            }
                            Ok(o) => {
                                let msg = String::from_utf8_lossy(&o.stderr);
                                show_alert(&win2, "Import Failed", &format!("pandoc error:\n{}", msg.lines().take(5).collect::<Vec<_>>().join("\n")));
                            }
                            Err(_) => {
                                show_alert(&win2, "Import Failed",
                                    "pandoc was not found. Install it to use DOCX import:\n\
                                     \n  zypper install pandoc\
                                     \n  apt   install pandoc\
                                     \n  brew  install pandoc\
                                     \n  dnf   install pandoc\
                                     \nVersion 3.1 or later is required.");
                            }
                        }
                    }
                }
            });
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
                        let stem = input_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output").to_string();
                        let out_path = input_path.with_file_name(format!("{stem}.typ"));
                        let output = std::process::Command::new("pdftotext")
                            .arg("-layout")
                            .arg(&input_path)
                            .arg("-")
                            .output();
                        match output {
                            Ok(o) if o.status.success() => {
                                let extracted = String::from_utf8_lossy(&o.stdout).to_string();
                                let typst_doc = post_process_pdf_import(&extracted, stem.as_str());
                                let _ = std::fs::write(&out_path, &typst_doc);
                                ep2.open_file(out_path, &typst_doc);
                            }
                            Ok(_) => {
                                show_alert(&win2, "Import Failed", "pdftotext could not extract text from this PDF.");
                            }
                            Err(_) => {
                                show_alert(&win2, "Import Failed",
                                    "pdftotext was not found. Install poppler-utils to use PDF import:\n\
                                     \n  zypper install poppler-tools\
                                     \n  apt   install poppler-utils\
                                     \n  brew  install poppler\
                                     \n  dnf   install poppler-utils");
                            }
                        }
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
            let dlg = TemplateDialog::new(&window_for_template, &project_root_for_template);
            dlg.set_bib_path(cfg_for_template.borrow().bib_path.clone());
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
        menu_reapply_template_item.connect_clicked(move |_| {
            menu_popover_for_reapply.popdown();
            let Some(current_path) = editor_for_reapply.get_active_path() else { return };
            let current_content = editor_for_reapply.get_active_content().unwrap_or_default();
            let dlg = TemplateDialog::new(&window_for_reapply, &project_root_for_reapply);
            dlg.set_bib_path(cfg_for_reapply.borrow().bib_path.clone());

            if let Some(sidecar) = super::template_dialog::load_sidecar(&current_path) {
                dlg.preselect_from_sidecar(&sidecar);
            } else {
                dlg.preselect_style(
                    &super::template_dialog::parse_style_key(&current_content)
                        .unwrap_or_default(),
                );
                if let Some(f) = super::template_dialog::parse_font(&current_content) {
                    dlg.preselect_font(&f);
                }
                if let Some(p) = super::template_dialog::parse_paper(&current_content) {
                    dlg.preselect_paper(&p);
                }
                if let Some(s) = super::template_dialog::parse_spacing(&current_content) {
                    dlg.preselect_spacing(&s);
                }
                dlg.preselect_margin(super::template_dialog::parse_margin(&current_content));
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
            }
            // Always read metadata from the document — the user may have edited the
            // #let doc-* variables directly, and the sidecar won't reflect those changes.
            dlg.preselect_metadata(
                &super::template_dialog::parse_meta(&current_content, "title"),
                &super::template_dialog::parse_meta(&current_content, "subtitle"),
                &super::template_dialog::parse_meta(&current_content, "author"),
                &super::template_dialog::parse_meta(&current_content, "affiliation"),
                &super::template_dialog::parse_meta(&current_content, "course"),
                &super::template_dialog::parse_meta(&current_content, "date"),
            );

            let ep = editor_for_reapply.clone();
            let win_for_apply = window_for_reapply.clone();
            dlg.set_on_apply(move |new_content, sidecar| {
                let do_apply = {
                    let cc = current_content.clone();
                    let nc = new_content.clone();
                    let sc = sidecar.clone();
                    let path = current_path.clone();
                    let ep2 = ep.clone();
                    move || {
                        let updated = super::template_dialog::apply_body_splice(&cc, &nc);
                        super::template_dialog::save_sidecar(&path, &sc);
                        if let Err(e) = std::fs::write(&path, &updated) {
                            tracing::error!("Failed to write updated template: {e}");
                        } else {
                            ep2.reload_file(path.clone(), &updated);
                        }
                    }
                };

                if super::template_dialog::has_body_marker(&current_content) {
                    do_apply();
                } else {
                    let confirm = adw::MessageDialog::new(
                        Some(&win_for_apply),
                        Some("Replace entire document?"),
                        Some("This document has no body marker, so the template \
                              will replace the whole file. Your current text will \
                              be lost. Make sure you have a backup."),
                    );
                    confirm.add_response("cancel", "Cancel");
                    confirm.add_response("replace", "Replace Document");
                    confirm.set_response_appearance(
                        "replace",
                        adw::ResponseAppearance::Destructive,
                    );
                    confirm.set_default_response(Some("cancel"));
                    confirm.set_close_response("cancel");
                    confirm.connect_response(None, move |_, id| {
                        if id == "replace" {
                            do_apply();
                        }
                    });
                    confirm.present();
                }
            });
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
                            let _ = std::fs::write(&path, "// New document\n\n");
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
        menu_save_item.connect_clicked(move |_| {
            menu_popover_for_save.popdown();
            if editor_for_menu_save.save_current().is_some() {
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
            let win_c = window_for_save_as.clone();
            let ep_c = editor_for_save_as.clone();
            let pv_c = preview_for_save_as.clone();
            dialog.save(Some(&win_c), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        if std::fs::write(&path, content.as_bytes()).is_ok() {
                            ep_c.open_file(path.clone(), &content);
                            pv_c.set_root_file(path);
                            pv_c.trigger_compile();
                        }
                    }
                }
            });
        });

        // ── Sync button ─────────────────────────────────────────────────────

        let project_root_for_sync = project_root.clone();
        let window_for_sync = window.clone();
        let sync_btn_ref = sync_btn.clone();
        let editor_for_sync = editor_pane.clone();
        let toast_overlay = adw::ToastOverlay::new();
        let toast_for_sync_btn = toast_overlay.clone();
        let toast_for_sync_closure = toast_overlay.clone();
        sync_btn.connect_clicked(move |_| {
            editor_for_sync.save_all_modified();
            let root = project_root_for_sync.clone();
            let win = window_for_sync.clone();
            let btn = sync_btn_ref.clone();
            let toasts = toast_for_sync_closure.clone();

            if !git_sync::has_remote(&root) {
                let dialog = SyncDialog::new(&win);
                let root2 = root.clone();
                let win2 = win.clone();
                let btn2 = btn.clone();
                let toasts2 = toasts.clone();

                let confirmed = Rc::new(RefCell::new(false));
                let confirmed_set = confirmed.clone();
                dialog.set_on_confirm(move |url| {
                    *confirmed_set.borrow_mut() = true;
                    match git_sync::add_remote(&root2, &url) {
                        Ok(()) => do_sync(root2.clone(), win2.clone(), toasts2.clone(), btn2.clone()),
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

            do_sync(root, win, toasts, btn);
        });

        // ── Debounced compile + outline update + LSP ────────────────────────

        let preview_for_change = preview_pane.clone();
        let editor_for_change = editor_pane.clone();
        let debounce_for_change = debounce_ms.clone();
        let auto_compile_for_change = auto_compile.clone();
        let outline_for_change = outline_panel.clone();
        let refs_for_change = ref_manager.clone();
        let lsp_for_change = lsp_client.clone();
        let gen: Rc<RefCell<u64>> = Rc::new(RefCell::new(0));
        let gen2 = gen.clone();
        let editor_pane_for_delta = editor_pane.clone();
        editor_pane.set_on_change(move || {
            *gen2.borrow_mut() += 1;
            let my_gen = *gen2.borrow();
            let preview = preview_for_change.clone();
            let editor = editor_for_change.clone();
            let gen3 = gen2.clone();
            let auto = auto_compile_for_change.clone();
            let outline = outline_for_change.clone();
            let refs = refs_for_change.clone();
            let lsp = lsp_for_change.clone();
            let delay = Duration::from_millis(*debounce_for_change.borrow());
            let delta = editor_pane_for_delta.get_active_session_delta();
            editor_pane_for_delta.set_session_delta(delta);
            glib::timeout_add_local(delay, move || {
                if *gen3.borrow() == my_gen {
                    if *auto.borrow() {
                        if let Some(path) = editor.get_active_path() {
                            if let Some(content) = editor.get_active_content() {
                                preview.set_buffer_snapshot(path.clone(), content);
                            }
                            preview.set_root_file(path);
                        }
                        preview.trigger_compile();
                    }
                    // Outline + ref manager update
                    if let Some(path) = editor.get_active_path() {
                        if let Some(content) = editor.get_active_content() {
                            outline.update(&content, &path);
                            refs.update_used_keys(&content);
                        }
                    }
                    // LSP didChange
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

        // ── Outline + title: update on tab switch ──────────────────────────

        let outline_for_switch = outline_panel.clone();
        let refs_for_switch = ref_manager.clone();
        let dep_graph_for_switch = dep_graph.clone();
        let title_widget_for_switch = file_title_widget.clone();
        let preview_for_switch = preview_pane.clone();
        let todo_panel_for_switch = todo_panel.clone();
        let style_btn_for_switch = style_btn.clone();
        let editor_pane_for_switch_delta = editor_pane.clone();
        editor_pane.set_on_page_switch(move |content, path| {
            let delta = editor_pane_for_switch_delta.get_active_session_delta();
            editor_pane_for_switch_delta.set_session_delta(delta);
            outline_for_switch.update(&content, &path);
            refs_for_switch.update_used_keys(&content);
            dep_graph_for_switch.refresh(Some(&path));
            preview_for_switch.set_buffer_snapshot(path.clone(), content.clone());
            preview_for_switch.set_root_file(path.clone());
            preview_for_switch.trigger_compile();
            todo_panel_for_switch.set_current_file(Some(&path));
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                let display = name.strip_suffix(".typ").unwrap_or(name);
                title_widget_for_switch.set_title(display);
            }
            let basename = path.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.strip_suffix(".typ").unwrap_or(s).to_string())
                .unwrap_or_default();
            let style_name = super::template_dialog::parse_style_key(&content)
                .and_then(|key| super::template_dialog::style_name_for_key(&key))
                .unwrap_or("Style");
            let style_label = if basename.is_empty() {
                style_name.to_string()
            } else {
                format!("{style_name} · {basename}")
            };
            style_btn_for_switch.set_label(&style_label);
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
        let editor_for_recovery = editor_pane.clone();
        let window_for_recovery = window.clone();
        let style_btn_for_open = style_btn.clone();
        let file_start_words_for_open = file_start_words.clone();
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
            let basename = path.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.strip_suffix(".typ").unwrap_or(s).to_string())
                .unwrap_or_default();
            let style_name = super::template_dialog::parse_style_key(&content)
                .and_then(|key| super::template_dialog::style_name_for_key(&key))
                .unwrap_or("Style");
            let style_label = if basename.is_empty() {
                style_name.to_string()
            } else {
                format!("{style_name} · {basename}")
            };
            style_btn_for_open.set_label(&style_label);
            let mut cfg = current_config_for_open.borrow_mut();
            cfg.push_recent(path.clone());
            let _ = cfg.save();

            // Auto-save recovery check
            if let Some((recovered, save_time)) = crate::auto_save::find_recovery(&path) {
                let ts = chrono::DateTime::<chrono::Local>::from(save_time)
                    .format("%H:%M:%S")
                    .to_string();
                let dlg = adw::MessageDialog::new(
                    Some(&window_for_recovery),
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
                let ep = editor_for_recovery.clone();
                let path_c = path.clone();
                dlg.connect_response(None, move |_, resp| {
                    if resp == "restore" {
                        ep.set_content(&path_c, &recovered);
                    }
                    crate::auto_save::clear(&path_c);
                });
                dlg.present();
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

        let error_panel_for_compile = error_panel.clone();
        let editor_for_diag = editor_pane.clone();
        let root_for_compile = project_root.clone();
        let popout_pane_for_compile = popout_pane.clone();
        let dep_graph_for_compile = dep_graph.clone();
        let lsp_diags_for_compile = lsp_has_diags.clone();
        let error_banner_for_compile = error_banner_scroll.clone();
        let error_banner_lbl_for_compile = error_banner.clone();
        let file_tree_holder_for_compile = file_tree_holder.clone();
        let root_file_for_compile = project_model.root_file.clone();
        let toast_for_compile = toast_overlay.clone();
        preview_pane.set_on_compile_done(move |result| {
            match &result {
                None => {
                    error_panel_for_compile.clear();
                    error_panel_for_compile.widget().set_visible(false);
                    editor_for_diag.clear_diagnostic_marks();
                    editor_for_diag.set_diag_summary(0, 0);
                    error_banner_for_compile.set_visible(false);
                    error_banner_lbl_for_compile.set_visible(false);
                    if let Some(ref p) = root_file_for_compile {
                        if let Some(ft) = file_tree_holder_for_compile.borrow().as_ref() {
                            ft.set_file_error(p.as_path(), false);
                        }
                    }
                    let t = adw::Toast::new("Compiled successfully");
                    t.set_timeout(2);
                    toast_for_compile.add_toast(t);
                }
                Some(stderr) => {
                    // Show first error line in the inline banner above the preview toolbar
                    let first_line = stderr.lines().next().unwrap_or("Compile error").to_string();
                    error_banner_lbl_for_compile.set_text(&first_line);
                    error_banner_lbl_for_compile.set_visible(true);
                    error_banner_for_compile.set_visible(true);
                    if let Some(ref p) = root_file_for_compile {
                        if let Some(ft) = file_tree_holder_for_compile.borrow().as_ref() {
                            ft.set_file_error(p.as_path(), true);
                        }
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
                    let diags: Vec<(std::path::PathBuf, u32, bool)> = errors
                        .iter()
                        .map(|e| (e.file.clone(), e.line, matches!(e.severity, Severity::Error)))
                        .collect();
                    let err_count = diags.iter().filter(|(_, _, is_err)| *is_err).count() as u32;
                    let warn_count = diags.iter().filter(|(_, _, is_err)| !*is_err).count() as u32;
                    editor_for_diag.mark_diagnostics(&diags);
                    error_panel_for_compile.show_errors(errors);
                    error_panel_for_compile.widget().set_visible(true);
                    editor_for_diag.set_diag_summary(err_count, warn_count);
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

        // ── Startup: warn if required tools are missing ──────────────────────

        let win_for_check = window.clone();
        glib::timeout_add_local(Duration::from_millis(900), move || {
            // typst is no longer checked — compilation is built in
            let git_ok = std::process::Command::new("git")
                .arg("--version").output().is_ok();
            let hunspell_ok = std::process::Command::new("hunspell")
                .arg("--version").output().is_ok();
            let pandoc_ok = std::process::Command::new("pandoc")
                .arg("--version").output().is_ok();
            let tinymist_ok = std::process::Command::new("tinymist")
                .arg("--version").output().is_ok();

            if !git_ok {
                tracing::warn!("git not found in PATH");
                show_alert(
                    &win_for_check,
                    "Missing: git",
                    "git was not found. Install it to enable git sync:\n\
                     \n  zypper install git\
                     \n  apt   install git\
                     \n  brew  install git\
                     \n  dnf   install git"
                );
            }
            if !hunspell_ok {
                tracing::warn!("hunspell not found in PATH — spell check disabled");
                show_alert(
                    &win_for_check,
                    "Missing: hunspell",
                    "hunspell was not found. Install it to enable spell checking:\n\
                     \n  zypper install hunspell hunspell-en\
                     \n  apt   install hunspell hunspell-en-us\
                     \n  brew  install hunspell\
                     \n  dnf   install hunspell hunspell-en"
                );
            }
            if !pandoc_ok {
                tracing::info!("pandoc not found — LaTeX/DOCX import disabled");
            }
            if !tinymist_ok {
                tracing::info!("tinymist not found — LSP completions disabled");
                show_alert(
                    &win_for_check,
                    "Optional: tinymist",
                    "tinymist was not found. Install it to enable LSP completions and diagnostics:\n\
                     \n  cargo install tinymist\
                     \n  # or download from: https://github.com/Myriad-Dreamin/tinymist/releases"
                );
            }
            glib::ControlFlow::Break
        });

        // ── Welcome window (shows on install or version upgrade) ─────────────

        let win_for_welcome = window.clone();
        glib::timeout_add_local(Duration::from_millis(1200), move || {
            if super::welcome_window::WelcomeWindow::should_show() {
                super::welcome_window::WelcomeWindow::mark_shown();
                super::welcome_window::WelcomeWindow::new(&win_for_welcome).present();
            }
            glib::ControlFlow::Break
        });

        // ── Auto-save: write modified buffers every 30 seconds ──────────────

        let editor_for_autosave = editor_pane.clone();
        let toast_for_autosave = toast_overlay.clone();
        glib::timeout_add_local(Duration::from_secs(30), move || {
            let buffers: Vec<_> = editor_for_autosave.modified_buffers();
            if !buffers.is_empty() {
                for (path, content) in &buffers {
                    crate::auto_save::save(path, content);
                }
                let t = adw::Toast::new("Autosaved");
                t.set_timeout(2);
                toast_for_autosave.add_toast(t);
            }
            glib::ControlFlow::Continue
        });

        // ── Setup wizard (shows when git identity or remote is missing) ──────

        let win_for_setup2 = window.clone();
        let root_for_setup2 = project_root.clone();
        glib::timeout_add_local(Duration::from_millis(1800), move || {
            if super::setup_wizard::SetupWizard::should_show(&root_for_setup2) {
                super::setup_wizard::SetupWizard::new(&win_for_setup2, &root_for_setup2).present();
            }
            glib::ControlFlow::Break
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
            if let Some(client) = lsp_poll.borrow().as_ref() {
                let diags = client.poll();
                if !diags.is_empty() {
                    *lsp_diags_for_poll.borrow_mut() = true;
                    let errors: Vec<CompileError> = diags
                        .into_iter()
                        .map(|d| CompileError {
                            file: d.file,
                            line: d.line,
                            col: d.col,
                            message: d.message,
                            severity: match d.severity {
                                DiagSeverity::Error => Severity::Error,
                                _ => Severity::Warning,
                            },
                        })
                        .collect();
                    let diag_marks: Vec<(std::path::PathBuf, u32, bool)> = errors
                        .iter()
                        .map(|e| (e.file.clone(), e.line, matches!(e.severity, Severity::Error)))
                        .collect();
                    let err_count = diag_marks.iter().filter(|(_, _, is_err)| *is_err).count() as u32;
                    let warn_count = diag_marks.iter().filter(|(_, _, is_err)| !*is_err).count() as u32;
                    editor_for_lsp_diag.mark_diagnostics(&diag_marks);
                    error_panel_for_lsp.show_errors(errors);
                    error_panel_for_lsp.widget().set_visible(true);
                    editor_for_lsp_diag.set_diag_summary(err_count, warn_count);
                } else {
                    *lsp_diags_for_poll.borrow_mut() = false;
                }
                if let Some((id, items)) = client.poll_completion() {
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

        let watch_btn = ToggleButton::new();
        watch_btn.set_icon_name("media-record-symbolic");
        watch_btn.add_css_class("flat");
        watch_btn.set_tooltip_text(Some("Watch mode: auto-recompile on save"));
        watch_btn.update_property(&[gtk4::accessible::Property::Label("Toggle watch mode")]);

        let popout_btn = Button::from_icon_name("window-new-symbolic");
        popout_btn.add_css_class("flat");
        popout_btn.update_property(&[gtk4::accessible::Property::Label("Pop out preview window")]);
        popout_btn.set_tooltip_text(Some("Pop out preview"));

        let ref_toggle_btn = ToggleButton::new();
        ref_toggle_btn.set_icon_name("help-contents-symbolic");
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
        preview_toolbar.append(&watch_btn);
        preview_toolbar.append(&ref_toggle_btn);
        preview_toolbar.append(&popout_btn);

        // Watch button wiring
        let preview_for_watch = preview_pane.clone();
        watch_btn.connect_toggled(move |btn| {
            if btn.is_active() {
                btn.add_css_class("suggested-action");
                preview_for_watch.start_watch();
            } else {
                btn.remove_css_class("suggested-action");
                preview_for_watch.stop_watch();
            }
        });

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
            preview_pane.set_on_compile_time(move |ms| {
                let secs = ms as f64 / 1000.0;
                lbl.set_text(&format!("{secs:.1}s"));
            });
        }

        // Zoom button wiring
        let preview_for_zoom_out = preview_pane.clone();
        zoom_out_btn.connect_clicked(move |_| {
            let new_z = (preview_for_zoom_out.zoom() - 0.25).max(0.25);
            preview_for_zoom_out.set_zoom(new_z);
        });

        let preview_for_zoom_in = preview_pane.clone();
        zoom_in_btn.connect_clicked(move |_| {
            let new_z = (preview_for_zoom_in.zoom() + 0.25).min(4.0);
            preview_for_zoom_in.set_zoom(new_z);
        });

        // Fit width / fit page buttons
        let preview_for_fw = preview_pane.clone();
        fit_width_btn.connect_clicked(move |_| {
            preview_for_fw.fit_width();
        });

        let preview_for_fp = preview_pane.clone();
        fit_page_btn.connect_clicked(move |_| {
            preview_for_fp.fit_page();
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
            let cs_scroll = super::help_window::cheatsheet_scroll();
            ref_notebook.append_page(&cs_scroll, Some(&cs_lbl));
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

        *preview_vis_holder.borrow_mut() = Some(preview_outer.clone());

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
            print_btn.set_tooltip_text(Some("Open PDF for printing"));
            let print_dir = secondary.output_dir();
            print_btn.connect_clicked(move |_| {
                std::process::Command::new("xdg-open")
                    .arg(print_dir.join("preview.pdf"))
                    .spawn()
                    .ok();
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
            file_tree.set_on_open(move |path| {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    ep.open_file(path, &content);
                }
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
            file_tree.set_on_delete(move |path| {
                let _ = std::fs::remove_file(&path);
                ft.refresh();
            });
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
            update_template_btn.connect_clicked(move |_| {
                let Some(current_path) = ep_ut.get_active_path() else { return };
                let current_content = ep_ut.get_active_content().unwrap_or_default();
                let dlg = TemplateDialog::new(&win_ut, &root_ut);

                if let Some(sidecar) = super::template_dialog::load_sidecar(&current_path) {
                    dlg.preselect_from_sidecar(&sidecar);
                } else {
                    dlg.preselect_style(
                        &super::template_dialog::parse_style_key(&current_content)
                            .unwrap_or_default(),
                    );
                    if let Some(f) = super::template_dialog::parse_font(&current_content) {
                        dlg.preselect_font(&f);
                    }
                    if let Some(p) = super::template_dialog::parse_paper(&current_content) {
                        dlg.preselect_paper(&p);
                    }
                    if let Some(s) = super::template_dialog::parse_spacing(&current_content) {
                        dlg.preselect_spacing(&s);
                    }
                    dlg.preselect_margin(super::template_dialog::parse_margin(&current_content));
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
                }
                // Always read metadata from the document — the user may have edited the
                // #let doc-* variables directly, and the sidecar won't reflect those changes.
                dlg.preselect_metadata(
                    &super::template_dialog::parse_meta(&current_content, "title"),
                    &super::template_dialog::parse_meta(&current_content, "subtitle"),
                    &super::template_dialog::parse_meta(&current_content, "author"),
                    &super::template_dialog::parse_meta(&current_content, "affiliation"),
                    &super::template_dialog::parse_meta(&current_content, "course"),
                    &super::template_dialog::parse_meta(&current_content, "date"),
                );

                let ep2 = ep_ut.clone();
                let win_ut2 = win_ut.clone();
                dlg.set_on_apply(move |new_content, sidecar| {
                    let do_apply = {
                        let cc = current_content.clone();
                        let nc = new_content.clone();
                        let sc = sidecar.clone();
                        let path = current_path.clone();
                        let ep3 = ep2.clone();
                        move || {
                            let updated = super::template_dialog::apply_body_splice(&cc, &nc);
                            super::template_dialog::save_sidecar(&path, &sc);
                            if std::fs::write(&path, &updated).is_ok() {
                                ep3.reload_file(path.clone(), &updated);
                            }
                        }
                    };

                    if super::template_dialog::has_body_marker(&current_content) {
                        do_apply();
                    } else {
                        let confirm = adw::MessageDialog::new(
                            Some(&win_ut2),
                            Some("Replace entire document?"),
                            Some("This document has no body marker, so the template \
                                  will replace the whole file. Your current text will \
                                  be lost. Make sure you have a backup."),
                        );
                        confirm.add_response("cancel", "Cancel");
                        confirm.add_response("replace", "Replace Document");
                        confirm.set_response_appearance(
                            "replace",
                            adw::ResponseAppearance::Destructive,
                        );
                        confirm.set_default_response(Some("cancel"));
                        confirm.set_close_response("cancel");
                        confirm.connect_response(None, move |_, id| {
                            if id == "replace" {
                                do_apply();
                            }
                        });
                        confirm.present();
                    }
                });
                dlg.present();
            });
        }

        let structure_header = Label::new(Some("Structure"));
        structure_header.add_css_class("dim-label");
        structure_header.add_css_class("caption");
        structure_header.set_halign(Align::Start);
        structure_header.set_margin_start(12);
        structure_header.set_margin_top(8);
        structure_header.set_margin_bottom(2);

        let left_box = GtkBox::new(Orientation::Vertical, 0);
        left_box.set_hexpand(false);
        left_box.set_vexpand(true);
        left_box.set_overflow(gtk4::Overflow::Hidden);
        left_box.add_css_class("zerkalo-sidebar");
        left_box.append(&sidebar_toolbar);
        left_box.append(&Separator::new(Orientation::Horizontal));
        left_box.append(&structure_header);
        left_box.append(outline_panel.widget());
        left_box.append(&Separator::new(Orientation::Horizontal));
        left_box.append(citation_panel.widget());
        *left_paned_holder.borrow_mut() = Some(left_box.clone());

        // ── Right sidebar (plan panel) ────────────────────────────────────────
        let right_sidebar = GtkBox::new(Orientation::Vertical, 0);
        right_sidebar.set_width_request(240);
        right_sidebar.set_vexpand(true);
        todo_panel.widget().set_vexpand(true);
        right_sidebar.append(todo_panel.widget());
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

        // Search panel is hidden by default; Ctrl+Shift+F toggles it
        search_panel.widget().set_visible(false);

        let right_col = GtkBox::new(Orientation::Vertical, 0);
        right_col.set_hexpand(true);
        right_col.set_vexpand(true);
        right_col.append(&inner_paned);
        right_col.append(search_panel.widget());
        right_col.append(error_panel.widget());
        right_col.append(&Separator::new(Orientation::Horizontal));
        right_col.append(editor_pane.status_bar_widget());


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
        {
            let cfg = current_config.clone();
            let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
            outer_paned.connect_position_notify(move |p| {
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
            let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
            inner_paned.connect_position_notify(move |p| {
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

        let main_content = GtkBox::new(Orientation::Horizontal, 0);
        main_content.set_hexpand(true);
        main_content.set_vexpand(true);
        main_content.append(&outer_paned);

        toast_for_sync_btn.set_child(Some(&main_content));

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&toast_for_sync_btn));

        window.set_content(Some(&toolbar_view));

        Self {
            window,
            editor_pane,
            preview_pane,
            error_panel,
            outline_panel,
            project_root,
            project_model,
            sync_btn,
            search_panel,
            toast_overlay: toast_for_sync_btn,
            file_tree,
            writing_log,
            file_start_words,
            session_start,
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
        let controller = gtk4::EventControllerKey::new();

        // ── Command palette (Ctrl+P) ────────────────────────────────────────
        let palette = Rc::new(CommandPalette::new(&self.window));
        {
            let editor_for_pal = self.editor_pane.clone();
            let window_for_pal = self.window.clone();
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
                        "toggle_find" => editor_for_pal.toggle_find(),
                        "save"        => { editor_for_pal.save_all_modified(); }
                        "help"        => { HelpWindow::new(&w).present(); }
                        _             => {}
                    }
                }
            });
        }
        let palette_for_key = palette.clone();
        let editor_for_palette_key = editor.clone();

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
                            preview.trigger_compile();
                        }
                    }
                }
                return glib::Propagation::Stop;
            }
            if matches_binding(&kb.compile, ctrl, shift, alt, key) {
                editor.save_all_modified();
                preview.trigger_compile();
                return glib::Propagation::Stop;
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
                    HelpWindow::new(&window).present();
                    return glib::Propagation::Stop;
                }
                // Ctrl+P — command palette
                if ctrl && !shift && key == Key::p {
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

        if !session.open_files.is_empty() {
            for path in &session.open_files {
                if let Ok(content) = std::fs::read_to_string(path) {
                    self.editor_pane.open_file(path.clone(), &content);
                }
            }
            // Switch to the previously active file
            if let Some(ref active) = session.active_file {
                self.editor_pane.switch_to_file(active);
            }
            // Restore cursor positions after layout settles
            let ep = self.editor_pane.clone();
            let positions = session.cursor_positions.clone();
            glib::idle_add_local_once(move || {
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
) {
    use std::sync::mpsc::TryRecvError;

    btn.set_sensitive(false);

    let (tx, rx) = std::sync::mpsc::sync_channel::<git_sync::SyncResult>(1);
    std::thread::spawn(move || {
        tx.send(git_sync::sync(&root)).ok();
    });

    let rx = Rc::new(rx);
    glib::timeout_add_local(Duration::from_millis(100), move || match rx.try_recv() {
        Ok(result) => {
            btn.set_sensitive(true);
            show_sync_result(&window, &overlay, result);
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
) {
    if let Some(err) = result.error {
        show_alert(window, "Sync Failed", &err);
        return;
    }
    if !result.push_errors.is_empty() {
        let detail = result.push_errors.join("\n");
        if result.pushed {
            // Some remotes failed — toast for success, alert for the failures
            let summary = result.commit_message.lines().next().unwrap_or("Synced").to_string();
            overlay.add_toast(adw::Toast::new(&format!("Synced — {summary}")));
            show_alert(window, "Some remotes failed", &detail);
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

fn show_backup_remote_dialog(window: &adw::ApplicationWindow, repo_path: &std::path::Path) {
    let dialog = adw::Window::builder()
        .title("Backup Remotes")
        .transient_for(window)
        .modal(true)
        .default_width(520)
        .default_height(560)
        .build();

    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    let close_btn = Button::with_label("Close");
    close_btn.add_css_class("flat");
    header.pack_start(&close_btn);

    let page = adw::PreferencesPage::new();

    // ── How it works ─────────────────────────────────────────────────────────
    let how_group = adw::PreferencesGroup::new();
    how_group.set_description(Some(
        "Every sync (Ctrl+Shift+G) pushes to your primary remote (origin) \
         AND to every backup remote listed here. You can have as many as you like — \
         one for local storage, one for a privacy-respecting git host, etc.",
    ));
    page.add(&how_group);

    // ── Current backup remotes ────────────────────────────────────────────────
    let current_group = adw::PreferencesGroup::new();
    current_group.set_title("Current Backup Remotes");

    let root_for_rebuild = repo_path.to_path_buf();
    let current_group_c = current_group.clone();

    // Populate the current list — we rebuild it after each add/remove
    let rebuild_current = {
        let group = current_group_c.clone();
        let root = root_for_rebuild.clone();
        move || {
            // Remove all existing children from the group
            while let Some(child) = group.first_child() {
                group.remove(&child);
            }
            let remotes = git_sync::list_backup_remotes(&root);
            if remotes.is_empty() {
                let row = adw::ActionRow::new();
                row.set_title("No backup remotes configured");
                row.add_css_class("dim-label");
                group.add(&row);
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
                    let grp2 = group.clone();
                    rm_btn.connect_clicked(move |_| {
                        let _ = git_sync::remove_remote(&root2, &name);
                        // Rebuild the list in place
                        while let Some(child) = grp2.first_child() {
                            grp2.remove(&child);
                        }
                        let remotes2 = git_sync::list_backup_remotes(&root2);
                        if remotes2.is_empty() {
                            let placeholder = adw::ActionRow::new();
                            placeholder.set_title("No backup remotes configured");
                            grp2.add(&placeholder);
                        } else {
                            for (n, u) in remotes2 {
                                let r = adw::ActionRow::new();
                                r.set_title(&n);
                                r.set_subtitle(&u);
                                grp2.add(&r);
                            }
                        }
                    });
                    row.add_suffix(&rm_btn);
                    group.add(&row);
                }
            }
        }
    };
    rebuild_current();
    page.add(&current_group);

    // ── Add a new backup remote ───────────────────────────────────────────────
    let add_group = adw::PreferencesGroup::new();
    add_group.set_title("Add a Backup Remote");
    add_group.set_description(Some(
        "Enter a name (e.g. \"disroot\", \"backup\", \"nas\") and a URL or local path.",
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
        let grp = current_group.clone();
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
                    // Refresh the current-remotes list
                    while let Some(child) = grp.first_child() {
                        grp.remove(&child);
                    }
                    for (n, u) in git_sync::list_backup_remotes(&root_c) {
                        let row = adw::ActionRow::new();
                        row.set_title(&n);
                        row.set_subtitle(&u);
                        grp.add(&row);
                    }
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
    // For #let definitions which use content/code blocks
    let bracket_depth = |s: &str| -> i32 {
        s.chars().fold(0i32, |d, c| match c {
            '[' | '{' => d + 1,
            ']' | '}' => d - 1,
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
                let d = d + bracket_depth(t);
                if d <= 0 {
                    macro_defs.push(std::mem::take(&mut let_buf));
                    Scan::Body
                } else {
                    Scan::CollectLet(d)
                }
            }

            // ── Normal body scan ─────────────────────────────────────────────────
            Scan::Body => {
                if t.starts_with("#set page(")
                    || t.starts_with("#set text(")
                    || t.starts_with("#set par(")
                {
                    // Discard; track depth for multi-line blocks
                    let d = paren_depth(t);
                    if d > 0 { Scan::SkipSet(d) } else { Scan::Body }
                } else if t.starts_with("#set heading(") {
                    // Always single-line in practice; discard silently
                    Scan::Body
                } else if t.starts_with("#show heading") {
                    // Must use total_depth: block(...)[\n] opens with `(` before `[`
                    let d = total_depth(t);
                    if d > 0 { Scan::SkipShow(d) } else { Scan::Body }
                } else if t.starts_with("#import ") {
                    macro_defs.push(line.to_string());
                    Scan::Body
                } else if t.starts_with("#let ") {
                    let d = bracket_depth(t);
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
fn post_process_pdf_import(text: &str, title: &str) -> String {
    let escaped_title = title.replace('"', "\\\"");
    let preamble = super::template_dialog::default_import_preamble();
    let mut out = format!(
        "{preamble}\n\
         // ── Document body ───────────────────────────────────────────────────────\n\
         // Imported from PDF — plain text only, formatting not preserved.\n\
         \n\
         = {escaped_title}\n\
         \n"
    );

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            out.push('\n');
        } else {
            out.push_str(trimmed);
            out.push('\n');
        }
    }

    // Bibliography stub so Zerkalo can locate it
    out.push_str(
        "\n// ── Bibliography ────────────────────────────────────────────────────\n\
         // #bibliography(\"refs.bib\", style: \"chicago-author-date\")\n",
    );

    out
}

fn load_app_css() {
    let css = gtk4::CssProvider::new();
    css.load_from_data(
        ".navigation-sidebar > row:hover:not(:selected) { \
            background-color: alpha(@accent_color, 0.08); \
        } \
        .navigation-sidebar > row:selected { \
            background-color: @accent_bg_color; \
            color: @accent_fg_color; \
        } \
        .linked > toggle:checked, \
        .linked > button:checked { \
            background-color: @accent_bg_color; \
            color: @accent_fg_color; \
        } \
        .paned > separator { \
            min-width: 4px; \
            min-height: 4px; \
            transition: background-color 200ms; \
        } \
        .paned > separator:hover { \
            background-color: alpha(@accent_color, 0.3); \
        } \
        .zerkalo-sidebar { \
            transition: opacity 250ms; \
        } \
        .zerkalo-sidebar entry, \
        .zerkalo-sidebar button, \
        .zerkalo-sidebar label { \
            min-width: 0; \
        } \
        window.zen-writing .zerkalo-sidebar { \
            opacity: 0.3; \
        } \
        window.zen-writing textview text { \
            padding-left: 40px; \
            padding-right: 40px; \
        } \
        window.high-contrast textview { \
            color: #ffffff; \
            background-color: #000000; \
        } \
        window.high-contrast textview text { \
            color: #ffffff; \
        } \
        textview.view { \
            caret-color: @accent_color; \
        } \
        notebook tab button.circular { \
            min-width: 20px; \
            min-height: 20px; \
            padding: 2px; \
        }",
    );
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

struct HamburgerItems {
    menu_new_template_item: Button,
    menu_reapply_template_item: Button,
    menu_new_item: Button,
    menu_open_item: Button,
    menu_open_project_item: Button,
    menu_recent_projects_item: Button,
    menu_save_item: Button,
    menu_save_as_item: Button,
    menu_export_item: Button,
    menu_import_item: Button,
    menu_docs_item: Button,
    menu_fonts_item: Button,
    menu_settings_item: Button,
    menu_setup_item: Button,
    menu_backup_remote_item: Button,
    menu_help_item: Button,
    menu_writing_stats_item: Button,
    menu_about_item: Button,
    menu_import_latex_item: Button,
    menu_import_docx_item: Button,
    menu_import_pdf_item: Button,
}

fn build_hamburger_menu_items() -> HamburgerItems {
    HamburgerItems {
        menu_new_template_item:    make_menu_item("New from Template…",         None),
        menu_reapply_template_item: make_menu_item("Update Template Settings…", None),
        menu_new_item:             make_menu_item("New Blank Document…",         None),
        menu_open_item:            make_menu_item("Open File…",                  None),
        menu_open_project_item:    make_menu_item("Open Project Folder…",        None),
        menu_recent_projects_item: make_menu_item("Recent Projects…",            None),
        menu_save_item:            make_menu_item("Save",                        Some("Ctrl+S")),
        menu_save_as_item:         make_menu_item("Save As…",                    None),
        menu_export_item:          make_menu_item("Export…",                     None),
        menu_import_item:          make_menu_item("Import…",                     None),
        menu_docs_item:            make_menu_item("Browse Documents…",           None),
        menu_fonts_item:           make_menu_item("Font Management…",            None),
        menu_settings_item:        make_menu_item("Settings",                    None),
        menu_setup_item:           make_menu_item("Setup & Onboarding…",         None),
        menu_backup_remote_item:   make_menu_item("Backup Remotes…",             None),
        menu_help_item:            make_menu_item("Keyboard Shortcuts & Help",   Some("Ctrl+?")),
        menu_writing_stats_item:   make_menu_item("Writing Stats",               None),
        menu_about_item:           make_menu_item("About Zerkalo",               None),
        menu_import_latex_item:    make_menu_item("Import LaTeX File…",          None),
        menu_import_docx_item:     make_menu_item("Import DOCX File…",           None),
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
    project_root: Option<&std::path::Path>,
) {
    let words = text.split_whitespace().count();
    let chars = text.chars().filter(|c| !c.is_whitespace()).count();
    let chars_with_spaces = text.chars().count();
    let paragraphs = text.split("\n\n").filter(|s| !s.trim().is_empty()).count();
    let sentences = text
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

    let mut body = format!(
        "Words            {words}  ({session_delta} this session)\n\
         Characters       {chars}  ({chars_with_spaces} with spaces)\n\
         Paragraphs       {paragraphs}\n\
         Sentences        {sentences}\n\
         Reading time     {reading_mins} min",
    );

    if let Some(root) = project_root {
        let total: u32 = crate::project::collect_typ_files(root)
            .iter()
            .filter_map(|p| std::fs::read_to_string(p).ok())
            .map(|c| c.split_whitespace().count() as u32)
            .sum();
        body.push_str(&format!("\n\nProject total    {total} words"));
    }

    let win = adw::Window::new();
    win.set_title(Some("Document Statistics"));
    win.set_default_width(340);
    win.set_default_height(-1);
    win.set_transient_for(Some(parent));
    win.set_modal(false);

    let buf = gtk4::TextBuffer::new(None);
    buf.set_text(&body);

    let view = gtk4::TextView::with_buffer(&buf);
    view.set_editable(false);
    view.set_cursor_visible(false);
    view.set_monospace(true);
    view.set_left_margin(16);
    view.set_right_margin(16);
    view.set_top_margin(12);
    view.set_bottom_margin(12);

    let header = adw::HeaderBar::new();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&view));
    win.set_content(Some(&toolbar));
    win.present();
}

fn show_changelog(parent: &impl IsA<gtk4::Window>) {
    const CHANGELOG: &str = include_str!("../../CHANGELOG.md");

    let win = adw::Window::new();
    win.set_title(Some("Changelog — Zerkalo"));
    win.set_default_width(640);
    win.set_default_height(520);
    win.set_transient_for(Some(parent));
    win.set_modal(false);

    let buf = gtk4::TextBuffer::new(None);
    buf.set_text(CHANGELOG);

    let view = gtk4::TextView::with_buffer(&buf);
    view.set_editable(false);
    view.set_cursor_visible(false);
    view.set_wrap_mode(gtk4::WrapMode::Word);
    view.set_monospace(true);
    view.set_left_margin(16);
    view.set_right_margin(16);
    view.set_top_margin(12);
    view.set_bottom_margin(12);

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_child(Some(&view));
    scroll.set_hexpand(true);
    scroll.set_vexpand(true);

    let header = adw::HeaderBar::new();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scroll));
    win.set_content(Some(&toolbar));
    win.present();
}

#[cfg(test)]
mod tests {
    use super::{post_process_latex_import, strip_pandoc_preamble};

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
