use std::path::Path;

use biblatex::{Bibliography, ChunksExt, DateValue, PermissiveType};

#[derive(Clone, Debug, Default)]
pub struct BibEntry {
    pub key: String,
    // Parsed for completeness alongside every other BibEntry field but not yet surfaced in
    // ref_manager's citation list UI (which only shows key/author/title/year today).
    #[allow(dead_code)]
    pub entry_type: String,
    pub author: String,
    pub title: String,
    pub year: String,
    /// Journal, book title or publisher — one more thing a search can find
    /// and a row can show beneath the title.
    pub venue: String,
    /// Each author's surname (an institution's whole name), in order, so the
    /// label is built from real names instead of re-splitting `author`.
    pub families: Vec<String>,
    /// Surnames of the editors, used when a work has no author of its own.
    pub editors: Vec<String>,
}

/// Reads the entries of a bibliography source, or says why it couldn't. An
/// empty file is a fine, empty bibliography; an unreadable or malformed one is
/// a problem the person should be told about, not an empty list that invites
/// them to start a new file over it.
pub fn try_load(path: &Path) -> Result<Vec<BibEntry>, String> {
    if is_vault_dir(path) {
        return try_load_vault(path);
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("the bibliography");
    let content = std::fs::read_to_string(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            format!("Couldn't find {name}. It may have been moved or renamed.")
        }
        _ => format!("Couldn't open {name}: {e}"),
    })?;
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml") => {
            if content.trim().is_empty() {
                return Ok(Vec::new());
            }
            let library = hayagriva::io::from_yaml_str(&content)
                .map_err(|e| format!("Couldn't read {name}: {e}"))?;
            Ok(library.iter().map(bib_entry_from_hayagriva).collect())
        }
        _ => {
            if content.trim().is_empty() {
                return Ok(Vec::new());
            }
            Bibliography::parse(&content)
                .map(|bib| entries_from_bibliography(&bib))
                .map_err(|e| format!("Couldn't read {name}: {e}"))
        }
    }
}

fn try_load_vault(dir: &Path) -> Result<Vec<BibEntry>, String> {
    let library = fond_bib::Library::open(dir)
        .map_err(|e| format!("Couldn't open the Kartoteka vault: {e}"))?;
    let keys = library
        .keys_sorted()
        .map_err(|e| format!("Couldn't list the vault's entries: {e}"))?;
    Ok(keys
        .iter()
        .filter_map(|key| library.load_entry(key).ok())
        .map(|parsed| bib_entry_from_hayagriva(&parsed.entry))
        .collect())
}

/// The entries of `path`, or none if it can't be read.
#[cfg(test)]
pub fn load_bib(path: &Path) -> Vec<BibEntry> {
    try_load(path).unwrap_or_default()
}

/// Whether `path` is a Kartoteka vault root — recognized by its `entries/`
/// subdirectory, the one part of the on-disk layout every vault has
/// (`fond_bib::library::ENTRIES_DIR`).
pub fn is_vault_dir(path: &Path) -> bool {
    path.is_dir() && path.join("entries").is_dir()
}

/// The path a Typst `#bibliography(...)` call should actually name for
/// `bib_path`: unchanged for a plain `.bib`/`.yaml` file, or the vault's
/// `library.yml` when `bib_path` is a Kartoteka vault directory. Typst's
/// `bibliography()` reads a data *file*, not a directory — a raw vault path
/// used to be written verbatim, which meant a configured vault could never
/// actually resolve any citation at compile time even once it was readable
/// (see `compiler::ZerkaloWorld`'s `extra_root`), since Typst had nothing to
/// parse there. `library.yml` is the vault's own committed, Typst-consumable
/// export (`fond_bib::Library::regenerate_library_yml`), kept in sync with
/// `entries/` automatically by Kartoteka.
pub fn bib_target_path(bib_path: &Path) -> std::path::PathBuf {
    if is_vault_dir(bib_path) {
        bib_path.join("library.yml")
    } else {
        bib_path.to_path_buf()
    }
}

