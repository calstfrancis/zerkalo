mod bibliography;
mod config;
mod error;
mod fonts;
mod git_sync;
mod keybindings;
mod lsp;
mod project;
mod project_model;
mod session;
mod spellcheck;
mod styles;
mod ui;

use std::cell::RefCell;
use std::env;
use std::path::PathBuf;
use std::rc::Rc;

use glib::ExitCode;
use gtk4::gio::ApplicationFlags;
use gtk4::prelude::*;
use libadwaita as adw;

use config::Config;
use ui::app_window::AppWindow;

fn main() -> ExitCode {
    // ── Logging: persistent file + stderr ────────────────────────────────────
    let log_dir = glib::user_data_dir().join("zerkalo");
    std::fs::create_dir_all(&log_dir).ok();

    let file_appender = tracing_appender::rolling::never(&log_dir, "zerkalo.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(non_blocking),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    tracing::info!(
        "Zerkalo starting — log: {}",
        log_dir.join("zerkalo.log").display()
    );

    // ── CLI help ─────────────────────────────────────────────────────────────
    if env::args().any(|a| a == "--help" || a == "-h") {
        println!(
            "Zerkalo — a Typst editor\n\
             \n\
             Usage: zerkalo [OPTIONS] [FILE]\n\
             \n\
             Arguments:\n\
               FILE           .typ file to open on startup\n\
             \n\
             Options:\n\
               -h, --help     Print this help message\n\
               --version      Print version\n\
             \n\
             Configuration: ~/.config/zerkalo/config.toml\n\
             Log file:       ~/.local/share/zerkalo/zerkalo.log\n\
             Work folder:    set via config.toml work_dir key"
        );
        return ExitCode::SUCCESS;
    }
    if env::args().any(|a| a == "--version") {
        println!("zerkalo {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    fonts::ensure_gost_font();

    let initial_file: Option<PathBuf> = env::args()
        .nth(1)
        .map(|arg| {
            let path_str = if let Some(rest) = arg.strip_prefix("file://") {
                percent_decode_uri(rest)
            } else {
                arg
            };
            PathBuf::from(path_str)
        })
        .filter(|p| p.is_file() && p.extension().map(|e| e == "typ").unwrap_or(false));

    // ── Application ──────────────────────────────────────────────────────────
    let app = adw::Application::new(Some("io.github.calstfrancis.Zerkalo"), ApplicationFlags::HANDLES_OPEN);

    // Shared handle so connect_open can reach the already-running window.
    let shared_window: Rc<RefCell<Option<AppWindow>>> = Rc::new(RefCell::new(None));

    let sw_activate = shared_window.clone();
    app.connect_activate(move |app| {
        let config = Config::load().unwrap_or_default();
        let _ = std::fs::create_dir_all(&config.work_dir);
        let window = AppWindow::new(app, config);
        window.setup_keybindings();
        window.open_initial_file(initial_file.clone());
        window.present();
        *sw_activate.borrow_mut() = Some(window);
    });

    // Fired by the desktop session (Nautilus, xdg-open, etc.) when the app is
    // already running and a file is sent to it via D-Bus activation.
    let sw_open = shared_window.clone();
    app.connect_open(move |_app, files, _hint| {
        let borrow = sw_open.borrow();
        if let Some(w) = borrow.as_ref() {
            let paths: Vec<PathBuf> = files
                .iter()
                .filter_map(|f| f.path())
                .filter(|p| p.extension().map(|e| e == "typ").unwrap_or(false))
                .collect();
            w.open_external(&paths);
        }
    });

    // _guard is kept alive here until app.run() returns, ensuring logs are flushed.
    app.run_with_args(&env::args().collect::<Vec<_>>())
}

fn percent_decode_uri(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Ok(hex), true) = (
                std::str::from_utf8(&bytes[i + 1..i + 3]),
                bytes[i + 1].is_ascii_hexdigit() && bytes[i + 2].is_ascii_hexdigit(),
            ) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte as char);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}
