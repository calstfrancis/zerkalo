use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;

/// Client ID for Zerkalo's GitHub OAuth App (Device Flow enabled).
/// Client IDs are not secret — safe to bake into the binary.
pub const CLIENT_ID: &str = "Ov23lija1PxCztBKxkT6";

const USER_AGENT: &str = "Zerkalo (https://github.com/calstfrancis/zerkalo)";

#[derive(Debug, Error)]
pub enum GithubAuthError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Sign-in was cancelled or denied.")]
    AccessDenied,
    #[error("Sign-in was cancelled.")]
    Cancelled,
    #[error("The sign-in code expired before it was approved. Try again.")]
    ExpiredToken,
    #[error("GitHub error: {0}")]
    Api(String),
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Deserialize)]
struct AccessTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubUser {
    login: String,
    id: u64,
    #[serde(default)]
    name: Option<String>,
}

/// Who the signed-in user is, in the form git needs to record a commit.
///
/// This exists so setup never has to ask for a name and email: getting them
/// wrong is silent and permanent (every commit is attributed to the typo), and
/// they are the least meaningful thing to ask a writer for.
#[derive(Debug, Clone, PartialEq)]
pub struct Identity {
    pub login: String,
    pub name: String,
    pub email: String,
}

/// The address GitHub guarantees will attribute a commit to this account.
///
/// Deliberately not the account's public `email` field: that is null for
/// anyone with email privacy switched on, and a commit pushed with an address
/// GitHub doesn't recognise is attributed to nobody at all.
fn noreply_email(id: u64, login: &str) -> String {
    format!("{id}+{login}@users.noreply.github.com")
}

#[derive(Debug, Deserialize)]
struct CreatedRepo {
    clone_url: String,
}

fn client() -> Result<reqwest::blocking::Client, GithubAuthError> {
    Ok(reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .build()?)
}

/// Starts the device flow: requests a user code and verification URL the
/// caller should display, plus a device code used to poll for approval.
pub fn request_device_code(client_id: &str) -> Result<DeviceCodeResponse, GithubAuthError> {
    let resp = client()?
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[("client_id", client_id), ("scope", "repo")])
        .send()?
        .error_for_status()?;
    Ok(resp.json()?)
}

/// Blocks, polling GitHub until the user approves (or denies/expires) the
/// device code, or `cancelled` is set. Intended to run on a background
/// thread — sleeps between polls per the server-provided interval, checking
/// `cancelled` every second so a cancel is picked up promptly rather than
/// only at the next multi-second poll interval.
pub fn poll_for_access_token(
    client_id: &str,
    device: &DeviceCodeResponse,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<String, GithubAuthError> {
    use std::sync::atomic::Ordering;

    let http = client()?;
    let mut interval = Duration::from_secs(device.interval.max(1));
    let deadline = std::time::Instant::now() + Duration::from_secs(device.expires_in);

    loop {
        let mut slept = Duration::ZERO;
        while slept < interval {
            if cancelled.load(Ordering::Relaxed) {
                return Err(GithubAuthError::Cancelled);
            }
            let step = Duration::from_secs(1).min(interval - slept);
            std::thread::sleep(step);
            slept += step;
        }
        if cancelled.load(Ordering::Relaxed) {
            return Err(GithubAuthError::Cancelled);
        }
        if std::time::Instant::now() > deadline {
            return Err(GithubAuthError::ExpiredToken);
        }

        let resp: AccessTokenResponse = http
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&[
                ("client_id", client_id),
                ("device_code", &device.device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()?
            .error_for_status()?
            .json()?;

        if let Some(token) = resp.access_token {
            return Ok(token);
        }

        match resp.error.as_deref() {
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                interval += Duration::from_secs(5);
                continue;
            }
            Some("expired_token") => return Err(GithubAuthError::ExpiredToken),
            Some("access_denied") => return Err(GithubAuthError::AccessDenied),
            Some(other) => return Err(GithubAuthError::Api(other.to_string())),
            None => return Err(GithubAuthError::Api("unknown response".to_string())),
        }
    }
}

/// Returns the login name of the authenticated user.
pub fn fetch_username(token: &str) -> Result<String, GithubAuthError> {
    fetch_identity(token).map(|i| i.login)
}

/// Returns the signed-in account as a git identity — display name and a commit
/// address that GitHub will attribute correctly.
pub fn fetch_identity(token: &str) -> Result<Identity, GithubAuthError> {
    let resp = client()?
        .get("https://api.github.com/user")
        .bearer_auth(token)
        .send()?
        .error_for_status()?;
    let user: GithubUser = resp.json()?;
    let name = user
        .name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| user.login.clone());
    let email = noreply_email(user.id, &user.login);
    Ok(Identity { login: user.login, name, email })
}

/// Creates a new repository under the authenticated user's account and
/// returns its HTTPS clone URL.
pub fn create_repo(token: &str, name: &str, private: bool) -> Result<String, GithubAuthError> {
    let resp = client()?
        .post("https://api.github.com/user/repos")
        .bearer_auth(token)
        .json(&serde_json::json!({ "name": name, "private": private }))
        .send()?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(GithubAuthError::Api(format!("{status}: {body}")));
    }

    let repo: CreatedRepo = resp.json()?;
    Ok(repo.clone_url)
}

