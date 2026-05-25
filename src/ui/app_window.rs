use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Entry, Label, ListBox, ListBoxRow, MenuButton,
    Notebook, Orientation, Paned, Popover, ScrolledWindow, Separator, ToggleButton,
};
use libadwaita as adw;
use adw::prelude::*;

use crate::bibliography;
use crate::config::{Config, ProjectConfig, Theme};
use crate::git_sync;
use crate::lsp::{DiagSeverity, LspClient};
use crate::project_model::ProjectModel;
use super::dep_graph::DepGraph;
use super::docs_browser::DocsBrowser;
use super::editor_pane::EditorPane;
use super::error_panel::{parse_typst_errors, CompileError, ErrorPanel, Severity};
use super::export_dialog::ExportDialog;
use super::help_window::HelpWindow;
use super::history_panel::HistoryPanel;
use super::outline_panel::OutlinePanel;
use super::package_browser::PackageBrowser;
use super::preview_pane::PreviewPane;
use super::ref_manager::RefManager;
use super::settings_dialog::SettingsDialog;
use super::sync_dialog::SyncDialog;

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
}

impl AppWindow {
    pub fn new(app: &adw::Application, config: Config) -> Self {
        let project_root = config.work_dir.clone();

        let window = adw::ApplicationWindow::new(app);
        window.set_title(Some("Zerkalo"));
        window.set_default_width(1600);
        window.set_default_height(1000);

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
        header.pack_start(&sidebar_btn);

        let docs_btn = Button::from_icon_name("folder-open-symbolic");
        docs_btn.set_tooltip_text(Some("Browse documents"));
        docs_btn.add_css_class("flat");
        header.pack_start(&docs_btn);

        let focus_btn = ToggleButton::new();
        focus_btn.set_icon_name("view-fullscreen-symbolic");
        focus_btn.set_tooltip_text(Some("Focus mode — hide sidebar and preview"));
        focus_btn.add_css_class("flat");
        header.pack_start(&focus_btn);

        // End: compile (primary), sync (secondary), hamburger menu
        let compile_btn = Button::from_icon_name("media-playback-start-symbolic");
        compile_btn.set_tooltip_text(Some("Compile & Preview (Ctrl+Shift+P)"));
        compile_btn.add_css_class("suggested-action");
        header.pack_end(&compile_btn);

        let sync_btn = Button::from_icon_name("emblem-synchronizing-symbolic");
        sync_btn.set_tooltip_text(Some("Commit & Push to Git"));
        sync_btn.add_css_class("flat");
        header.pack_end(&sync_btn);

        // Hamburger menu (item 8)
        let menu_new_item = Button::new();
        menu_new_item.set_label("New Document…");
        menu_new_item.set_halign(Align::Start);
        menu_new_item.add_css_class("flat");
        menu_new_item.set_size_request(190, -1);

        let menu_save_item = Button::new();
        menu_save_item.set_label("Save");
        menu_save_item.set_halign(Align::Start);
        menu_save_item.add_css_class("flat");
        menu_save_item.set_size_request(190, -1);

        let menu_save_as_item = Button::new();
        menu_save_as_item.set_label("Save As…");
        menu_save_as_item.set_halign(Align::Start);
        menu_save_as_item.add_css_class("flat");
        menu_save_as_item.set_size_request(190, -1);

        let menu_settings_item = Button::new();
        menu_settings_item.set_label("Settings");
        menu_settings_item.set_halign(Align::Start);
        menu_settings_item.add_css_class("flat");
        menu_settings_item.set_size_request(190, -1);

        let menu_help_item = Button::new();
        menu_help_item.set_label("Help & Shortcuts");
        menu_help_item.set_halign(Align::Start);
        menu_help_item.add_css_class("flat");
        menu_help_item.set_size_request(190, -1);

        let menu_about_item = Button::new();
        menu_about_item.set_label("About Zerkalo");
        menu_about_item.set_halign(Align::Start);
        menu_about_item.add_css_class("flat");
        menu_about_item.set_size_request(190, -1);

        let menu_export_item = Button::new();
        menu_export_item.set_label("Export…");
        menu_export_item.set_halign(Align::Start);
        menu_export_item.add_css_class("flat");
        menu_export_item.set_size_request(190, -1);

        let menu_popover_box = GtkBox::new(Orientation::Vertical, 0);
        menu_popover_box.set_margin_top(4);
        menu_popover_box.set_margin_bottom(4);
        menu_popover_box.append(&menu_new_item);
        menu_popover_box.append(&menu_save_item);
        menu_popover_box.append(&menu_save_as_item);
        menu_popover_box.append(&Separator::new(Orientation::Horizontal));
        menu_popover_box.append(&menu_settings_item);
        menu_popover_box.append(&menu_export_item);
        menu_popover_box.append(&Separator::new(Orientation::Horizontal));
        menu_popover_box.append(&menu_help_item);
        menu_popover_box.append(&menu_about_item);

        let menu_popover = Popover::new();
        menu_popover.set_child(Some(&menu_popover_box));
        let menu_btn = MenuButton::new();
        menu_btn.set_icon_name("open-menu-symbolic");
        menu_btn.add_css_class("flat");
        menu_btn.set_popover(Some(&menu_popover));
        header.pack_end(&menu_btn);

        // ── Setzer-style open dropdown ───────────────────────────────────────
        let open_search = Entry::new();
        open_search.set_placeholder_text(Some("Search documents…"));
        open_search.set_hexpand(true);
        open_search.set_margin_start(8);
        open_search.set_margin_end(8);
        open_search.set_margin_top(8);
        open_search.set_margin_bottom(4);

        let open_list_box = ListBox::new();
        open_list_box.set_selection_mode(gtk4::SelectionMode::None);

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

        let file_title_widget = adw::WindowTitle::new("untitled.typ", "");

        let file_selector = MenuButton::new();
        file_selector.add_css_class("flat");
        file_selector.set_child(Some(&file_title_widget));
        file_selector.set_popover(Some(&recent_popover));
        header.set_title_widget(Some(&file_selector));

        // ── Panels ──────────────────────────────────────────────────────────

        let editor_pane = EditorPane::new();
        let project_model = ProjectModel::scan(project_root.clone());
        let outline_panel = OutlinePanel::new();
        let ref_manager = RefManager::new();
        let history_panel = HistoryPanel::new(project_root.clone());
        let dep_graph = DepGraph::new(project_root.clone());
        let package_browser = PackageBrowser::new();

        // Wire outline symbol insert → editor
        {
            let ep = editor_pane.clone();
            outline_panel.set_on_symbol_insert(move |ch| ep.insert_at_cursor(&ch));
        }

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

            let rebuild: Rc<dyn Fn(&str)> = Rc::new(move |query: &str| {
                while let Some(child) = open_list_rc.first_child() {
                    open_list_rc.remove(&child);
                }
                let mut files = super::docs_browser::scan_typ_files(&work_dir_open, 2);
                files.sort_by(|a, b| b.1.cmp(&a.1));
                let q = query.to_lowercase();
                for (path, mtime) in files.into_iter().take(30) {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                    if !q.is_empty() && !name.to_lowercase().contains(&q) {
                        continue;
                    }
                    let date_str = format_file_mtime(mtime);
                    let row = ListBoxRow::new();
                    row.set_activatable(true);
                    let row_box = GtkBox::new(Orientation::Vertical, 2);
                    row_box.set_margin_start(10);
                    row_box.set_margin_end(10);
                    row_box.set_margin_top(5);
                    row_box.set_margin_bottom(5);
                    let name_lbl = Label::new(Some(&name));
                    name_lbl.set_xalign(0.0);
                    name_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                    let date_lbl = Label::new(Some(&date_str));
                    date_lbl.set_xalign(0.0);
                    date_lbl.add_css_class("caption");
                    date_lbl.add_css_class("dim-label");
                    row_box.append(&name_lbl);
                    row_box.append(&date_lbl);
                    row.set_child(Some(&row_box));
                    let ep = editor_for_open.clone();
                    let pop = pop_for_open.clone();
                    let p = path.clone();
                    row.connect_activate(move |_| {
                        if let Ok(content) = std::fs::read_to_string(&p) {
                            ep.open_file(p.clone(), &content);
                        }
                        pop.popdown();
                    });
                    open_list_rc.append(&row);
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

        let preview_pane = PreviewPane::new(
            project_model.root_file.clone(),
            effective_output_dir,
            extra_compiler_args,
        );
        let error_panel = ErrorPanel::new();

        // ── LSP client ──────────────────────────────────────────────────────

        let lsp_client: Rc<RefCell<Option<LspClient>>> = Rc::new(RefCell::new(None));

        // ── Apply initial settings ──────────────────────────────────────────

        editor_pane.apply_font_size(config.editor_font_size);
        editor_pane.apply_font_family(&config.editor_font_family);
        editor_pane.apply_word_wrap(config.editor_word_wrap);
        editor_pane.apply_show_whitespace(config.editor_show_whitespace);
        editor_pane.apply_tab_width(config.editor_tab_width);
        preview_pane.set_zoom(config.preview_zoom);
        apply_theme(&config.theme);

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
            editor_pane.set_bib_entries(entries);
            ref_manager.load_bib(bp);

            let editor_for_bib = editor_pane.clone();
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
                    editor_for_bib.set_bib_entries(entries);
                }
                glib::ControlFlow::Continue
            });
        }

