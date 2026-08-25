//! User-saved templates: a named set of template settings, kept in
//! `~/.local/share/zerkalo/templates/<slug>.toml`.
//!
//! The stored payload is a [`SidecarSettings`] — the same shape a document's
//! `.zerkalo.toml` uses — so saving is "take what the form collected" and
//! applying is the same pre-fill path a document's own sidecar goes through.
//! Nothing new has to be kept in step when a setting is added to the form.

use std::path::{Path, PathBuf};

use crate::templates::slugify;
use crate::ui::template_dialog::SidecarSettings;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct UserTemplate {
    pub name: String,
    pub settings: SidecarSettings,
}

pub fn templates_dir() -> PathBuf {
    crate::config::zerkalo_data_dir().join("templates")
}

/// The file a template with this name lives in. Two names that slugify the
/// same ("My Thesis" / "my thesis") are deliberately the same template rather
/// than two rows that look identical in the gallery.
pub fn path_for(dir: &Path, name: &str) -> Option<PathBuf> {
    let slug = slugify(name);
    (!slug.is_empty()).then(|| dir.join(format!("{slug}.toml")))
}

pub fn list() -> Vec<UserTemplate> {
    list_in(&templates_dir())
}

/// Every readable template, by name. A file that won't parse is skipped and
/// logged rather than taking the gallery down with it — the rest still work,
/// and the bad file stays on disk for the user to look at.
pub fn list_in(dir: &Path) -> Vec<UserTemplate> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<UserTemplate> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("toml"))
        .filter_map(|p| match std::fs::read_to_string(&p) {
            Ok(text) => match toml::from_str::<UserTemplate>(&text) {
                Ok(t) if !t.name.trim().is_empty() => Some(t),
                Ok(_) => {
                    tracing::warn!("Template {:?} has no name; skipping", p);
                    None
                }
                Err(e) => {
                    tracing::warn!("Template {:?} is corrupt ({e}); skipping", p);
                    None
                }
            },
            Err(e) => {
                tracing::warn!("Cannot read template {:?}: {e}", p);
                None
            }
        })
        .collect();
    out.sort_by_key(|t| t.name.to_lowercase());
    out
}

pub fn exists(name: &str) -> bool {
    exists_in(&templates_dir(), name)
}

pub fn exists_in(dir: &Path, name: &str) -> bool {
    path_for(dir, name).is_some_and(|p| p.exists())
}

pub fn save(name: &str, settings: &SidecarSettings) -> Result<PathBuf, String> {
    save_in(&templates_dir(), name, settings)
}

/// Store `settings` under `name`, minus the parts that belong to one document
/// rather than to a template — a saved template that carried a title, date,
/// abstract and keyword line would stamp another document's front matter onto
/// every new document made from it.
pub fn save_in(dir: &Path, name: &str, settings: &SidecarSettings) -> Result<PathBuf, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("A template needs a name.".into());
    }
    let path =
        path_for(dir, name).ok_or("That name has no letters or numbers in it — try another.")?;

    let template = UserTemplate {
        name: name.to_string(),
        settings: strip_document_fields(settings),
    };
    let text = toml::to_string_pretty(&template).map_err(|e| e.to_string())?;

    std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
    crate::ui::template_dialog::write_atomically(&path, &text)
        .map_err(|e| format!("Cannot save the template: {e}"))?;
    Ok(path)
}

pub fn delete(name: &str) -> Result<(), String> {
    delete_in(&templates_dir(), name)
}

pub fn delete_in(dir: &Path, name: &str) -> Result<(), String> {
    let path = path_for(dir, name).ok_or("No such template.")?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("Cannot delete the template: {e}")),
    }
}

