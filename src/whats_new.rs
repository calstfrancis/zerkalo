//! What the "What's New" dialog shows, taken straight from `CHANGELOG.md`.
//!
//! The dialog used to carry hand-typed bullets that had to be remembered at every
//! release — and were not, so it kept announcing the tab bar long after. Now the
//! changelog is the one source: every release already has to write its entry
//! there, and a test fails the build if the entry for the version being built is
//! missing. A person who skips versions is shown each release they missed, not
//! just the newest.

const CHANGELOG: &str = include_str!("../CHANGELOG.md");

/// One release, ready to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub name: String,
    pub subtitle: String,
    /// One short line per change: the bold lead of each changelog bullet.
    pub items: Vec<String>,
}

fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
    let mut parts = v.trim().split('.').map(|p| p.parse::<u32>());
    Some((
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    ))
}

/// `## [0.42.0] "Joined Glass" — 2026-10-02 — Subtitle`
fn parse_heading(line: &str) -> Option<(String, String, String)> {
    let rest = line.strip_prefix("## [")?;
    let (version, rest) = rest.split_once(']')?;
    parse_version(version)?;
    let name = rest
        .split('"')
        .nth(1)
        .map(str::to_string)
        .unwrap_or_default();
    // After the date: whatever follows the last " — ".
    let subtitle = rest
        .rsplit_once(" — ")
        .map(|(_, s)| s.trim().to_string())
        .filter(|s| !s.starts_with("20") || s.len() > 10)
        .unwrap_or_default();
    Some((version.to_string(), name, subtitle))
}

fn plain(text: &str) -> String {
    text.replace("**", "").replace('`', "")
}

/// The short line for one bullet: its bold lead if it has one, otherwise its
/// first sentence.
fn item_text(bullet: &str) -> Option<String> {
    let body = bullet.strip_prefix("- ")?.trim();
    let text = if let Some(rest) = body.strip_prefix("**") {
        rest.split_once("**")?.0.to_string()
    } else {
        body.split_once(". ")
            .map(|(s, _)| s.to_string())
            .unwrap_or_else(|| body.to_string())
    };
    let text = plain(&text)
        .trim()
        .trim_end_matches(['.', ':'])
        .trim()
        .to_string();
    (!text.is_empty()).then_some(text)
}

/// Every released version in `changelog`, newest first. `[Unreleased]` is left out.
pub fn releases_in(changelog: &str) -> Vec<Release> {
    let mut out: Vec<Release> = Vec::new();
    let mut counting = false;
    for line in changelog.lines() {
        if line.starts_with("## ") {
            counting = false;
            if let Some((version, name, subtitle)) = parse_heading(line) {
                out.push(Release {
                    version,
                    name,
                    subtitle,
                    items: Vec::new(),
                });
            }
        } else if let Some(heading) = line.strip_prefix("### ") {
            counting = matches!(
                heading.trim(),
                "Added" | "Changed" | "Fixed" | "Removed" | "Security"
            );
        } else if counting && line.starts_with("- ") {
            if let (Some(rel), Some(text)) = (out.last_mut(), item_text(line)) {
                rel.items.push(text);
            }
        }
    }
    out
}

/// The releases a person hasn't seen: everything newer than `last_seen`, newest
/// first, at most `max` of them. With no `last_seen` (or one that isn't in the
/// list) only the newest release is shown.
pub fn since(changelog: &str, last_seen: Option<&str>, max: usize) -> Vec<Release> {
    let all = releases_in(changelog);
    let seen = last_seen.and_then(parse_version);
    let newer: Vec<Release> = match seen {
        Some(seen) => all
            .iter()
            .filter(|r| parse_version(&r.version).is_some_and(|v| v > seen))
            .cloned()
            .collect(),
        None => all.iter().take(1).cloned().collect(),
    };
    let newer = if newer.is_empty() {
        all.into_iter().take(1).collect()
    } else {
        newer
    };
    newer.into_iter().take(max).collect()
}

