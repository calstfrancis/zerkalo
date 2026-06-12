use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Label, ListBox, ListBoxRow, Orientation, Revealer,
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
                errors.push(CompileError { file, line: lineno, col, message: msg, severity: sev });
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

fn enrich_error_message(msg: &str) -> String {
    if msg.contains("does not exist in the document") && (msg.contains('<') || msg.contains('@')) {
        return format!(
            "{msg}\n\
             → The bibliography key was not found. Check that:\n\
             \x20 1. Your .bib file is referenced: #bibliography(\"refs.bib\")\n\
             \x20 2. The .bib file is in the same folder as your .typ file\n\
             \x20 3. The citation key spelling matches the .bib entry exactly"
        );
    }
    if msg.contains("expected string or function") {
        return format!(
            "{msg}\n\
             → A #show rule has an invalid or missing body. This sometimes happens\n\
             \x20 when Zerkalo updates heading styles. Try:\n\
             \x20 1. Open 'Update Template Settings' and re-apply your chosen style\n\
             \x20 2. Or manually delete any incomplete '#show heading:' lines in your document"
        );
    }
    if msg.contains("file not found") || msg.contains("not found") && msg.contains(".typ") {
        return format!(
            "{msg}\n\
             → A file your document includes could not be found. Check that all\n\
             \x20 #include \"…\" and #import \"…\" paths are correct and the files exist."
        );
    }
    if msg.contains("package not found") || msg.contains("@preview/") && msg.contains("not") {
        return format!(
            "{msg}\n\
             → A Typst package is missing from the local cache. Packages are\n\
             \x20 downloaded on first use; try compiling again while online.\n\
             \x20 Cached packages live in: ~/.cache/typst/packages/"
        );
    }
    if msg.contains("unexpected end of file") || msg.contains("unexpected token") {
        return format!(
            "{msg}\n\
             → The document has a syntax error — usually a missing closing bracket,\n\
             \x20 parenthesis, or quote. Check the line shown for an unclosed delimiter."
        );
    }
    if msg.contains("unknown variable") || (msg.contains("not found in") && msg.contains("scope")) {
        return format!(
            "{msg}\n\
             → A variable or function is used but not defined. Make sure any\n\
             \x20 #let definitions or #import statements appear before their first use."
        );
    }
    if msg.to_lowercase().contains("font") && (msg.contains("not found") || msg.contains("missing")) {
        return format!(
            "{msg}\n\
             → A font used in the document is not installed. Either install the font\n\
             \x20 or change the font in 'Update Template Settings' (Layout → Body Font)."
        );
    }
    msg.to_string()
}

// Returns true if we have an automatic fix for this error pattern.
fn is_quick_fixable(err: &CompileError) -> bool {
    err.message.contains("unexpected end of file")
}

