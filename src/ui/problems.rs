use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;

use super::diagnostics_model::{DiagMark, DiagnosticsModel};
use super::editor_pane::EditorPane;
use super::error_panel::{CompileError, ErrorPanel, Severity};

type Signature = Vec<(PathBuf, u32, u32, String, bool)>;

/// Draws the merged diagnostics model onto the editor (underlines, gutter,
/// counts) and the Problems panel. The compiler and the language server both
/// report through here, so neither can overwrite or suppress the other.
#[derive(Clone)]
pub struct Problems {
    model: Rc<RefCell<DiagnosticsModel>>,
    panel: ErrorPanel,
    editor: EditorPane,
    shown: Rc<RefCell<Signature>>,
}

impl Problems {
    pub fn new(panel: ErrorPanel, editor: EditorPane) -> Self {
        Problems {
            model: Rc::new(RefCell::new(DiagnosticsModel::default())),
            panel,
            editor,
            shown: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// The compiler's latest verdict: errors, warnings, or empty for a clean
    /// build. Always redraws, so the panel can tell the same problem is
    /// still there after another attempt.
    pub fn set_compile(&self, results: Vec<CompileError>) {
        self.model.borrow_mut().set_compile(results);
        self.refresh(true);
    }

    /// The language server's latest report for each file it mentioned.
    pub fn set_lsp(&self, reports: Vec<(PathBuf, Vec<CompileError>)>) {
        if reports.is_empty() {
            return;
        }
        {
            let mut model = self.model.borrow_mut();
            for (file, errors) in reports {
                model.set_lsp_file(file, errors);
            }
        }
        self.refresh(false);
    }

    pub fn clear_lsp(&self) {
        self.model.borrow_mut().clear_lsp();
        self.refresh(false);
    }

    fn refresh(&self, force: bool) {
        let merged = self.model.borrow().merged();
        let signature: Signature = merged
            .iter()
            .map(|e| {
                (
                    e.file.clone(),
                    e.line,
                    e.col,
                    e.technical.clone(),
                    e.severity == Severity::Error,
                )
            })
            .collect();
        let changed = *self.shown.borrow() != signature;
        if !force && !changed {
            return;
        }
        let had_errors = self.shown.borrow().iter().any(|s| s.4);
        *self.shown.borrow_mut() = signature;

        let errors = merged
            .iter()
            .filter(|e| e.severity == Severity::Error)
            .count() as u32;
        let notes = merged.len() as u32 - errors;

        if merged.is_empty() {
            self.editor.clear_diagnostic_marks();
        } else {
            let marks: Vec<DiagMark> = merged.iter().map(DiagMark::from).collect();
            self.editor.mark_diagnostics(&marks);
        }
        self.editor.set_diag_summary(errors, notes);

        if errors == 0 {
            self.panel.clear();
            if notes == 0 || had_errors {
                self.panel.widget().set_visible(false);
            }
            if notes > 0 {
                self.panel.show_errors(merged);
            }
        } else {
            self.panel.show_errors(merged);
            if changed {
                self.panel.widget().set_visible(true);
            }
        }
    }
}