        // ── Reference manager: insert citation at cursor ────────────────────

        let editor_for_ref = editor_pane.clone();
        ref_manager.set_on_insert(move |citation| {
            editor_for_ref.insert_at_cursor(&citation);
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

        // ── Focus mode toggle ───────────────────────────────────────────────
        let focus_active_c = focus_active.clone();
        let lpane_for_focus = left_paned_holder.clone();
        let preview_vis_for_focus = preview_vis_holder.clone();
        focus_btn.connect_toggled(move |btn| {
            let focused = btn.is_active();
            *focus_active_c.borrow_mut() = focused;
            if let Some(lp) = lpane_for_focus.borrow().as_ref() {
                lp.set_visible(!focused);
            }
            if let Some(pc) = preview_vis_for_focus.borrow().as_ref() {
                pc.set_visible(!focused);
            }
        });

        // ── Docs browser button ─────────────────────────────────────────────
        let window_for_docs = window.clone();
        let editor_for_docs = editor_pane.clone();
        let root_for_docs = project_root.clone();
        docs_btn.connect_clicked(move |_| {
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
            editor_for_btn.save_all_modified();
            preview_for_btn.trigger_compile();
        });

        // ── Menu: Settings ──────────────────────────────────────────────────

        let window_for_settings = window.clone();
        let editor_for_settings = editor_pane.clone();
        let debounce_for_settings = debounce_ms.clone();
        let auto_compile_for_settings = auto_compile.clone();
        let current_config_for_settings = current_config.clone();
        let menu_popover_for_settings = menu_popover.clone();
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
            dialog.set_on_save(move |new_cfg| {
                *debounce.borrow_mut() = new_cfg.debounce_ms;
                *auto_flag.borrow_mut() = new_cfg.auto_compile;
                editor.apply_font_size(new_cfg.editor_font_size);
                editor.apply_font_family(&new_cfg.editor_font_family);
                editor.apply_word_wrap(new_cfg.editor_word_wrap);
                editor.apply_show_whitespace(new_cfg.editor_show_whitespace);
                editor.apply_tab_width(new_cfg.editor_tab_width);
                apply_theme(&new_cfg.theme);
                editor.apply_style_scheme(adw::StyleManager::default().is_dark());
                let old_bib = cfg_rc.borrow().bib_path.clone();
                if old_bib != new_cfg.bib_path {
                    match new_cfg.bib_path.as_ref() {
                        Some(bp) => editor.set_bib_entries(bibliography::load_bib(bp)),
                        None => editor.set_bib_entries(Vec::new()),
                    }
                }
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

        // ── Menu: About ─────────────────────────────────────────────────────

        let window_for_about = window.clone();
        let menu_popover_for_about = menu_popover.clone();
        menu_about_item.connect_clicked(move |_| {
            menu_popover_for_about.popdown();
            let dlg = adw::MessageDialog::new(
                Some(&window_for_about),
                Some("Zerkalo 0.1.0"),
                Some(
                    "A contemplative Typst editor.\n\n\
                     Built with Rust · GTK4 · libadwaita · sourceview5\n\n\
                     https://github.com/calstfrancis/zerkalo"
                ),
            );
            dlg.add_response("ok", "OK");
            dlg.present();
        });

        // ── Menu: Export ────────────────────────────────────────────────────

        let preview_for_export = preview_pane.clone();
        let window_for_export = window.clone();
        let menu_popover_for_export = menu_popover.clone();
        menu_export_item.connect_clicked(move |_| {
            menu_popover_for_export.popdown();
            ExportDialog::new(
                &window_for_export,
                preview_for_export.root_file_path(),
                preview_for_export.output_dir(),
            )
            .present();
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
        let toast_for_sync_btn = adw::ToastOverlay::new();
        let toast_overlay = toast_for_sync_btn.clone();
        sync_btn.connect_clicked(move |_| {
            let root = project_root_for_sync.clone();
            let win = window_for_sync.clone();
            let btn = sync_btn_ref.clone();
            let toasts = toast_overlay.clone();

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
        let lsp_for_change = lsp_client.clone();
        let gen: Rc<RefCell<u64>> = Rc::new(RefCell::new(0));
        let gen2 = gen.clone();
        editor_pane.set_on_change(move || {
            *gen2.borrow_mut() += 1;
            let my_gen = *gen2.borrow();
            let preview = preview_for_change.clone();
            let editor = editor_for_change.clone();
            let gen3 = gen2.clone();
            let auto = auto_compile_for_change.clone();
            let outline = outline_for_change.clone();
            let lsp = lsp_for_change.clone();
            let delay = Duration::from_millis(*debounce_for_change.borrow());
            glib::timeout_add_local(delay, move || {
                if *gen3.borrow() == my_gen {
                    if *auto.borrow() {
                        editor.save_all_modified();
                        preview.trigger_compile();
                    }
                    // Outline update
                    if let Some(path) = editor.get_active_path() {
                        if let Some(content) = editor.get_active_content() {
                            outline.update(&content, &path);
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
        let history_for_switch = history_panel.clone();
        let dep_graph_for_switch = dep_graph.clone();
        let title_widget_for_switch = file_title_widget.clone();
        editor_pane.set_on_page_switch(move |content, path| {
            outline_for_switch.update(&content, &path);
            history_for_switch.load_file_history(&path);
            dep_graph_for_switch.refresh(Some(&path));
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                title_widget_for_switch.set_title(name);
            }
        });

        // ── LSP: did_open + recent tracking when a file is opened ───────────

        let lsp_for_open = lsp_client.clone();
        let current_config_for_open = current_config.clone();
        editor_pane.set_on_file_opened(move |path, content| {
            if let Some(client) = lsp_for_open.borrow_mut().as_mut() {
                client.did_open(&path, &content);
            }
            let mut cfg = current_config_for_open.borrow_mut();
            cfg.push_recent(path.clone());
            let _ = cfg.save();
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

        let error_panel_for_compile = error_panel.clone();
        let root_for_compile = project_root.clone();
        let popout_pane_for_compile = popout_pane.clone();
        let dep_graph_for_compile = dep_graph.clone();
        preview_pane.set_on_compile_done(move |result| {
            match result {
                None => error_panel_for_compile.clear(),
                Some(stderr) => {
                    let errors = parse_typst_errors(&stderr, &root_for_compile);
                    error_panel_for_compile.show_errors(errors);
                }
            }
            dep_graph_for_compile.refresh(None);
            // Refresh pop-out window if it is open
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

        // ── Initial compile ─────────────────────────────────────────────────

        let preview_init = preview_pane.clone();
        glib::timeout_add_local(Duration::from_millis(600), move || {
            preview_init.trigger_compile();
            glib::ControlFlow::Break
        });

        // ── Startup: warn if required tools are missing ──────────────────────

        let win_for_check = window.clone();
        glib::timeout_add_local(Duration::from_millis(900), move || {
            let mut missing: Vec<&'static str> = Vec::new();
            if std::process::Command::new("typst")
                .arg("--version")
                .output()
                .is_err()
            {
                missing.push("typst");
            }
            if std::process::Command::new("git")
                .arg("--version")
                .output()
                .is_err()
            {
                missing.push("git");
            }
            if !missing.is_empty() {
                let list = missing.join(" and ");
                tracing::warn!("Required tools not found in PATH: {list}");
                show_alert(
                    &win_for_check,
                    "Missing Tools",
                    &format!(
                        "The following tools were not found in your PATH:\n\n  {list}\n\n\
                         Install them via your package manager to enable compile and sync:\n\
                         \n  zypper install typst git\
                         \n  apt  install  typst git\
                         \n  brew install  typst git"
                    ),
                );
            }
            glib::ControlFlow::Break
        });

        // ── First-start guide (item 4) ───────────────────────────────────────

        let win_for_guide = window.clone();
        glib::timeout_add_local(Duration::from_millis(1200), move || {
            let marker = glib::user_data_dir().join("zerkalo/.first_start_shown");
            if !marker.exists() {
                if let Some(parent) = marker.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(&marker, "");
                let dlg = adw::MessageDialog::new(
                    Some(&win_for_guide),
                    Some("Welcome to Zerkalo"),
                    Some(
                        "Zerkalo is a Typst editor with live preview and git sync.\n\n\
                         QUICK START\n\
                         • Edit main.typ — the preview updates automatically\n\
                         • Type # for function completions (requires tinymist)\n\
                         • Type @ for citation completions (set a .bib file in Settings)\n\
                         • Ctrl+Shift+P compiles manually\n\
                         • Ctrl+S saves; the sync button (⟳) commits and pushes\n\
                         • Use the hamburger menu (≡) for Help & Settings\n\n\
                         The sidebar shows files; the header dropdown switches open tabs.",
                    ),
                );
                dlg.add_response("ok", "Get Started");
                dlg.present();
            }
            glib::ControlFlow::Break
        });

        // ── LSP: initialise 500 ms after startup ────────────────────────────

        let lsp_init = lsp_client.clone();
        let root_for_lsp = project_root.clone();
        glib::timeout_add_local(Duration::from_millis(500), move || {
            *lsp_init.borrow_mut() = LspClient::new(&root_for_lsp);
            if lsp_init.borrow().is_some() {
                tracing::info!("tinymist LSP active");
            } else {
                tracing::info!("tinymist not found — LSP disabled");
            }
            glib::ControlFlow::Break
        });

        // ── LSP: poll for diagnostics + completions ──────────────────────────

        let lsp_poll = lsp_client.clone();
        let error_panel_for_lsp = error_panel.clone();
        let editor_for_comp_poll = editor_pane.clone();
        let last_req_poll = last_completion_request.clone();
        glib::timeout_add_local(Duration::from_millis(400), move || {
            if let Some(client) = lsp_poll.borrow().as_ref() {
                let diags = client.poll();
                if !diags.is_empty() {
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
                    error_panel_for_lsp.show_errors(errors);
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

        let zoom_in_btn = Button::from_icon_name("zoom-in-symbolic");
        zoom_in_btn.set_tooltip_text(Some("Zoom in"));

        let fit_width_btn = Button::from_icon_name("zoom-fit-best-symbolic");
        fit_width_btn.set_tooltip_text(Some("Fit page width"));
        fit_width_btn.add_css_class("flat");

        let fit_page_btn = Button::from_icon_name("view-fullscreen-symbolic");
        fit_page_btn.set_tooltip_text(Some("Fit page to window"));
        fit_page_btn.add_css_class("flat");

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

        let popout_btn = Button::from_icon_name("window-new-symbolic");
        popout_btn.add_css_class("flat");
        popout_btn.set_tooltip_text(Some("Pop out preview"));

        let preview_toolbar = GtkBox::new(Orientation::Horizontal, 4);
        preview_toolbar.set_margin_start(8);
        preview_toolbar.set_margin_end(8);
        preview_toolbar.set_margin_top(4);
        preview_toolbar.set_margin_bottom(4);
        preview_toolbar.append(&fit_width_btn);
        preview_toolbar.append(&fit_page_btn);
        preview_toolbar.append(&zoom_box);
        preview_toolbar.append(&zoom_label);
        let preview_spacer = GtkBox::new(Orientation::Horizontal, 0);
        preview_spacer.set_hexpand(true);
        preview_toolbar.append(&preview_spacer);
        preview_toolbar.append(&watch_btn);
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

        let preview_container = GtkBox::new(Orientation::Vertical, 0);
        preview_container.set_hexpand(true);
        preview_container.set_vexpand(true);
        preview_container.append(&Separator::new(Orientation::Horizontal));
        preview_container.append(&preview_toolbar);
        preview_container.append(&Separator::new(Orientation::Horizontal));
        preview_container.append(preview_pane.widget());
        *preview_vis_holder.borrow_mut() = Some(preview_container.clone());

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

        // ── Sidebar header with outline/symbol toggle and mode button ────────
        let sidebar_header = GtkBox::new(Orientation::Horizontal, 0);
        sidebar_header.set_margin_start(10);
        sidebar_header.set_margin_end(8);
        sidebar_header.set_margin_top(6);
        sidebar_header.set_margin_bottom(6);

        let sidebar_title = Label::new(Some("Outline"));
        sidebar_title.set_xalign(0.0);
        sidebar_title.set_hexpand(true);
        sidebar_title.add_css_class("heading");
        sidebar_header.append(&sidebar_title);

        let sym_toggle = ToggleButton::new();
        sym_toggle.set_icon_name("input-keyboard-symbolic");
        sym_toggle.add_css_class("flat");
        sym_toggle.set_tooltip_text(Some("Switch to symbol insert"));
        sidebar_header.append(&sym_toggle);

        let mode_btn = ToggleButton::new();
        mode_btn.set_icon_name("view-more-symbolic");
        mode_btn.add_css_class("flat");
        mode_btn.set_tooltip_text(Some("Show advanced panels (Refs, History, Graph, Pkgs)"));
        sidebar_header.append(&mode_btn);

        // Wire sym_toggle ↔ outline panel
        {
            let outline_c = outline_panel.clone();
            let title_c = sidebar_title.clone();
            sym_toggle.connect_toggled(move |btn| {
                if btn.is_active() {
                    outline_c.set_mode("symbols");
                    title_c.set_text("Symbols");
                } else {
                    outline_c.set_mode("outline");
                    title_c.set_text("Outline");
                }
            });
        }

        // ── Advanced panels (Refs, History, Graph, Pkgs) — hidden by default
        let advanced_notebook = Notebook::new();
        advanced_notebook.set_vexpand(true);
        advanced_notebook.set_tab_pos(gtk4::PositionType::Top);
        advanced_notebook.set_scrollable(true);
        for (widget, label) in [
            (ref_manager.widget().upcast_ref::<gtk4::Widget>(), "Refs"),
            (history_panel.widget().upcast_ref(), "History"),
            (dep_graph.widget().upcast_ref(), "Graph"),
            (package_browser.widget().upcast_ref(), "Pkgs"),
        ] {
            let tab_lbl = Label::new(Some(label));
            tab_lbl.add_css_class("caption");
            advanced_notebook.append_page(widget, Some(&tab_lbl));
        }

        let advanced_section = GtkBox::new(Orientation::Vertical, 0);
        advanced_section.set_visible(false);
        advanced_section.append(&Separator::new(Orientation::Horizontal));
        advanced_section.append(&advanced_notebook);

        {
            let adv_c = advanced_section.clone();
            mode_btn.connect_toggled(move |btn| {
                adv_c.set_visible(btn.is_active());
            });
        }

        let left_box = GtkBox::new(Orientation::Vertical, 0);
        left_box.set_hexpand(false);
        left_box.set_vexpand(true);
        left_box.append(&sidebar_header);
        left_box.append(&Separator::new(Orientation::Horizontal));
        left_box.append(outline_panel.widget());
        left_box.append(&advanced_section);
        *left_paned_holder.borrow_mut() = Some(left_box.clone());

        let inner_paned = Paned::new(Orientation::Horizontal);
        inner_paned.set_position(600);
        inner_paned.set_hexpand(true);
        inner_paned.set_vexpand(true);
        inner_paned.set_start_child(Some(editor_pane.widget()));
        inner_paned.set_end_child(Some(&preview_container));

        let right_col = GtkBox::new(Orientation::Vertical, 0);
        right_col.set_hexpand(true);
        right_col.set_vexpand(true);
        right_col.append(&inner_paned);
        right_col.append(error_panel.widget());

        let outer_paned = Paned::new(Orientation::Horizontal);
        outer_paned.set_position(220);
        outer_paned.set_resize_start_child(false);
        outer_paned.set_shrink_start_child(false);
        outer_paned.set_hexpand(true);
        outer_paned.set_vexpand(true);
        outer_paned.set_start_child(Some(&left_box));
        outer_paned.set_end_child(Some(&right_col));

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
        }
    }

    pub fn setup_keybindings(&self) {
        let editor = self.editor_pane.clone();
        let preview = self.preview_pane.clone();
        let window = self.window.clone();
        let controller = gtk4::EventControllerKey::new();

        controller.connect_key_pressed(move |_, key, _, modifier| {
            use gtk4::gdk::{Key, ModifierType};
            let ctrl = modifier.contains(ModifierType::CONTROL_MASK);
            let shift = modifier.contains(ModifierType::SHIFT_MASK);

            if ctrl && !shift && key == Key::s {
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
            if ctrl && shift && (key == Key::P || key == Key::p) {
                editor.save_all_modified();
                preview.trigger_compile();
                return glib::Propagation::Stop;
            }
            if ctrl && !shift && key == Key::f {
                editor.toggle_find();
                return glib::Propagation::Stop;
            }
            if ctrl && !shift && key == Key::q {
                window.close();
                return glib::Propagation::Stop;
            }
            if ctrl && !shift && key == Key::Tab {
                editor.next_tab();
                return glib::Propagation::Stop;
            }
            if ctrl && (key == Key::ISO_Left_Tab || (shift && key == Key::Tab)) {
                editor.prev_tab();
                return glib::Propagation::Stop;
            }

            glib::Propagation::Proceed
        });

        self.window.add_controller(controller);
    }

    pub fn open_initial_file(&self, initial: Option<PathBuf>) {
        let path = initial.unwrap_or_else(|| self.project_root.join("main.typ"));
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

    pub fn present(&self) {
        self.window.present();
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
    } else if result.pushed {
        let summary = result.commit_message.lines().next().unwrap_or("Synced").to_string();
        overlay.add_toast(adw::Toast::new(&format!("Synced — {summary}")));
    } else if result.committed {
        overlay.add_toast(adw::Toast::new("Committed locally — no remote push"));
    } else {
        overlay.add_toast(adw::Toast::new("Nothing to sync"));
    }
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
