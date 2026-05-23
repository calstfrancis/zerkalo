use std::path::PathBuf;

use gtk4::prelude::*;
use gtk4::{ApplicationWindow, Label, Orientation, Paned};

use super::editor_pane::EditorPane;
use super::file_tree::FileTree;

pub struct AppWindow {
    window: ApplicationWindow,
    editor_pane: EditorPane,
    file_tree: FileTree,
    project_root: PathBuf,
}

impl AppWindow {
    pub fn new(app: &gtk4::Application, project_root: PathBuf) -> Self {
        let window = ApplicationWindow::new(app);
        window.set_title(Some("Зеркало"));
        window.set_default_width(1600);
        window.set_default_height(1000);

        let editor_pane = EditorPane::new();
        let file_tree = FileTree::new(project_root.clone());

        let editor_for_open = editor_pane.clone();
        file_tree.set_on_open(move |path| {
            match std::fs::read_to_string(&path) {
                Ok(content) => editor_for_open.open_file(path, &content),
                Err(e) => eprintln!("Cannot open {}: {}", path.display(), e),
            }
        });

        let preview_stub = Label::new(Some("Preview (coming soon)"));
        preview_stub.set_hexpand(true);
        preview_stub.set_vexpand(true);

        let inner_paned = Paned::new(Orientation::Horizontal);
        inner_paned.set_position(800);
        inner_paned.set_hexpand(true);
        inner_paned.set_vexpand(true);
        inner_paned.set_start_child(Some(editor_pane.widget()));
        inner_paned.set_end_child(Some(&preview_stub));

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
            project_root,
        }
    }

    pub fn setup_keybindings(&self) {
        let editor = self.editor_pane.clone();
        let file_tree = self.file_tree.clone();
        let controller = gtk4::EventControllerKey::new();
        controller.connect_key_pressed(move |_, key, _, modifier| {
            use gtk4::gdk::{Key, ModifierType};
            let ctrl = modifier.contains(ModifierType::CONTROL_MASK);
            if ctrl && key == Key::s {
                if let Some(path) = editor.get_active_path() {
                    if let Some(content) = editor.get_active_content() {
                        if std::fs::write(&path, content).is_ok() {
                            editor.mark_saved(&path);
                        }
                    }
                }
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
                // Should not happen since init_project creates main.typ, but be safe
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