fn bib_entry_from_hayagriva(entry: &hayagriva::Entry) -> BibEntry {
    let author = entry
        .authors()
        .map(|people| {
            people
                .iter()
                .map(format_hayagriva_person)
                .collect::<Vec<_>>()
                .join(" and ")
        })
        .unwrap_or_default();

    let title = entry
        .title()
        .map(|t| t.value.to_string())
        .unwrap_or_default();
    let year = entry.date().map(|d| d.year.to_string()).unwrap_or_default();

    let families = entry
        .authors()
        .map(|people| {
            people
                .iter()
                .map(|p| clean_name(&family_with_prefix(p)))
                .collect()
        })
        .unwrap_or_default();
    let editors = entry
        .editors()
        .map(|people| {
            people
                .iter()
                .map(|p| clean_name(&family_with_prefix(p)))
                .collect()
        })
        .unwrap_or_default();
    let venue = entry
        .parents()
        .first()
        .and_then(|parent| parent.title())
        .map(|t| t.value.to_string())
        .or_else(|| {
            entry
                .publisher()
                .and_then(|p| p.name().map(|n| n.value.to_string()))
        })
        .unwrap_or_default();

    BibEntry {
        key: entry.key().to_string(),
        entry_type: format!("{:?}", entry.entry_type()).to_lowercase(),
        author,
        title,
        year,
        venue,
        families,
        editors,
    }
}

#[cfg(test)]
pub fn parse_yaml_bib(content: &str) -> Vec<BibEntry> {
    let library = match hayagriva::io::from_yaml_str(content) {
        Ok(lib) => lib,
        Err(_) => return Vec::new(),
    };

    library.iter().map(bib_entry_from_hayagriva).collect()
}

fn format_hayagriva_person(p: &hayagriva::types::Person) -> String {
    match &p.given_name {
        Some(given) if !given.is_empty() => format!("{given} {}", p.name),
        _ => p.name.clone(),
    }
}

/// Returns "Last, 2019", "Last & Other, 2019" or "Last et al., 2019" — used as the primary label in the
/// citation popup so authors can search by name rather than by key.
fn family_with_prefix(p: &hayagriva::types::Person) -> String {
    match p.prefix.as_deref() {
        Some(prefix) if !prefix.is_empty() => format!("{prefix} {}", p.name),
        _ if p.given_name.as_deref().is_none_or(str::is_empty) => family_of_unsplit(&p.name),
        _ => p.name.clone(),
    }
}

/// A name written in one piece ("Alice Brown", "Simone de Beauvoir",
/// "World Council of Churches"): its surname, or the whole of it for an
/// institution. Words like "of" and "for" mark an institution or a place-name;
/// particles like "de" and "van" belong to the surname.
fn family_of_unsplit(name: &str) -> String {
    const INSTITUTION: [&str; 7] = ["of", "for", "the", "and", "&", "on", "in"];
    const PARTICLE: [&str; 12] = [
        "de", "van", "von", "der", "den", "la", "le", "du", "da", "di", "ibn", "al",
    ];
    let words: Vec<&str> = name.split_whitespace().collect();
    if words.len() < 2 || words.iter().any(|w| INSTITUTION.contains(w)) {
        return name.to_string();
    }
    if let Some(i) = words
        .iter()
        .skip(1)
        .position(|w| PARTICLE.contains(w))
        .map(|i| i + 1)
    {
        return words[i..].join(" ");
    }
    if words.len() <= 4 {
        words[words.len() - 1].to_string()
    } else {
        name.to_string()
    }
}

fn biblatex_family(p: &biblatex::Person) -> String {
    if p.prefix.is_empty() {
        p.name.clone()
    } else {
        format!("{} {}", p.prefix, p.name)
    }
}

