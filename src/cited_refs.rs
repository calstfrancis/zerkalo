//! "Export cited references" — writes a `.bib` or Hayagriva `.yaml` file
//! holding only the bibliography entries a document actually cites, so a
//! paper can be handed to a co-author, journal, or another tool without
//! dragging a whole personal library along with it.
//!
//! The source is whatever the document compiles against: its own
//! `#bibliography("...")` file when it names one, otherwise the configured
//! `Config::bib_path` (a `.bib`, a `.yaml`, or a Kartoteka vault).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::bibliography::{bib_target_path, is_vault_dir};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefFormat {
    Bib,
    Yaml,
}

impl RefFormat {
    pub fn extension(self) -> &'static str {
        match self {
            RefFormat::Bib => "bib",
            RefFormat::Yaml => "yaml",
        }
    }
}

/// Every citation key written in `root` and any `.typ` file it pulls in via
/// `#include`/`#import` (followed transitively, relative to each file).
/// Over-collects on purpose — `@preview`, e-mail addresses, and labels also
/// match `@…` — since the result is only ever intersected with real
/// bibliography keys.
pub fn collect_cited_keys(root: &Path) -> BTreeSet<String> {
    static CITE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static INCLUDE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let cite = CITE.get_or_init(|| {
        regex::Regex::new(&format!(
            r#"@({k})|#cite\(\s*<({k})>|#cite\(\s*"({k})""#,
            k = "[A-Za-z][A-Za-z0-9_:.-]*"
        ))
        .unwrap()
    });
    let include = INCLUDE
        .get_or_init(|| regex::Regex::new(r#"#(?:include|import)\s+"([^"]+\.typ)""#).unwrap());

    let mut keys = BTreeSet::new();
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    let mut queue = vec![root.to_path_buf()];
    while let Some(path) = queue.pop() {
        let canon = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if !seen.insert(canon) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for cap in cite.captures_iter(&content) {
            if let Some(m) = cap.get(1).or(cap.get(2)).or(cap.get(3)) {
                // A trailing `.`/`:` is sentence punctuation, not part of the key.
                keys.insert(m.as_str().trim_end_matches(['.', ':']).to_string());
            }
        }
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        for cap in include.captures_iter(&content) {
            queue.push(dir.join(&cap[1]));
        }
    }
    keys
}

/// The bibliography file `root` compiles against: its own
/// `#bibliography(...)` path if that resolves to an existing file, else
/// `configured` (resolved to `library.yml` for a Kartoteka vault).
pub fn resolve_source(root: &Path, configured: Option<&Path>) -> Option<PathBuf> {
    let from_doc = std::fs::read_to_string(root).ok().and_then(|c| {
        crate::styles::find_bibliography_path(&c).map(|p| {
            let p = Path::new(p);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                root.parent().unwrap_or(Path::new(".")).join(p)
            }
        })
    });
    from_doc
        .filter(|p| p.is_file())
        .or_else(|| {
            configured.map(|c| {
                if is_vault_dir(c) {
                    bib_target_path(c)
                } else {
                    c.to_path_buf()
                }
            })
        })
        .filter(|p| p.is_file())
}

/// Result of filtering a bibliography down to cited entries.
pub struct CitedExport {
    pub text: String,
    pub count: usize,
}

/// Filters `source` down to the entries whose keys appear in `keys`, and
/// renders them as `format`. A `.bib` → `.bib` export keeps each entry's
/// fields exactly as written; every other combination goes through
/// Hayagriva.
pub fn export_cited(
    source: &Path,
    keys: &BTreeSet<String>,
    format: RefFormat,
) -> Result<CitedExport, String> {
    let raw = std::fs::read_to_string(source)
        .map_err(|e| format!("Couldn't read {}: {e}", source.display()))?;
    let is_yaml = source
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("yaml") || e.eq_ignore_ascii_case("yml"));

    if !is_yaml && format == RefFormat::Bib {
        return filter_bib_text(&raw, keys);
    }

    let library = if is_yaml {
        hayagriva::io::from_yaml_str(&raw).map_err(|e| format!("Couldn't parse YAML: {e}"))?
    } else {
        let text = crate::bib_sanitize::sanitize_bib(&raw).unwrap_or(raw);
        hayagriva::io::from_biblatex_str(&text)
            .map_err(|_| "Couldn't parse the .bib file".to_string())?
    };
    let mut filtered = hayagriva::Library::new();
    for entry in library.iter() {
        if keys.contains(entry.key()) {
            filtered.push(entry);
        }
    }
    let count = filtered.len();
    let text = match format {
        RefFormat::Yaml => hayagriva::io::to_yaml_str(&filtered)
            .map_err(|e| format!("Couldn't write YAML: {e}"))?,
        RefFormat::Bib => filtered
            .iter()
            .map(hayagriva_to_bibtex)
            .collect::<Vec<_>>()
            .join("\n"),
    };
    Ok(CitedExport { text, count })
}

