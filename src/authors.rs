//! Author tags: "Lastname, I." for everyone a document cites.
//!
//! These are not hand-made tags. A document's author set is derived from what
//! it cites — its `@keys` (and `#cite(...)`s, following `#include`/`#import`)
//! looked up in the bibliography it compiles against — and replaced wholesale
//! whenever the document is scanned, opened or saved, so it can never drift
//! from the text. The Library lists them in their own sidebar section and
//! searches them, which is what answers "what have I cited Butler in?".

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

/// One bibliography author, before it is reduced to a tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorName {
    /// Surname including any particle ("de Beauvoir", "van der Waals"). For an
    /// institutional author ("World Council of Churches") this is the whole name.
    pub family: String,
    pub given: String,
}

/// `Butler, J.` — surname, then the first initial of the given names. One
/// initial rather than all of them so "James Cone", "James H. Cone" and
/// "J. H. Cone" land on the same tag; two different people sharing a surname
/// and first initial do too, which is the cheaper mistake of the two. An
/// institutional author has no given name and is just its name.
pub fn author_tag(name: &AuthorName) -> Option<String> {
    let family = clean(&name.family);
    if family.is_empty() {
        return None;
    }
    match clean(&name.given)
        .chars()
        .find(|c| c.is_alphabetic())
        .map(|c| c.to_uppercase().collect::<String>())
    {
        Some(initial) => Some(format!("{family}, {initial}.")),
        None => Some(family),
    }
}