/// Drops BibTeX grouping braces and stray whitespace from a name.
fn clean_name(name: &str) -> String {
    name.replace(['{', '}'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// How a work's authors read in a citation: "Smith", "Smith & Doe",
/// "Smith et al.", "Cone (ed.)", an institution by its whole name.
fn author_label(entry: &BibEntry) -> String {
    let (names, suffix) = if !entry.families.is_empty() {
        (&entry.families, "")
    } else {
        (&entry.editors, " (ed.)")
    };
    match names.as_slice() {
        [] => first_last_name(&entry.author),
        [one] => format!("{one}{suffix}"),
        [a, b] => format!("{a} & {b}{suffix}"),
        [a, ..] => format!("{a} et al.{suffix}"),
    }
}

pub fn format_author_year(entry: &BibEntry) -> String {
    let last = author_label(entry);
    match (last.is_empty(), entry.year.is_empty()) {
        (false, false) => format!("{last}, {}", entry.year),
        (false, true) => last,
        (true, false) => entry.year.clone(),
        (true, true) => entry.key.clone(),
    }
}

fn first_last_name(author: &str) -> String {
    if author.is_empty() {
        return String::new();
    }
    let first_author = author.split(" and ").next().unwrap_or(author).trim();
    let last_name = if first_author.contains(',') {
        first_author
            .split(',')
            .next()
            .unwrap_or(first_author)
            .trim()
            .to_string()
    } else {
        first_author
            .split_whitespace()
            .last()
            .unwrap_or(first_author)
            .to_string()
    };
    if author.contains(" and ") {
        format!("{last_name} et al.")
    } else {
        last_name
    }
}

#[cfg(test)]
pub fn parse_bib(content: &str) -> Vec<BibEntry> {
    match Bibliography::parse(content) {
        Ok(b) => entries_from_bibliography(&b),
        Err(_) => Vec::new(),
    }
}

fn entries_from_bibliography(bib: &Bibliography) -> Vec<BibEntry> {
    bib.iter()
        .map(|entry| {
            let author = entry
                .author()
                .map(|people| {
                    people
                        .iter()
                        .map(format_biblatex_person)
                        .collect::<Vec<_>>()
                        .join(" and ")
                })
                .unwrap_or_default();

            let title = entry
                .title()
                .map(|c| c.format_verbatim())
                .unwrap_or_default();

            let year = match entry.date() {
                Ok(PermissiveType::Typed(date)) => {
                    let dt = match date.value {
                        DateValue::At(dt)
                        | DateValue::After(dt)
                        | DateValue::Before(dt)
                        | DateValue::Between(dt, _) => dt,
                    };
                    dt.year.to_string()
                }
                Ok(PermissiveType::Chunks(c)) => c.format_verbatim(),
                Err(_) => String::new(),
            };

            let families = entry
                .author()
                .map(|people| {
                    people
                        .iter()
                        .map(|p| clean_name(&biblatex_family(p)))
                        .collect()
                })
                .unwrap_or_default();
            let editors = entry
                .editors()
                .map(|groups| {
                    groups
                        .iter()
                        .flat_map(|(people, _)| people.iter())
                        .map(|p| clean_name(&biblatex_family(p)))
                        .collect()
                })
                .unwrap_or_default();
            let venue = ["journaltitle", "journal", "booktitle", "publisher"]
                .iter()
                .find_map(|f| {
                    entry
                        .get(f)
                        .map(|c| c.format_verbatim())
                        .filter(|v| !v.trim().is_empty())
                })
                .unwrap_or_default();

            BibEntry {
                key: entry.key.clone(),
                entry_type: entry.entry_type.to_string(),
                author,
                title,
                year,
                venue,
                families,
                editors,
            }
        })
        .collect()
}

/// Writes `text` to `path` so a crash or full disk can't leave half a file:
/// the new text goes to a sibling temp file which then replaces the original.
/// A symlinked file (a Zotero export linked into the project) is written
/// through to its target rather than replaced by a plain file.
pub fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut tmp = target.clone().into_os_string();
    tmp.push(".tmp-zerkalo");
    let tmp = std::path::PathBuf::from(tmp);
    std::fs::write(&tmp, text)?;
    if let Ok(meta) = std::fs::metadata(&target) {
        let _ = std::fs::set_permissions(&tmp, meta.permissions());
    }
    std::fs::rename(&tmp, &target).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Matches an entry header `@type{key,` — group 3 is the key. Works on a
/// file the full BibTeX parser would reject.
fn entry_header_regex(key: &str) -> regex::Regex {
    regex::Regex::new(&format!(
        r"(?mi)^([ \t]*@([a-z]+)[ \t]*[{{(][ \t\r\n]*)({})([ \t]*,)",
        regex::escape(key)
    ))
    .unwrap()
}

/// The captures of every real entry (not `@comment`, `@string`, `@preamble`)
/// whose key is `key`.
fn entry_headers<'a>(content: &'a str, key: &str) -> Vec<regex::Captures<'a>> {
    entry_header_regex(key)
        .captures_iter(content)
        .filter(|c| {
            !matches!(
                c[2].to_ascii_lowercase().as_str(),
                "comment" | "string" | "preamble"
            )
        })
        .collect()
}

/// Renames a citation key in BibTeX text by replacing just the key in the
/// entry's header line. Everything else — comments, `@string` macros, the
/// author's own layout and ordering — stays exactly as it was. `None` if
/// `old_key` isn't an entry here exactly once.
pub fn rename_key_in_bib_text(content: &str, old_key: &str, new_key: &str) -> Option<String> {
    let found = entry_headers(content, old_key);
    if found.len() != 1 {
        return None;
    }
    let key = found[0].get(3)?;
    let mut out = String::with_capacity(content.len() + new_key.len());
    out.push_str(&content[..key.start()]);
    out.push_str(new_key);
    out.push_str(&content[key.end()..]);
    Some(out)
}

/// Whether BibTeX `text` already has an entry called `key` (case-sensitive,
/// like citation keys are).
pub fn bib_text_has_key(content: &str, key: &str) -> bool {
    !entry_headers(content, key).is_empty()
}

/// Renames a citation key in a BibTeX file, touching only the key itself.
/// Returns an error if the file can't be read or written, if `old_key`
/// isn't present exactly once, or if `new_key` already names an entry
/// (renaming would silently merge two works otherwise).
pub fn rename_key_in_bib_file(path: &Path, old_key: &str, new_key: &str) -> std::io::Result<()> {
    let content = std::fs::read_to_string(path)?;
    if new_key != old_key && bib_text_has_key(&content, new_key) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("key '{new_key}' already exists"),
        ));
    }
    let updated = rename_key_in_bib_text(&content, old_key, new_key).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("key '{old_key}' not found"),
        )
    })?;
    write_atomic(path, &updated)
}

