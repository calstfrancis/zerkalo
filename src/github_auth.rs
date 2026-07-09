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
/// device code. Intended to run on a background thread — sleeps between
/// polls per the server-provided interval.
pub fn poll_for_access_token(
    client_id: &str,
    device: &DeviceCodeResponse,
) -> Result<String, GithubAuthError> {
    let http = client()?;
    let mut interval = Duration::from_secs(device.interval.max(1));
    let deadline = std::time::Instant::now() + Duration::from_secs(device.expires_in);

    loop {
        std::thread::sleep(interval);
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
    let resp = client()?
        .get("https://api.github.com/user")
        .bearer_auth(token)
        .send()?
        .error_for_status()?;
    let user: GithubUser = resp.json()?;
    Ok(user.login)
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
