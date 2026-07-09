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
