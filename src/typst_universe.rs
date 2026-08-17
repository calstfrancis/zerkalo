//! Fetches and caches the public Typst Universe `@preview` package index, so
//! the package browser can offer search/install instead of only listing
//! packages already downloaded to the local cache.
//!
//! Network access happens here only — callers are responsible for running
//! [`fetch_index`] off the GTK main thread.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use serde::Deserialize;

const INDEX_URL: &str = "https://packages.typst.org/preview/index.json";
const CACHE_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, Deserialize)]
struct RawEntry {
    name: String,
    version: String,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UniversePackage {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
}

fn cache_path() -> PathBuf {
    glib::user_cache_dir()
        .join("zerkalo")
        .join("typst-universe-index.json")
}

/// True if a cached index exists and is recent enough that a caller can show
/// it immediately without waiting on a network round-trip first.
pub fn cache_is_fresh() -> bool {
    std::fs::metadata(cache_path())
        .and_then(|m| m.modified())
        .map(|modified| {
            SystemTime::now()
                .duration_since(modified)
                .map(|age| age < CACHE_MAX_AGE)
                .unwrap_or(true) // clock skew into the future — treat as fresh rather than refetch
        })
        .unwrap_or(false)
}

/// Loads whatever is on disk, regardless of age — for instant first paint
/// while a background refresh (if warranted) is still in flight.
pub fn load_cached_only() -> Option<Vec<UniversePackage>> {
    let body = std::fs::read_to_string(cache_path()).ok()?;
    parse_index(&body).ok()
}

/// Fetches the live index over the network, writing a fresh copy to the
/// on-disk cache on success. Blocking — must not run on the main thread.
pub fn fetch_index() -> Result<Vec<UniversePackage>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(concat!("zerkalo/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;

    let body = client
        .get(INDEX_URL)
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .text()
        .map_err(|e| e.to_string())?;

    let packages = parse_index(&body)?;

    if let Some(parent) = cache_path().parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(cache_path(), &body).ok();

    Ok(packages)
}

/// Folds the raw index (one entry per package *version*) down to the latest
/// version of each package by name, sorted alphabetically.
fn parse_index(body: &str) -> Result<Vec<UniversePackage>, String> {
    let raw: Vec<RawEntry> = serde_json::from_str(body).map_err(|e| e.to_string())?;

    let mut latest: HashMap<String, RawEntry> = HashMap::new();
    for entry in raw {
        match latest.get(&entry.name) {
            Some(existing) if version_key(&existing.version) >= version_key(&entry.version) => {}
            _ => {
                latest.insert(entry.name.clone(), entry);
            }
        }
    }

    let mut out: Vec<UniversePackage> = latest
        .into_values()
        .map(|e| UniversePackage {
            name: e.name,
            version: e.version,
            description: e.description,
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn version_key(v: &str) -> (u32, u32, u32) {
    let mut parts = v.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_index_keeps_latest_version_per_name() {
        let json = r#"[
            {"name":"foo","version":"0.1.0","description":"old"},
            {"name":"foo","version":"0.2.0","description":"new"},
            {"name":"bar","version":"1.0.0","description":"bar pkg"}
        ]"#;
        let pkgs = parse_index(json).unwrap();
        assert_eq!(pkgs.len(), 2);
        let foo = pkgs.iter().find(|p| p.name == "foo").unwrap();
        assert_eq!(foo.version, "0.2.0");
        assert_eq!(foo.description.as_deref(), Some("new"));
    }

    #[test]
    fn parse_index_sorts_alphabetically_by_name() {
        let json = r#"[{"name":"zeta","version":"1.0.0"},{"name":"alpha","version":"1.0.0"}]"#;
        let pkgs = parse_index(json).unwrap();
        assert_eq!(pkgs[0].name, "alpha");
        assert_eq!(pkgs[1].name, "zeta");
    }

    #[test]
    fn version_key_orders_numerically_not_lexically() {
        assert!(version_key("0.9.0") < version_key("0.10.0"));
    }

    #[test]
    fn parse_index_rejects_malformed_json() {
        assert!(parse_index("not json").is_err());
    }

    #[test]
    fn parse_index_handles_missing_description() {
        let json = r#"[{"name":"nodesc","version":"1.0.0"}]"#;
        let pkgs = parse_index(json).unwrap();
        assert_eq!(pkgs[0].description, None);
    }
}
