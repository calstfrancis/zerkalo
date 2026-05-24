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
