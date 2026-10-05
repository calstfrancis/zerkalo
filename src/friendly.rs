//! Turning technical failures into one calm sentence.
//!
//! A person who sees "os error 28" or a page of git output learns only that
//! something is wrong. [`reason`] gives the plain cause when we know it; the
//! raw text is kept for a "Details" line or the clipboard, never as the
//! headline.

use std::io::ErrorKind;

/// A short, blame-free reason for an `io::Error`, as a clause that reads after
/// "because" (no capital, no full stop).
pub fn io_reason(e: &std::io::Error) -> String {
    if e.raw_os_error() == Some(28) {
        return "the disk is full".into();
    }
    match e.kind() {
        ErrorKind::PermissionDenied => "Zerkalo isn't allowed to write there".into(),
        ErrorKind::NotFound => "the folder or file isn't there any more".into(),
        ErrorKind::AlreadyExists => "something with that name is already there".into(),
        ErrorKind::ReadOnlyFilesystem => "that location is read-only".into(),
        _ => "something unexpected got in the way".into(),
    }
}

/// A plain sentence for text output by git, pandoc and friends. Returns `None`
/// when nothing in it is recognised, so the caller can fall back to its own
/// wording and keep the raw text for details.
pub fn reason(raw: &str) -> Option<&'static str> {
    let l = raw.to_lowercase();
    let has = |s: &str| l.contains(s);
    if has("no space left") || has("disk full") {
        Some("The disk is full.")
    } else if has("permission denied") || has("read-only file system") {
        Some("Zerkalo isn't allowed to write to that place.")
    } else if has("could not resolve host")
        || has("network is unreachable")
        || has("connection timed out")
        || has("unable to access")
        || has("failed to connect")
        || has("temporary failure in name resolution")
    {
        Some("Zerkalo couldn't reach the internet.")
    } else if has("authentication failed")
        || has("invalid username")
        || has("bad credentials")
        || has("could not read username")
    {
        Some("GitHub didn't accept Zerkalo's sign-in.")
    } else if has("command not found") || has("executable file not found") {
        Some("A program Zerkalo needs isn't installed.")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phrases the voice guide (docs/VOICE.md) retires from anything a person
    /// reads. Technical text belongs under "Details", not in a headline.
    #[test]
    fn the_voice_guide_is_kept() {
        const BANNED: &[&str] = &[
            "Import Failed",
            "Backup Failed",
            "Export Failed",
            "Some backups failed",
            "Rename failed",
            "Repair failed",
            "\"Discard\"",
            "cannot be undone",
            "This cannot be undone",
            "Permanently Delete",
            "\"LSP",
            "no root file",
            "Invalid regex",
            "Commit & push",
            "Could not load diff",
            "Failed to check the import",
            "\"Error: {e}\"",
            "Failed to start command",
        ];
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs" || x == "ftl") {
                    out.push(p);
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = Vec::new();
        walk(&root.join("src/ui"), &mut files);
        walk(&root.join("locales"), &mut files);
        let mut hits = Vec::new();
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap();
            let body = text.split("#[cfg(test)]").next().unwrap_or(&text);
            for (n, line) in body.lines().enumerate() {
                let t = line.trim_start();
                if t.starts_with("//") {
                    continue;
                }
                for b in BANNED {
                    if line.contains(b) {
                        hits.push(format!("{}:{}: {b}", f.display(), n + 1));
                    }
                }
            }
        }
        assert!(
            hits.is_empty(),
            "words retired by docs/VOICE.md are back:\n{}",
            hits.join("\n")
        );
    }

    #[test]
    fn known_causes_get_a_plain_sentence() {
        assert_eq!(
            reason("fatal: unable to access 'https://x': Could not resolve host: github.com"),
            Some("Zerkalo couldn't reach the internet.")
        );
        assert_eq!(
            reason("remote: Invalid username or password.\nfatal: Authentication failed"),
            Some("GitHub didn't accept Zerkalo's sign-in.")
        );
        assert_eq!(
            reason("write: No space left on device"),
            Some("The disk is full.")
        );
        assert!(reason("bash: pandoc: command not found").is_some());
    }

    #[test]
    fn unknown_text_is_left_for_the_caller() {
        assert_eq!(reason("something odd happened"), None);
    }

    #[test]
    fn io_errors_never_show_os_codes() {
        let full = std::io::Error::from_raw_os_error(28);
        assert_eq!(io_reason(&full), "the disk is full");
        let denied = std::io::Error::from(ErrorKind::PermissionDenied);
        assert!(!io_reason(&denied).contains("os error"));
    }
}