/// One file's worth of a project-wide rename: what it said, what it will say.
pub struct RenameEdit {
    pub path: std::path::PathBuf,
    pub before: String,
    pub after: String,
}

/// Writes every edit, then the bibliography, and if any write fails puts back
/// everything already written, so a rename lands in all the files or none.
pub fn apply_rename(
    bib_path: &Path,
    old_key: &str,
    new_key: &str,
    docs: &[RenameEdit],
) -> std::io::Result<()> {
    let mut done: Vec<&RenameEdit> = Vec::new();
    let undo = |done: &[&RenameEdit]| {
        for e in done {
            let _ = write_atomic(&e.path, &e.before);
        }
    };
    for e in docs {
        if let Err(err) = write_atomic(&e.path, &e.after) {
            undo(&done);
            return Err(err);
        }
        done.push(e);
    }
    if let Err(err) = rename_key_in_bib_file(bib_path, old_key, new_key) {
        undo(&done);
        return Err(err);
    }
    Ok(())
}

/// Renames occurrences of a citation key (`@key` shorthand or `#cite(<key>…)` /
/// `#cite("key"…)`, with or without further arguments) in Typst source text.
/// Only the key's own characters change; every other byte is left as it was.
/// Returns the rewritten text and whether anything changed.
pub fn rename_key_in_text(text: &str, old_key: &str, new_key: &str) -> (String, bool) {
    crate::citation_keys::rename_in_text(text, old_key, new_key)
}

