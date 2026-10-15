//! Whether a document's in-text citations count toward its word total.
//!
//! In an author-date style (APA, Chicago author-date, Harvard, MLA…) a
//! citation like `@smith2020[p. 5]` prints as "(Smith 2020, p. 5)" — real
//! words in the body, which some institutions' limits include and others
//! don't. Zerkalo's counters read the *source*, where `@smith2020` is a single
//! token that was always dropped. This module decides, per document, whether
//! to count what the citation prints instead, and renders exactly that text
//! with hayagriva and the document's own CSL style, so the count matches the
//! compiled output rather than an estimate (a two-author "(Smith & Jones
//! 2020)" is four words, "(Smith et al. 2020)" is four, "(Smith 2020)" is two).
//!
//! The choice lives in the document as a marker comment, like
//! `styles::COMBINE_MARKER`, so it travels with the file and every counter
//! (status bar, goal ring, statistics, outline, project total) agrees.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use hayagriva::archive::ArchivedStyle;
use hayagriva::citationberg::{CitationFormat, IndependentStyle, Style, StyleCategory};
use hayagriva::{
    BibliographyDriver, BibliographyRequest, BufWriteFormat, CitationItem, CitationRequest,
};

const MARKER: &str = "// zerkalo-count-citations: on";

/// Whether the document asks for its in-text citations to be counted.
pub fn counts_citations(content: &str) -> bool {
    content.lines().any(|l| l.trim() == MARKER)
}

/// Adds or removes the marker, at the very top of the document.
pub fn set_counts_citations(content: &str, count: bool) -> String {
    let trailing_nl = content.ends_with('\n');
    let mut lines: Vec<&str> = content.lines().filter(|l| l.trim() != MARKER).collect();
    if count {
        lines.insert(0, MARKER);
    }
    let mut out = lines.join("\n");
    if trailing_nl {
        out.push('\n');
    }
    out
}

/// The `style:` of the document's `#bibliography(...)` call — live, or in the
/// commented-out line a templated document starts with — looking through the
/// files it includes when the active file has none (a chapter rarely does).
fn style_name(content: &str, doc: &Path, project_root: Option<&Path>) -> Option<String> {
    let find = |text: &str| -> Option<String> {
        crate::styles::find_bibliography_style(text)
            .map(str::to_string)
            .or_else(|| {
                let line = text.lines().find_map(|l| {
                    let rest = l.trim_start().strip_prefix("//")?;
                    let rest = rest.trim_start_matches('/').trim_start();
                    rest.starts_with("#bibliography(").then_some(rest)
                })?;
                crate::styles::find_bibliography_style(line).map(str::to_string)
            })
    };
    find(content).or_else(|| {
        let root = project_root?;
        crate::project::manuscript_files(doc, root)
            .iter()
            .find_map(|(_, text)| find(text))
    })
}

fn load_style(name: &str, doc: &Path, project_root: Option<&Path>) -> Option<IndependentStyle> {
    if name.to_ascii_lowercase().ends_with(".csl") {
        let named = Path::new(name);
        let path = if named.is_absolute() {
            named.to_path_buf()
        } else {
            let from_doc = doc.parent().map(|d| d.join(named));
            match from_doc {
                Some(p) if p.is_file() => p,
                _ => project_root?.join(named),
            }
        };
        return IndependentStyle::from_xml(&std::fs::read_to_string(path).ok()?).ok();
    }
    match ArchivedStyle::by_name(name)?.get() {
        Style::Independent(style) => Some(style),
        Style::Dependent(_) => None,
    }
}

/// A style whose in-text citation is parenthetical text — "(Smith 2020)",
/// "(Smith 5)" — as opposed to a number or a footnote, which have no
/// parenthetical to count.
fn is_parenthetical(style: &IndependentStyle) -> bool {
    style.info.category.iter().any(|c| {
        matches!(
            c,
            StyleCategory::CitationFormat {
                format: CitationFormat::AuthorDate | CitationFormat::Author
            }
        )
    })
}

