use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Label, ListBox, ListBoxRow, Orientation, Revealer,
    RevealerTransitionType, ScrolledWindow, SelectionMode, Separator,
};
use regex::Regex;

// ── Error parsing ─────────────────────────────────────────────────────────────

static LOC_RE: OnceLock<Regex> = OnceLock::new();

fn loc_re() -> &'static Regex {
    LOC_RE.get_or_init(|| Regex::new(r"-->\s+([^:]+):(\d+):(\d+)").unwrap())
}

pub enum Severity {
    Error,
    Warning,
}

pub struct CompileError {
    pub file: PathBuf,
    pub line: u32,
    pub col: u32,
    pub message: String,
    pub severity: Severity,
}

/// Parse typst stderr into a list of located errors.
///
/// Typst stderr format (simplified):
///   error: <message>
///    --> path/to/file.typ:line:col
pub fn parse_typst_errors(stderr: &str, project_root: &Path) -> Vec<CompileError> {
    let mut errors: Vec<CompileError> = Vec::new();
    let mut current_msg: Option<(String, Severity)> = None;

    for line in stderr.lines() {
        let trimmed = line.trim();

        if let Some(caps) = loc_re().captures(trimmed) {
            let rel: &str = caps.get(1).map_or("", |m| m.as_str()).trim();
            let lineno: u32 = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
            let col: u32 = caps.get(3).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);

            let file = if Path::new(rel).is_absolute() {
                PathBuf::from(rel)
            } else {
                project_root.join(rel)
            };

            if let Some((msg, sev)) = current_msg.take() {
                errors.push(CompileError {
                    file,
                    line: lineno,
                    col,
                    message: msg,
                    severity: sev,
                });
            } else {
                errors.push(CompileError {
                    file,
                    line: lineno,
                    col,
                    message: "Compile error".into(),
                    severity: Severity::Error,
                });
            }
        } else if trimmed.starts_with("error:") {
            let raw = trimmed.trim_start_matches("error:").trim().to_string();
            let msg = enrich_error_message(&raw);
            current_msg = Some((msg, Severity::Error));
        } else if trimmed.starts_with("warning:") {
            let msg = trimmed.trim_start_matches("warning:").trim().to_string();
            current_msg = Some((msg, Severity::Warning));
        }
    }

    // Also consume any trailing hint lines (= hint: …) and attach to the last error
    // (already parsed above, but make sure hints become part of the message)

    // Any trailing message with no location — include the full text, not just first line
    if errors.is_empty() && !stderr.trim().is_empty() {
        let first_msg = stderr.lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("Compile error")
            .trim()
            .to_string();
        errors.push(CompileError {
            file: project_root.to_path_buf(),
            line: 1,
            col: 1,
            message: enrich_error_message(&first_msg),
            severity: Severity::Error,
        });
    }

    errors
}

// ── Error enrichment ─────────────────────────────────────────────────────────
// Each pattern below maps a raw Typst error message to a plain-English explanation
// suitable for a writer who may not know Typst internals.

fn enrich_error_message(msg: &str) -> String {
    // Citation key not found in bibliography
    if msg.contains("does not exist in the document") && (msg.contains('<') || msg.contains('@')) {
        return format!(
            "{msg}\n\
             → The bibliography key was not found. Check that:\n\
             \x20 1. Your .bib file is referenced: #bibliography(\"refs.bib\")\n\
             \x20 2. The .bib file is in the same folder as your .typ file\n\
             \x20 3. The citation key spelling matches the .bib entry exactly"
        );
    }

    // Show rule type error — "expected string or function, found none/something"
    if msg.contains("expected string or function") {
        return format!(
            "{msg}\n\
             → A #show rule has an invalid or missing body. This sometimes happens\n\
             \x20 when Zerkalo updates heading styles. Try:\n\
             \x20 1. Open 'Update Template Settings' and re-apply your chosen style\n\
             \x20 2. Or manually delete any incomplete '#show heading:' lines in your document"
        );
    }

    // File not found
    if msg.contains("file not found") || msg.contains("not found") && msg.contains(".typ") {
        return format!(
            "{msg}\n\
             → A file your document includes could not be found. Check that all\n\
             \x20 #include \"…\" and #import \"…\" paths are correct and the files exist."
        );
    }

    // Package not found
    if msg.contains("package not found") || msg.contains("@preview/") && msg.contains("not") {
        return format!(
            "{msg}\n\
             → A Typst package is missing from the local cache. Packages are\n\
             \x20 downloaded on first use; try compiling again while online.\n\
             \x20 Cached packages live in: ~/.cache/typst/packages/"
        );
    }

    // Unexpected token / unexpected end of file
    if msg.contains("unexpected end of file") || msg.contains("unexpected token") {
        return format!(
            "{msg}\n\
             → The document has a syntax error — usually a missing closing bracket,\n\
             \x20 parenthesis, or quote. Check the line shown for an unclosed delimiter."
        );
    }

    // Variable/function not found
    if msg.contains("unknown variable") || (msg.contains("not found in") && msg.contains("scope")) {
        return format!(
            "{msg}\n\
             → A variable or function is used but not defined. Make sure any\n\
             \x20 #let definitions or #import statements appear before their first use."
        );
    }

    // Font not found — common for GOST type B or other custom fonts
    if msg.to_lowercase().contains("font") && (msg.contains("not found") || msg.contains("missing")) {
        return format!(
            "{msg}\n\
             → A font used in the document is not installed. Either install the font\n\
             \x20 or change the font in 'Update Template Settings' (Layout → Body Font)."
        );
    }

    msg.to_string()
}

