mod error;
mod ui;

use gtk4::prelude::*;
use gtk4::Application;
use glib::ExitCode;
use std::env;

use ui::app_window::AppWindow;

fn main() -> ExitCode {
    let app = Application::new(Some("io.github.calstfrancis.Zerkalo"), Default::default());
    app.connect_activate(|app| {
        let window = AppWindow::new(app);
        window.setup_keybindings();
        window.open_initial_file();
        window.present();
    });
    app.run_with_args(&env::args().collect::<Vec<_>>())
}
