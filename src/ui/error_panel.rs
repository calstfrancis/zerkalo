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
    // Greedy path capture, anchored on the trailing :line:col, so a folder
    // name containing a colon doesn't truncate the path.
    LOC_RE.get_or_init(|| Regex::new(r"-->\s+(.+):(\d+):(\d+)\s*$").unwrap())
}

#[derive(Debug)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug)]
pub struct CompileError {
    pub file: PathBuf,
    pub line: u32,
    pub col: u32,
    /// A plain-language headline: what went wrong, in a sentence, with no
    /// compiler jargon. This is what the panel shows first.
    pub message: String,
    /// What to do about it, in plain language. Empty when we have nothing
    /// better to say than the headline already does.
    pub advice: String,
    /// Typst's own hints. These are frequently the single most useful part of a
    /// diagnostic ("if you meant subtraction, try adding spaces…") and were
    /// being dropped on the floor by the parser.
    pub hints: Vec<String>,
    /// The compiler's original wording, kept so the exact text is still
    /// available to copy, search for, or paste into a forum.
    pub technical: String,
    pub severity: Severity,
}

pub fn parse_typst_errors(stderr: &str, project_root: &Path) -> Vec<CompileError> {
    let mut errors: Vec<CompileError> = Vec::new();
    // The diagnostic being accumulated: its raw text, severity, and any
    // location and hints seen since. A diagnostic is only pushed once the next
    // one starts or the input ends, because its ` --> ` and ` = hint: ` lines
    // follow the `error:` line rather than preceding it.
    let mut pending: Option<(String, Severity)> = None;
    let mut loc: Option<(PathBuf, u32, u32)> = None;
    let mut hints: Vec<String> = Vec::new();

    macro_rules! flush {
        () => {
            if let Some((raw, sev)) = pending.take() {
                let (file, line, col) = loc
                    .take()
                    .unwrap_or_else(|| (project_root.to_path_buf(), 1, 1));
                errors.push(build_error(
                    file,
                    line,
                    col,
                    raw,
                    std::mem::take(&mut hints),
                    sev,
                ));
            }
            #[allow(unused_assignments)]
            {
                loc = None;
            }
            hints.clear();
        };
    }

    for line in stderr.lines() {
        let trimmed = line.trim();

        if let Some(caps) = loc_re().captures(trimmed) {
            let rel: &str = caps.get(1).map_or("", |m| m.as_str()).trim();
            let lineno: u32 = caps
                .get(2)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(1);
            let col: u32 = caps
                .get(3)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(1);
            let file = if Path::new(rel).is_absolute() {
                PathBuf::from(rel)
            } else {
                project_root.join(rel)
            };
            loc = Some((file, lineno, col));
        } else if let Some(hint) = trimmed.strip_prefix("= hint:") {
            // Typst's own suggestion. Previously matched none of the arms here
            // and was silently discarded along with the rest of the diagnostic.
            hints.push(hint.trim().to_string());
        } else if let Some(rest) = trimmed.strip_prefix("error:") {
            flush!();
            pending = Some((rest.trim().to_string(), Severity::Error));
        } else if let Some(rest) = trimmed.strip_prefix("warning:") {
            flush!();
            pending = Some((rest.trim().to_string(), Severity::Warning));
        }
    }
    flush!();

    if errors.is_empty() && !stderr.trim().is_empty() {
        let first_line = stderr
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("Compile error")
            .trim();
        let severity = if first_line.starts_with("warning:") {
            Severity::Warning
        } else {
            Severity::Error
        };
        let raw = first_line
            .trim_start_matches("warning:")
            .trim_start_matches("error:")
            .trim()
            .to_string();
        errors.push(build_error(
            project_root.to_path_buf(),
            1,
            1,
            raw,
            Vec::new(),
            severity,
        ));
    }

    // A malformed bibliography entry fails the whole file's parse, which
    // means every citation in the document also fails to resolve and reports
    // its own "label does not exist" error — one real problem masquerading
    // as dozens. Once the actual bibliography error is in the list, those are
    // pure noise: drop them so the one actionable error isn't buried.
    let bib_parse_failed = errors.iter().any(|e| {
        let t = e.technical.to_lowercase();
        t.contains("failed to parse biblatex") || t.contains("failed to parse hayagriva")
    });
    if bib_parse_failed {
        errors.retain(|e| {
            !e.technical
                .to_lowercase()
                .contains("does not exist in the document")
        });
    }

    errors
}

fn build_error(
    file: PathBuf,
    line: u32,
    col: u32,
    raw: String,
    hints: Vec<String>,
    severity: Severity,
) -> CompileError {
    let (message, advice) = humanize(&raw);
    CompileError {
        file,
        line,
        col,
        message,
        advice,
        hints,
        technical: raw,
        severity,
    }
}

/// Extracts the value after the first `:` in a Typst message, e.g.
/// `unknown variable: foo` -> `foo`.
fn subject(raw: &str) -> Option<String> {
    let v = raw.split_once(':')?.1.trim();
    if v.is_empty() {
        None
    } else {
        Some(v.trim_matches('`').to_string())
    }
}