/// Whether the counting toggle means anything for this document: its citation
/// style prints parenthetical text in the body.
pub fn applies_to(content: &str, doc: &Path, project_root: Option<&Path>) -> bool {
    style_name(content, doc, project_root)
        .and_then(|n| load_style(&n, doc, project_root))
        .is_some_and(|s| is_parenthetical(&s))
}

/// What each cited key prints as in the body, plain text.
#[derive(Debug, Default)]
pub struct CitationWords {
    rendered: HashMap<String, String>,
}

impl CitationWords {
    /// The printed form of `key`'s citation with an optional supplement
    /// (`@key[p. 5]` → "(Smith 2020, p. 5)"). `None` for a key that isn't in
    /// the bibliography — that's a cross-reference or a mistake, and either
    /// way prints nothing the citation counter should claim.
    pub fn text(&self, key: &str, supplement: Option<&str>) -> Option<String> {
        let base = self.rendered.get(key)?;
        Some(match supplement.map(str::trim).filter(|s| !s.is_empty()) {
            // The supplement prints inside the closing parenthesis, so the
            // word count is the same wherever it lands; keep it simple.
            Some(s) => format!("{base} {s}"),
            None => base.clone(),
        })
    }
}

fn read_library(path: &Path) -> Option<hayagriva::Library> {
    let text = std::fs::read_to_string(crate::bibliography::bib_target_path(path)).ok()?;
    let is_yaml = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("yaml") || e.eq_ignore_ascii_case("yml"));
    if is_yaml || crate::bibliography::is_vault_dir(path) {
        hayagriva::io::from_yaml_str(&text).ok()
    } else {
        hayagriva::io::from_biblatex_str(&text).ok()
    }
}

/// Renders every key in `keys` once, all in one pass, so the style can
/// disambiguate between sources ("(J. Smith 2020)" vs "(A. Smith 2020)") the
/// way it will in the finished document.
fn render(
    library: &hayagriva::Library,
    style: &IndependentStyle,
    keys: &[String],
) -> HashMap<String, String> {
    let locales = hayagriva::archive::locales();
    let mut driver = BibliographyDriver::new();
    let mut order = Vec::new();
    for key in keys {
        if let Some(entry) = library.get(key) {
            driver.citation(CitationRequest::from_items(
                vec![CitationItem::with_entry(entry)],
                style,
                &locales,
            ));
            order.push(key.clone());
        }
    }
    let rendered = driver.finish(BibliographyRequest {
        style,
        locale: None,
        locale_files: &locales,
    });
    order
        .into_iter()
        .zip(rendered.citations)
        .map(|(key, cite)| {
            let mut text = String::new();
            let _ = cite.citation.write_buf(&mut text, BufWriteFormat::Plain);
            (key, text)
        })
        .collect()
}

type CacheKey = (PathBuf, Option<SystemTime>, String, Vec<String>);

/// The last result: counting runs on every pause in typing, and the
/// bibliography and style almost never change between two of them.
fn cache() -> &'static Mutex<Option<(CacheKey, Arc<CitationWords>)>> {
    static CACHE: OnceLock<Mutex<Option<(CacheKey, Arc<CitationWords>)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// The citations of `content` as they print, when the document has asked for
