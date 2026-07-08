use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Skrizhal CV-entry/CV-section Typst helpers, made available in CV mode
/// (see `effective_cv_elements` in `app_window.rs`) without ever touching
/// disk — injected as a virtual `cv-helpers.typ` alongside the document via
/// `PreviewPane::set_buffer_snapshot` / `cv_mode_compile_extras` below.
pub const CV_HELPERS_TYPST: &str = include_str!("../templates/cv-helpers.typ");

/// Builds the `(overrides, sys_inputs)` a one-shot compile (export, library
/// thumbnails, etc.) needs for CV mode — the live-preview path gets the same
/// data via `PreviewPane::set_cv_elements_path`'s fresh-read-per-compile
/// mechanism instead, since it compiles repeatedly.
pub fn cv_mode_compile_extras(
    project_root: &Path,
    cv_elements_path: Option<&Path>,
) -> (HashMap<PathBuf, String>, HashMap<String, String>) {
    let mut overrides = HashMap::new();
    let mut sys_inputs = HashMap::new();
    if let Some(cv_path) = cv_elements_path {
        overrides.insert(project_root.join("cv-helpers.typ"), CV_HELPERS_TYPST.to_string());
        match std::fs::read_to_string(cv_path) {
            Ok(yaml) => {
                sys_inputs.insert("skrizhal-cv-data".to_string(), yaml);
            }
            Err(e) => tracing::warn!("CV mode: couldn't read {}: {e}", cv_path.display()),
        }
    }
    (overrides, sys_inputs)
}

/// Renames occurrences of a CV entry key (`#cv-entry("key")`) in Typst
/// source text. Returns the rewritten text and whether anything changed.
///
/// Deliberately separate from `bibliography::rename_key_in_text` rather than
/// folded into the same regex/pass: a bib-key rename and a CV-entry-key
/// rename are triggered by different UI actions on different data sources,
/// and — since Skrizhal keys and BibTeX keys are independent namespaces —
/// could coincidentally share a literal key string. Keeping them separate
/// means renaming one can never accidentally rewrite the other's references.
pub fn rename_cv_entry_key_in_text(text: &str, old_key: &str, new_key: &str) -> (String, bool) {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r#"#cv-entry\("([^"]+)"\)"#).unwrap());

    let result = re.replace_all(text, |caps: &regex::Captures| {
        let whole = caps.get(0).unwrap().as_str();
        let key = caps.get(1).unwrap().as_str();
        if key == old_key {
            format!("#cv-entry(\"{new_key}\")")
        } else {
            whole.to_string()
        }
    });
    let changed = result != text;
    (result.into_owned(), changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renames_matching_cv_entry_key() {
        let (out, changed) = rename_cv_entry_key_in_text(
            "See #cv-entry(\"old-key\") for details.",
            "old-key",
            "new-key",
        );
        assert!(changed);
        assert_eq!(out, "See #cv-entry(\"new-key\") for details.");
    }

    #[test]
    fn leaves_non_matching_keys_alone() {
        let (out, changed) =
            rename_cv_entry_key_in_text("#cv-entry(\"other-key\")", "old-key", "new-key");
        assert!(!changed);
        assert_eq!(out, "#cv-entry(\"other-key\")");
    }

    #[test]
    fn renames_all_occurrences() {
        let (out, changed) = rename_cv_entry_key_in_text(
            "#cv-entry(\"k\") and again #cv-entry(\"k\")",
            "k",
            "k2",
        );
        assert!(changed);
        assert_eq!(out, "#cv-entry(\"k2\") and again #cv-entry(\"k2\")");
    }

    #[test]
    fn does_not_touch_citation_syntax() {
        let (out, changed) =
            rename_cv_entry_key_in_text("@old-key and #cite(<old-key>)", "old-key", "new-key");
        assert!(!changed);
        assert_eq!(out, "@old-key and #cite(<old-key>)");
    }
}
