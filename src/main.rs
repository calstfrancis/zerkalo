mod bibliography;
mod config;
mod error;
mod fonts;
mod git_sync;
mod lsp;
mod project;
mod project_model;
mod ui;

use std::env;
use std::path::PathBuf;

use glib::ExitCode;
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
    let app = adw::Application::new(Some("io.github.calstfrancis.Zerkalo"), Default::default());
    app.connect_activate(move |app| {
        let config = Config::load().unwrap_or_default();
        let _ = std::fs::create_dir_all(&config.work_dir);
        open_main_window(app, config, initial_file.clone());
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

fn open_main_window(app: &adw::Application, config: Config, initial_file: Option<PathBuf>) {
    let window = AppWindow::new(app, config);
    window.setup_keybindings();
    window.open_initial_file(initial_file);
    window.present();
}
