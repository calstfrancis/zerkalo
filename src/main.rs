// Nearly every UI struct stores its callbacks as
// `Rc<RefCell<Option<Box<dyn Fn(..)>>>>` slots. Clippy counts each of those as a
// complex type; naming ~90 one-use aliases would add indirection without making
// anything clearer, and the shape is uniform enough to read at a glance.
#![allow(clippy::type_complexity)]

mod auto_save;
mod bib_sanitize;
mod bibliography;
mod comments;
mod compile_stats;
mod compiler;
mod config;
mod cv_mode;
mod doc_import;
mod error;
mod error_patterns;
mod file_watcher;
mod fonts;
mod git_sync;
mod github_auth;
mod i18n;
mod import_log;
mod imposition;
mod keybindings;
mod library;
mod lsp;
mod print_layout;
mod project;
mod project_model;
mod secret_store;
mod session;
mod spellcheck;
mod styles;
mod templates;
mod typst_universe;
mod ui;
mod user_templates;
mod vault_watch;
mod web_export;
mod writing_log;

use std::cell::RefCell;
use std::env;
use std::path::PathBuf;
use std::rc::Rc;

use glib::ExitCode;
use gtk4::gio::ApplicationFlags;
use gtk4::prelude::*;
use libadwaita as adw;

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
    let app = adw::Application::new(
        Some("io.github.calstfrancis.Zerkalo"),
        ApplicationFlags::HANDLES_OPEN,
    );

    // Shared handle so connect_open can reach the already-running window.
    let shared_window: Rc<RefCell<Option<AppWindow>>> = Rc::new(RefCell::new(None));

    let sw_activate = shared_window.clone();
    app.connect_activate(move |app| {
        let window = new_window(app);
        window.open_initial_file(initial_file.clone());
        window.present();
        *sw_activate.borrow_mut() = Some(window);
    });

    // Fired instead of `activate` whenever GLib's default command-line
    // handling sees file arguments — both when the app is already running
    // (Nautilus/xdg-open handing a file to the running instance) and, less
    // obviously, on the very first launch with a file argument, since
    // HANDLES_OPEN routes that through `open` too and `activate` never fires
    // at all. Previously only the already-running case was handled here, so
    // `zerkalo somefile.typ` from a cold start opened no window and exited.
    let sw_open = shared_window.clone();
    app.connect_open(move |app, files, _hint| {
        let paths: Vec<PathBuf> = files
            .iter()
            .filter_map(|f| f.path())
            .filter(|p| p.extension().map(|e| e == "typ").unwrap_or(false))
            .collect();

        let mut borrow = sw_open.borrow_mut();
        if borrow.is_none() {
            let window = new_window(app);
            window.open_initial_file(paths.first().cloned());
            window.present();
            if paths.len() > 1 {
                window.open_external(&paths[1..]);
            }
            *borrow = Some(window);
            return;
        }
        if let Some(w) = borrow.as_ref() {
            w.open_external(&paths);
        }
    });

    // _guard is kept alive here until app.run() returns, ensuring logs are flushed.
    app.run_with_args(&env::args().collect::<Vec<_>>())
}

/// Migrates the old config-dir location, retires stale recovery copies, and
/// constructs a fresh `AppWindow` — the setup shared by `activate` and the
/// first-launch-with-a-file branch of `open` (see `main`), which needs its
/// own window since GLib never fires `activate` when there are file
/// arguments to open.
fn new_window(app: &adw::Application) -> AppWindow {
    crate::auto_save::prune();
    crate::ui::app_window::prune_import_staging();
    let config = crate::config::shared().borrow().clone();
    let window = AppWindow::new(app, config);
    window.setup_keybindings();
    window
}

fn percent_decode_uri(s: &str) -> String {
    // Decode into raw bytes first so multi-byte UTF-8 sequences (non-ASCII
    // filenames) are reassembled correctly before converting to String.
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_hexdigit()
            && bytes[i + 2].is_ascii_hexdigit()
        {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