// ── Widget ───────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ErrorPanel {
    root_widget: GtkBox,
    revealer: Revealer,
    list_box: ListBox,
    header_label: Label,
    on_jump: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32)>>>>,
}

impl ErrorPanel {
    pub fn new() -> Self {
        let root_widget = GtkBox::new(Orientation::Vertical, 0);
        root_widget.set_hexpand(true);
        root_widget.set_vexpand(false);

        let revealer = Revealer::new();
        revealer.set_transition_type(RevealerTransitionType::SlideDown);
        revealer.set_transition_duration(150);
        revealer.set_reveal_child(false);

        let inner = GtkBox::new(Orientation::Vertical, 0);

        root_widget.append(&Separator::new(Orientation::Horizontal));

        // ── Header bar ───────────────────────────────────────────────────────
        let header = GtkBox::new(Orientation::Horizontal, 6);
        header.set_margin_top(4);
        header.set_margin_bottom(4);
        header.set_margin_start(10);
        header.set_margin_end(10);

        let header_label = Label::new(Some("Errors"));
        header_label.set_halign(Align::Start);
        header_label.set_hexpand(true);
        header_label.add_css_class("heading");
        header.append(&header_label);

        inner.append(&header);
        inner.append(&Separator::new(Orientation::Horizontal));

        // ── Error list ───────────────────────────────────────────────────────
        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::None);
        list_box.add_css_class("boxed-list");

        let scroll = ScrolledWindow::new();
        scroll.set_child(Some(&list_box));
        scroll.set_min_content_height(120);
        scroll.set_max_content_height(220);
        scroll.set_propagate_natural_height(true);
        inner.append(&scroll);

        revealer.set_child(Some(&inner));
        root_widget.append(&revealer);

        Self {
            root_widget,
            revealer,
            list_box,
            header_label,
            on_jump: Rc::new(RefCell::new(None)),
        }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.root_widget
    }

    pub fn set_on_jump(&self, f: impl Fn(PathBuf, u32) + 'static) {
        *self.on_jump.borrow_mut() = Some(Box::new(f));
    }

    pub fn show_errors(&self, errors: Vec<CompileError>) {
        self.clear_rows();

        let count = errors.len();
        if count == 0 {
            self.revealer.set_reveal_child(false);
            return;
        }

        let label = if count == 1 {
            "1 Error".to_string()
        } else {
            format!("{count} Errors")
        };
        self.header_label.set_label(&label);

        for err in errors {
            self.append_row(err);
        }

        self.revealer.set_reveal_child(true);
    }

    pub fn clear(&self) {
        self.clear_rows();
        self.revealer.set_reveal_child(false);
    }

    fn clear_rows(&self) {
        while let Some(row) = self.list_box.row_at_index(0) {
            self.list_box.remove(&row);
        }
    }

    fn append_row(&self, err: CompileError) {
        let row = ListBoxRow::new();
        row.set_activatable(true);

        let row_box = GtkBox::new(Orientation::Horizontal, 8);
        row_box.set_margin_top(6);
        row_box.set_margin_bottom(6);
        row_box.set_margin_start(10);
        row_box.set_margin_end(10);

        // Severity icon
        let icon_lbl = match err.severity {
            Severity::Error => {
                let l = Label::new(Some("✗"));
                l.add_css_class("error");
                l
            }
            Severity::Warning => {
                let l = Label::new(Some("⚠"));
                l.add_css_class("warning");
                l
            }
        };
        icon_lbl.set_valign(Align::Start);
        row_box.append(&icon_lbl);

        // Message + location
        let text_box = GtkBox::new(Orientation::Vertical, 2);

        let msg_lbl = Label::new(Some(&err.message));
        msg_lbl.set_halign(Align::Start);
        msg_lbl.set_wrap(true);
        msg_lbl.set_xalign(0.0);
        text_box.append(&msg_lbl);

        let filename = err
            .file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?");
        let loc_text = format!("{}:{}:{}", filename, err.line, err.col);
        let loc_lbl = Label::new(Some(&loc_text));
        loc_lbl.set_halign(Align::Start);
        loc_lbl.add_css_class("dim-label");
        loc_lbl.set_xalign(0.0);
        text_box.append(&loc_lbl);

        row_box.append(&text_box);
        row.set_child(Some(&row_box));

        let on_jump = self.on_jump.clone();
        let file = err.file.clone();
        let line = err.line;
        row.connect_activate(move |_| {
            if let Some(f) = on_jump.borrow().as_ref() {
                f(file.clone(), line);
            }
        });

        self.list_box.append(&row);
    }
}
