use gtk4::prelude::*;
use gtk4::{ApplicationWindow, Label, Orientation, Paned};

use super::editor_pane::EditorPane;

pub struct AppWindow {
    window: ApplicationWindow,
    editor_pane: EditorPane,
}

impl AppWindow {
    pub fn new(app: &gtk4::Application) -> Self {
        let window = ApplicationWindow::new(app);
        window.set_title(Some("Зеркало"));
        window.set_default_width(1600);
        window.set_default_height(1000);

        let editor_pane = EditorPane::new();

        let paned = Paned::new(Orientation::Horizontal);
        paned.set_position(800);
        paned.set_hexpand(true);
        paned.set_vexpand(true);

        paned.set_start_child(Some(editor_pane.widget()));

        let preview_stub = Label::new(Some("Preview (coming soon)"));
        preview_stub.set_hexpand(true);
        preview_stub.set_vexpand(true);
        paned.set_end_child(Some(&preview_stub));

        window.set_child(Some(&paned));

        Self {
            window,
            editor_pane,
        }
    }

    pub fn setup_keybindings(&self) {
        let editor = self.editor_pane.clone();
        let controller = gtk4::EventControllerKey::new();
        controller.connect_key_pressed(move |_, key, _, modifier| {
            use gtk4::gdk::{Key, ModifierType};
            if modifier.contains(ModifierType::CONTROL_MASK) && key == Key::s {
                if let Some(path) = editor.get_active_path() {
                    if let Some(content) = editor.get_active_content() {
                        if std::fs::write(&path, content).is_ok() {
                            editor.mark_saved(&path);
                        }
                    }
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.window.add_controller(controller);
    }

    pub fn open_initial_file(&self) {
        let path = std::path::PathBuf::from("main.typ");
        let content = "// Welcome to Зеркало\n\n= Introduction\n\nStart writing here...\n";
        self.editor_pane.open_file(path, content);
    }

    pub fn present(&self) {
        self.window.present();
    }
}