/// them to count *and* its style makes them parenthetical text. `None` means
/// "count as before" — the toggle is off, or doesn't apply, or the
/// bibliography couldn't be read (never a reason to show a wrong number).
///
/// `configured` is the project's or Settings' bibliography, used when the
/// document names none itself (see `bib_source`).
pub fn for_document(
    content: &str,
    doc: &Path,
    project_root: Option<&Path>,
    configured: Option<&Path>,
) -> Option<Arc<CitationWords>> {
    if !counts_citations(content) {
        return None;
    }
    let name = style_name(content, doc, project_root)?;
    let style = load_style(&name, doc, project_root)?;
    if !is_parenthetical(&style) {
        return None;
    }
    let bib = crate::bib_source::resolve(
        Some((doc, Some(content))),
        configured,
        None,
        project_root.or(doc.parent())?,
    )?
    .path;

    let mut keys: Vec<String> = Vec::new();
    let mut texts = vec![content.to_string()];
    if let Some(root) = project_root {
        texts.extend(
            crate::project::manuscript_files(doc, root)
                .into_iter()
                .skip(1)
                .map(|(_, t)| t),
        );
    }
    for text in &texts {
        for u in crate::citation_keys::find_uses(text) {
            if !keys.contains(&u.key) {
                keys.push(u.key);
            }
        }
    }
    let mtime = std::fs::metadata(crate::bibliography::bib_target_path(&bib))
        .and_then(|m| m.modified())
        .ok();
    let cache_key: CacheKey = (bib.clone(), mtime, name, keys.clone());
    if let Some((k, v)) = cache().lock().ok()?.as_ref() {
        if *k == cache_key {
            return Some(v.clone());
        }
    }
    let library = read_library(&bib)?;
    let words = Arc::new(CitationWords {
        rendered: render(&library, &style, &keys),
    });
    *cache().lock().ok()? = Some((cache_key, words.clone()));
    Some(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BIB: &str = "\
smith2020:
  type: article
  title: One
  author: Smith, Jane
  date: 2020
  parent: {type: periodical, title: J}
duo2019:
  type: article
  title: Two
  author: ['Smith, Jane', 'Jones, Bo']
  date: 2019
  parent: {type: periodical, title: J}
trio2018:
  type: article
  title: Three
  author: ['Smith, Jane', 'Jones, Bo', 'Lee, Ann']
  date: 2018
  parent: {type: periodical, title: J}
";

    fn project(style: &str, body: &str, marker: bool) -> (tempfile::TempDir, PathBuf, String) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("refs.yaml"), BIB).unwrap();
        let text = format!(
            "{}{body}\n#bibliography(\"refs.yaml\", style: \"{style}\")\n",
            if marker {
                format!("{MARKER}\n")
            } else {
                String::new()
            }
        );
        let doc = dir.path().join("main.typ");
        std::fs::write(&doc, &text).unwrap();
        (dir, doc, text)
    }

    fn words(cites: &CitationWords, key: &str, supp: Option<&str>) -> usize {
        cites.text(key, supp).unwrap().split_whitespace().count()
    }

    #[test]
    fn the_marker_round_trips_and_is_off_by_default() {
        assert!(!counts_citations("Hello\n"));
        let on = set_counts_citations("Hello\n", true);
        assert!(counts_citations(&on));
        assert_eq!(on, format!("{MARKER}\nHello\n"));
        assert_eq!(set_counts_citations(&on, true), on);
        assert_eq!(set_counts_citations(&on, false), "Hello\n");
    }

    #[test]
    fn off_means_none_even_for_an_author_date_style() {
        let (dir, doc, text) = project("apa", "See @smith2020.", false);
        assert!(for_document(&text, &doc, Some(dir.path()), None).is_none());
    }

    #[test]
    fn the_counted_text_is_what_the_style_prints() {
        let (dir, doc, text) = project("apa", "See @smith2020 and @duo2019 and @trio2018.", true);
        let c = for_document(&text, &doc, Some(dir.path()), None).unwrap();
        assert_eq!(c.text("smith2020", None).unwrap(), "(Smith, 2020)");
        assert_eq!(words(&c, "smith2020", None), 2);
        assert_eq!(c.text("duo2019", None).unwrap(), "(Smith & Jones, 2019)");
        assert_eq!(words(&c, "duo2019", None), 4);
        assert_eq!(words(&c, "trio2018", None), 4); // (Smith et al., 2018)
        assert_eq!(words(&c, "smith2020", Some("p. 5")), 4);
        assert!(c.text("fig-1", None).is_none());
    }

    #[test]
    fn a_numeric_or_note_style_has_nothing_to_count() {
        for style in ["ieee", "chicago-notes"] {
            let (dir, doc, text) = project(style, "See @smith2020.", true);
            assert!(!applies_to(&text, &doc, Some(dir.path())), "{style}");
            assert!(for_document(&text, &doc, Some(dir.path()), None).is_none());
        }
        for style in [
            "apa",
            "chicago-author-date",
            "harvard-cite-them-right",
            "mla",
        ] {
            let (dir, doc, text) = project(style, "See @smith2020.", false);
            assert!(applies_to(&text, &doc, Some(dir.path())), "{style}");
        }
    }
}