/// Turns a Typst diagnostic into (headline, advice) in plain language.
///
/// The old version prepended an arrow-bulleted explanation to the compiler's
/// own wording, so the jargon still came first and the reader had to get past
/// "unknown variable: foo" before reaching anything they could act on. Here the
/// plain sentence *is* the message; Typst's exact text stays available under
/// "Technical detail" for searching and reporting.
///
/// Wording rules: name the thing that's wrong, in the second person, and say
/// what to do. No "invalid", no "malformed", no "expected token".
pub fn humanize(raw: &str) -> (String, String) {
    let lower = raw.to_lowercase();

    if lower.starts_with("unknown variable") {
        let name = subject(raw).unwrap_or_else(|| "that name".into());
        return (
            format!("Zerkalo doesn't know what \u{201c}{name}\u{201d} means"),
            "It's used here but never defined. Check the spelling, or add a \
             definition (#let, which creates a named value) or an import above \
             this point. If you meant to write it as ordinary text rather than \
             a command, remove the # in front of it."
                .into(),
        );
    }

    if lower.starts_with("unknown font family")
        || (lower.contains("font") && lower.contains("not found"))
    {
        let name = subject(raw).unwrap_or_else(|| "that font".into());
        return (
            format!("The font \u{201c}{name}\u{201d} isn't installed"),
            "The document will use a substitute, so the layout may look wrong. \
             Either install the font, or pick another in Template \u{2192} Body Font."
                .into(),
        );
    }

    if lower.contains("file not found") {
        // Typst's real wording is `file not found (searched at /abs/path)`, so
        // the name lives inside the parentheses rather than after a colon.
        // Show the bare filename — the absolute path it searched is noise to
        // someone who just wants to know which picture is missing.
        let name = raw
            .split_once("searched at ")
            .map(|(_, rest)| rest.trim_end_matches(')').trim())
            .map(|p| {
                Path::new(p)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| p.to_string())
            })
            .or_else(|| {
                raw.split_once("file not found")
                    .and_then(|(_, rest)| rest.split('(').next())
                    .map(|s| {
                        s.trim_matches(|c: char| c == ':' || c.is_whitespace())
                            .to_string()
                    })
            })
            .filter(|s| !s.is_empty());
        return (
            match name {
                Some(n) => format!("Zerkalo can't find the file \u{201c}{n}\u{201d}"),
                None => "Zerkalo can't find a file this document uses".into(),
            },
            "Check the name is spelled the same as the real file, and that it \
             sits in the same folder as your document (or that the path in the \
             #include or #image line matches where it actually is)."
                .into(),
        );
    }

    if lower.contains("failed to parse biblatex") || lower.contains("failed to parse hayagriva") {
        return (
            "Your bibliography file has an entry Zerkalo's compiler can't read".into(),
            "One malformed entry breaks the whole file, not just that entry \u{2014} \
             which is also why every citation in the document may be showing as \
             \u{201c}not found\u{201d} right now; they'll resolve again once this is \
             fixed. A common cause from Zotero/BetterBibTeX exports: a non-numeric \
             year like \u{201c}Winter 2001\u{201d} instead of a plain \u{201c}2001\u{201d}. \
             The line below points at the exact entry."
                .into(),
        );
    }

    if lower.contains("package not found") || lower.contains("failed to download package") {
        return (
            "A Typst package this document uses couldn't be fetched".into(),
            "Packages download the first time they're used, so this usually means \
             there was no internet connection. Reconnect and compile again."
                .into(),
        );
    }

    // Every unclosed-delimiter phrasing Typst uses, collapsed into one message.
    for (needle, thing) in [
        ("expected closing brace", "}"),
        ("expected closing bracket", "]"),
        ("expected closing paren", ")"),
        ("unclosed delimiter", ""),
    ] {
        if lower.contains(needle) {
            return (
                if thing.is_empty() {
                    "Something opened here was never closed".into()
                } else {
                    format!("A \u{201c}{thing}\u{201d} is missing")
                },
                "Every ( [ { and \" you open has to be closed again. The place \
                 marked here is where Zerkalo ran out of document still looking \
                 for the closing one."
                    .into(),
            );
        }
    }

    if lower.contains("unexpected end of file") {
        return (
            "The document ends in the middle of something".into(),
            "A bracket, parenthesis, brace or quotation mark was opened and never \
             closed, so Zerkalo reached the end still waiting for it."
                .into(),
        );
    }

    if lower.starts_with("missing argument") {
        return (
            match subject(raw) {
                Some(what) => format!("This command still needs \u{201c}{what}\u{201d}"),
                None => "This command is missing something it needs".into(),
            },
            "A command was used without one of the things it needs \u{2014} for \
             example #image() needs the name of a picture inside the brackets."
                .into(),
        );
    }

    if lower.starts_with("unexpected argument") {
        return (
            match subject(raw) {
                Some(name) => {
                    format!("\u{201c}{name}\u{201d} isn't something this command accepts")
                }
                None => "A command was given something it doesn't take".into(),
            },
            "This is usually a misspelled option name (fill: rather than colour:), \
             a missing comma between two values, or one value too many."
                .into(),
        );
    }

    if lower.contains("does not exist in the document") {
        let key = raw
            .split_once("label ")
            .and_then(|(_, rest)| rest.split_whitespace().next())
            .map(|s| {
                s.trim_matches(|c| c == '`' || c == '<' || c == '>')
                    .to_string()
            })
            .filter(|s| !s.is_empty());
        return (
            match key {
                Some(k) => format!("Nothing in the document is labelled \u{201c}{k}\u{201d}"),
                None => "A reference points at something that isn't in the document".into(),
            },
            "If this is a citation, check the key matches an entry in your .bib \
             file (your list of citation sources) exactly, and that the file is \
             attached in Settings \u{2192} Bibliography. If it's a cross-reference, \
             check the <label> it points at is really there and spelled the same."
                .into(),
        );
    }

    if lower.starts_with("expected") && lower.contains(", found ") {
        return (
            "A value here isn't the kind that was needed".into(),
            "Words meant as text usually need to be in \"quotes\", and passages of \
             document content need to be in [square brackets]. A number shouldn't \
             be in quotes."
                .into(),
        );
    }

    if lower.contains("cannot divide by zero") {
        return (
            "Something here divides by zero".into(),
            "Check the value being divided by \u{2014} it works out to zero.".into(),
        );
    }

    if lower.contains("expected string or function") {
        return (
            "A styling rule here is incomplete".into(),
            "Re-apply your style from Template to rewrite these rules, or delete \
             any half-finished #show line (a formatting rule) at this spot."
                .into(),
        );
    }

    if lower.contains("cannot access file system") {
        return (
            "This document isn't allowed to read that file".into(),
            "Typst can only read files inside your project folder. Move the file \
             in beside your document."
                .into(),
        );
    }

    // Nothing matched: keep Typst's wording as the headline rather than
    // inventing a vague one, but capitalise it so it reads as a sentence.
    let mut chars = raw.chars();
    let headline = match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Something went wrong while compiling".to_string(),
    };
    (headline, String::new())
}