/// Turns a folder name into a repository name GitHub will accept: it allows
/// only letters, digits, `.`, `-` and `_`, so a work folder called "My Thesis
/// (2026)" has to become something before it is sent, or repository creation
/// fails with a raw API error the user can do nothing with.
pub fn sanitize_repo_name(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for c in raw.trim().chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches(['-', '.'].as_slice());
    if trimmed.is_empty() { "zerkalo-docs".to_string() } else { trimmed.to_string() }
}

/// The repository name offered for a work folder — the folder's own name with
/// `-docs` after it, so the account's repository list says what the repository
/// holds rather than which program made it.
pub fn suggested_repo_name(work_dir: &std::path::Path) -> String {
    let stem = work_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("zerkalo");
    let base = sanitize_repo_name(stem);
    if base.ends_with("-docs") { base } else { format!("{base}-docs") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noreply_address_uses_the_id_prefixed_form_github_attributes() {
        assert_eq!(
            noreply_email(583231, "octocat"),
            "583231+octocat@users.noreply.github.com"
        );
    }

    #[test]
    fn an_account_with_no_display_name_falls_back_to_its_login() {
        let user: GithubUser =
            serde_json::from_str(r#"{"login":"octocat","id":1,"name":null}"#).unwrap();
        assert!(user.name.is_none(), "name absent, so the login must be used");
    }

    #[test]
    fn a_blank_display_name_is_treated_as_absent() {
        // GitHub returns "" rather than null for a name that was set and then
        // cleared; committing as "" would leave every commit unattributed.
        let user: GithubUser =
            serde_json::from_str(r#"{"login":"octocat","id":1,"name":"   "}"#).unwrap();
        let name = user
            .name
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| user.login.clone());
        assert_eq!(name, "octocat");
    }

    #[test]
    fn repo_names_drop_characters_github_rejects() {
        assert_eq!(sanitize_repo_name("My Thesis (2026)"), "my-thesis-2026");
        assert_eq!(sanitize_repo_name("Zerkalo"), "zerkalo");
        assert_eq!(sanitize_repo_name("a//b"), "a-b");
    }

    #[test]
    fn a_folder_name_made_entirely_of_rejected_characters_still_yields_a_name() {
        assert_eq!(sanitize_repo_name("!!!"), "zerkalo-docs");
        assert_eq!(sanitize_repo_name(""), "zerkalo-docs");
    }

    #[test]
    fn the_default_work_folder_suggests_zerkalo_docs() {
        assert_eq!(
            suggested_repo_name(std::path::Path::new("/home/x/Documents/Zerkalo")),
            "zerkalo-docs"
        );
    }

    #[test]
    fn a_named_project_folder_keeps_its_name() {
        assert_eq!(
            suggested_repo_name(std::path::Path::new("/home/x/Documents/My Thesis")),
            "my-thesis-docs"
        );
    }

    #[test]
    fn a_folder_already_ending_in_docs_is_not_given_a_second_suffix() {
        assert_eq!(
            suggested_repo_name(std::path::Path::new("/home/x/thesis-docs")),
            "thesis-docs"
        );
    }
}
