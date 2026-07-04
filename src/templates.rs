#![allow(dead_code)]

/// Project template definitions. Built-in templates are embedded via include_str!
/// from templates/ at the project root. User templates are loaded at runtime from
/// ~/.config/zerkalo/templates/<name>/manifest.toml.

use std::path::{Path, PathBuf};
use serde::Deserialize;
use crate::error::Result;

// ── Built-in templates ─────────────────────────────────────────────────────────

pub(crate) struct BuiltinTemplate {
    label: &'static str,
    description: &'static str,
    root_file: &'static str,
    /// (filename, content-with-__NAME__-placeholder)
    files: &'static [(&'static str, &'static str)],
}

static BLANK: BuiltinTemplate = BuiltinTemplate {
    label: "Blank",
    description: "An empty document. Start from scratch.",
    root_file: "main.typ",
    files: &[
        ("main.typ", include_str!("../templates/blank/main.typ")),
    ],
};

static ESSAY: BuiltinTemplate = BuiltinTemplate {
    label: "Essay",
    description: "Single-file essay with title block and bibliography.",
    root_file: "main.typ",
    files: &[
        ("main.typ",         include_str!("../templates/essay/main.typ")),
        ("bibliography.bib", include_str!("../templates/essay/bibliography.bib")),
    ],
};

static JOURNAL_THESIS: BuiltinTemplate = BuiltinTemplate {
    label: "Journal / Thesis",
    description: "Multi-chapter document: title page, intro chapter, bibliography.",
    root_file: "main.typ",
    files: &[
        ("main.typ",               include_str!("../templates/journal-thesis/main.typ")),
        ("title.typ",              include_str!("../templates/journal-thesis/title.typ")),
        ("ch01-introduction.typ",  include_str!("../templates/journal-thesis/ch01-introduction.typ")),
        ("bibliography.bib",       include_str!("../templates/journal-thesis/bibliography.bib")),
    ],
};

static THEOLOGICAL_JOURNAL: BuiltinTemplate = BuiltinTemplate {
    label: "Theological Journal",
    description: "Journal issue: front matter, article stub, bibliography.",
    root_file: "main.typ",
    files: &[
        ("main.typ",        include_str!("../templates/theological-journal/main.typ")),
        ("front-matter.typ",include_str!("../templates/theological-journal/front-matter.typ")),
        ("article-01.typ",  include_str!("../templates/theological-journal/article-01.typ")),
        ("bibliography.bib",include_str!("../templates/theological-journal/bibliography.bib")),
    ],
};

// ── User templates ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct UserManifest {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_root_file")]
    root_file: String,
}

fn default_root_file() -> String { "main.typ".into() }

/// A template loaded at runtime from ~/.config/zerkalo/templates/<name>/manifest.toml.
#[derive(Clone)]
pub struct UserTemplate {
    pub label: String,
    pub description: String,
    pub root_file: String,
    /// The directory containing the template files.
    pub source_dir: PathBuf,
}

impl UserTemplate {
    fn generate(&self, project_dir: &Path, project_name: &str) -> Result<()> {
        let entries = std::fs::read_dir(&self.source_dir)?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.file_name().map(|n| n == "manifest.toml").unwrap_or(false) {
                continue;
            }
            if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                let content = if fname.ends_with(".typ") {
                    content.replace("__NAME__", project_name)
                } else {
                    content
                };
                std::fs::write(project_dir.join(fname), content)?;
            }
        }
        Ok(())
    }
}

// ── Public AnyTemplate type ────────────────────────────────────────────────────

#[derive(Clone)]
pub enum AnyTemplate {
    Builtin(&'static BuiltinTemplate),
    User(UserTemplate),
}

impl AnyTemplate {
    pub fn label(&self) -> &str {
        match self {
            Self::Builtin(t) => t.label,
            Self::User(t)    => &t.label,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::Builtin(t) => t.description,
            Self::User(t)    => &t.description,
        }
    }

    pub fn root_file(&self) -> &str {
        match self {
            Self::Builtin(t) => t.root_file,
            Self::User(t)    => &t.root_file,
        }
    }

