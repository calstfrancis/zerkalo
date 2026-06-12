use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

static ENTRY_RE: OnceLock<Regex> = OnceLock::new();

fn entry_re() -> &'static Regex {
    ENTRY_RE.get_or_init(|| Regex::new(r"(?i)@(\w+)\s*\{\s*([^,\s\}]+)").unwrap())
}

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
    match std::fs::read_to_string(path) {
        Ok(content) => parse_bib(&content),
        Err(_) => Vec::new(),
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
    let mut entries = Vec::new();

    for caps in entry_re().captures_iter(content) {
        let entry_type = caps[1].to_lowercase();
        let key = caps[2].trim().to_string();

        if matches!(entry_type.as_str(), "string" | "preamble" | "comment") {
            continue;
        }

        let body_start = caps.get(0).unwrap().end();
        let body = extract_body(content, body_start);

        let mut author = String::new();
        let mut title = String::new();
        let mut year = String::new();

        for (name, val) in parse_fields(body) {
            match name.to_lowercase().as_str() {
                "author" => author = clean_braces(&val),
                "title"  => title  = clean_braces(&val),
                "year"   => year   = val,
                _ => {}
            }
        }

        entries.push(BibEntry { key, entry_type, author, title, year });
    }

    entries
}

/// Parse `field = {value}` or `field = "value"` pairs from an entry body,
/// handling arbitrarily nested braces in the value.
fn parse_fields(body: &str) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    let bytes = body.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Skip whitespace and commas between fields
        while i < len && (bytes[i].is_ascii_whitespace() || bytes[i] == b',') {
            i += 1;
        }
        if i >= len { break; }

        // Read field name (word chars)
        let name_start = i;
        while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'-') {
            i += 1;
        }
        if i == name_start { i += 1; continue; } // skip unexpected char
        let name = body[name_start..i].to_string();

        // Skip whitespace then expect '='
        while i < len && bytes[i].is_ascii_whitespace() { i += 1; }
        if i >= len || bytes[i] != b'=' { continue; }
        i += 1;

        // Skip whitespace
        while i < len && bytes[i].is_ascii_whitespace() { i += 1; }
        if i >= len { break; }

        let value = if bytes[i] == b'{' {
            // Brace-delimited value — track depth so nested braces are included
            i += 1; // skip opening brace
            let val_start = i;
            let mut depth = 1i32;
            while i < len {
                match bytes[i] {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 { break; }
                    }
                    _ => {}
                }
                i += 1;
            }
            let val = body[val_start..i].to_string();
            if i < len { i += 1; } // skip closing brace
            val
        } else if bytes[i] == b'"' {
            // Quote-delimited value — no nesting, but respect escaped quotes
            i += 1;
            let val_start = i;
            while i < len && bytes[i] != b'"' {
                if bytes[i] == b'\\' { i += 1; } // skip escaped char
                i += 1;
            }
            let val = body[val_start..i].to_string();
            if i < len { i += 1; } // skip closing quote
            val
        } else {
            // Bare value (e.g. year = 2020) — read until comma or brace
            let val_start = i;
            while i < len && bytes[i] != b',' && bytes[i] != b'}' {
                i += 1;
            }
            body[val_start..i].trim().to_string()
        };

        if !name.is_empty() {
            fields.push((name, value));
        }
    }

    fields
}

fn extract_body(content: &str, start: usize) -> &str {
    let bytes = content.as_bytes();
    let mut depth = 1i32;
    let mut i = start;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &content[start..i];
                }
            }
            _ => {}
        }
        i += 1;
    }
    &content[start..]
}

fn clean_braces(s: &str) -> String {
    s.replace('{', "").replace('}', "").trim().to_string()
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
    fn load_bib_returns_empty_for_nonexistent_file() {
        let entries = load_bib(std::path::Path::new("/nonexistent/path/refs.bib"));
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