// ── Widget ───────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ErrorPanel {
    root_widget: GtkBox,
    revealer: Revealer,
    list_revealer: Revealer,
    list_box: ListBox,
    header_label: Label,
    chevron_btn: Button,
    stuck_label: Label,
    live_label: Label,
    collapsed: Rc<Cell<bool>>,
    on_jump: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32)>>>>,
    on_try_fix: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32)>>>>,
    last_errors_key: Rc<RefCell<String>>,
    repeat_count: Rc<Cell<u32>>,
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

        // "Stuck?" badge — shown after 3 consecutive identical error sets
        let stuck_label = Label::new(Some("Stuck?"));
        stuck_label.add_css_class("dim-label");
        stuck_label.add_css_class("caption");
        stuck_label.set_tooltip_text(Some(
            "Same error for 3+ compiles in a row.\n\
             Tip: check for a missing closing bracket, parenthesis, or quote.\n\
             Or open 'Update Template Settings' to reset the template."
        ));
        stuck_label.set_visible(false);
        header.append(&stuck_label);

        // Collapse/expand chevron
        let chevron_btn = Button::from_icon_name("pan-down-symbolic");
        chevron_btn.add_css_class("flat");
        chevron_btn.add_css_class("circular");
        chevron_btn.set_tooltip_text(Some("Collapse error list"));
        chevron_btn.update_property(&[gtk4::accessible::Property::Label("Toggle error list")]);
        header.append(&chevron_btn);

        inner.append(&header);
        inner.append(&Separator::new(Orientation::Horizontal));

        // ── Error list ───────────────────────────────────────────────────────
        let list_box = ListBox::new();
        // Browse mode so rows receive keyboard focus and can be activated with Enter
        list_box.set_selection_mode(SelectionMode::Browse);
        list_box.add_css_class("boxed-list");

        let scroll = ScrolledWindow::new();
        scroll.set_child(Some(&list_box));
        scroll.set_min_content_height(120);
        scroll.set_max_content_height(220);
        scroll.set_propagate_natural_height(true);

        let list_revealer = Revealer::new();
        list_revealer.set_transition_type(RevealerTransitionType::SlideDown);
        list_revealer.set_transition_duration(120);
        list_revealer.set_reveal_child(true);
        list_revealer.set_child(Some(&scroll));

        inner.append(&list_revealer);

        revealer.set_child(Some(&inner));
        root_widget.append(&revealer);

        // Visually-hidden live region for screen readers
        let live_label = Label::new(None);
        live_label.set_accessible_role(gtk4::AccessibleRole::Status);
        live_label.set_visible(false);
        root_widget.append(&live_label);

        let collapsed = Rc::new(Cell::new(false));

        // Wire chevron click to toggle list visibility
        {
            let list_rev_c = list_revealer.clone();
            let chevron_c = chevron_btn.clone();
            let collapsed_c = collapsed.clone();
            chevron_btn.connect_clicked(move |_| {
                let now_collapsed = !collapsed_c.get();
                collapsed_c.set(now_collapsed);
                list_rev_c.set_reveal_child(!now_collapsed);
                if now_collapsed {
                    chevron_c.set_icon_name("pan-end-symbolic");
                    chevron_c.set_tooltip_text(Some("Expand error list"));
                } else {
                    chevron_c.set_icon_name("pan-down-symbolic");
                    chevron_c.set_tooltip_text(Some("Collapse error list"));
                }
            });
        }

        Self {
            root_widget,
            revealer,
            list_revealer,
            list_box,
            header_label,
            chevron_btn,
            stuck_label,
            live_label,
            collapsed,
            on_jump: Rc::new(RefCell::new(None)),
            on_try_fix: Rc::new(RefCell::new(None)),
            last_errors_key: Rc::new(RefCell::new(String::new())),
            repeat_count: Rc::new(Cell::new(0)),
        }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.root_widget
    }

    pub fn set_on_jump(&self, f: impl Fn(PathBuf, u32) + 'static) {
        *self.on_jump.borrow_mut() = Some(Box::new(f));
    }

    /// Callback for the Try-Fix button; receives (file, line) so the caller
    /// can read the source and apply the fix using `begin/end_user_action`.
    pub fn set_on_try_fix(&self, f: impl Fn(PathBuf, u32) + 'static) {
        *self.on_try_fix.borrow_mut() = Some(Box::new(f));
    }

    /// Show errors from the compiler.
    pub fn show_compile_errors(&self, errors: Vec<CompileError>) {
        self.show_errors_inner(errors, "Compile Errors");
    }

    /// Show errors from LSP diagnostics.
    pub fn show_errors(&self, errors: Vec<CompileError>) {
        self.show_errors_inner(errors, "Diagnostics");
    }

    fn show_errors_inner(&self, errors: Vec<CompileError>, section: &str) {
        self.clear_rows();

        let count = errors.len();
        if count == 0 {
            self.revealer.set_reveal_child(false);
            self.live_label.set_text("");
            return;
        }

        // Breakdown header: "Compile Errors — 2 errors, 1 warning"
        let err_count = errors.iter().filter(|e| matches!(e.severity, Severity::Error)).count();
        let warn_count = count - err_count;
        let breakdown = match (err_count, warn_count) {
            (e, 0) => format!("{e} error{}", if e == 1 { "" } else { "s" }),
            (0, w) => format!("{w} warning{}", if w == 1 { "" } else { "s" }),
            (e, w) => format!(
                "{e} error{}, {w} warning{}",
                if e == 1 { "" } else { "s" },
                if w == 1 { "" } else { "s" }
            ),
        };
        self.header_label.set_label(&format!("{section} — {breakdown}"));

        // Trend: detect when the same errors repeat 3+ times (user is stuck)
        let key: String = errors.iter().map(|e| e.message.as_str()).collect::<Vec<_>>().join("\x00");
        {
            let mut prev = self.last_errors_key.borrow_mut();
            if *prev == key {
                let n = self.repeat_count.get().saturating_add(1);
                self.repeat_count.set(n);
                // Show "Stuck?" after 2 repeats (i.e. 3rd occurrence)
                self.stuck_label.set_visible(n >= 2);
            } else {
                self.repeat_count.set(0);
                self.stuck_label.set_visible(false);
                *prev = key;
            }
        }

        // Screen reader announcement
        let first_msg = errors.first()
            .map(|e| e.message.lines().next().unwrap_or(""))
            .unwrap_or("");
        let announcement = if count == 1 {
            format!("{section}: {first_msg}")
        } else {
            format!("{breakdown}. First: {first_msg}")
        };
        self.live_label.set_text(&announcement);

        for err in errors {
            self.append_row(err);
        }

        // Auto-expand if the panel was collapsed when new errors arrive
        if self.collapsed.get() {
            self.collapsed.set(false);
            self.list_revealer.set_reveal_child(true);
            self.chevron_btn.set_icon_name("pan-down-symbolic");
            self.chevron_btn.set_tooltip_text(Some("Collapse error list"));
        }

        self.revealer.set_reveal_child(true);
    }

    pub fn clear(&self) {
        self.clear_rows();
        self.revealer.set_reveal_child(false);
        self.live_label.set_text("");
        self.stuck_label.set_visible(false);
        self.repeat_count.set(0);
        *self.last_errors_key.borrow_mut() = String::new();
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
                l.update_property(&[gtk4::accessible::Property::Label("Compile error")]);
                l
            }
            Severity::Warning => {
                let l = Label::new(Some("⚠"));
                l.update_property(&[gtk4::accessible::Property::Label("Compile warning")]);
                l.add_css_class("warning");
                l
            }
        };
        icon_lbl.set_valign(Align::Start);
        row_box.append(&icon_lbl);

        // Message text column
        let text_box = GtkBox::new(Orientation::Vertical, 2);
        text_box.set_hexpand(true);

        // First line of message as the title
        let first_line = err.message.lines().next().unwrap_or(&err.message).to_string();
        let msg_lbl = Label::new(Some(&first_line));
        msg_lbl.set_halign(Align::Start);
        msg_lbl.set_wrap(true);
        msg_lbl.set_xalign(0.0);
        text_box.append(&msg_lbl);

        // Enrichment hint lines shown below, dimmed
        if err.message.contains('\n') {
            let hint: String = err.message.lines().skip(1).collect::<Vec<_>>().join("\n");
            let hint_lbl = Label::new(Some(&hint));
            hint_lbl.set_halign(Align::Start);
            hint_lbl.set_xalign(0.0);
            hint_lbl.set_wrap(true);
            hint_lbl.add_css_class("dim-label");
            hint_lbl.add_css_class("caption");
            text_box.append(&hint_lbl);
        }

        let filename = err.file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        let loc_text = format!("Line {} · {}:{}", err.line, filename, err.col);
        let loc_lbl = Label::new(Some(&loc_text));
        loc_lbl.set_halign(Align::Start);
        loc_lbl.add_css_class("dim-label");
        loc_lbl.set_xalign(0.0);
        text_box.append(&loc_lbl);

        row_box.append(&text_box);

        // Action buttons (right side, vertically centred)
        let btn_box = GtkBox::new(Orientation::Horizontal, 4);
        btn_box.set_valign(Align::Center);

        // Copy button — copies "message\nlocation" to clipboard
        let copy_btn = Button::from_icon_name("edit-copy-symbolic");
        copy_btn.add_css_class("flat");
        copy_btn.add_css_class("circular");
        copy_btn.set_tooltip_text(Some("Copy error message"));
        copy_btn.update_property(&[gtk4::accessible::Property::Label("Copy error message")]);
        {
            let msg_c = err.message.clone();
            let loc_c = loc_text.clone();
            copy_btn.connect_clicked(move |btn| {
                let text = format!("{}\n{}", msg_c, loc_c);
                btn.clipboard().set_text(&text);
            });
        }
        btn_box.append(&copy_btn);

        // Jump button — explicit alternative to row activation
        let jump_btn = Button::from_icon_name("go-jump-symbolic");
        jump_btn.add_css_class("flat");
        jump_btn.add_css_class("circular");
        jump_btn.set_tooltip_text(Some("Jump to error in editor"));
        jump_btn.update_property(&[gtk4::accessible::Property::Label("Jump to error in editor")]);
        {
            let on_jump_j = self.on_jump.clone();
            let file_j = err.file.clone();
            let line_j = err.line;
            jump_btn.connect_clicked(move |_| {
                if let Some(f) = on_jump_j.borrow().as_ref() {
                    f(file_j.clone(), line_j);
                }
            });
        }
        btn_box.append(&jump_btn);

        // Try-Fix button — only for patterns we recognise as auto-fixable
        if is_quick_fixable(&err) {
            let fix_btn = Button::with_label("Fix");
            fix_btn.add_css_class("flat");
            fix_btn.set_tooltip_text(Some("Attempt automatic fix (undoable with Ctrl+Z)"));
            fix_btn.update_property(&[gtk4::accessible::Property::Label("Attempt automatic fix")]);
            let on_fix = self.on_try_fix.clone();
            let file_f = err.file.clone();
            let line_f = err.line;
            fix_btn.connect_clicked(move |_| {
                if let Some(f) = on_fix.borrow().as_ref() {
                    f(file_f.clone(), line_f);
                }
            });
            btn_box.append(&fix_btn);
        }

        row_box.append(&btn_box);
        row.set_child(Some(&row_box));

        // Row activation via Enter key or click → jump to error
        {
            let on_jump = self.on_jump.clone();
            let file = err.file.clone();
            let line = err.line;
            row.connect_activate(move |_| {
                if let Some(f) = on_jump.borrow().as_ref() {
                    f(file.clone(), line);
                }
            });
        }

        self.list_box.append(&row);
    }
}
