mod bibliography;
mod config;
mod error;
mod git_sync;
mod project;
mod project_model;
mod ui;

use std::env;

use glib::ExitCode;
use gtk4::prelude::*;
use libadwaita as adw;

use config::Config;
use ui::app_window::AppWindow;
use ui::project_dialog::ProjectDialog;

fn main() -> ExitCode {
    tracing_subscriber::fmt::init();

    let app = adw::Application::new(Some("io.github.calstfrancis.Zerkalo"), Default::default());
    app.connect_activate(|app| {
        let existing = Config::load()
            .ok()
            .filter(|c| c.project_path.is_dir());

        if let Some(config) = existing {
            open_main_window(app, config);
        } else {
            let dialog = ProjectDialog::new(app);
            let app_clone = app.clone();
            dialog.set_on_project_chosen(move |path| {
                let mut cfg = Config::default();
                cfg.project_path = path.clone();
                if let Err(e) = cfg.save() {
                    eprintln!("Failed to save config: {e}");
                }
                open_main_window(&app_clone, cfg);
            });
            dialog.present();
        }
    });
    app.run_with_args(&env::args().collect::<Vec<_>>())
}

fn open_main_window(app: &adw::Application, config: Config) {
    let window = AppWindow::new(app, config);
    window.setup_keybindings();
    window.open_initial_file();
    window.present();
}