/// Quick-fix patterns are written against Typst's own phrasing, so they must be
/// matched on the untranslated text — matching the plain-language headline
/// would silently retire every Fix button.
fn is_quick_fixable(err: &CompileError) -> bool {
    crate::error_patterns::match_fix(&err.technical).is_some_and(|fix| fix.fix_fn.is_some())
}

fn current_time_hhmm() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mins = (secs / 60) % 60;
    let hours = (secs / 3600) % 24;
    format!("{hours:02}:{mins:02}")
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
    last_clean_label: Label,
    live_label: Label,
    collapsed: Rc<Cell<bool>>,
    on_jump: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32)>>>>,
    on_try_fix: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32, String)>>>>,
    /// Supplies the live text of a line from the editor's buffers.
    #[allow(clippy::type_complexity)]
    source_line: Rc<RefCell<Option<Box<dyn Fn(&Path, u32) -> Option<String>>>>>,
    on_export_done: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    last_errors_key: Rc<RefCell<String>>,
    repeat_count: Rc<Cell<u32>>,
    log_lines: Rc<RefCell<Vec<String>>>,
    build_log_revealer: Revealer,
    build_log_label: Label,
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
             Or open 'Change Document Style' to reset the template.",
        ));
        stuck_label.set_visible(false);
        header.append(&stuck_label);

        // Export log button
        let export_btn = Button::from_icon_name("document-save-symbolic");
        export_btn.add_css_class("flat");
        export_btn.add_css_class("circular");
        export_btn.set_tooltip_text(Some("Save error log to file"));
        export_btn.update_property(&[gtk4::accessible::Property::Label("Save error log")]);
        header.append(&export_btn);

        // Collapse/expand chevron
        let chevron_btn = Button::from_icon_name("pan-down-symbolic");
        chevron_btn.add_css_class("flat");
        chevron_btn.add_css_class("circular");
        chevron_btn.set_tooltip_text(Some("Collapse error list"));
        chevron_btn.update_property(&[gtk4::accessible::Property::Label("Toggle error list")]);
        header.append(&chevron_btn);

        inner.append(&header);
        inner.append(&Separator::new(Orientation::Horizontal));

        // ── Search bar ───────────────────────────────────────────────────────
        let search_entry = gtk4::SearchEntry::new();
        search_entry.set_margin_start(8);
        search_entry.set_margin_end(8);
        search_entry.set_margin_top(4);
        search_entry.set_margin_bottom(4);
        search_entry.set_placeholder_text(Some("Filter errors…"));
        inner.append(&search_entry);

        // ── Error list ───────────────────────────────────────────────────────
        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::Browse);
        list_box.add_css_class("fond-list");
        list_box.set_margin_start(8);
        list_box.set_margin_end(8);
        list_box.set_margin_bottom(6);

        let scroll = ScrolledWindow::new();
        scroll.set_child(Some(&list_box));
        scroll.set_min_content_height(100);
        scroll.set_max_content_height(220);
        scroll.set_propagate_natural_height(true);

        let list_revealer = Revealer::new();
        list_revealer.set_transition_type(RevealerTransitionType::SlideDown);
        list_revealer.set_transition_duration(120);
        list_revealer.set_reveal_child(true);
        list_revealer.set_child(Some(&scroll));

        inner.append(&list_revealer);

        // ── Last-clean footer ─────────────────────────────────────────────────
        let last_clean_label = Label::new(None);
        last_clean_label.add_css_class("dim-label");
        last_clean_label.add_css_class("caption");
        last_clean_label.set_margin_start(10);
        last_clean_label.set_margin_top(2);
        last_clean_label.set_margin_bottom(4);
        last_clean_label.set_halign(Align::Start);
        last_clean_label.set_visible(false);
        inner.append(&last_clean_label);

        revealer.set_child(Some(&inner));
        root_widget.append(&revealer);

        // Visually-hidden live region for screen readers
        let live_label = Label::new(None);
        live_label.set_accessible_role(gtk4::AccessibleRole::Status);
        live_label.set_visible(false);
        root_widget.append(&live_label);

        let collapsed = Rc::new(Cell::new(false));
        let search_text: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
        let log_lines: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

        // Search filter function
        {
            let st = search_text.clone();
            list_box.set_filter_func(move |row| {
                let text = st.borrow().to_lowercase();
                if text.is_empty() {
                    return true;
                }
                let name = row.widget_name().to_string().to_lowercase();
                name.contains(&text)
            });
        }

        // Search entry changes → invalidate filter
        {
            let lb = list_box.clone();
            let st = search_text.clone();
            search_entry.connect_search_changed(move |e| {
                *st.borrow_mut() = e.text().to_string();
                lb.invalidate_filter();
            });
        }

        // Wire chevron click to collapse/expand list
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

        let on_export_done: Rc<RefCell<Option<Box<dyn Fn(String)>>>> = Rc::new(RefCell::new(None));

        // Export button: write log_lines to ~/.local/share/zerkalo/error_log.txt
        {
            let ll = log_lines.clone();
            let cb = on_export_done.clone();
            export_btn.connect_clicked(move |_| {
                let lines = ll.borrow().join("\n");
                if lines.is_empty() {
                    return;
                }
                let dir = glib::user_data_dir().join("zerkalo");
                let _ = std::fs::create_dir_all(&dir);
                let path = dir.join("error_log.txt");
                if std::fs::write(&path, &lines).is_ok() {
                    if let Some(f) = cb.borrow().as_ref() {
                        f(path.display().to_string());
                    }
                }
            });
        }

        // ── Build Log section (collapsible, shown on compile error) ─────────────
        let build_log_outer = GtkBox::new(Orientation::Vertical, 0);
        build_log_outer.append(&gtk4::Separator::new(Orientation::Horizontal));

        let log_header = GtkBox::new(Orientation::Horizontal, 6);
        log_header.set_margin_top(4);
        log_header.set_margin_bottom(4);
        log_header.set_margin_start(10);
        log_header.set_margin_end(10);
        let log_header_lbl = Label::new(Some("Build Log"));
        log_header_lbl.set_halign(Align::Start);
        log_header_lbl.set_hexpand(true);
        log_header_lbl.add_css_class("heading");
        log_header.append(&log_header_lbl);
        let log_chevron = Button::from_icon_name("pan-end-symbolic");
        log_chevron.add_css_class("flat");
        log_chevron.add_css_class("circular");
        log_chevron.set_tooltip_text(Some("Expand build log"));
        log_header.append(&log_chevron);
        build_log_outer.append(&log_header);

        let build_log_label = Label::new(None);
        build_log_label.set_halign(Align::Start);
        build_log_label.set_wrap(true);
        build_log_label.set_selectable(true);
        build_log_label.set_xalign(0.0);
        build_log_label.add_css_class("monospace");
        build_log_label.add_css_class("caption");
        build_log_label.set_margin_start(12);
        build_log_label.set_margin_end(12);
        build_log_label.set_margin_bottom(8);

        let log_scroll = gtk4::ScrolledWindow::new();
        log_scroll.set_max_content_height(160);
        log_scroll.set_propagate_natural_height(true);
        log_scroll.set_child(Some(&build_log_label));

        let build_log_revealer = Revealer::new();
        build_log_revealer.set_transition_type(RevealerTransitionType::SlideDown);
        build_log_revealer.set_transition_duration(120);
        build_log_revealer.set_reveal_child(false);
        build_log_revealer.set_child(Some(&log_scroll));
        build_log_outer.append(&build_log_revealer);

        {
            let rev = build_log_revealer.clone();
            let btn = log_chevron.clone();
            log_chevron.connect_clicked(move |_| {
                let open = !rev.reveals_child();
                rev.set_reveal_child(open);
                if open {
                    btn.set_icon_name("pan-down-symbolic");
                    btn.set_tooltip_text(Some("Collapse build log"));
                } else {
                    btn.set_icon_name("pan-end-symbolic");
                    btn.set_tooltip_text(Some("Expand build log"));
                }
            });
        }

        let build_log_revealer_outer = Revealer::new();
        build_log_revealer_outer.set_transition_type(RevealerTransitionType::SlideDown);
        build_log_revealer_outer.set_transition_duration(150);
        build_log_revealer_outer.set_reveal_child(false);
        build_log_revealer_outer.set_child(Some(&build_log_outer));
        root_widget.append(&build_log_revealer_outer);

        Self {
            root_widget,
            revealer,
            list_revealer,
            list_box,
            header_label,
            chevron_btn,
            stuck_label,
            last_clean_label,
            live_label,
            collapsed,
            on_jump: Rc::new(RefCell::new(None)),
            on_try_fix: Rc::new(RefCell::new(None)),
            source_line: Rc::new(RefCell::new(None)),
            on_export_done,
            last_errors_key: Rc::new(RefCell::new(String::new())),
            repeat_count: Rc::new(Cell::new(0)),
            log_lines,
            build_log_revealer: build_log_revealer_outer,
            build_log_label,
        }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.root_widget
    }

    pub fn set_build_log(&self, raw: &str) {
        self.build_log_label.set_text(raw);
        self.build_log_revealer.set_reveal_child(true);
    }

    pub fn set_on_jump(&self, f: impl Fn(PathBuf, u32) + 'static) {
        *self.on_jump.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_try_fix(&self, f: impl Fn(PathBuf, u32, String) + 'static) {
        *self.on_try_fix.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_source_line_provider(&self, f: impl Fn(&Path, u32) -> Option<String> + 'static) {
        *self.source_line.borrow_mut() = Some(Box::new(f));
    }

    /// Callback receives the saved file path so the caller can show a toast.
    pub fn set_on_export_done(&self, f: impl Fn(String) + 'static) {
        *self.on_export_done.borrow_mut() = Some(Box::new(f));
    }

    /// Focus the first visible error row. Returns false if no rows are present.
    pub fn grab_first_focus(&self) -> bool {
        let mut idx = 0;
        loop {
            match self.list_box.row_at_index(idx) {
                None => return false,
                Some(row) if row.is_visible() => {
                    self.list_box.select_row(Some(&row));
                    row.grab_focus();
                    return true;
                }
                Some(_) => idx += 1,
            }
        }
    }

    pub fn show_compile_errors(&self, errors: Vec<CompileError>) {
        self.show_errors_inner(errors, "Compile Errors");
    }

    pub fn show_errors(&self, errors: Vec<CompileError>) {
        self.show_errors_inner(errors, "Diagnostics");
    }

    fn show_errors_inner(&self, errors: Vec<CompileError>, section: &str) {
        self.clear_rows();

        if errors.is_empty() {
            self.revealer.set_reveal_child(false);
            self.live_label.set_text("");
            return;
        }

        // Deduplicate by (file, line, first message line)
        let mut seen: std::collections::HashSet<(PathBuf, u32, String)> = Default::default();
        let errors: Vec<CompileError> = errors
            .into_iter()
            .filter(|e| {
                let k = (
                    e.file.clone(),
                    e.line,
                    e.message.lines().next().unwrap_or("").to_string(),
                );
                seen.insert(k)
            })
            .collect();

        let count = errors.len();
        let err_count = errors
            .iter()
            .filter(|e| matches!(e.severity, Severity::Error))
            .count();
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
        self.header_label
            .set_label(&format!("{section} — {breakdown}"));

        // Trend: detect when the same errors repeat 3+ times
        let key: String = errors
            .iter()
            .map(|e| e.message.as_str())
            .collect::<Vec<_>>()
            .join("\x00");
        {
            let mut prev = self.last_errors_key.borrow_mut();
            if *prev == key {
                let n = self.repeat_count.get().saturating_add(1);
                self.repeat_count.set(n);
                self.stuck_label.set_visible(n >= 2);
            } else {
                self.repeat_count.set(0);
                self.stuck_label.set_visible(false);
                *prev = key;
            }
        }

        // Screen reader announcement
        let first_msg = errors
            .first()
            .map(|e| e.message.lines().next().unwrap_or(""))
            .unwrap_or("");
        let announcement = if count == 1 {
            format!("{section}: {first_msg}")
        } else {
            format!("{breakdown}. First: {first_msg}")
        };
        self.live_label.set_text(&announcement);

        // Build export log
        {
            let mut log = self.log_lines.borrow_mut();
            log.clear();
            log.push(format!("=== {} — {} ===", section, current_time_hhmm()));
            for e in &errors {
                let fname = e.file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                log.push(format!("  [{}:{}] {}", fname, e.line, e.message));
            }
        }

        // Single pass: insert a file header whenever the current file changes
        let multiple_files = {
            let mut files: Vec<&PathBuf> = Vec::new();
            for e in &errors {
                if !files.contains(&&e.file) {
                    files.push(&e.file);
                }
            }
            files.len() > 1
        };
        let mut last_file_path: Option<PathBuf> = None;
        for err in errors {
            if multiple_files {
                let changed = last_file_path.as_ref() != Some(&err.file);
                if changed {
                    let new_path = err.file.clone();
                    self.append_file_header(&new_path);
                    last_file_path = Some(new_path);
                }
            }
            self.append_row(err);
        }
        self.round_card_runs();

        if self.collapsed.get() {
            self.collapsed.set(false);
            self.list_revealer.set_reveal_child(true);
            self.chevron_btn.set_icon_name("pan-down-symbolic");
            self.chevron_btn
                .set_tooltip_text(Some("Collapse error list"));
        }

        self.last_clean_label.set_visible(false);
        self.revealer.set_reveal_child(true);
    }

    pub fn clear(&self) {
        let had_errors = !self.log_lines.borrow().is_empty();
        self.clear_rows();
        self.revealer.set_reveal_child(false);
        self.live_label.set_text("");
        self.stuck_label.set_visible(false);
        self.repeat_count.set(0);
        *self.last_errors_key.borrow_mut() = String::new();
        self.log_lines.borrow_mut().clear();
        self.build_log_revealer.set_reveal_child(false);
        // Show last-clean timestamp only when recovering from real errors
        if had_errors {
            self.last_clean_label
                .set_text(&format!("Last clean compile: {}", current_time_hhmm()));
            self.last_clean_label.set_visible(true);
        }
    }

    fn clear_rows(&self) {
        while let Some(row) = self.list_box.row_at_index(0) {
            self.list_box.remove(&row);
        }
    }

    /// Round the ends of each run of error rows. A file header breaks the run,
    /// so a list covering several files reads as one card per file rather than
    /// one long box with headings inside it.
    fn round_card_runs(&self) {
        let mut i = 0;
        let mut run_start: Option<gtk4::ListBoxRow> = None;
        let mut prev: Option<gtk4::ListBoxRow> = None;
        loop {
            let row = self.list_box.row_at_index(i);
            let is_card = row.as_ref().is_some_and(|r| r.has_css_class("fond-card"));
            if is_card {
                let r = row.clone().unwrap();
                if run_start.is_none() {
                    r.add_css_class("fond-card-first");
                    run_start = Some(r.clone());
                }
                prev = Some(r);
            } else {
                if let Some(last) = prev.take() {
                    last.add_css_class("fond-card-last");
                }
                run_start = None;
            }
            if row.is_none() {
                break;
            }
            i += 1;
        }
    }

    fn append_file_header(&self, file: &Path) {
        let row = ListBoxRow::new();
        row.set_activatable(false);
        row.set_selectable(false);
        row.add_css_class("fond-section");

        // A file name grouping errors beneath it is a section header, so it is
        // set like one rather than as a dim caption.
        let lbl = Label::new(Some(
            file.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
        ));
        lbl.set_halign(Align::Start);
        lbl.set_margin_start(4);
        lbl.set_margin_top(8);
        lbl.set_margin_bottom(2);
        lbl.add_css_class("fond-section-title");

        row.set_child(Some(&lbl));
        self.list_box.append(&row);
    }

    fn append_row(&self, err: CompileError) {
        let row = ListBoxRow::new();
        row.set_activatable(true);
        row.add_css_class("fond-card");
        row.add_css_class("fond-row");
        // Store the message as the widget name so the filter function can read it
        // Filtering searches the compiler's wording as well as the plain one,
        // so a user who knows the Typst term can still find the row.
        row.set_widget_name(&format!("{} {}", err.message, err.technical).to_lowercase());

        let row_box = GtkBox::new(Orientation::Horizontal, 8);
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

        // Plain-language headline first — the compiler's own wording is kept
        // below under "Technical detail" rather than leading with it.
        let msg_lbl = Label::new(Some(&err.message));
        msg_lbl.set_halign(Align::Start);
        msg_lbl.set_wrap(true);
        msg_lbl.set_xalign(0.0);
        msg_lbl.add_css_class("heading");
        text_box.append(&msg_lbl);

        if !err.advice.is_empty() {
            let advice_lbl = Label::new(Some(&err.advice));
            advice_lbl.set_halign(Align::Start);
            advice_lbl.set_xalign(0.0);
            advice_lbl.set_wrap(true);
            text_box.append(&advice_lbl);
        }

        // Typst's own hints, which the parser used to discard. They are often
        // the most specific thing anyone can say about the problem.
        for hint in &err.hints {
            let hint_lbl = Label::new(Some(&format!("\u{1f4a1} {hint}")));
            hint_lbl.set_halign(Align::Start);
            hint_lbl.set_xalign(0.0);
            hint_lbl.set_wrap(true);
            hint_lbl.add_css_class("caption");
            text_box.append(&hint_lbl);
        }

        // Source context: the offending line, taken from the open buffer when
        // there is one. Compiles run against the unsaved buffer, so reading
        // from disk quoted a line the compiler never saw whenever the document
        // had unsaved edits.
        let source_line = self
            .source_line
            .borrow()
            .as_ref()
            .and_then(|f| f(&err.file, err.line))
            .or_else(|| {
                std::fs::read_to_string(&err.file).ok().and_then(|content| {
                    content
                        .lines()
                        .nth((err.line as usize).saturating_sub(1))
                        .map(|l| l.trim().to_string())
                })
            })
            .filter(|l| !l.is_empty());
        if let Some(src) = source_line {
            let src_lbl = Label::new(Some(&format!("  {src}")));
            src_lbl.set_halign(Align::Start);
            src_lbl.set_xalign(0.0);
            src_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            src_lbl.add_css_class("monospace");
            src_lbl.add_css_class("dim-label");
            src_lbl.add_css_class("caption");
            text_box.append(&src_lbl);
        }

        // The compiler's exact words, behind a disclosure: useless to most
        // readers, indispensable to anyone searching the Typst forum. Shown
        // only when the plain-language pass actually changed the wording.
        if err.technical != err.message {
            let expander = gtk4::Expander::new(Some("Technical detail"));
            expander.add_css_class("caption");
            let tech_lbl = Label::new(Some(&err.technical));
            tech_lbl.set_halign(Align::Start);
            tech_lbl.set_xalign(0.0);
            tech_lbl.set_wrap(true);
            tech_lbl.set_selectable(true);
            tech_lbl.add_css_class("monospace");
            tech_lbl.add_css_class("caption");
            tech_lbl.add_css_class("dim-label");
            tech_lbl.set_margin_top(2);
            tech_lbl.set_margin_start(4);
            expander.set_child(Some(&tech_lbl));
            text_box.append(&expander);
        }

        let filename = err.file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        let loc_text = format!("Line {} · {}:{}", err.line, filename, err.col);
        let loc_lbl = Label::new(Some(&loc_text));
        loc_lbl.set_halign(Align::Start);
        loc_lbl.add_css_class("dim-label");
        loc_lbl.set_xalign(0.0);
        text_box.append(&loc_lbl);

        row_box.append(&text_box);

        // Action buttons
        let btn_box = GtkBox::new(Orientation::Horizontal, 4);
        btn_box.set_valign(Align::Center);

        // Copy button
        let copy_btn = Button::from_icon_name("edit-copy-symbolic");
        copy_btn.add_css_class("flat");
        copy_btn.add_css_class("circular");
        copy_btn.set_tooltip_text(Some("Copy error message"));
        copy_btn.update_property(&[gtk4::accessible::Property::Label("Copy error message")]);
        {
            // Copies the compiler's exact wording alongside the plain one —
            // the technical text is what's worth pasting into a search.
            let msg_c = err.message.clone();
            let tech_c = err.technical.clone();
            let hints_c = err.hints.clone();
            let loc_c = loc_text.clone();
            copy_btn.connect_clicked(move |btn| {
                let mut out = format!("{msg_c}\n{loc_c}\n\n{tech_c}");
                for h in &hints_c {
                    out.push_str(&format!("\nhint: {h}"));
                }
                btn.clipboard().set_text(&out);
            });
        }
        btn_box.append(&copy_btn);

        // Jump button
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

        // Try-Fix button
        if is_quick_fixable(&err) {
            let fix_btn = Button::with_label("Fix");
            fix_btn.add_css_class("flat");
            fix_btn.set_tooltip_text(Some("Attempt automatic fix (undoable with Ctrl+Z)"));
            fix_btn.update_property(&[gtk4::accessible::Property::Label("Attempt automatic fix")]);
            let on_fix = self.on_try_fix.clone();
            let file_f = err.file.clone();
            let line_f = err.line;
            let msg_f = err.technical.clone();
            fix_btn.connect_clicked(move |_| {
                if let Some(f) = on_fix.borrow().as_ref() {
                    f(file_f.clone(), line_f, msg_f.clone());
                }
            });
            btn_box.append(&fix_btn);
        }

        row_box.append(&btn_box);
        row.set_child(Some(&row_box));

        // Row activation (Enter key) → jump to error
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Vec<CompileError> {
        parse_typst_errors(text, Path::new("/project"))
    }

    #[test]
    fn a_diagnostic_keeps_the_line_the_compiler_reported() {
        let errs = parse("error: unknown variable: foo\n --> /project/main.typ:12:5");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line, 12);
        assert_eq!(errs[0].col, 5);
        assert_eq!(errs[0].file, PathBuf::from("/project/main.typ"));
    }

    #[test]
    fn typst_hints_are_kept_rather_than_discarded() {
        // The parser recognised only `error:`, `warning:` and ` --> ` lines, so
        // every `= hint:` line — often the most useful part of the diagnostic —
        // fell through every arm and was dropped.
        let errs = parse(
            "error: unknown variable: no-such\n \
             --> /project/main.typ:5:1\n   \
             = hint: if you meant subtraction, try adding spaces around the minus sign",
        );
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].hints.len(),
            1,
            "hint should survive: {:?}",
            errs[0].hints
        );
        assert!(errs[0].hints[0].contains("subtraction"));
    }

    #[test]
    fn each_diagnostic_keeps_its_own_location_and_hints() {
        // Two diagnostics in one run must not pool their locations: the second
        // error's line has to stay with the second error.
        let errs = parse(
            "error: first problem\n \
             --> /project/a.typ:3:1\n   \
             = hint: hint for the first\n\
             error: second problem\n \
             --> /project/b.typ:99:2",
        );
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].line, 3);
        assert_eq!(errs[0].hints.len(), 1);
        assert_eq!(errs[1].line, 99);
        assert_eq!(errs[1].file, PathBuf::from("/project/b.typ"));
        assert!(
            errs[1].hints.is_empty(),
            "the first error's hint must not leak forward"
        );
    }

    #[test]
    fn a_diagnostic_with_no_location_still_reports_once() {
        let errs = parse("warning: something vague");
        assert_eq!(errs.len(), 1);
        assert!(matches!(errs[0].severity, Severity::Warning));
        assert_eq!(errs[0].line, 1);
    }

    #[test]
    fn a_relative_path_is_resolved_against_the_project_root() {
        let errs = parse("error: boom\n --> chapters/one.typ:7:1");
        assert_eq!(errs[0].file, PathBuf::from("/project/chapters/one.typ"));
    }

    // ── Plain language ───────────────────────────────────────────────────────

    #[test]
    fn an_undefined_name_is_explained_without_jargon() {
        let (headline, advice) = humanize("unknown variable: foo");
        assert!(headline.contains("foo"), "should name it: {headline}");
        assert!(
            !headline.to_lowercase().contains("variable"),
            "headline should not use compiler jargon: {headline}"
        );
        assert!(
            advice.contains('#'),
            "advice should say what to do: {advice}"
        );
    }

    #[test]
    fn a_missing_file_names_the_file() {
        // Typst's actual phrasing puts the path in parentheses after
        // "searched at", and gives it absolute. The reader wants the filename.
        let (headline, _) =
            humanize("file not found (searched at /home/me/docs/missing-picture.png)");
        assert!(headline.contains("missing-picture.png"), "got: {headline}");
        assert!(
            !headline.contains("/home/me"),
            "path noise leaked in: {headline}"
        );

        let (headline, _) = humanize("file not found: figures/plot.png");
        assert!(headline.contains("plot.png"), "got: {headline}");
    }

    #[test]
    fn a_wrong_option_name_is_quoted_in_the_headline() {
        let (headline, advice) = humanize("unexpected argument: colour");
        assert!(headline.contains("colour"), "should name it: {headline}");
        assert!(
            advice.contains("fill:"),
            "should suggest the real one: {advice}"
        );
    }

    #[test]
    fn a_missing_argument_is_named() {
        let (headline, _) = humanize("missing argument: body");
        assert!(headline.contains("body"), "got: {headline}");
    }

    #[test]
    fn every_unclosed_delimiter_phrasing_gets_the_same_explanation() {
        for raw in [
            "expected closing brace",
            "expected closing bracket",
            "expected closing paren",
        ] {
            let (headline, advice) = humanize(raw);
            assert!(!advice.is_empty(), "{raw} should carry advice");
            assert!(
                !headline.to_lowercase().contains("expected"),
                "{raw} still reads like a parser message: {headline}"
            );
        }
    }

    #[test]
    fn a_malformed_bibliography_entry_gets_a_plain_language_explanation() {
        let (headline, advice) = humanize("failed to parse BibLaTeX (wrong number of digits)");
        assert!(
            headline.to_lowercase().contains("bibliography"),
            "got: {headline}"
        );
        assert!(
            advice.contains("year"),
            "should mention the common Zotero cause: {advice}"
        );
    }

    #[test]
    fn a_malformed_bibliography_entry_suppresses_the_resulting_label_error_flood() {
        // One bad .bib entry fails the whole file's parse, so every @citation
        // in the document also reports its own "label does not exist" —
        // dozens of errors from one real problem. Only the actionable one
        // should survive.
        let errs = parse(
            "error: failed to parse BibLaTeX (wrong number of digits)\n \
             --> /project/refs.bib:42:11\n\
             error: label `<key1>` does not exist in the document\n \
             --> /project/main.typ:5:1\n\
             error: label `<key2>` does not exist in the document\n \
             --> /project/main.typ:9:1",
        );
        assert_eq!(
            errs.len(),
            1,
            "the label errors should be dropped as noise: {errs:?}"
        );
        assert!(errs[0].technical.to_lowercase().contains("biblatex"));
    }

    #[test]
    fn a_label_error_with_no_bibliography_failure_present_is_kept() {
        // Only suppress label errors when they're a known consequence of a
        // bibliography parse failure — a genuine broken cross-reference with
        // no bib error alongside it must still be reported.
        let errs = parse("error: label `<fig:missing>` does not exist in the document\n --> /project/main.typ:5:1");
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn an_unrecognised_message_is_kept_verbatim_but_capitalised() {
        // Better to show Typst's exact words than to invent a vague headline
        // that tells the reader nothing.
        let (headline, advice) = humanize("some completely novel error text");
        assert_eq!(headline, "Some completely novel error text");
        assert!(advice.is_empty());
    }

    #[test]
    fn a_path_containing_a_colon_is_not_truncated() {
        let errs = parse("error: boom\n --> /project/odd:name/one.typ:7:2");
        assert_eq!(errs[0].file, PathBuf::from("/project/odd:name/one.typ"));
        assert_eq!(errs[0].line, 7);
    }

    #[test]
    fn quick_fixes_still_match_after_the_message_is_rewritten() {
        // error_patterns matches Typst's phrasing ("expected closing brace").
        // Once the headline became plain language it no longer contained those
        // words, so matching on it would have quietly removed every Fix button.
        let errs = parse("error: expected closing brace\n --> /project/main.typ:4:1");
        assert!(
            is_quick_fixable(&errs[0]),
            "should still offer a fix; headline is now {:?}",
            errs[0].message
        );
    }

    #[test]
    fn the_technical_wording_is_preserved_for_searching() {
        let errs = parse("error: unknown variable: foo\n --> /project/main.typ:2:1");
        assert_eq!(errs[0].technical, "unknown variable: foo");
        assert_ne!(
            errs[0].message, errs[0].technical,
            "headline should be rewritten"
        );
    }
}