    pub fn generate(&self, project_dir: &Path, project_name: &str) -> Result<()> {
        match self {
            Self::Builtin(t) => {
                for (filename, content) in t.files {
                    let content = if filename.ends_with(".typ") {
                        content.replace("__NAME__", project_name)
                    } else {
                        content.to_string()
                    };
                    std::fs::write(project_dir.join(filename), content)?;
                }
                Ok(())
            }
            Self::User(t) => t.generate(project_dir, project_name),
        }
    }
}

/// All built-in templates in display order.
pub fn builtin_templates() -> Vec<AnyTemplate> {
    vec![
        AnyTemplate::Builtin(&BLANK),
        AnyTemplate::Builtin(&ESSAY),
        AnyTemplate::Builtin(&JOURNAL_THESIS),
        AnyTemplate::Builtin(&THEOLOGICAL_JOURNAL),
    ]
}

/// User-defined templates from ~/.config/zerkalo/templates/.
/// Silently skips directories with missing or invalid manifests.
pub fn user_templates() -> Vec<AnyTemplate> {
    let base = dirs_sys_config_dir().join("zerkalo").join("templates");
    if !base.is_dir() { return vec![]; }

    let mut out = vec![];
    if let Ok(entries) = std::fs::read_dir(&base) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() { continue; }
            let manifest_path = dir.join("manifest.toml");
            let Ok(src) = std::fs::read_to_string(&manifest_path) else { continue };
            let Ok(m) = toml::from_str::<UserManifest>(&src) else { continue };
            out.push(AnyTemplate::User(UserTemplate {
                label:       m.name,
                description: m.description,
                root_file:   m.root_file,
                source_dir:  dir,
            }));
        }
    }
    out.sort_by(|a, b| a.label().cmp(b.label()));
    out
}

/// Builtin templates followed by any user-defined templates.
pub fn all_templates() -> Vec<AnyTemplate> {
    let mut v = builtin_templates();
    v.extend(user_templates());
    v
}

fn dirs_sys_config_dir() -> PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs_home_dir().join(".config")
        })
}

fn dirs_home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/"))
}

// ── slugify ────────────────────────────────────────────────────────────────────

/// Turn a project name into a safe folder name: lowercase, spaces → hyphens,
/// non-alphanumeric (except hyphens) stripped.
pub fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_lowercases_and_hyphenates_spaces() {
        assert_eq!(slugify("My Great Essay"), "my-great-essay");
    }

    #[test]
    fn slugify_collapses_consecutive_separators() {
        assert_eq!(slugify("Foo___Bar  Baz"), "foo-bar-baz");
    }

    #[test]
    fn slugify_strips_leading_trailing_separators() {
        assert_eq!(slugify("  -Hello-  "), "hello");
    }

    #[test]
    fn slugify_empty_input_is_empty() {
        assert_eq!(slugify(""), "");
    }

    #[test]
    fn builtin_templates_have_unique_labels_and_root_files() {
        let templates = builtin_templates();
        assert_eq!(templates.len(), 4);
        let labels: Vec<&str> = templates.iter().map(|t| t.label()).collect();
        let mut unique = labels.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(labels.len(), unique.len(), "labels should be unique");
        for t in &templates {
            assert_eq!(t.root_file(), "main.typ");
        }
    }

    #[test]
    fn builtin_template_generate_writes_files_and_substitutes_name() {
        let dir = tempfile::tempdir().unwrap();
        let templates = builtin_templates();
        let essay = templates.iter().find(|t| t.label() == "Essay").unwrap();
        essay.generate(dir.path(), "My Project").unwrap();

        let main_content = std::fs::read_to_string(dir.path().join("main.typ")).unwrap();
        assert!(main_content.contains("My Project"), "project name should replace __NAME__ placeholder");
        assert!(!main_content.contains("__NAME__"));
        assert!(dir.path().join("bibliography.bib").exists());
    }

    #[test]
    fn user_templates_empty_when_config_dir_missing() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", dir.path());
        assert!(user_templates().is_empty());
        std::env::remove_var("XDG_CONFIG_HOME");
    }
}
