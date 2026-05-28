use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

static ENTRY_RE: OnceLock<Regex> = OnceLock::new();
static FIELD_RE: OnceLock<Regex> = OnceLock::new();

fn entry_re() -> &'static Regex {
    ENTRY_RE.get_or_init(|| Regex::new(r"(?i)@(\w+)\s*\{\s*([^,\s\}]+)").unwrap())
}

fn field_re() -> &'static Regex {
    FIELD_RE.get_or_init(|| {
        Regex::new(r#"(?is)(\w+)\s*=\s*(?:\{([^{}]*)\}|"([^"]*)")"#).unwrap()
    })
}

#[derive(Clone, Debug, Default)]
pub struct BibEntry {
    pub key: String,
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

        for fc in field_re().captures_iter(body) {
            let name = fc[1].to_lowercase();
            let val = fc
                .get(2)
                .or_else(|| fc.get(3))
                .map_or("", |m| m.as_str())
                .trim()
                .to_string();
            match name.as_str() {
                "author" => author = clean_braces(&val),
                "title" => title = clean_braces(&val),
                "year" => year = val,
                _ => {}
            }
        }

        entries.push(BibEntry {
            key,
            entry_type,
            author,
            title,
            year,
        });
    }

    entries
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
    fn load_bib_returns_empty_for_nonexistent_file() {
        let entries = load_bib(std::path::Path::new("/nonexistent/path/refs.bib"));
        assert!(entries.is_empty());
    }
}
