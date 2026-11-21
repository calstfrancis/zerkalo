const SERVICE: &str = "io.github.calstfrancis.Zerkalo";
const USERNAME: &str = "github_token";

fn entry() -> Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new(SERVICE, USERNAME)
}

pub fn save_github_token(token: &str) -> Result<(), String> {
    entry()
        .and_then(|e| e.set_password(token))
        .map_err(|e| e.to_string())
}

pub fn load_github_token() -> Option<String> {
    entry().ok()?.get_password().ok()
}

pub fn delete_github_token() {
    if let Ok(e) = entry() {
        let _ = e.delete_credential();
    }
}

/// Reads the token without ever blocking the window: the keyring is asked on
/// a worker thread and `f` runs on the main loop with the answer. The Secret
/// Service can hang indefinitely on some systems, so nothing that runs on the
/// GTK main thread may call `load_github_token` directly.
pub fn with_token(f: impl FnOnce(Option<String>) + 'static) {
    gtk4::glib::spawn_future_local(async move {
        let token = gtk4::gio::spawn_blocking(load_github_token)
            .await
            .ok()
            .flatten();
        f(token);
    });
}

pub fn save_in_background(token: String) {
    std::thread::spawn(move || {
        let _ = save_github_token(&token);
    });
}

pub fn delete_in_background() {
    std::thread::spawn(delete_github_token);
}
