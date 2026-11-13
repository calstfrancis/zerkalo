//! `zerkalo --search-json QUERY…`: search the `.typ` files in the work folder and print the hits
//! as one line of JSON, for the Porog launcher (`flatpak run --command=zerkalo-search …`).
//! No window, no keyring, no network and no writes: it reads the work folder and prints.
//!
//! It reads the config file for `work_dir` only. `Config::load` is not used because it also
//! migrates a stored token into the keyring, which can hang and is no job for a helper.
//! Each hit carries the `args` that open it (the file). Failures are printed as
//! `{"schema":1,"error":"…"}` with exit code 0.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Version of the JSON this prints. Bumped only for a change an older Porog cannot ignore.
const SCHEMA: u32 = 1;
/// Most hits one answer carries; Porog shows a handful.
const LIMIT: usize = 8;
/// How far below the work folder to look, and how many files to read at most.
const MAX_DEPTH: usize = 4;
const MAX_FILES: usize = 2000;
/// Larger files are skipped: a `.typ` this big is generated, not written.
const MAX_BYTES: u64 = 2 * 1024 * 1024;
/// Longest context line a hit carries.
const SNIPPET_CHARS: usize = 90;

pub fn run(query: &[String]) {
    println!("{}", answer(query));
}

fn answer(query: &[String]) -> String {
    let words: Vec<String> = query.iter().map(|w| w.to_lowercase()).collect();
    if words.is_empty() {
        return serde_json::json!({ "schema": SCHEMA, "hits": [] }).to_string();
    }
    let root = work_dir();
    if !root.is_dir() {
        return serde_json::json!({ "schema": SCHEMA, "error": "Zerkalo has no work folder yet." })
            .to_string();
    }
    let hits: Vec<_> = search(&root, &words)
        .into_iter()
        .map(|h| {
            serde_json::json!({
                "title": h.title,
                "detail": h.detail,
                "args": [h.path.to_string_lossy()],
            })
        })
        .collect();
    serde_json::json!({ "schema": SCHEMA, "hits": hits }).to_string()
}

/// The `work_dir` from the config file, or Zerkalo's default folder.
fn work_dir() -> PathBuf {
    fs::read_to_string(crate::config::zerkalo_config_dir().join("config.toml"))
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .and_then(|table| {
            table
                .get("work_dir")
                .and_then(|v| v.as_str().map(PathBuf::from))
        })
        .unwrap_or_else(crate::config::default_work_dir_pub)
}

#[derive(Debug, PartialEq, Eq)]
struct Hit {
    path: PathBuf,
    title: String,
    detail: String,
}

/// Files whose name, or whose text, contains every word; name matches first, then the most
/// recently changed.
fn search(root: &Path, words: &[String]) -> Vec<Hit> {
    let mut files = Vec::new();
    collect(root, 0, &mut files);
    let mut found: Vec<(bool, SystemTime, Hit)> = Vec::new();
    for (path, modified) in files {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        let name = relative.to_lowercase();
        let by_name = words.iter().all(|w| name.contains(w.as_str()));
        let text = fs::read_to_string(&path).unwrap_or_default();
        let lower = text.to_lowercase();
        let by_text = words.iter().all(|w| lower.contains(w.as_str()));
        if !by_name && !by_text {
            continue;
        }
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let title = heading(&text).unwrap_or(stem);
        let detail = if by_name {
            relative
        } else {
            let has_word = |l: &&str| words.iter().any(|w| l.to_lowercase().contains(w.as_str()));
            let line = text
                .lines()
                .filter(|l| !l.starts_with("= "))
                .find(has_word)
                .or_else(|| text.lines().find(has_word))
                .unwrap_or("");
            format!("{relative} · {}", shorten(line))
        };
        found.push((
            by_name,
            modified,
            Hit {
                path,
                title,
                detail,
            },
        ));
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    found
        .into_iter()
        .take(LIMIT)
        .map(|(_, _, hit)| hit)
        .collect()
}

/// The `.typ` files under `dir`, skipping hidden folders and build output.
fn collect(dir: &Path, depth: usize, out: &mut Vec<(PathBuf, SystemTime)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= MAX_FILES {
            return;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            if depth < MAX_DEPTH
                && !name.starts_with('.')
                && name != "node_modules"
                && name != "target"
            {
                collect(&path, depth + 1, out);
            }
        } else if meta.is_file()
            && meta.len() <= MAX_BYTES
            && path.extension().is_some_and(|e| e == "typ")
        {
            out.push((path, meta.modified().unwrap_or(SystemTime::UNIX_EPOCH)));
        }
    }
}

/// The text of the first top-level `= Heading`, if the file has one.
fn heading(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.strip_prefix("= "))
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty())
}

fn shorten(line: &str) -> String {
    let one_line = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= SNIPPET_CHARS {
        return one_line;
    }
    let cut: String = one_line.chars().take(SNIPPET_CHARS).collect();
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("zerkalo-search-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("essays")).unwrap();
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::write(
            dir.join("essays/doors.typ"),
            "= On Doors\nA threshold is a door.\n",
        )
        .unwrap();
        fs::write(dir.join("notes.typ"), "Remember the threshold.\n").unwrap();
        fs::write(dir.join(".git/hidden.typ"), "threshold").unwrap();
        fs::write(dir.join("readme.md"), "threshold").unwrap();
        dir
    }

    fn words(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn finds_by_name_then_text_and_skips_hidden_and_non_typst() {
        let dir = scratch("names");
        let by_text = search(&dir, &words("threshold"));
        assert_eq!(by_text.len(), 2);
        let by_name = search(&dir, &words("doors"));
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].title, "On Doors");
        assert_eq!(by_name[0].detail, "essays/doors.typ");
        let both = search(&dir, &words("door threshold"));
        assert_eq!(both.len(), 1);
        assert!(search(&dir, &words("absent")).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_text_match_shows_the_line_it_came_from() {
        let dir = scratch("lines");
        let hit = search(&dir, &words("remember"));
        assert_eq!(hit[0].title, "notes");
        assert!(
            hit[0].detail.ends_with("Remember the threshold."),
            "{}",
            hit[0].detail
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