fn filter_bib_text(raw: &str, keys: &BTreeSet<String>) -> Result<CitedExport, String> {
    let parsed = biblatex::Bibliography::parse(raw).or_else(|_| {
        crate::bib_sanitize::sanitize_bib(raw)
            .ok_or(())
            .and_then(|fixed| biblatex::Bibliography::parse(&fixed).map_err(|_| ()))
    });
    let bib = parsed.map_err(|_| "Couldn't parse the .bib file".to_string())?;
    let entries: Vec<String> = bib
        .iter()
        .filter(|e| keys.contains(&e.key))
        .map(|e| e.to_biblatex_string())
        .collect();
    Ok(CitedExport {
        count: entries.len(),
        text: entries.join("\n\n") + "\n",
    })
}

/// A small, lossy Hayagriva → BibTeX writer covering the fields that matter
/// for a reference list (Hayagriva has no BibTeX writer of its own). Used
/// only when the source is YAML or a Kartoteka vault and `.bib` is asked for.
fn hayagriva_to_bibtex(entry: &hayagriva::Entry) -> String {
    use hayagriva::types::EntryType as T;

    let parent = entry.parents().first();
    let parent_title = parent.and_then(|p| p.title()).map(|t| t.value.to_string());
    let kind = match entry.entry_type() {
        T::Article => "article",
        T::Book => "book",
        T::Chapter | T::Anthos => "incollection",
        T::Thesis => "thesis",
        T::Report => "report",
        T::Web => "online",
        T::Proceedings => "proceedings",
        T::Anthology => "collection",
        _ if parent.is_some_and(|p| matches!(p.entry_type(), T::Proceedings | T::Conference)) => {
            "inproceedings"
        }
        _ => "misc",
    };

    let people = |ps: &[hayagriva::types::Person]| {
        ps.iter()
            .map(|p| match &p.given_name {
                Some(g) if !g.is_empty() => format!("{}, {g}", p.name),
                _ => p.name.clone(),
            })
            .collect::<Vec<_>>()
            .join(" and ")
    };

    let mut fields: Vec<(&str, String)> = Vec::new();
    if let Some(a) = entry.authors() {
        fields.push(("author", people(a)));
    }
    if let Some(e) = entry.editors().or_else(|| parent.and_then(|p| p.editors())) {
        fields.push(("editor", people(e)));
    }
    if let Some(t) = entry.title() {
        fields.push(("title", t.value.to_string()));
    }
    if let Some(pt) = parent_title {
        let field = match kind {
            "article" => "journaltitle",
            _ => "booktitle",
        };
        fields.push((field, pt));
    }
    if let Some(d) = entry.date().or_else(|| parent.and_then(|p| p.date())) {
        let mut date = format!("{:04}", d.year);
        if let Some(m) = d.month {
            date.push_str(&format!("-{:02}", m + 1));
            if let Some(day) = d.day {
                date.push_str(&format!("-{:02}", day + 1));
            }
        }
        fields.push(("date", date));
    }
    let publisher = entry
        .publisher()
        .or_else(|| parent.and_then(|p| p.publisher()));
    if let Some(p) = publisher {
        if let Some(n) = p.name() {
            fields.push(("publisher", n.value.to_string()));
        }
        if let Some(l) = p.location() {
            fields.push(("location", l.value.to_string()));
        }
    }
    if let Some(v) = entry.volume().or_else(|| parent.and_then(|p| p.volume())) {
        fields.push(("volume", v.to_string()));
    }
    if let Some(i) = entry.issue().or_else(|| parent.and_then(|p| p.issue())) {
        fields.push(("number", i.to_string()));
    }
    if let Some(e) = entry.edition() {
        fields.push(("edition", e.to_string()));
    }
    if let Some(pr) = entry.page_range() {
        fields.push(("pages", pr.to_string()));
    }
    if let Some(doi) = entry.doi() {
        fields.push(("doi", doi.to_string()));
    }
    if let Some(isbn) = entry.isbn() {
        fields.push(("isbn", isbn.to_string()));
    }
    if let Some(u) = entry.url() {
        fields.push(("url", u.value.to_string()));
    }
    if let Some(n) = entry.note() {
        fields.push(("note", n.value.to_string()));
    }

    let mut out = format!("@{kind}{{{},\n", entry.key());
    for (name, value) in fields {
        out.push_str(&format!("  {name} = {{{value}}},\n"));
    }
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("zerkalo_cited_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const BIB: &str = "@book{smith2020,\n  author = {Smith, Jane},\n  title = {A Book},\n  year = {2020},\n}\n\n@article{doe-2019,\n  author = {Doe, John},\n  title = {An Article},\n  journal = {Journal},\n  year = {2019},\n}\n\n@misc{unused,\n  title = {Never Cited},\n}\n";

    #[test]
    fn collects_keys_from_includes_and_strips_trailing_punctuation() {
        let d = tmp_dir("keys");
        std::fs::write(
            d.join("main.typ"),
            "As @smith2020 says. #include \"ch1.typ\"\n",
        )
        .unwrap();
        std::fs::write(d.join("ch1.typ"), "See #cite(<doe-2019>).\n").unwrap();
        let keys = collect_cited_keys(&d.join("main.typ"));
        assert!(keys.contains("smith2020"));
        assert!(keys.contains("doe-2019"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn bib_to_bib_keeps_only_cited_entries() {
        let d = tmp_dir("bib");
        std::fs::write(d.join("refs.bib"), BIB).unwrap();
        let keys: BTreeSet<String> = ["smith2020", "doe-2019", "preview"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let out = export_cited(&d.join("refs.bib"), &keys, RefFormat::Bib).unwrap();
        assert_eq!(out.count, 2);
        assert!(out.text.contains("smith2020"));
        assert!(!out.text.contains("unused"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn bib_to_yaml_and_back_to_bib_round_trips_keys() {
        let d = tmp_dir("yaml");
        std::fs::write(d.join("refs.bib"), BIB).unwrap();
        let keys: BTreeSet<String> = ["smith2020"].iter().map(|s| s.to_string()).collect();
        let yaml = export_cited(&d.join("refs.bib"), &keys, RefFormat::Yaml).unwrap();
        assert_eq!(yaml.count, 1);
        std::fs::write(d.join("refs.yaml"), &yaml.text).unwrap();
        let bib = export_cited(&d.join("refs.yaml"), &keys, RefFormat::Bib).unwrap();
        assert_eq!(bib.count, 1);
        assert!(bib.text.starts_with("@book{smith2020,"));
        assert!(bib.text.contains("author = {Smith, Jane}"));
        assert!(biblatex::Bibliography::parse(&bib.text).is_ok());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn document_bibliography_call_wins_over_configured_path() {
        let d = tmp_dir("resolve");
        std::fs::write(d.join("refs.bib"), BIB).unwrap();
        std::fs::write(d.join("other.bib"), BIB).unwrap();
        std::fs::write(d.join("main.typ"), "#bibliography(\"refs.bib\")\n").unwrap();
        let got = resolve_source(&d.join("main.typ"), Some(&d.join("other.bib"))).unwrap();
        assert_eq!(got, d.join("refs.bib"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
