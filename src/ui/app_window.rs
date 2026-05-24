use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use gtk4::prelude::*;
use gtk4::{Button, Orientation, Paned};
use libadwaita as adw;

use crate::bibliography;
use crate::config::{Config, ProjectConfig, Theme};
use crate::git_sync;
use crate::project_model::ProjectModel;
use super::editor_pane::EditorPane;
use super::error_panel::{parse_typst_errors, ErrorPanel};
use super::file_tree::FileTree;
use super::preview_pane::PreviewPane;
use super::settings_dialog::SettingsDialog;
use super::sync_dialog::SyncDialog;

pub struct AppWindow {
    window: adw::ApplicationWindow,
    editor_pane: EditorPane,
    file_tree: FileTree,
    preview_pane: PreviewPane,
    error_panel: ErrorPanel,
    project_root: PathBuf,
    #[allow(dead_code)]
    project_model: ProjectModel,
}

impl AppWindow {
    pub fn new(app: &adw::Application, config: Config) -> Self {
        let project_root = config.project_path.clone();

        let window = adw::ApplicationWindow::new(app);
        window.set_title(Some("Зеркало"));
        window.set_default_width(1600);
        window.set_default_height(1000);

        // ── Per-project config (6.2): load and merge with global ────────────

        let proj_cfg = ProjectConfig::load(&project_root).unwrap_or_default();
        let effective_bib = proj_cfg.bib_path.clone().or_else(|| config.bib_path.clone());
        let effective_output_dir = proj_cfg.output_dir.clone();
        let extra_compiler_args = proj_cfg.compiler_args.clone();

        // ── Runtime-configurable values ─────────────────────────────────────

        let debounce_ms: Rc<RefCell<u64>> = Rc::new(RefCell::new(config.debounce_ms));
        let auto_compile: Rc<RefCell<bool>> = Rc::new(RefCell::new(config.auto_compile));
        let current_config: Rc<RefCell<Config>> = Rc::new(RefCell::new(config.clone()));

        // ── Header bar ─────────────────────────────────────────────────────

        let header = adw::HeaderBar::new();

        let compile_btn = Button::from_icon_name("media-playback-start-symbolic");
        compile_btn.set_tooltip_text(Some("Compile & Preview (Ctrl+Shift+P)"));
        compile_btn.add_css_class("flat");
        header.pack_end(&compile_btn);

        let sync_btn = Button::from_icon_name("emblem-synchronizing-symbolic");
        sync_btn.set_tooltip_text(Some("Commit & Push to Git"));
        sync_btn.add_css_class("flat");
        header.pack_end(&sync_btn);

        let settings_btn = Button::from_icon_name("open-menu-symbolic");
        settings_btn.set_tooltip_text(Some("Settings"));
        settings_btn.add_css_class("flat");
        header.pack_end(&settings_btn);

        // ── Panels ─────────────────────────────────────────────────────────

        let editor_pane = EditorPane::new();
        let file_tree = FileTree::new(project_root.clone());
        let project_model = ProjectModel::scan(project_root.clone());

        if let Some(f) = &project_model.root_file {
            tracing::info!("Detected root file: {}", f.display());
        }

        let preview_pane = PreviewPane::new(
            project_model.root_file.clone(),
            effective_output_dir,
            extra_compiler_args,
        );
        let error_panel = ErrorPanel::new();

        // ── Apply initial settings ──────────────────────────────────────────

        editor_pane.apply_font_size(config.editor_font_size);
        apply_theme(&config.theme);

        // ── Bibliography loading & file watch ──────────────────────────────

        if let Some(ref bp) = effective_bib {
            let entries = bibliography::load_bib(bp);
            if !entries.is_empty() {
                tracing::info!("Loaded {} bib entries from {}", entries.len(), bp.display());
            }
            editor_pane.set_bib_entries(entries);

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

        // ── File tree callbacks ─────────────────────────────────────────────

        let editor_open = editor_pane.clone();
        file_tree.set_on_open(move |path| {
            match std::fs::read_to_string(&path) {
                Ok(content) => editor_open.open_file(path, &content),
                Err(e) => eprintln!("Cannot open {}: {e}", path.display()),
            }
        });

        let editor_new = editor_pane.clone();
        let tree_new = file_tree.clone();
        let root_new = project_root.clone();
        file_tree.set_on_new_file(move |name| {
            let mut filename = name.trim().to_string();
            if !filename.ends_with(".typ") {
                filename.push_str(".typ");
            }
            let path = root_new.join(&filename);
            if !path.exists() {
                if let Err(e) = std::fs::write(&path, "") {
                    eprintln!("Cannot create {}: {e}", path.display());
                    return;
                }
            }
            tree_new.refresh();
            match std::fs::read_to_string(&path) {
                Ok(content) => editor_new.open_file(path, &content),
                Err(e) => eprintln!("Cannot open {}: {e}", path.display()),
            }
        });

        let editor_del = editor_pane.clone();
        let tree_del = file_tree.clone();
        file_tree.set_on_delete(move |path| {
            if let Err(e) = std::fs::remove_file(&path) {
                eprintln!("Cannot delete {}: {e}", path.display());
                return;
            }
            editor_del.close_file(&path);
            tree_del.refresh();
        });

        // ── Compile button ──────────────────────────────────────────────────

        let preview_for_btn = preview_pane.clone();
        let editor_for_btn = editor_pane.clone();
        compile_btn.connect_clicked(move |_| {
            editor_for_btn.save_all_modified();
            preview_for_btn.trigger_compile();
        });

        // ── Settings button ─────────────────────────────────────────────────

        let window_for_settings = window.clone();
        let editor_for_settings = editor_pane.clone();
        let debounce_for_settings = debounce_ms.clone();
        let auto_compile_for_settings = auto_compile.clone();
        let current_config_for_settings = current_config.clone();
        settings_btn.connect_clicked(move |_| {
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
                apply_theme(&new_cfg.theme);

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

        // ── Debounced preview: configurable delay, respects auto_compile ────

        let preview_for_change = preview_pane.clone();
        let editor_for_change = editor_pane.clone();
        let debounce_for_change = debounce_ms.clone();
        let auto_compile_for_change = auto_compile.clone();
        let gen: Rc<RefCell<u64>> = Rc::new(RefCell::new(0));
        let gen2 = gen.clone();
        editor_pane.set_on_change(move || {
            *gen2.borrow_mut() += 1;
            let my_gen = *gen2.borrow();
            let preview = preview_for_change.clone();
            let editor = editor_for_change.clone();
            let gen3 = gen2.clone();
            let auto = auto_compile_for_change.clone();
            let delay = Duration::from_millis(*debounce_for_change.borrow());
            glib::timeout_add_local(delay, move || {
                if *gen3.borrow() == my_gen && *auto.borrow() {
                    editor.save_all_modified();
                    preview.trigger_compile();
                }
                glib::ControlFlow::Break
            });
        });

        // ── Periodic auto-save every 30 s ───────────────────────────────────

        let editor_for_autosave = editor_pane.clone();
        glib::timeout_add_local(Duration::from_secs(30), move || {
            editor_for_autosave.save_all_modified();
            glib::ControlFlow::Continue
        });

        // ── Error panel callbacks ───────────────────────────────────────────

        let error_panel_for_compile = error_panel.clone();
        let root_for_compile = project_root.clone();
        preview_pane.set_on_compile_done(move |result| match result {
            None => error_panel_for_compile.clear(),
            Some(stderr) => {
                let errors = parse_typst_errors(&stderr, &root_for_compile);
                error_panel_for_compile.show_errors(errors);
            }
        });

        let editor_for_jump = editor_pane.clone();
        error_panel.set_on_jump(move |path, line| {
            if let Ok(content) = std::fs::read_to_string(&path) {
                editor_for_jump.open_file(path.clone(), &content);
            }
            editor_for_jump.jump_to_line(&path, line);
        });

        // ── Layout ─────────────────────────────────────────────────────────

        let inner_paned = Paned::new(Orientation::Horizontal);
        inner_paned.set_position(800);
        inner_paned.set_hexpand(true);
        inner_paned.set_vexpand(true);
        inner_paned.set_start_child(Some(editor_pane.widget()));
        inner_paned.set_end_child(Some(preview_pane.widget()));

        let right_col = gtk4::Box::new(Orientation::Vertical, 0);
        right_col.set_hexpand(true);
        right_col.set_vexpand(true);
        right_col.append(&inner_paned);
        right_col.append(error_panel.widget());

        let outer_paned = Paned::new(Orientation::Horizontal);
        outer_paned.set_position(220);
        outer_paned.set_hexpand(true);
        outer_paned.set_vexpand(true);
        outer_paned.set_start_child(Some(file_tree.widget()));
        outer_paned.set_end_child(Some(&right_col));

        // Toast overlay wraps the main content area
        toast_for_sync_btn.set_child(Some(&outer_paned));

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&toast_for_sync_btn));

        window.set_content(Some(&toolbar_view));

        Self {
            window,
            editor_pane,
            file_tree,
            preview_pane,
            error_panel,
            project_root,
            project_model,
        }
    }

    pub fn setup_keybindings(&self) {
        let editor = self.editor_pane.clone();
        let file_tree = self.file_tree.clone();
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
            if ctrl && shift && key == Key::p {
                editor.save_all_modified();
                preview.trigger_compile();
                return glib::Propagation::Stop;
            }
            if ctrl && !shift && key == Key::r {
                file_tree.refresh();
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

    pub fn open_initial_file(&self) {
        let path = self.project_root.join("main.typ");
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => {
                let default = "// Welcome to \u{0417}\u{0435}\u{0440}\u{043a}\u{0430}\u{043b}\u{043e}\n\n= Introduction\n\nStart writing here...\n";
                let _ = std::fs::write(&path, default);
                self.file_tree.refresh();
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