/// What to show for this build, given the version recorded the last time the
/// dialog was shown.
pub fn for_dialog(last_seen: Option<&str>) -> Vec<Release> {
    since(CHANGELOG, last_seen, 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Changelog\n\n## [Unreleased]\n\n### Added\n\n- **Not out yet.** x\n\n---\n\n\
## [0.42.0] \"Joined Glass\" — 2026-10-02 — Your writing follows you\n\n### Added\n\n- **Get Latest from GitHub.** It brings everything down.\n- **A force option, with a safety net.** Keeps a copy.\n\n### Upgrading — nothing is touched\n\n- Your files are safe.\n\n---\n\n\
## [0.41.1] \"Plain Glass\" — 2026-10-02 — A steadier editor\n\n### Changed\n\n- **Word wrap is always on, and the setting is gone.** Text.\n- A plain bullet with no bold lead. More words.\n\n---\n\n\
## [0.41.0] \"Steady Glass\" — 2026-10-02 — One place to cite from\n\n### Fixed\n\n- **Renaming a key changes only the key.** y\n";

    #[test]
    fn releases_are_read_newest_first_without_unreleased() {
        let r = releases_in(SAMPLE);
        let versions: Vec<_> = r.iter().map(|r| r.version.as_str()).collect();
        assert_eq!(versions, ["0.42.0", "0.41.1", "0.41.0"]);
        assert_eq!(r[0].name, "Joined Glass");
        assert_eq!(r[0].subtitle, "Your writing follows you");
    }

    #[test]
    fn items_are_the_bold_leads_and_upgrade_notes_are_left_out() {
        let r = releases_in(SAMPLE);
        assert_eq!(
            r[0].items,
            [
                "Get Latest from GitHub",
                "A force option, with a safety net"
            ]
        );
        assert_eq!(
            r[1].items,
            [
                "Word wrap is always on, and the setting is gone",
                "A plain bullet with no bold lead"
            ]
        );
    }

    #[test]
    fn someone_who_skipped_versions_sees_each_one_they_missed() {
        let seen = since(SAMPLE, Some("0.41.0"), 4);
        let v: Vec<_> = seen.iter().map(|r| r.version.as_str()).collect();
        assert_eq!(v, ["0.42.0", "0.41.1"]);
    }

    #[test]
    fn up_to_date_or_unknown_shows_just_the_newest() {
        assert_eq!(since(SAMPLE, Some("0.42.0"), 4).len(), 1);
        assert_eq!(since(SAMPLE, Some("0.42.0"), 4)[0].version, "0.42.0");
        assert_eq!(since(SAMPLE, None, 4).len(), 1);
        assert_eq!(since(SAMPLE, Some("garbage"), 4)[0].version, "0.42.0");
    }

    #[test]
    fn the_cap_is_respected() {
        assert_eq!(since(SAMPLE, Some("0.0.1"), 2).len(), 2);
    }

    /// The release being built must have a changelog entry with something to show —
    /// this is what makes "What's New" (and the changelog itself) impossible to forget.
    #[test]
    fn this_version_has_a_changelog_entry_with_items_to_show() {
        let all = releases_in(CHANGELOG);
        let newest = all.first().expect("the changelog has releases");
        assert_eq!(
            newest.version,
            env!("CARGO_PKG_VERSION"),
            "CHANGELOG.md's newest entry must be the version in Cargo.toml"
        );
        assert!(
            !newest.items.is_empty(),
            "the changelog entry for {} has no bullets for What's New to show",
            newest.version
        );
        assert!(!newest.name.is_empty(), "the entry needs a release name");
        assert_eq!(
            newest.name,
            crate::ui::welcome_window::RELEASE_NAME,
            "RELEASE_NAME in welcome_window.rs must match the changelog heading"
        );
    }

    /// The GitHub Release body (`RELEASE.md`) is written by hand each release; its
    /// first line names the version it was written for, so a forgotten update
    /// fails the build instead of shipping last release's notes.
    #[test]
    fn release_notes_were_written_for_this_version() {
        let notes = include_str!("../RELEASE.md");
        let marker = notes
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("<!-- zerkalo-release:"))
            .and_then(|l| l.strip_suffix("-->"))
            .map(str::trim);
        assert_eq!(
            marker,
            Some(env!("CARGO_PKG_VERSION")),
            "RELEASE.md must start with `<!-- zerkalo-release: {} -->` and its notes must describe this release",
            env!("CARGO_PKG_VERSION")
        );
    }
}
