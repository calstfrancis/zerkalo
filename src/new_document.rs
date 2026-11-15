//! `zerkalo --new-document=TITLE`: a blank document named for TITLE in the work folder, opened
//! at startup like any file argument. Porog's "New essay" action starts Zerkalo this way.
//!
//! It runs before GTK, so like the search helper it reads only `work_dir` from the config.

use std::fs;
use std::path::{Path, PathBuf};

/// The flag's prefix; the rest of the argument is the title.
pub const FLAG: &str = "--new-document=";

/// Create the document in the configured work folder and return its path.
pub fn create(title: &str) -> Option<PathBuf> {
    create_in(&crate::search_helper::work_dir(), title)
}

fn create_in(dir: &Path, title: &str) -> Option<PathBuf> {
    let title = title.trim();
    if title.is_empty() {
        return None;
    }
    fs::create_dir_all(dir).ok()?;
    let name = crate::ui::name_prompt::suggest_free_name(dir, &file_stem(title));
    let path = dir.join(format!("{name}.typ"));
    fs::write(&path, format!("= {title}\n\n")).ok()?;
    Some(path)
}

/// The title as a file name: path separators and the characters Windows forbids become
/// spaces, runs of spaces collapse, and dot-only words and a leading dot are dropped so it is never hidden.
fn file_stem(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\n' | '\r' | '\t' => ' ',
            c => c,
        })
        .collect();
    let words: Vec<&str> = cleaned
        .split_whitespace()
        .filter(|w| !w.chars().all(|c| c == '.'))
        .collect();
    words
        .join(" ")
        .trim_start_matches('.')
        .chars()
        .take(100)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("zerkalo-newdoc-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn the_file_is_named_for_the_title_and_starts_with_it_as_a_heading() {
        let dir = scratch("basic");
        let path = create_in(&dir, "THM 501: Essay on Grace").unwrap();
        assert_eq!(path, dir.join("THM 501 Essay on Grace.typ"));
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "= THM 501: Essay on Grace\n\n"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_second_document_with_the_same_title_gets_the_next_free_name() {
        let dir = scratch("twice");
        let first = create_in(&dir, "Essay").unwrap();
        let second = create_in(&dir, "Essay").unwrap();
        assert_eq!(first, dir.join("Essay.typ"));
        assert_eq!(second, dir.join("Essay 2.typ"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_empty_title_creates_nothing() {
        let dir = scratch("empty");
        assert_eq!(create_in(&dir, "   "), None);
        assert!(!dir.exists());
    }

    #[test]
    fn path_characters_cannot_escape_the_folder() {
        assert_eq!(file_stem("../../etc/passwd"), "etc passwd");
        assert_eq!(file_stem("a\\b"), "a b");
    }
}
