use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{ApplicationWindow, Orientation, Paned};

use crate::project_model::ProjectModel;
use super::editor_pane::EditorPane;
use super::file_tree::FileTree;
use super::preview_pane::PreviewPane;

pub struct AppWindow {
    window: ApplicationWindow,
    editor_pane: EditorPane,
    file_tree: FileTree,
    preview_pane: PreviewPane,
    project_root: PathBuf,
    #[allow(dead_code)]
    project_model: ProjectModel,
}

impl AppWindow {
    pub fn new(app: &gtk4::Application, project_root: PathBuf) -> Self {
        let window = ApplicationWindow::new(app);
        window.set_title(Some("Зеркало"));
        window.set_default_width(1600);
        window.set_default_height(1000);

        let editor_pane = EditorPane::new();
        let file_tree = FileTree::new(project_root.clone());
        let project_model = ProjectModel::scan(project_root.clone());

        if let Some(f) = &project_model.root_file {
            tracing::info!("Detected root file: {}", f.display());
        }

        let preview_pane = PreviewPane::new(project_model.root_file.clone());

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

        // ── Debounced preview: fire 500 ms after last buffer change ─────────
        // Counter-based: each keystroke increments `gen`; the timer only
        // compiles when the generation it captured still matches.
        let preview_for_change = preview_pane.clone();
        let gen: Rc<RefCell<u64>> = Rc::new(RefCell::new(0));
        let gen2 = gen.clone();
        editor_pane.set_on_change(move || {
            *gen2.borrow_mut() += 1;
            let my_gen = *gen2.borrow();
            let preview = preview_for_change.clone();
            let gen3 = gen2.clone();
            glib::timeout_add_local(Duration::from_millis(500), move || {
                if *gen3.borrow() == my_gen {
                    preview.trigger_compile();
                }
                glib::ControlFlow::Break
            });
        });

        // ── Layout ─────────────────────────────────────────────────────────

        let inner_paned = Paned::new(Orientation::Horizontal);
        inner_paned.set_position(800);
        inner_paned.set_hexpand(true);
        inner_paned.set_vexpand(true);
        inner_paned.set_start_child(Some(editor_pane.widget()));
        inner_paned.set_end_child(Some(preview_pane.widget()));

        let outer_paned = Paned::new(Orientation::Horizontal);
        outer_paned.set_position(220);
        outer_paned.set_hexpand(true);
        outer_paned.set_vexpand(true);
        outer_paned.set_start_child(Some(file_tree.widget()));
        outer_paned.set_end_child(Some(&inner_paned));

        window.set_child(Some(&outer_paned));

        Self {
            window,
            editor_pane,
            file_tree,
            preview_pane,
            project_root,
            project_model,
        }
    }

    pub fn setup_keybindings(&self) {
        let editor = self.editor_pane.clone();
        let file_tree = self.file_tree.clone();
        let preview = self.preview_pane.clone();
        let controller = gtk4::EventControllerKey::new();
        controller.connect_key_pressed(move |_, key, _, modifier| {
            use gtk4::gdk::{Key, ModifierType};
            let ctrl = modifier.contains(ModifierType::CONTROL_MASK);
            let shift = modifier.contains(ModifierType::SHIFT_MASK);

            if ctrl && key == Key::s {
                if let Some(path) = editor.get_active_path() {
                    if let Some(content) = editor.get_active_content() {
                        if std::fs::write(&path, content).is_ok() {
                            editor.mark_saved(&path);
                            // Compile after save
                            preview.trigger_compile();
                        }
                    }
                }
                return glib::Propagation::Stop;
            }

            if ctrl && shift && key == Key::p {
                preview.trigger_compile();
                return glib::Propagation::Stop;
            }

            if ctrl && key == Key::r {
                file_tree.refresh();
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