fn format_biblatex_person(p: &biblatex::Person) -> String {
    [&p.given_name, &p.prefix, &p.name, &p.suffix]
        .into_iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_BIB: &str = r#"
@article{smith2020,
  author = {John Smith},
  title = {A Great Paper},
  year = {2020},
  journal = {Journal of Things},
}

@book{doe2019,
  author = {Jane Doe},
  title = {Important Book},
  year = {2019},
  publisher = {Academic Press},
}

@misc{anon,
  title = {Anonymous Entry},
}
"#;

    #[test]
    fn parse_bib_entry_count() {
        let entries = parse_bib(SAMPLE_BIB);
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn parse_bib_article_fields() {
        let entries = parse_bib(SAMPLE_BIB);
        let art = entries.iter().find(|e| e.key == "smith2020").unwrap();
        assert_eq!(art.entry_type, "article");
        assert_eq!(art.author, "John Smith");
        assert_eq!(art.title, "A Great Paper");
        assert_eq!(art.year, "2020");
    }

    #[test]
    fn parse_bib_missing_fields_use_empty_strings() {
        let entries = parse_bib(SAMPLE_BIB);
        let anon = entries.iter().find(|e| e.key == "anon").unwrap();
        assert!(anon.author.is_empty());
        assert_eq!(anon.title, "Anonymous Entry");
        assert!(anon.year.is_empty());
    }

    #[test]
    fn parse_bib_ignores_string_preamble_comment() {
        let bib = r#"
@string{jot = "Journal of Things"}
@preamble{"Some preamble"}
@comment{this is a comment}
@article{real,
  author = {Real Author},
  title = {Real Title},
  year = {2024},
}
"#;
        let entries = parse_bib(bib);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "real");
    }

    #[test]
    fn parse_bib_nested_braces_in_title() {
        let bib = r#"
@book{patristic,
  author = {Ignatius of {Antioch}},
  title = {On the {Epistle} to the {Romans}},
  year = {2021},
}
"#;
        let entries = parse_bib(bib);
        assert_eq!(entries[0].title, "On the Epistle to the Romans");
        assert_eq!(entries[0].author, "Ignatius of Antioch");
    }

    #[test]
    fn parse_bib_double_braced_title() {
        let bib = r#"
@article{caps,
  title = {{A Title With Protected Caps}},
  year = {2020},
}
"#;
        let entries = parse_bib(bib);
        assert_eq!(entries[0].title, "A Title With Protected Caps");
    }

    #[test]
    fn parse_bib_bare_year() {
        let bib = r#"
@article{bare,
  author = {Someone},
  title = {A Title},
  year = 2022,
}
"#;
        let entries = parse_bib(bib);
        assert_eq!(entries[0].year, "2022");
    }

    #[test]
    fn parse_bib_string_macro_expansion() {
        let bib = r#"
@string{jot = "Journal of Great Things"}
@article{macro_test,
  author = {Someone Else},
  title = {A Macro Title},
  journal = jot,
  year = {2023},
}
"#;
        let entries = parse_bib(bib);
        let e = entries.iter().find(|e| e.key == "macro_test").unwrap();
        assert_eq!(e.title, "A Macro Title");
    }

    #[test]
    fn parse_bib_at_sign_in_email_field_does_not_break_parsing() {
        let bib = r#"
@article{withemail,
  author = {Someone},
  title = {A Title},
  year = {2020},
  note = {contact: someone@example.com},
}
@article{after,
  author = {Another},
  title = {Another Title},
  year = {2021},
}
"#;
        let entries = parse_bib(bib);
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.key == "after"));
    }

    #[test]
    fn load_bib_returns_empty_for_nonexistent_file() {
        let entries = load_bib(std::path::Path::new("/nonexistent/path/refs.bib"));
        assert!(entries.is_empty());
    }

    #[test]
    fn is_vault_dir_requires_entries_subdir() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_vault_dir(dir.path()));
        std::fs::create_dir(dir.path().join("entries")).unwrap();
        assert!(is_vault_dir(dir.path()));
    }

    #[test]
    fn bib_target_path_appends_library_yml_for_a_vault() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("entries")).unwrap();
        assert_eq!(bib_target_path(dir.path()), dir.path().join("library.yml"));
    }

    #[test]
    fn bib_target_path_leaves_a_plain_file_unchanged() {
        let path = Path::new("/home/user/refs.bib");
        assert_eq!(bib_target_path(path), path);
    }

    #[test]
    fn load_bib_reads_a_kartoteka_vault_directly() {
        let dir = tempfile::tempdir().unwrap();
        let entries_dir = dir.path().join("entries");
        std::fs::create_dir(&entries_dir).unwrap();
        std::fs::write(
            entries_dir.join("smith2020.yml"),
            "smith2020:\n  type: article\n  title: A Great Paper\n  author: John Smith\n  date: 2020\n",
        )
        .unwrap();

        let entries = load_bib(dir.path());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "smith2020");
        assert_eq!(entries[0].title, "A Great Paper");
        assert_eq!(entries[0].author, "John Smith");
        assert_eq!(entries[0].year, "2020");
    }

    #[test]
    fn rename_key_in_text_shorthand() {
        let (out, changed) =
            rename_key_in_text("See @smith2020 for details.", "smith2020", "smith2021");
        assert!(changed);
        assert_eq!(out, "See @smith2021 for details.");
    }

    #[test]
    fn rename_key_in_text_cite_label_form() {
        let (out, changed) = rename_key_in_text("#cite(<smith2020>)", "smith2020", "smith2021");
        assert!(changed);
        assert_eq!(out, "#cite(<smith2021>)");
    }

    #[test]
    fn rename_key_in_text_handles_hyphenated_keys() {
        let (out, changed) =
            rename_key_in_text("See @smith-2020 for details.", "smith-2020", "smith-2021");
        assert!(changed);
        assert_eq!(out, "See @smith-2021 for details.");
    }

    #[test]
    fn rename_key_in_text_no_match_is_noop() {
        let (out, changed) = rename_key_in_text("See @doe2019.", "smith2020", "smith2021");
        assert!(!changed);
        assert_eq!(out, "See @doe2019.");
    }

    #[test]
    fn rename_key_in_text_does_not_touch_prefix_matches() {
        let (out, changed) = rename_key_in_text("See @smith2020extra.", "smith2020", "smith2021");
        assert!(!changed);
        assert_eq!(out, "See @smith2020extra.");
    }

    #[test]
    fn rename_key_in_bib_file_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("refs.bib");
        std::fs::write(
            &path,
            "@article{smith2020,\n  author = {John Smith},\n  year = {2020},\n}\n",
        )
        .unwrap();
        rename_key_in_bib_file(&path, "smith2020", "smith2021").unwrap();
        let entries = load_bib(&path);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "smith2021");
    }

    #[test]
    fn rename_key_in_bib_file_rejects_collision_with_existing_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("refs.bib");
        let original = "@article{smith2020,\n  author = {John Smith},\n  year = {2020},\n}\n\
                         @article{smith2021,\n  author = {Jane Smith},\n  year = {2021},\n}\n";
        std::fs::write(&path, original).unwrap();

        let err = rename_key_in_bib_file(&path, "smith2020", "smith2021").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);

        // The file must be untouched — both entries still present under their original keys.
        let entries = load_bib(&path);
        let mut keys: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
        keys.sort();
        assert_eq!(keys, vec!["smith2020", "smith2021"]);
    }

    #[test]
    fn rename_in_bib_leaves_every_other_byte_alone() {
        let original = "% my notes\n@string{jt = \"Journal of Things\"}\n\n@Article{ smith2020 ,\n  author = {John Smith},\n  journal = jt,\n}\n@comment{smith2020, not an entry}\n@book{doe2019,\n    title   = {Keep   my   spacing},\n}\n";
        let out = rename_key_in_bib_text(original, "doe2019", "doe2020").unwrap();
        assert_eq!(out, original.replace("doe2019", "doe2020"));
        let out = rename_key_in_bib_text(original, "smith2020", "smith2021").unwrap();
        assert_eq!(out, original.replacen("smith2020 ,", "smith2021 ,", 1));
    }

    #[test]
    fn rename_in_bib_refuses_when_unsure() {
        assert!(rename_key_in_bib_text("@book{a,\n}\n@book{a,\n}\n", "a", "b").is_none());
        assert!(rename_key_in_bib_text("@book{a,\n}\n", "z", "b").is_none());
        assert!(rename_key_in_bib_text("@comment{a,\n}\n", "a", "b").is_none());
    }

    #[test]
    fn rename_works_on_a_file_the_parser_rejects() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("refs.bib");
        let original =
            "@book{a,\n  title = {Unbalanced { brace},\n}\n@book{b,\n  year = {Winter 2001},\n}\n";
        std::fs::write(&path, original).unwrap();
        rename_key_in_bib_file(&path, "b", "c").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            original.replace("{b,", "{c,")
        );
    }

    #[test]
    fn a_failed_rename_puts_every_file_back() {
        let dir = tempfile::tempdir().unwrap();
        let bib = dir.path().join("refs.bib");
        std::fs::write(&bib, "@book{a,\n}\n").unwrap();
        let doc = dir.path().join("p.typ");
        std::fs::write(&doc, "@a").unwrap();
        let edits = [RenameEdit {
            path: doc.clone(),
            before: "@a".into(),
            after: "@zz".into(),
        }];
        // `zz` is not in the bib's way but `a`→`a` collision-free; force a bib failure.
        let err = apply_rename(&bib, "missing", "zz", &edits).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        assert_eq!(std::fs::read_to_string(&doc).unwrap(), "@a");
        assert_eq!(std::fs::read_to_string(&bib).unwrap(), "@book{a,\n}\n");
    }

    #[test]
    fn write_atomic_follows_a_symlink_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.bib");
        std::fs::write(&real, "old").unwrap();
        let link = dir.path().join("link.bib");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&real, &link).unwrap();
            write_atomic(&link, "new").unwrap();
            assert_eq!(std::fs::read_to_string(&real).unwrap(), "new");
            assert!(std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink());
            assert!(!dir.path().join("real.bib.tmp-zerkalo").exists());
        }
    }

    #[test]
    fn parse_yaml_bib_basic() {
        let yaml = r#"
smith2020:
  type: article
  title: A Great Paper
  author: John Smith
  date: 2020
"#;
        let entries = parse_yaml_bib(yaml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "smith2020");
        assert_eq!(entries[0].title, "A Great Paper");
        assert_eq!(entries[0].author, "John Smith");
        assert_eq!(entries[0].year, "2020");
    }

    #[test]
    fn parse_yaml_bib_multiple_authors() {
        let yaml = r#"
multi:
  type: article
  title: Collaborative Work
  author:
    - Alice Brown
    - Bob Green
  date: 2021
"#;
        let entries = parse_yaml_bib(yaml);
        assert_eq!(entries[0].author, "Alice Brown and Bob Green");
        assert_eq!(format_author_year(&entries[0]), "Brown & Green, 2021");
    }

    #[test]
    fn unsplit_names_find_their_surname() {
        assert_eq!(family_of_unsplit("Alice Brown"), "Brown");
        assert_eq!(family_of_unsplit("Simone de Beauvoir"), "de Beauvoir");
        assert_eq!(
            family_of_unsplit("World Council of Churches"),
            "World Council of Churches"
        );
        assert_eq!(
            family_of_unsplit("Ignatius of Antioch"),
            "Ignatius of Antioch"
        );
        assert_eq!(family_of_unsplit("Plato"), "Plato");
    }

    #[test]
    fn parse_yaml_bib_invalid_returns_empty() {
        let entries = parse_yaml_bib("not: valid: yaml: at all: [");
        assert!(entries.is_empty());
    }

    #[test]
    fn format_author_year_single_author_first_last() {
        let e = BibEntry {
            key: "smith2020".into(),
            entry_type: "article".into(),
            author: "John Smith".into(),
            title: "A Paper".into(),
            year: "2020".into(),
            ..Default::default()
        };
        assert_eq!(format_author_year(&e), "Smith, 2020");
    }

    #[test]
    fn format_author_year_last_first_format() {
        let e = BibEntry {
            key: "doe2019".into(),
            entry_type: "book".into(),
            author: "Doe, Jane".into(),
            title: "A Book".into(),
            year: "2019".into(),
            ..Default::default()
        };
        assert_eq!(format_author_year(&e), "Doe, 2019");
    }

    #[test]
    fn format_author_year_multiple_authors() {
        let e = BibEntry {
            key: "multi".into(),
            entry_type: "article".into(),
            author: "Alice Brown and Bob Green and Carol White".into(),
            title: "Collaborative Work".into(),
            year: "2021".into(),
            ..Default::default()
        };
        assert_eq!(format_author_year(&e), "Brown et al., 2021");
    }

    #[test]
    fn format_author_year_no_year_falls_back_to_name() {
        let e = BibEntry {
            key: "anon".into(),
            entry_type: "misc".into(),
            author: "Ivan Petrov".into(),
            title: String::new(),
            year: String::new(),
            ..Default::default()
        };
        assert_eq!(format_author_year(&e), "Petrov");
    }

    #[test]
    fn format_author_year_no_author_no_year_falls_back_to_key() {
        let e = BibEntry {
            key: "nodata".into(),
            entry_type: "misc".into(),
            author: String::new(),
            title: String::new(),
            year: String::new(),
            ..Default::default()
        };
        assert_eq!(format_author_year(&e), "nodata");
    }

    // Representative of Zotero's default BibTeX export: non-standard extra
    // fields (urldate, file, keywords), a `date` field instead of `year`,
    // an attachment path with colons/spaces, and LaTeX escapes in the
    // abstract — real quirks Zotero's export is known to include, any one
    // of which could silently zero out the whole file if the parser
    // choked on it (parse_bib returns an empty Vec on any parse error,
    // with nothing surfaced to the user).
    const ZOTERO_EXPORT_SAMPLE: &str = r#"