/// Drop the fields that describe one particular document. The identity fields
/// (author, affiliation, and the CV rows that reuse them for email/phone/links)
/// stay: a personal template is precisely where someone wants those kept.
/// `bib_path` goes too — it's an absolute path to one project's `.bib`, and a
/// template pointing at a file that isn't there generates a document that
/// won't compile.
fn strip_document_fields(s: &SidecarSettings) -> SidecarSettings {
    let is_cv = s.body_kind == "cv";
    SidecarSettings {
        title: String::new(),
        // For a CV this row holds the email address, not a document subtitle.
        subtitle: if is_cv {
            s.subtitle.clone()
        } else {
            String::new()
        },
        date: String::new(),
        abstract_text: String::new(),
        keywords_text: String::new(),
        bib_path: None,
        ..s.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "zerkalo-user-templates-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn settings() -> SidecarSettings {
        SidecarSettings {
            title: "My Dissertation Chapter 3".into(),
            subtitle: "A subtitle".into(),
            author: "Jane Doe".into(),
            date: "2026-08-10".into(),
            abstract_text: "This chapter argues…".into(),
            keywords_text: "one, two".into(),
            bib_path: Some("/home/jane/thesis/refs.bib".into()),
            style: "chicago-notes".into(),
            paper: "a4".into(),
            font: "EB Garamond".into(),
            font_size: "14pt".into(),
            body_kind: "academic".into(),
            ..Default::default()
        }
    }

    #[test]
    fn saving_keeps_the_formatting_and_drops_the_document() {
        let dir = temp_dir("roundtrip");
        save_in(&dir, "Thesis Chapter", &settings()).unwrap();

        let loaded = list_in(&dir);
        assert_eq!(loaded.len(), 1);
        let t = &loaded[0];
        assert_eq!(t.name, "Thesis Chapter");

        // Formatting survives…
        assert_eq!(t.settings.style, "chicago-notes");
        assert_eq!(t.settings.paper, "a4");
        assert_eq!(t.settings.font, "EB Garamond");
        assert_eq!(t.settings.font_size, "14pt");
        assert_eq!(t.settings.author, "Jane Doe");

        // …the previous document's front matter does not.
        assert!(t.settings.title.is_empty());
        assert!(t.settings.subtitle.is_empty());
        assert!(t.settings.date.is_empty());
        assert!(t.settings.abstract_text.is_empty());
        assert!(t.settings.keywords_text.is_empty());
        assert!(t.settings.bib_path.is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_cv_template_keeps_the_contact_row_that_shares_the_subtitle_field() {
        let dir = temp_dir("cv");
        let mut s = settings();
        s.body_kind = "cv".into();
        s.subtitle = "jane@example.com".into();
        save_in(&dir, "My CV", &s).unwrap();

        assert_eq!(list_in(&dir)[0].settings.subtitle, "jane@example.com");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn saving_the_same_name_twice_replaces_rather_than_duplicates() {
        let dir = temp_dir("replace");
        save_in(&dir, "Report", &settings()).unwrap();
        let mut second = settings();
        second.paper = "us-letter".into();
        save_in(&dir, "report", &second).unwrap();

        let loaded = list_in(&dir);
        assert_eq!(
            loaded.len(),
            1,
            "a name differing only in case is the same template"
        );
        assert_eq!(loaded[0].settings.paper, "us-letter");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_nameless_or_punctuation_only_name_is_refused() {
        let dir = temp_dir("names");
        assert!(save_in(&dir, "   ", &settings()).is_err());
        assert!(save_in(&dir, "!!!", &settings()).is_err());
        assert!(list_in(&dir).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_corrupt_template_does_not_hide_the_good_ones() {
        let dir = temp_dir("corrupt");
        save_in(&dir, "Good", &settings()).unwrap();
        std::fs::write(dir.join("broken.toml"), "this is not : valid toml [").unwrap();

        let loaded = list_in(&dir);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "Good");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn deleting_is_idempotent_and_leaves_the_others_alone() {
        let dir = temp_dir("delete");
        save_in(&dir, "One", &settings()).unwrap();
        save_in(&dir, "Two", &settings()).unwrap();

        delete_in(&dir, "One").unwrap();
        delete_in(&dir, "One").unwrap();

        let names: Vec<String> = list_in(&dir).into_iter().map(|t| t.name).collect();
        assert_eq!(names, vec!["Two".to_string()]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_name_with_a_path_separator_cannot_escape_the_templates_folder() {
        let dir = temp_dir("traversal");
        save_in(&dir, "../../evil", &settings()).unwrap();

        let written: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].parent().unwrap(), dir);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn listing_a_missing_folder_is_empty_rather_than_an_error() {
        assert!(list_in(&std::env::temp_dir().join("zerkalo-no-such-templates-dir")).is_empty());
    }
}
