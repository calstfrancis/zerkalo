use std::path::Path;

use biblatex::{Bibliography, ChunksExt, DateValue, PermissiveType};

/// Character class for a valid BibTeX citation key, shared by every regex
/// that matches or renames citation keys (here and in `ui/ref_manager.rs`).
/// Includes `-` — a normal BibTeX convention (e.g. `smith-2020`) that's easy
/// to miss since it's not a valid identifier character in most other contexts.
pub const CITE_KEY_CHARS: &str = "[A-Za-z][A-Za-z0-9_:-]*";

#[derive(Clone, Debug, Default)]
pub struct BibEntry {
    pub key: String,
    #[allow(dead_code)]
    pub entry_type: String,
    pub author: String,
    pub title: String,
    pub year: String,
}

pub fn load_bib(path: &Path) -> Vec<BibEntry> {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml") => {
            load_yaml_bib(path)
        }
        _ => match std::fs::read_to_string(path) {
            Ok(content) => parse_bib(&content),
            Err(_) => Vec::new(),
        },
    }
}

fn load_yaml_bib(path: &Path) -> Vec<BibEntry> {
    match std::fs::read_to_string(path) {
        Ok(content) => parse_yaml_bib(&content),
        Err(_) => Vec::new(),
    }
}

pub fn parse_yaml_bib(content: &str) -> Vec<BibEntry> {
    let library = match hayagriva::io::from_yaml_str(content) {
        Ok(lib) => lib,
        Err(_) => return Vec::new(),
    };

    library
        .iter()
        .map(|entry| {
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

            let title = entry.title().map(|t| t.value.to_string()).unwrap_or_default();
            let year = entry.date().map(|d| d.year.to_string()).unwrap_or_default();

            BibEntry {
                key: entry.key().to_string(),
                entry_type: format!("{:?}", entry.entry_type()).to_lowercase(),
                author,
                title,
                year,
            }
        })
        .collect()
}

fn format_hayagriva_person(p: &hayagriva::types::Person) -> String {
    match &p.given_name {
        Some(given) if !given.is_empty() => format!("{given} {}", p.name),
        _ => p.name.clone(),
    }
}

/// Returns "Last 2019" or "Last et al. 2019" — used as the primary label in the
/// citation popup so authors can search by name rather than by key.
pub fn format_author_year(entry: &BibEntry) -> String {
    let last = first_last_name(&entry.author);
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
        first_author.split(',').next().unwrap_or(first_author).trim().to_string()
    } else {
        first_author.split_whitespace().last().unwrap_or(first_author).to_string()
    };
    if author.contains(" and ") {
        format!("{last_name} et al.")
    } else {
        last_name
    }
}

pub fn parse_bib(content: &str) -> Vec<BibEntry> {
    let bib = match Bibliography::parse(content) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };

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

            let title = entry.title().map(|c| c.format_verbatim()).unwrap_or_default();

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

            BibEntry {
                key: entry.key.clone(),
                entry_type: entry.entry_type.to_string(),
                author,
                title,
                year,
            }
        })
        .collect()
}

/// Renames a citation key in place in a BibTeX file. Returns an error if the
/// file can't be read/parsed/written, if `old_key` isn't present, or if
/// `new_key` already names a different entry (renaming would silently
/// clobber it otherwise).
pub fn rename_key_in_bib_file(path: &Path, old_key: &str, new_key: &str) -> std::io::Result<()> {
    let content = std::fs::read_to_string(path)?;
    let mut bib = Bibliography::parse(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    if new_key != old_key && bib.get(new_key).is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("key '{new_key}' already exists"),
        ));
    }
    let mut entry = bib.remove(old_key).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, format!("key '{old_key}' not found"))
    })?;
    entry.key = new_key.to_string();
    bib.insert(entry);
    std::fs::write(path, bib.to_bibtex_string())
}

/// Renames occurrences of a citation key (`@key` shorthand or `#cite(<key>)` /
/// `#cite("key")`) in Typst source text. Returns the rewritten text and
/// whether anything changed.
pub fn rename_key_in_text(text: &str, old_key: &str, new_key: &str) -> (String, bool) {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(&format!(
            r#"@({CITE_KEY_CHARS})|#cite\(<([^>]+)>\)|#cite\("([^"]+)"\)"#
        ))
        .unwrap()
    });

    let result = re.replace_all(text, |caps: &regex::Captures| {
        let whole = caps.get(0).unwrap().as_str();
        if let Some(k) = caps.get(1) {
            if k.as_str() == old_key {
                return format!("@{new_key}");
            }
        } else if let Some(k) = caps.get(2) {
            if k.as_str() == old_key {
                return format!("#cite(<{new_key}>)");
            }
        } else if let Some(k) = caps.get(3) {
            if k.as_str() == old_key {
                return format!("#cite(\"{new_key}\")");
            }
        }
        whole.to_string()
    });
    let changed = result != text;
    (result.into_owned(), changed)
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
    fn rename_key_in_text_shorthand() {
        let (out, changed) = rename_key_in_text("See @smith2020 for details.", "smith2020", "smith2021");
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
        let (out, changed) = rename_key_in_text("See @smith-2020 for details.", "smith-2020", "smith-2021");
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
        std::fs::write(&path, "@article{smith2020,\n  author = {John Smith},\n  year = {2020},\n}\n").unwrap();
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
        assert_eq!(format_author_year(&entries[0]), "Brown et al., 2021");
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
        };
        assert_eq!(format_author_year(&e), "nodata");
    }
}