@article{smith_study_2020,
	title = {A study of everything, {With} a colon: subtitle},
	volume = {12},
	issn = {1234-5678},
	url = {https://example.com/article},
	abstract = {An abstract with {\'e} and \& an ampersand.},
	number = {3},
	journal = {Journal of Examples},
	author = {Smith, John and Doe, Jane},
	urldate = {2023-01-15},
	date = {2020},
	note = {shortDOI: 10.1234/abcd},
	keywords = {example, test, multi-word tag},
	file = {Snapshot:/home/user/Zotero/storage/ABCD1234/smith - 2020 - A study.pdf:application/pdf},
}

@book{doe_important_2019,
	title = {Important {Book} on {Zotero} Export Quirks},
	publisher = {Academic Press},
	author = {Doe, Jane},
	date = {2019},
	keywords = {book},
}
"#;

    #[test]
    fn parse_bib_handles_a_realistic_zotero_export() {
        let entries = parse_bib(ZOTERO_EXPORT_SAMPLE);
        assert_eq!(
            entries.len(),
            2,
            "a real Zotero export must not silently parse to zero entries"
        );
        assert_eq!(entries[0].key, "smith_study_2020");
        assert_eq!(entries[0].author, "John Smith and Jane Doe");
        assert_eq!(entries[0].year, "2020");
        assert_eq!(entries[1].key, "doe_important_2019");
        assert_eq!(entries[1].year, "2019");
    }

    #[test]
    fn the_repos_own_zotero_style_library_loads_and_renames_byte_for_byte() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("citations.bib");
        let Ok(original) = std::fs::read_to_string(&path) else {
            return;
        };
        let entries = try_load(&path).expect("the repo's own bibliography reads");
        assert!(entries.len() > 100);
        let key = &entries[0].key;
        let renamed = rename_key_in_bib_text(&original, key, "zz-renamed-key").unwrap();
        assert_eq!(renamed.replace("zz-renamed-key", key), original);
    }

    #[test]
    fn labels_are_built_from_real_names() {
        let bib = parse_bib(
            "@book{wcc, author = {{World Council of Churches}}, title = {Baptism}, year = {1982}}\n\
             @book{two, author = {Smith, John and Doe, Jane}, title = {T}, year = {2001}}\n\
             @book{three, author = {A, Ann and B, Bo and C, Cy}, title = {T}, year = {2002}}\n\
             @book{ed, editor = {Cone, James}, title = {Reader}, year = {1979}, publisher = {Orbis}}\n\
             @book{part, author = {de Beauvoir, Simone}, title = {T}, year = {1949}}",
        );
        let label = |k: &str| format_author_year(bib.iter().find(|e| e.key == k).unwrap());
        assert_eq!(label("wcc"), "World Council of Churches, 1982");
        assert_eq!(label("two"), "Smith & Doe, 2001");
        assert_eq!(label("three"), "A et al., 2002");
        assert_eq!(label("ed"), "Cone (ed.), 1979");
        assert_eq!(label("part"), "de Beauvoir, 1949");
        assert_eq!(bib.iter().find(|e| e.key == "ed").unwrap().venue, "Orbis");
    }
}
