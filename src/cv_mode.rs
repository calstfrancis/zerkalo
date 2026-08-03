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
///
/// `cv-helpers.typ` is injected unconditionally, even with no Skrizhal file
/// configured — CV templates unconditionally `#import` it, and `cv-data`
/// degrades to an empty dict, so leaving it out here would fail every export
/// of a CV document that hasn't been pointed at a Skrizhal file yet.
pub fn cv_mode_compile_extras(
    project_root: &Path,
    cv_elements_path: Option<&Path>,
) -> (HashMap<PathBuf, String>, HashMap<String, String>) {
    let mut overrides = HashMap::new();
    let mut sys_inputs = HashMap::new();
    overrides.insert(project_root.join("cv-helpers.typ"), CV_HELPERS_TYPST.to_string());
    if let Some(cv_path) = cv_elements_path {
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
#[allow(dead_code)] // Skrizhal rename-propagation, kept for the CV editor work
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

/// Drops reserved (`_`-prefixed) top-level blocks from CV-elements YAML.
///
/// Skrizhal reserves underscore-prefixed top-level keys for configuration
/// rather than CV entries — `_profiles` since Skrizhal 0.4.0. The pinned
/// `skrizhal-core` (v0.3.0) predates that convention and deserializes the
/// whole file as one map of entries, so a single `_profiles` block makes the
/// *entire* parse fail — and every call site here uses `unwrap_or_default()`,
/// which turns that into a silently empty entry list: no `!` autocomplete, no
/// CV panel contents, no error shown.
///
/// Filtering textually rather than bumping the dependency keeps this
/// independent of which `skrizhal-core` is pinned, which is worth having on
/// its own: one unrecognized block should never cost the user every entry in
/// the file. Newer `skrizhal-core` skips these keys itself, and then this
/// simply has nothing left to do.
///
/// Top-level YAML keys sit at column 0 and Skrizhal writes plain block-style
/// YAML, so "drop from a `_`-prefixed key until the next column-0 line" is
/// sufficient — and strictly narrower than a full YAML round-trip, which
/// would risk reformatting content this function has no business touching.
pub fn strip_reserved_blocks(yaml: &str) -> String {
    let mut out = String::with_capacity(yaml.len());
    let mut skipping = false;
    for line in yaml.lines() {
        let starts_at_column_zero =
            !line.is_empty() && !line.starts_with(char::is_whitespace) && !line.starts_with('#');
        if starts_at_column_zero {
            // A comment or blank line inside a skipped block is dropped with
            // it; a new column-0 key always ends the skip.
            skipping = line.starts_with('_') && line.contains(':');
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Reads and parses a CV-elements file, tolerating reserved blocks the
/// pinned `skrizhal-core` doesn't recognize. Returns an empty list on any
/// read/parse failure, matching what the call sites did before.
pub fn load_cv_entries(path: &std::path::Path) -> Vec<skrizhal_core::CvEntry> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    match skrizhal_core::parse_str(&strip_reserved_blocks(&text)) {
        Ok(entries) => entries,
        Err(err) => {
            tracing::warn!("Failed to parse CV elements at {}: {err}", path.display());
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WITH_PROFILES: &str = "job-one:\n\
                                 \x20 category: Employment\n\
                                 \x20 title: A Job\n\
                                 _profiles:\n\
                                 \x20 academic:\n\
                                 \x20   label: Academic CV\n\
                                 \x20   sections:\n\
                                 \x20     - heading: Work\n\
                                 job-two:\n\
                                 \x20 category: Award\n\
                                 \x20 title: An Award\n";

    /// The regression this exists for: with the pinned skrizhal-core v0.3.0,
    /// a `_profiles` block fails the whole parse, and `unwrap_or_default()`
    /// turns that into an empty CV list with nothing shown to the user.
    #[test]
    fn a_reserved_block_does_not_cost_us_the_other_entries() {
        let entries = skrizhal_core::parse_str(&strip_reserved_blocks(WITH_PROFILES))
            .expect("filtered YAML should parse");
        let keys: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["job-one", "job-two"]);
    }

    #[test]
    fn unfiltered_input_still_breaks_the_parser() {
        // Guards the premise: if this ever starts succeeding, the pin has
        // moved and the filter is redundant rather than load-bearing.
        assert!(skrizhal_core::parse_str(WITH_PROFILES).is_err());
    }

    #[test]
    fn the_reserved_block_and_its_children_are_removed() {
        let out = strip_reserved_blocks(WITH_PROFILES);
        assert!(!out.contains("_profiles"));
        assert!(!out.contains("Academic CV"));
        assert!(!out.contains("heading: Work"));
        assert!(out.contains("job-one"));
        assert!(out.contains("job-two"));
    }

    #[test]
    fn a_file_with_no_reserved_blocks_is_unchanged_apart_from_a_trailing_newline() {
        let yaml = "a:\n  category: Award\n  title: Thing\n";
        assert_eq!(strip_reserved_blocks(yaml), yaml);
    }

    #[test]
    fn a_reserved_block_at_the_end_of_the_file_is_dropped() {
        let yaml = "a:\n  category: Award\n  title: Thing\n_profiles:\n  p:\n    sections: []\n";
        let out = strip_reserved_blocks(yaml);
        assert!(!out.contains("_profiles"));
        assert!(out.contains("title: Thing"));
    }

    /// An underscore inside a key is normal in citation-style slugs and must
    /// not be mistaken for the reserved prefix.
    #[test]
    fn only_a_leading_underscore_counts_as_reserved() {
        let yaml = "some_key:\n  category: Award\n  title: Kept\n";
        assert!(strip_reserved_blocks(yaml).contains("some_key"));
    }

    #[test]
    fn empty_input_stays_empty() {
        assert_eq!(strip_reserved_blocks(""), "");
    }

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
