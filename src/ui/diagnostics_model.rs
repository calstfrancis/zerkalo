use std::collections::HashMap;
use std::path::PathBuf;

use super::error_panel::{CompileError, Severity};
use crate::diagnostic_catalog::Kind;

/// What the editor needs to underline one problem and explain it on hover:
/// where, how far, and its plain-language wording and kind.
#[derive(Debug, Clone)]
pub struct DiagMark {
    pub file: PathBuf,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub is_error: bool,
    pub headline: String,
    pub advice: String,
    pub kind: Kind,
    /// The wording describes a package's own code, so no fix here applies.
    pub in_package: bool,
}

impl From<&CompileError> for DiagMark {
    fn from(e: &CompileError) -> Self {
        DiagMark {
            file: e.file.clone(),
            line: e.line,
            col: e.col,
            end_line: e.end_line,
            end_col: e.end_col,
            is_error: e.severity == Severity::Error,
            headline: e.message.clone(),
            advice: e.advice.clone(),
            kind: e.kind,
            in_package: e.origin.is_some(),
        }
    }
}

/// The single owner of "what is wrong right now". The compiler and the
/// language server each report on their own schedule; both write here, and
/// everything on screen (underlines, panel, counts) is drawn from `merged()`,
/// so the two can no longer overwrite or hide each other.
#[derive(Default)]
pub struct DiagnosticsModel {
    compile: Vec<CompileError>,
    /// Latest report per file. A report replaces that file's previous one, and
    /// an empty report clears it.
    lsp: HashMap<PathBuf, Vec<CompileError>>,
}

impl DiagnosticsModel {
    pub fn set_compile(&mut self, errors: Vec<CompileError>) {
        self.compile = errors;
    }

    pub fn set_lsp_file(&mut self, file: PathBuf, errors: Vec<CompileError>) {
        if errors.is_empty() {
            self.lsp.remove(&file);
        } else {
            self.lsp.insert(file, errors);
        }
    }

    pub fn clear_lsp(&mut self) {
        self.lsp.clear();
    }

    /// Compile results first (they carry the exact range, hints and package
    /// origin), then language-server findings the compiler didn't report.
    /// Errors sort ahead of notes; otherwise order is stable so the panel
    /// doesn't reshuffle between updates.
    pub fn merged(&self) -> Vec<CompileError> {
        let mut out: Vec<CompileError> = Vec::new();
        for e in &self.compile {
            out.push(e.clone());
        }
        let mut lsp_files: Vec<&PathBuf> = self.lsp.keys().collect();
        lsp_files.sort();
        for file in lsp_files {
            for e in &self.lsp[file] {
                let covered = out.iter().any(|c| {
                    c.file == e.file
                        && c.line == e.line
                        && (c.technical == e.technical
                            || (c.severity == Severity::Error && e.severity == Severity::Error))
                });
                if !covered {
                    out.push(e.clone());
                }
            }
        }
        out.sort_by_key(|e| e.severity == Severity::Warning);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(m: &DiagnosticsModel) -> (usize, usize) {
        let merged = m.merged();
        let errors = merged
            .iter()
            .filter(|e| e.severity == Severity::Error)
            .count();
        (errors, merged.len() - errors)
    }

    fn diag(file: &str, line: u32, msg: &str, severity: Severity) -> CompileError {
        CompileError {
            file: PathBuf::from(file),
            line,
            col: 1,
            end_line: line,
            end_col: 2,
            origin: None,
            message: msg.into(),
            advice: String::new(),
            kind: Kind::Other,
            hints: Vec::new(),
            technical: msg.into(),
            severity,
        }
    }

    #[test]
    fn an_lsp_error_on_a_line_the_compiler_already_reported_is_dropped() {
        let mut m = DiagnosticsModel::default();
        m.set_compile(vec![diag(
            "a.typ",
            3,
            "unknown variable: x",
            Severity::Error,
        )]);
        m.set_lsp_file(
            "a.typ".into(),
            vec![diag("a.typ", 3, "unknown variable `x`", Severity::Error)],
        );
        assert_eq!(counts(&m), (1, 0));
    }

    #[test]
    fn lsp_findings_the_compiler_missed_are_kept() {
        let mut m = DiagnosticsModel::default();
        m.set_compile(vec![diag("a.typ", 3, "boom", Severity::Error)]);
        m.set_lsp_file(
            "a.typ".into(),
            vec![diag("a.typ", 9, "unclosed", Severity::Error)],
        );
        assert_eq!(counts(&m), (2, 0));
    }

    #[test]
    fn a_clean_report_clears_only_its_own_file() {
        let mut m = DiagnosticsModel::default();
        m.set_lsp_file("a.typ".into(), vec![diag("a.typ", 1, "x", Severity::Error)]);
        m.set_lsp_file("b.typ".into(), vec![diag("b.typ", 1, "y", Severity::Error)]);
        m.set_lsp_file("a.typ".into(), Vec::new());
        let left = m.merged();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].file, PathBuf::from("b.typ"));
    }

    #[test]
    fn a_warnings_only_report_never_hides_a_compile_error() {
        let mut m = DiagnosticsModel::default();
        m.set_compile(vec![diag("a.typ", 3, "boom", Severity::Error)]);
        m.set_lsp_file(
            "a.typ".into(),
            vec![diag("a.typ", 1, "meh", Severity::Warning)],
        );
        assert_eq!(counts(&m), (1, 1));
    }

    #[test]
    fn errors_sort_before_notes() {
        let mut m = DiagnosticsModel::default();
        m.set_compile(vec![
            diag("a.typ", 1, "note", Severity::Warning),
            diag("a.typ", 5, "error", Severity::Error),
        ]);
        assert_eq!(m.merged()[0].technical, "error");
    }
}