/// Drops BibTeX's grouping braces and collapses runs of whitespace.
fn clean(s: &str) -> String {
    s.replace(['{', '}'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Reads every entry's authors from a `.bib` or Hayagriva `.yaml` file,
/// keyed by citation key.
fn load_author_names(path: &Path) -> HashMap<String, Vec<AuthorName>> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return HashMap::new();
    };
    let is_yaml = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("yaml") || e.eq_ignore_ascii_case("yml"));
    let mut out = HashMap::new();
    if is_yaml {
        if let Ok(lib) = hayagriva::io::from_yaml_str(&raw) {
            for entry in lib.iter() {
                let names = entry
                    .authors()
                    .map(|people| {
                        people
                            .iter()
                            .map(|p| AuthorName {
                                family: match p.prefix.as_deref() {
                                    Some(prefix) if !prefix.is_empty() => {
                                        format!("{prefix} {}", p.name)
                                    }
                                    _ => p.name.clone(),
                                },
                                given: p.given_name.clone().unwrap_or_default(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                out.insert(entry.key().to_string(), names);
            }
        }
        return out;
    }
    let parsed = biblatex::Bibliography::parse(&raw).or_else(|_| {
        crate::bib_sanitize::sanitize_bib(&raw)
            .ok_or(())
            .and_then(|fixed| biblatex::Bibliography::parse(&fixed).map_err(|_| ()))
    });
    if let Ok(bib) = parsed {
        for entry in bib.iter() {
            let names = entry
                .author()
                .map(|people| {
                    people
                        .iter()
                        .map(|p| AuthorName {
                            family: if p.prefix.is_empty() {
                                p.name.clone()
                            } else {
                                format!("{} {}", p.prefix, p.name)
                            },
                            given: p.given_name.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            out.insert(entry.key.clone(), names);
        }
    }
    out
}

/// The bibliography documents fall back on when they don't name their own:
/// the project's override if it sets one, else the one in Settings.
pub fn configured_bibliography(work_dir: &Path, global: Option<&Path>) -> Option<PathBuf> {
    crate::config::ProjectConfig::load(work_dir)
        .unwrap_or_default()
        .bib_path
        .or_else(|| global.map(Path::to_path_buf))
}

type KeyedAuthors = Arc<HashMap<String, Vec<AuthorName>>>;

/// Works out which author tags a document earns, parsing each bibliography
/// file once (until it changes on disk) however many documents cite from it.
#[derive(Default)]
pub struct AuthorIndex {
    cache: HashMap<PathBuf, (Option<SystemTime>, KeyedAuthors)>,
}

impl AuthorIndex {
    fn bibliography(&mut self, source: &Path) -> KeyedAuthors {
        let mtime = std::fs::metadata(source).and_then(|m| m.modified()).ok();
        if let Some((cached, authors)) = self.cache.get(source) {
            if *cached == mtime {
                return authors.clone();
            }
        }
        let authors = Arc::new(load_author_names(source));
        self.cache
            .insert(source.to_path_buf(), (mtime, authors.clone()));
        authors
    }

    /// Sorted, de-duplicated author tags for everything `doc` cites. The
    /// bibliography is the one the document names itself, else `configured`.
    /// No bibliography, or nothing cited that it contains, means no tags.
    pub fn tags_for_document(&mut self, doc: &Path, configured: Option<&Path>) -> Vec<String> {
        let Some(source) = crate::cited_refs::resolve_source(doc, configured) else {
            return Vec::new();
        };
        let keys = crate::cited_refs::collect_cited_keys(doc);
        if keys.is_empty() {
            return Vec::new();
        }
        let bib = self.bibliography(&source);
        let mut tags: Vec<String> = keys
            .iter()
            .filter_map(|k| bib.get(k))
            .flatten()
            .filter_map(author_tag)
            .collect();
        tags.sort_by_key(|t| t.to_lowercase());
        tags.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        tags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(family: &str, given: &str) -> AuthorName {
        AuthorName {
            family: family.into(),
            given: given.into(),
        }
    }

    #[test]
    fn tag_is_surname_comma_first_initial() {
        assert_eq!(author_tag(&name("Butler", "Judith")).unwrap(), "Butler, J.");
    }

    #[test]
    fn only_the_first_given_name_gives_the_initial() {
        assert_eq!(author_tag(&name("Cone", "James Hal")).unwrap(), "Cone, J.");
        assert_eq!(author_tag(&name("Cone", "J. H.")).unwrap(), "Cone, J.");
    }

    #[test]
    fn an_initial_is_uppercased_and_accents_survive() {
        assert_eq!(author_tag(&name("Žižek", "slavoj")).unwrap(), "Žižek, S.");
        assert_eq!(author_tag(&name("Ørsted", "Ørjan")).unwrap(), "Ørsted, Ø.");
    }

    #[test]
    fn particles_stay_with_the_surname() {
        assert_eq!(
            author_tag(&name("de Beauvoir", "Simone")).unwrap(),
            "de Beauvoir, S."
        );
    }

    #[test]
    fn an_institution_has_no_initial() {
        assert_eq!(
            author_tag(&name("{World Council of Churches}", "")).unwrap(),
            "World Council of Churches"
        );
    }

    #[test]
    fn braces_and_stray_space_are_cleaned() {
        assert_eq!(
            author_tag(&name("{Fiorenza}", "  {Elisabeth}  Schüssler")).unwrap(),
            "Fiorenza, E."
        );
    }

    #[test]
    fn a_nameless_author_makes_no_tag() {
        assert_eq!(author_tag(&name("", "Judith")), None);
        assert_eq!(author_tag(&name("  ", "")), None);
    }

    fn write(dir: &Path, file: &str, body: &str) -> PathBuf {
        let p = dir.join(file);
        std::fs::write(&p, body).unwrap();
        p
    }

    const BIB: &str = r#"
@book{gender, author = {Butler, Judith}, title = {Gender Trouble}, year = {1990}}
@book{black, author = {Cone, James H.}, title = {A Black Theology}, year = {1970}}
@book{pair, author = {Cone, James H. and Wilmore, Gayraud S.}, title = {Reader}, year = {1979}}
@book{uncited, author = {Nobody, Nemo}, title = {Unread}, year = {2001}}
@report{wcc, author = {{World Council of Churches}}, title = {Baptism}, year = {1982}}
"#;

    #[test]
    fn a_document_earns_tags_for_exactly_what_it_cites() {
        let dir = tempfile::tempdir().unwrap();
        let bib = write(dir.path(), "refs.bib", BIB);
        let doc = write(
            dir.path(),
            "paper.typ",
            "As @gender argues, and @pair, and @wcc. #cite(<black>)\n",
        );
        let mut index = AuthorIndex::default();
        assert_eq!(
            index.tags_for_document(&doc, Some(&bib)),
            vec![
                "Butler, J.",
                "Cone, J.",
                "Wilmore, G.",
                "World Council of Churches"
            ]
        );
    }

    #[test]
    fn a_document_naming_its_own_bibliography_beats_the_configured_one() {
        let dir = tempfile::tempdir().unwrap();
        let configured = write(dir.path(), "other.bib", "@book{gender, author={Wrong, W.}}");
        write(dir.path(), "refs.bib", BIB);
        let doc = write(
            dir.path(),
            "paper.typ",
            "#bibliography(\"refs.bib\")\nSee @gender.\n",
        );
        let mut index = AuthorIndex::default();
        assert_eq!(
            index.tags_for_document(&doc, Some(&configured)),
            vec!["Butler, J."]
        );
    }

    #[test]
    fn citations_in_included_files_count() {
        let dir = tempfile::tempdir().unwrap();
        let bib = write(dir.path(), "refs.bib", BIB);
        write(dir.path(), "ch1.typ", "Per @gender.\n");
        let doc = write(dir.path(), "main.typ", "#include \"ch1.typ\"\n");
        let mut index = AuthorIndex::default();
        assert_eq!(
            index.tags_for_document(&doc, Some(&bib)),
            vec!["Butler, J."]
        );
    }

    #[test]
    fn no_bibliography_or_no_citations_means_no_tags() {
        let dir = tempfile::tempdir().unwrap();
        let bib = write(dir.path(), "refs.bib", BIB);
        let doc = write(dir.path(), "paper.typ", "Plain prose, a@b.example only.\n");
        let mut index = AuthorIndex::default();
        assert!(index.tags_for_document(&doc, Some(&bib)).is_empty());
        let cites = write(dir.path(), "cites.typ", "@gender\n");
        assert!(index.tags_for_document(&cites, None).is_empty());
    }

    #[test]
    fn an_edited_bibliography_is_picked_up() {
        let dir = tempfile::tempdir().unwrap();
        let bib = write(dir.path(), "refs.bib", "@book{k, author = {Old, Ann}}");
        let doc = write(dir.path(), "paper.typ", "@k\n");
        let mut index = AuthorIndex::default();
        assert_eq!(index.tags_for_document(&doc, Some(&bib)), vec!["Old, A."]);
        std::fs::write(&bib, "@book{k, author = {Newer, Bea}}").unwrap();
        // Same-second writes can leave mtime unchanged on coarse filesystems.
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(&bib)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert_eq!(index.tags_for_document(&doc, Some(&bib)), vec!["Newer, B."]);
    }

    #[test]
    fn yaml_bibliographies_work_too() {
        let dir = tempfile::tempdir().unwrap();
        let bib = write(
            dir.path(),
            "refs.yml",
            "gender:\n  type: Book\n  title: Gender Trouble\n  author: \"Butler, Judith\"\n  date: 1990\n",
        );
        let doc = write(dir.path(), "paper.typ", "@gender\n");
        let mut index = AuthorIndex::default();
        assert_eq!(
            index.tags_for_document(&doc, Some(&bib)),
            vec!["Butler, J."]
        );
    }
}
