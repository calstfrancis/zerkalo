use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GtkBox, Button, Image, Label, ListBox, ListBoxRow, MenuButton, Orientation,
    Popover, Revealer, RevealerTransitionType, ScrolledWindow, SelectionMode, Separator,
    TextBuffer, TextTag, TextView, WrapMode,
};
use regex::Regex;

use super::diagnostics_model::DiagMark;
use super::last_working::{self, Change, LastWorking};
use crate::diagnostic_catalog::{self, Kind};

// ── Error parsing ─────────────────────────────────────────────────────────────

static LOC_RE: OnceLock<Regex> = OnceLock::new();

fn loc_re() -> &'static Regex {
    // Greedy path capture, anchored on the trailing :line:col, so a folder
    // name containing a colon doesn't truncate the path.
    LOC_RE.get_or_init(|| Regex::new(r"-->\s+(.+):(\d+):(\d+)\s*$").unwrap())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct CompileError {
    pub file: PathBuf,
    pub line: u32,
    pub col: u32,
    /// Exclusive end of the offending text (1-based line, character column).
    /// Equal to the start when the source gave no extent.
    pub end_line: u32,
    pub end_col: u32,
    /// Set when the problem is really inside a package or template: the
    /// package's name/version. `file`/`line` then point at the user's own line
    /// that led there.
    pub origin: Option<String>,
    /// A plain-language headline: what went wrong, in a sentence, with no
    /// compiler jargon. This is what the panel shows first.
    pub message: String,
    /// What to do about it, in plain language. Empty when we have nothing
    /// better to say than the headline already does.
    pub advice: String,
    /// What sort of problem this is. Fixes are keyed on it, not on the wording.
    pub kind: Kind,
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
    let mut end: Option<(u32, u32)> = None;
    let mut origin: Option<String> = None;
    let mut at: Option<String> = None;
    let mut hints: Vec<String> = Vec::new();

    macro_rules! flush {
        () => {
            if let Some((raw, sev)) = pending.take() {
                let (file, line, col) = loc
                    .take()
                    .unwrap_or_else(|| (project_root.to_path_buf(), 1, 1));
                let mut e = build_error(
                    file,
                    line,
                    col,
                    raw,
                    at.take().as_deref(),
                    std::mem::take(&mut hints),
                    sev,
                );
                if let Some((el, ec)) = end.take() {
                    e.end_line = el;
                    e.end_col = ec;
                }
                e.origin = origin.take();
                errors.push(e);
            }
            #[allow(unused_assignments)]
            {
                loc = None;
                end = None;
                origin = None;
                at = None;
            }
            hints.clear();
        };
    }

    for line in stderr.lines() {
        let trimmed = line.trim();

        if let Some(a) = trimmed.strip_prefix("= at:") {
            at = Some(a.trim().to_string()).filter(|a| !a.is_empty());
        } else if let Some(caps) = loc_re().captures(trimmed) {
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
        } else if let Some(r) = trimmed.strip_prefix("= range:") {
            end = r
                .trim()
                .split_once(':')
                .and_then(|(l, c)| Some((l.trim().parse().ok()?, c.trim().parse().ok()?)));
        } else if let Some(o) = trimmed.strip_prefix("= origin:") {
            origin = Some(o.trim().to_string()).filter(|o| !o.is_empty());
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
            .unwrap_or("The preview couldn't update")
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
            None,
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
    at: Option<&str>,
    hints: Vec<String>,
    severity: Severity,
) -> CompileError {
    let diagnosis = diagnostic_catalog::diagnose(&raw, at);
    CompileError {
        file,
        line,
        col,
        end_line: line,
        end_col: col,
        origin: None,
        message: diagnosis.headline,
        advice: diagnosis.advice,
        kind: diagnosis.kind,
        hints,
        technical: raw,
        severity,
    }
}

/// Fixes are keyed on the kind of problem, decided from the engine's own
/// wording — matching the plain-language headline would silently retire every
/// Fix button.
fn is_quick_fixable(err: &CompileError, line_text: Option<&str>) -> bool {
    // A problem raised inside a package is reported at the user's own call, but
    // the wording describes the package's code: a fix aimed at this line would
    // change the wrong thing.
    err.origin.is_none() && diagnostic_catalog::fix_applies(err.kind, line_text, err.col)
}

/// `@preview/cetz:0.3.0` -> `cetz`.
fn package_short_name(origin: &str) -> &str {
    let name = origin.rsplit('/').next().unwrap_or(origin);
    name.split(':').next().unwrap_or(name)
}

fn origin_note(origin: &str) -> String {
    format!(
        "The problem is inside the \u{201c}{}\u{201d} package. Check how it is used on this line.",
        package_short_name(origin)
    )
}

/// The offending line as Pango markup, with the exact characters at fault
/// underlined. Leading whitespace is dropped and a very long line is cut
/// around the fault so it stays visible. Columns are 1-based characters.
fn snippet_markup(line: &str, col: u32, end_col: u32, same_line: bool) -> Option<String> {
    const CONTEXT_BEFORE: usize = 24;
    const MAX_AFTER: usize = 70;
    let chars: Vec<char> = line.chars().collect();
    let indent = chars.iter().take_while(|c| c.is_whitespace()).count();
    if indent == chars.len() {
        return None;
    }
    let start = (col as usize).saturating_sub(1).max(indent);
    let end = if same_line && end_col > col {
        ((end_col as usize).saturating_sub(1)).min(chars.len())
    } else {
        start
    };
    let end = end.max(start).min(chars.len());
    let show_from = if start > indent + CONTEXT_BEFORE {
        start - CONTEXT_BEFORE
    } else {
        indent
    };
    let show_to = chars.len().min(end.max(start) + MAX_AFTER);
    let piece = |a: usize, b: usize| -> String {
        gtk4::glib::markup_escape_text(&chars[a.min(b)..b].iter().collect::<String>()).to_string()
    };
    let mut out = String::new();
    if show_from > indent {
        out.push('\u{2026}');
    }
    out.push_str(&piece(show_from, start.min(chars.len())));
    if end > start {
        out.push_str("<span weight=\"bold\" underline=\"error\">");
        out.push_str(&piece(start, end));
        out.push_str("</span>");
    }
    out.push_str(&piece(end.max(start).min(chars.len()), show_to));
    if show_to < chars.len() {
        out.push('\u{2026}');
    }
    Some(out)
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

/// A read-only buffer holding `+ `/`- ` diff text, coloured with the theme's
/// diff colours as they are right now.
fn diff_buffer(text: &str) -> TextBuffer {
    let buf = TextBuffer::new(None);
    let colors = super::theme::diff_colors();
    let removed = TextTag::new(Some("removed"));
    removed.set_property("background", colors.removed_bg);
    removed.set_property("foreground", colors.removed_fg);
    removed.set_property("strikethrough", true);
    let added = TextTag::new(Some("added"));
    added.set_property("background", colors.added_bg);
    added.set_property("foreground", colors.added_fg);
    added.set_property("underline", gtk4::pango::Underline::Single);
    buf.tag_table().add(&removed);
    buf.tag_table().add(&added);
    super::diff_render::render_clean_diff(&buf, text);
    buf
}

fn set_toggle_label(label: &Label, text: &str, on: bool) {
    if on {
        label.set_markup(&format!("<b>{text}</b>"));
    } else {
        label.set_text(text);
    }
}

fn apply_technical_state(btn: &Button, label: &Label, widgets: &[gtk4::Widget], on: bool) {
    set_toggle_label(label, "Show technical details", on);
    let pressed = if on {
        gtk4::AccessibleTristate::True
    } else {
        gtk4::AccessibleTristate::False
    };
    btn.update_state(&[gtk4::accessible::State::Pressed(pressed)]);
    for w in widgets {
        w.set_visible(on);
    }
}

// ── Widget ───────────────────────────────────────────────────────────────────

/// How long a problem can stay unsolved before the panel offers more help.
const STUCK_AFTER: Duration = Duration::from_secs(120);
/// How many compiles in a row can report the same problems before it does.
const STUCK_REPEATS: u32 = 2;
const COPY_LABEL: &str = "Copy a help request";

type TextsFn = Box<dyn Fn() -> Vec<(PathBuf, String)>>;
type GoBackFn = Box<dyn Fn(Vec<(PathBuf, String)>, String)>;

#[derive(Clone)]
pub struct ErrorPanel {
    root_widget: GtkBox,
    revealer: Revealer,
    list_revealer: Revealer,
    list_box: ListBox,
    header_label: Label,
    chevron_btn: Button,
    stuck_revealer: Revealer,
    stuck_summary: Label,
    stuck_diff_view: TextView,
    stuck_diff_scroll: ScrolledWindow,
    go_back_btn: Button,
    stuck_triggered: Rc<Cell<bool>>,
    stuck_shown: Rc<Cell<bool>>,
    episode: Rc<Cell<u64>>,
    episode_active: Rc<Cell<bool>>,
    current_errors: Rc<RefCell<Vec<CompileError>>>,
    shown_changes: Rc<RefCell<Vec<Change>>>,
    shown_age: Rc<Cell<Option<Duration>>>,
    last_working: Rc<RefCell<Option<LastWorking>>>,
    current_texts: Rc<RefCell<Option<TextsFn>>>,
    on_go_back: Rc<RefCell<Option<GoBackFn>>>,
    last_clean_label: Label,
    live_label: Label,
    collapsed: Rc<Cell<bool>>,
    on_jump: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32)>>>>,
    on_try_fix: Rc<RefCell<Option<Box<dyn Fn(DiagMark)>>>>,
    /// Supplies the live text of a line from the editor's buffers.
    #[allow(clippy::type_complexity)]
    source_line: Rc<RefCell<Option<Box<dyn Fn(&Path, u32) -> Option<String>>>>>,
    on_export_done: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    last_errors_key: Rc<RefCell<String>>,
    repeat_count: Rc<Cell<u32>>,
    log_lines: Rc<RefCell<Vec<String>>>,
    build_log_revealer: Revealer,
    build_log_label: Label,
    search_entry: gtk4::SearchEntry,
    technical_btn: Button,
    technical_label: Label,
    show_technical: Rc<Cell<bool>>,
    technical_widgets: Rc<RefCell<Vec<gtk4::Widget>>>,
    on_technical_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    first_hint_label: Label,
    rows: Rc<RefCell<Vec<ListBoxRow>>>,
    targets: Rc<RefCell<Vec<(PathBuf, u32)>>>,
    current: Rc<Cell<Option<usize>>>,
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

        let header_label = Label::new(Some("Things to look at"));
        header_label.set_halign(Align::Start);
        header_label.set_hexpand(true);
        header_label.add_css_class("heading");
        header.append(&header_label);

        let technical_label = Label::new(None);
        set_toggle_label(&technical_label, "Show technical details", false);
        let technical_btn = Button::new();
        technical_btn.set_child(Some(&technical_label));
        technical_btn.add_css_class("flat");
        technical_btn.add_css_class("status-toggle");
        technical_btn.set_tooltip_text(Some(
            "Show the exact wording Zerkalo's engine used for each problem.\n\
             Handy when searching the Typst forum; safe to leave off.",
        ));
        header.append(&technical_btn);

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
        chevron_btn.set_tooltip_text(Some("Collapse list"));
        chevron_btn.update_property(&[gtk4::accessible::Property::Label("Toggle list")]);
        header.append(&chevron_btn);

        inner.append(&header);
        inner.append(&Separator::new(Orientation::Horizontal));

        let first_hint_label = Label::new(Some(
            "Start with the first one \u{2014} fixing it often clears the rest.",
        ));
        first_hint_label.add_css_class("dim-label");
        first_hint_label.add_css_class("caption");
        first_hint_label.set_halign(Align::Start);
        first_hint_label.set_margin_start(10);
        first_hint_label.set_margin_top(4);
        first_hint_label.set_visible(false);
        inner.append(&first_hint_label);

        // ── Search bar ───────────────────────────────────────────────────────
        let search_entry = gtk4::SearchEntry::new();
        search_entry.set_margin_start(8);
        search_entry.set_margin_end(8);
        search_entry.set_margin_top(4);
        search_entry.set_margin_bottom(4);
        search_entry.set_placeholder_text(Some("Filter…"));
        search_entry.set_visible(false);
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

        // ── Still stuck? ─────────────────────────────────────────────────────
        let stuck_revealer = Revealer::new();
        stuck_revealer.set_transition_type(RevealerTransitionType::SlideDown);
        stuck_revealer.set_transition_duration(150);
        stuck_revealer.set_reveal_child(false);
        let stuck_outer = GtkBox::new(Orientation::Vertical, 0);
        stuck_outer.append(&Separator::new(Orientation::Horizontal));
        let stuck_box = GtkBox::new(Orientation::Vertical, 6);
        stuck_box.set_margin_start(10);
        stuck_box.set_margin_end(10);
        stuck_box.set_margin_top(8);
        stuck_box.set_margin_bottom(8);

        let stuck_title = Label::new(Some("Still stuck?"));
        stuck_title.set_halign(Align::Start);
        stuck_title.add_css_class("heading");
        stuck_box.append(&stuck_title);

        let stuck_summary = Label::new(None);
        stuck_summary.set_halign(Align::Start);
        stuck_summary.set_xalign(0.0);
        stuck_summary.set_wrap(true);
        stuck_box.append(&stuck_summary);

        let stuck_diff_view = TextView::new();
        stuck_diff_view.set_editable(false);
        stuck_diff_view.set_cursor_visible(false);
        stuck_diff_view.set_monospace(true);
        stuck_diff_view.set_wrap_mode(WrapMode::None);
        stuck_diff_view.set_left_margin(6);
        stuck_diff_view.set_right_margin(6);
        stuck_diff_view.set_top_margin(4);
        stuck_diff_view.set_bottom_margin(4);
        stuck_diff_view.update_property(&[gtk4::accessible::Property::Label(
            "What changed since the document last worked",
        )]);
        let stuck_diff_scroll = ScrolledWindow::new();
        stuck_diff_scroll.set_max_content_height(150);
        stuck_diff_scroll.set_propagate_natural_height(true);
        stuck_diff_scroll.add_css_class("card");
        stuck_diff_scroll.set_child(Some(&stuck_diff_view));
        stuck_box.append(&stuck_diff_scroll);

        let stuck_buttons = GtkBox::new(Orientation::Horizontal, 6);
        stuck_buttons.set_halign(Align::Start);
        let go_back_btn = Button::with_label("Go back to the last working version");
        go_back_btn.set_tooltip_text(Some(
            "Puts the changed files back the way they were. You can undo it with Ctrl+Z.",
        ));
        stuck_buttons.append(&go_back_btn);
        let copy_btn = Button::with_label(COPY_LABEL);
        copy_btn.set_tooltip_text(Some(
            "Copies a summary to paste into an email or the Typst forum: the problems, \
             the lines they are on, and what changed. Not your whole document.",
        ));
        stuck_buttons.append(&copy_btn);
        stuck_box.append(&stuck_buttons);
        stuck_outer.append(&stuck_box);
        stuck_revealer.set_child(Some(&stuck_outer));
        inner.append(&stuck_revealer);

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
        let stuck_shown: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        {
            let list_rev_c = list_revealer.clone();
            let chevron_c = chevron_btn.clone();
            let collapsed_c = collapsed.clone();
            let stuck_rev_c = stuck_revealer.clone();
            let stuck_shown_c = stuck_shown.clone();
            chevron_btn.connect_clicked(move |_| {
                let now_collapsed = !collapsed_c.get();
                collapsed_c.set(now_collapsed);
                list_rev_c.set_reveal_child(!now_collapsed);
                stuck_rev_c.set_reveal_child(!now_collapsed && stuck_shown_c.get());
                if now_collapsed {
                    chevron_c.set_icon_name("pan-end-symbolic");
                    chevron_c.set_tooltip_text(Some("Expand list"));
                } else {
                    chevron_c.set_icon_name("pan-down-symbolic");
                    chevron_c.set_tooltip_text(Some("Collapse list"));
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
                let dir = crate::config::zerkalo_data_dir();
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
        log_chevron.update_property(&[gtk4::accessible::Property::Label("Expand build log")]);
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

        let show_technical = Rc::new(Cell::new(false));
        let technical_widgets: Rc<RefCell<Vec<gtk4::Widget>>> = Rc::new(RefCell::new(Vec::new()));
        let on_technical_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>> =
            Rc::new(RefCell::new(None));
        {
            let flag = show_technical.clone();
            let widgets = technical_widgets.clone();
            let cb = on_technical_toggle.clone();
            let btn = technical_btn.clone();
            let lbl = technical_label.clone();
            let log_rev = build_log_revealer_outer.clone();
            let log_lbl = build_log_label.clone();
            technical_btn.connect_clicked(move |_| {
                let on = !flag.get();
                flag.set(on);
                apply_technical_state(&btn, &lbl, &widgets.borrow(), on);
                log_rev.set_reveal_child(on && !log_lbl.text().is_empty());
                if let Some(f) = cb.borrow().as_ref() {
                    f(on);
                }
            });
        }

        let current_errors: Rc<RefCell<Vec<CompileError>>> = Rc::new(RefCell::new(Vec::new()));
        let shown_changes: Rc<RefCell<Vec<Change>>> = Rc::new(RefCell::new(Vec::new()));
        let shown_age: Rc<Cell<Option<Duration>>> = Rc::new(Cell::new(None));
        let on_go_back: Rc<RefCell<Option<GoBackFn>>> = Rc::new(RefCell::new(None));
        #[allow(clippy::type_complexity)]
        let source_line: Rc<RefCell<Option<Box<dyn Fn(&Path, u32) -> Option<String>>>>> =
            Rc::new(RefCell::new(None));

        {
            let changes = shown_changes.clone();
            let age = shown_age.clone();
            let cb = on_go_back.clone();
            go_back_btn.connect_clicked(move |_| {
                let texts: Vec<(PathBuf, String)> = changes
                    .borrow()
                    .iter()
                    .map(|c| (c.file.clone(), c.old.clone()))
                    .collect();
                if texts.is_empty() {
                    return;
                }
                let when = age.get().map(last_working::age_text).unwrap_or_default();
                if let Some(f) = cb.borrow().as_ref() {
                    f(texts, when);
                }
            });
        }
        {
            let errors = current_errors.clone();
            let changes = shown_changes.clone();
            let age = shown_age.clone();
            let lines = source_line.clone();
            copy_btn.connect_clicked(move |btn| {
                let text = last_working::help_request(
                    env!("CARGO_PKG_VERSION"),
                    &errors.borrow(),
                    &|path, line| lines.borrow().as_ref().and_then(|f| f(path, line)),
                    &changes.borrow(),
                    age.get(),
                );
                btn.clipboard().set_text(&text);
                btn.set_label("Copied");
                let btn = btn.clone();
                glib::timeout_add_local_once(Duration::from_secs(2), move || {
                    btn.set_label(COPY_LABEL);
                });
            });
        }

        Self {
            root_widget,
            revealer,
            list_revealer,
            list_box,
            header_label,
            chevron_btn,
            stuck_revealer,
            stuck_summary,
            stuck_diff_view,
            stuck_diff_scroll,
            go_back_btn,
            stuck_triggered: Rc::new(Cell::new(false)),
            stuck_shown,
            episode: Rc::new(Cell::new(0)),
            episode_active: Rc::new(Cell::new(false)),
            current_errors,
            shown_changes,
            shown_age,
            last_working: Rc::new(RefCell::new(None)),
            current_texts: Rc::new(RefCell::new(None)),
            on_go_back,
            last_clean_label,
            live_label,
            collapsed,
            on_jump: Rc::new(RefCell::new(None)),
            on_try_fix: Rc::new(RefCell::new(None)),
            source_line,
            on_export_done,
            last_errors_key: Rc::new(RefCell::new(String::new())),
            repeat_count: Rc::new(Cell::new(0)),
            log_lines,
            build_log_revealer: build_log_revealer_outer,
            build_log_label,
            search_entry,
            technical_btn,
            technical_label,
            show_technical,
            technical_widgets,
            on_technical_toggle,
            first_hint_label,
            rows: Rc::new(RefCell::new(Vec::new())),
            targets: Rc::new(RefCell::new(Vec::new())),
            current: Rc::new(Cell::new(None)),
        }
    }

    /// Step to the next (`1`) or previous (`-1`) problem, wrapping round, and
    /// take the editor there. Works whether or not the panel is open.
    pub fn go_to_problem(&self, delta: i32) -> bool {
        let n = self.targets.borrow().len();
        if n == 0 {
            return false;
        }
        let next = match self.current.get() {
            Some(i) if i < n => (i as i32 + delta).rem_euclid(n as i32) as usize,
            _ if delta >= 0 => 0,
            _ => n - 1,
        };
        self.current.set(Some(next));
        if let Some(row) = self.rows.borrow().get(next) {
            self.list_box.select_row(Some(row));
        }
        let (file, line) = self.targets.borrow()[next].clone();
        if let Some(f) = self.on_jump.borrow().as_ref() {
            f(file, line);
        }
        true
    }

    pub fn set_show_technical(&self, on: bool) {
        self.show_technical.set(on);
        apply_technical_state(
            &self.technical_btn,
            &self.technical_label,
            &self.technical_widgets.borrow(),
            on,
        );
        self.sync_build_log();
    }

    fn sync_build_log(&self) {
        self.build_log_revealer
            .set_reveal_child(self.show_technical.get() && !self.build_log_label.text().is_empty());
    }

    /// Open the list if it was collapsed, e.g. when the user asks to see the notes.
    pub fn expand(&self) {
        self.collapsed.set(false);
        self.list_revealer.set_reveal_child(true);
        self.chevron_btn.set_icon_name("pan-down-symbolic");
        self.chevron_btn.set_tooltip_text(Some("Collapse list"));
        self.revealer.set_reveal_child(true);
    }

    pub fn set_on_technical_toggle(&self, f: impl Fn(bool) + 'static) {
        *self.on_technical_toggle.borrow_mut() = Some(Box::new(f));
    }

    pub fn widget(&self) -> &GtkBox {
        &self.root_widget
    }

    pub fn set_build_log(&self, raw: &str) {
        self.build_log_label.set_text(raw);
        self.sync_build_log();
    }

    pub fn set_on_jump(&self, f: impl Fn(PathBuf, u32) + 'static) {
        *self.on_jump.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_try_fix(&self, f: impl Fn(DiagMark) + 'static) {
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
        self.show_errors_inner(errors, "Things to look at");
    }

    pub fn show_errors(&self, errors: Vec<CompileError>) {
        self.show_errors_inner(errors, "Things to look at");
    }

    fn show_errors_inner(&self, errors: Vec<CompileError>, section: &str) {
        let was_revealed = self.revealer.reveals_child();
        self.clear_rows();

        if errors.is_empty() {
            self.revealer.set_reveal_child(false);
            self.live_label.set_text("");
            self.first_hint_label.set_visible(false);
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
            (e, 0) => format!("{e} thing{} to look at", if e == 1 { "" } else { "s" }),
            (0, w) => format!("{w} note{}", if w == 1 { "" } else { "s" }),
            (e, w) => format!(
                "{e} thing{} to look at, {w} note{}",
                if e == 1 { "" } else { "s" },
                if w == 1 { "" } else { "s" }
            ),
        };
        self.header_label.set_label(match (err_count, warn_count) {
            (_, 0) => section,
            (0, _) => "Notes",
            _ => "Things to look at, and notes",
        });
        self.first_hint_label.set_visible(err_count > 1);
        let many = count > 8;
        self.search_entry.set_visible(many);
        if !many {
            self.search_entry.set_text("");
        }

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
                if n >= STUCK_REPEATS && err_count > 0 {
                    self.stuck_triggered.set(true);
                }
            } else {
                self.repeat_count.set(0);
                *prev = key;
            }
        }
        *self.current_errors.borrow_mut() = errors.clone();
        self.note_episode(err_count);

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

        if err_count > 0 {
            // Real errors always get shown, even if the list was collapsed
            // (manually, or by the warnings-only default just below) before
            // this compile.
            if self.collapsed.get() {
                self.collapsed.set(false);
                self.list_revealer.set_reveal_child(true);
                self.chevron_btn.set_icon_name("pan-down-symbolic");
                self.chevron_btn.set_tooltip_text(Some("Collapse list"));
            }
        } else if !was_revealed && !self.collapsed.get() {
            // Warnings only, and the panel wasn't already showing something —
            // default to collapsed (just the "N warnings" header) rather than
            // opening full-height for messages that don't block compiling.
            // Left alone once the user's expanded or collapsed it themselves,
            // so this only fires on the transition from clean/hidden.
            self.collapsed.set(true);
            self.list_revealer.set_reveal_child(false);
            self.chevron_btn.set_icon_name("pan-end-symbolic");
            self.chevron_btn.set_tooltip_text(Some("Expand list"));
        }

        self.last_clean_label.set_visible(false);
        self.revealer.set_reveal_child(true);
        self.refresh_stuck();
    }

    /// Tracks how long the current run of problems has lasted, and arms the
    /// timer that offers more help once it has gone on for a while.
    fn note_episode(&self, err_count: usize) {
        if err_count == 0 {
            self.episode_active.set(false);
            self.stuck_triggered.set(false);
            return;
        }
        if self.episode_active.replace(true) {
            return;
        }
        let id = self.episode.get() + 1;
        self.episode.set(id);
        let panel = self.clone();
        glib::timeout_add_local_once(STUCK_AFTER, move || {
            if panel.episode.get() == id && panel.episode_active.get() {
                panel.stuck_triggered.set(true);
                panel.refresh_stuck();
            }
        });
    }

    fn refresh_stuck(&self) {
        let errors: Vec<CompileError> = self
            .current_errors
            .borrow()
            .iter()
            .filter(|e| e.severity == Severity::Error)
            .cloned()
            .collect();
        if !self.stuck_triggered.get() || errors.is_empty() {
            self.stuck_shown.set(false);
            self.stuck_revealer.set_reveal_child(false);
            return;
        }

        let snapshot = self.last_working.borrow().as_ref().and_then(|lw| lw.get());
        let current = self
            .current_texts
            .borrow()
            .as_ref()
            .map(|f| f())
            .unwrap_or_default();
        let problem_files: Vec<PathBuf> = errors.iter().map(|e| e.file.clone()).collect();
        let (changes, age) = match &snapshot {
            Some(s) => (
                last_working::changes(s, &current, &problem_files),
                Some(s.taken.elapsed()),
            ),
            None => (Vec::new(), None),
        };

        let summary = match age {
            Some(age) if !changes.is_empty() => format!(
                "Zerkalo last compiled this document without problems {}. \
                 Here is what is different since then:",
                last_working::age_text(age)
            ),
            Some(age) => format!(
                "Nothing in the document has changed since it last worked ({}), so the \
                 cause may be outside it: a missing file, a font, or a package that needs \
                 the internet.",
                last_working::age_text(age)
            ),
            None => "Zerkalo hasn't seen this document compile without problems yet, so it \
                     can't show what changed. The most common cause is a missing closing \
                     bracket, parenthesis or quote just before the first problem."
                .to_string(),
        };
        self.stuck_summary.set_text(&summary);

        let has_changes = !changes.is_empty();
        if has_changes {
            let text = last_working::cap_lines(&last_working::combined_diff(&changes), 60);
            self.stuck_diff_view.set_buffer(Some(&diff_buffer(&text)));
        }
        self.stuck_diff_scroll.set_visible(has_changes);
        self.go_back_btn.set_visible(has_changes);

        *self.shown_changes.borrow_mut() = changes;
        self.shown_age.set(age);
        self.stuck_shown.set(true);
        self.stuck_revealer.set_reveal_child(!self.collapsed.get());
    }

    /// Where "what changed since it last worked" comes from: the remembered
    /// clean-compile snapshot, and a way to read the files as they are now.
    pub fn set_stuck_source(
        &self,
        last_working: LastWorking,
        current_texts: impl Fn() -> Vec<(PathBuf, String)> + 'static,
    ) {
        *self.last_working.borrow_mut() = Some(last_working);
        *self.current_texts.borrow_mut() = Some(Box::new(current_texts));
    }

    /// Called with each changed file's old text, and how long ago that was
    /// ("12 minutes ago"), when "Go back to the last working version" is pressed.
    pub fn set_on_go_back(&self, f: impl Fn(Vec<(PathBuf, String)>, String) + 'static) {
        *self.on_go_back.borrow_mut() = Some(Box::new(f));
    }

    pub fn clear(&self) {
        let had_errors = !self.log_lines.borrow().is_empty();
        self.clear_rows();
        self.revealer.set_reveal_child(false);
        self.live_label.set_text("");
        self.episode_active.set(false);
        self.stuck_triggered.set(false);
        self.stuck_shown.set(false);
        self.stuck_revealer.set_reveal_child(false);
        self.current_errors.borrow_mut().clear();
        self.first_hint_label.set_visible(false);
        self.current.set(None);
        self.repeat_count.set(0);
        *self.last_errors_key.borrow_mut() = String::new();
        self.log_lines.borrow_mut().clear();
        self.build_log_label.set_text("");
        self.sync_build_log();
        // Show last-clean timestamp only when recovering from real errors
        if had_errors {
            self.last_clean_label
                .set_text(&format!("Last clean compile: {}", current_time_hhmm()));
            self.last_clean_label.set_visible(true);
        }
    }

    fn clear_rows(&self) {
        self.technical_widgets.borrow_mut().clear();
        self.rows.borrow_mut().clear();
        self.targets.borrow_mut().clear();

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

        let (icon_name, icon_class, icon_desc) = match err.severity {
            Severity::Error => ("dialog-error-symbolic", "warning", "Thing to look at"),
            Severity::Warning => ("dialog-warning-symbolic", "warning", "Note"),
        };
        let icon_lbl = Image::from_icon_name(icon_name);
        icon_lbl.add_css_class(icon_class);
        icon_lbl.update_property(&[gtk4::accessible::Property::Label(icon_desc)]);
        icon_lbl.set_valign(Align::Start);
        icon_lbl.set_margin_top(2);
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

        if let Some(origin) = &err.origin {
            let origin_lbl = Label::new(Some(&origin_note(origin)));
            origin_lbl.set_halign(Align::Start);
            origin_lbl.set_xalign(0.0);
            origin_lbl.set_wrap(true);
            text_box.append(&origin_lbl);
        }

        // Typst's own hints, which the parser used to discard. They are often
        // the most specific thing anyone can say about the problem.
        for hint in &err.hints {
            let hint_box = GtkBox::new(Orientation::Horizontal, 6);
            hint_box.set_halign(Align::Start);
            let hint_icon = Image::from_icon_name("dialog-information-symbolic");
            hint_icon.set_pixel_size(12);
            hint_icon.set_valign(Align::Start);
            hint_icon.set_margin_top(2);
            hint_icon.add_css_class("dim-label");
            hint_box.append(&hint_icon);
            let hint_lbl = Label::new(Some(hint));
            hint_lbl.set_halign(Align::Start);
            hint_lbl.set_xalign(0.0);
            hint_lbl.set_wrap(true);
            hint_lbl.add_css_class("caption");
            hint_box.append(&hint_lbl);
            text_box.append(&hint_box);
        }

        // Source context: the offending line, taken from the open buffer when
        // there is one. Compiles run against the unsaved buffer, so reading
        // from disk quoted a line the compiler never saw whenever the document
        // had unsaved edits.
        let raw_line = self
            .source_line
            .borrow()
            .as_ref()
            .and_then(|f| f(&err.file, err.line))
            .or_else(|| {
                std::fs::read_to_string(&err.file).ok().and_then(|content| {
                    content
                        .lines()
                        .nth((err.line as usize).saturating_sub(1))
                        .map(str::to_string)
                })
            });
        let source_line = raw_line
            .as_deref()
            .and_then(|l| snippet_markup(l, err.col, err.end_col, err.end_line == err.line));
        if let Some(markup) = source_line {
            let src_lbl = Label::new(None);
            src_lbl.set_markup(&markup);
            src_lbl.set_halign(Align::Start);
            src_lbl.set_xalign(0.0);
            src_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            src_lbl.add_css_class("monospace");
            src_lbl.add_css_class("caption");
            text_box.append(&src_lbl);
        }

        // The compiler's exact words, behind a disclosure: useless to most
        // readers, indispensable to anyone searching the Typst forum. Shown
        // only when the plain-language pass actually changed the wording.
        if err.technical != err.message {
            let expander = gtk4::Expander::new(Some("Technical detail"));
            expander.add_css_class("caption");
            expander.set_expanded(true);
            expander.set_visible(self.show_technical.get());
            self.technical_widgets
                .borrow_mut()
                .push(expander.clone().upcast());
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
        let loc_text = format!("Line {} of {}", err.line, filename);
        let loc_lbl = Label::new(Some(&loc_text));
        loc_lbl.set_halign(Align::Start);
        loc_lbl.add_css_class("dim-label");
        loc_lbl.set_xalign(0.0);
        text_box.append(&loc_lbl);

        row_box.append(&text_box);

        let btn_box = GtkBox::new(Orientation::Horizontal, 4);
        btn_box.set_valign(Align::Center);

        let jump = {
            let on_jump = self.on_jump.clone();
            let file = err.file.clone();
            let line = err.line;
            Rc::new(move || {
                if let Some(f) = on_jump.borrow().as_ref() {
                    f(file.clone(), line);
                }
            })
        };

        let fixable = is_quick_fixable(&err, raw_line.as_deref());
        if fixable {
            let fix_btn = Button::with_label("Fix");
            fix_btn.add_css_class("suggested-action");
            fix_btn.set_tooltip_text(Some("Fix this automatically (undo with Ctrl+Z)"));
            fix_btn.update_property(&[gtk4::accessible::Property::Label("Fix this automatically")]);
            let on_fix = self.on_try_fix.clone();
            let mark = DiagMark::from(&err);
            fix_btn.connect_clicked(move |_| {
                if let Some(f) = on_fix.borrow().as_ref() {
                    f(mark.clone());
                }
            });
            btn_box.append(&fix_btn);
        } else {
            let show_btn = Button::with_label("Show me");
            show_btn.set_tooltip_text(Some("Go to this place in the document"));
            let jump = jump.clone();
            show_btn.connect_clicked(move |_| jump());
            btn_box.append(&show_btn);
        }

        let more_btn = MenuButton::new();
        more_btn.set_icon_name("view-more-symbolic");
        more_btn.add_css_class("flat");
        more_btn.add_css_class("circular");
        more_btn.set_tooltip_text(Some("More options"));
        more_btn.update_property(&[gtk4::accessible::Property::Label("More options")]);
        let popover = Popover::new();
        let menu = GtkBox::new(Orientation::Vertical, 0);
        let add_item = |label: &str, action: Box<dyn Fn(&Button)>| {
            let item = Button::new();
            let lbl = Label::new(Some(label));
            lbl.set_halign(Align::Start);
            item.set_child(Some(&lbl));
            item.add_css_class("flat");
            let pop = popover.clone();
            item.connect_clicked(move |b| {
                pop.popdown();
                action(b);
            });
            menu.append(&item);
        };
        if fixable {
            let jump = jump.clone();
            add_item("Show me", Box::new(move |_| jump()));
        }
        {
            let msg_c = err.message.clone();
            let tech_c = err.technical.clone();
            let hints_c = err.hints.clone();
            let loc_c = loc_text.clone();
            add_item(
                "Copy details",
                Box::new(move |btn| {
                    let mut out = format!("{msg_c}\n{loc_c}\n\n{tech_c}");
                    for h in &hints_c {
                        out.push_str(&format!("\nhint: {h}"));
                    }
                    btn.clipboard().set_text(&out);
                }),
            );
        }
        {
            let query = err.technical.lines().next().unwrap_or("").to_string();
            add_item(
                "Search the Typst forum",
                Box::new(move |_| {
                    let q = gtk4::glib::Uri::escape_string(&query, None, false);
                    let _ = gtk4::gio::AppInfo::launch_default_for_uri(
                        &format!("https://forum.typst.app/search?q={q}"),
                        None::<&gtk4::gio::AppLaunchContext>,
                    );
                }),
            );
        }
        popover.set_child(Some(&menu));
        more_btn.set_popover(Some(&popover));
        btn_box.append(&more_btn);

        row_box.append(&btn_box);
        row.set_child(Some(&row_box));

        {
            let jump = jump.clone();
            row.connect_activate(move |_| jump());
        }

        self.rows.borrow_mut().push(row.clone());
        self.targets.borrow_mut().push((err.file.clone(), err.line));
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
    fn the_snippet_underlines_exactly_the_characters_at_fault() {
        let m = snippet_markup("  #no-such-thing() end", 3, 19, true).unwrap();
        assert_eq!(
            m,
            "<span weight=\"bold\" underline=\"error\">#no-such-thing()</span> end"
        );
    }

    #[test]
    fn the_snippet_counts_characters_and_escapes_markup() {
        let m = snippet_markup("é <b> #oops", 7, 12, true).unwrap();
        assert_eq!(
            m,
            "é &lt;b&gt; <span weight=\"bold\" underline=\"error\">#oops</span>"
        );
    }

    #[test]
    fn a_snippet_with_no_extent_shows_the_whole_line_plain() {
        assert_eq!(snippet_markup("  hello", 1, 1, true).unwrap(), "hello");
        assert_eq!(snippet_markup("  hello", 3, 5, false).unwrap(), "hello");
    }

    #[test]
    fn a_blank_line_has_no_snippet() {
        assert!(snippet_markup("   ", 1, 2, true).is_none());
    }

    #[test]
    fn a_long_line_is_cut_around_the_fault() {
        let line = format!("{}#bad(){}", "a".repeat(100), "b".repeat(200));
        let m = snippet_markup(&line, 101, 107, true).unwrap();
        assert!(m.starts_with('\u{2026}'));
        assert!(m.ends_with('\u{2026}'));
        assert!(m.contains("underline=\"error\">#bad()</span>"));
        assert!(m.chars().count() < 200);
    }

    #[test]
    fn a_package_error_is_never_offered_a_quick_fix() {
        let mut errs = parse("error: unknown variable: foo\n --> /p/a.typ:1:1");
        assert!(is_quick_fixable(&errs[0], Some("#foo")));
        errs[0].origin = Some("@preview/x:1.0.0".into());
        assert!(!is_quick_fixable(&errs[0], Some("#foo")));
        assert!(origin_note("@preview/x:1.0.0").contains("\u{201c}x\u{201d}"));
    }

    #[test]
    fn range_and_origin_lines_reach_the_error() {
        let errs = parse(
            "error: boom\n --> /project/a.typ:3:2\n   = range: 3:15\n   = origin: @preview/cetz:0.3.0\n   = hint: try again",
        );
        assert_eq!(errs.len(), 1);
        assert_eq!((errs[0].line, errs[0].col), (3, 2));
        assert_eq!((errs[0].end_line, errs[0].end_col), (3, 15));
        assert_eq!(errs[0].origin.as_deref(), Some("@preview/cetz:0.3.0"));
        assert_eq!(errs[0].hints, vec!["try again".to_string()]);
    }

    #[test]
    fn range_and_origin_do_not_leak_into_the_next_error() {
        let errs = parse(
            "error: one\n --> /p/a.typ:1:1\n   = range: 1:4\n   = origin: @preview/x:1.0.0\nerror: two\n --> /p/a.typ:2:1",
        );
        assert_eq!(errs.len(), 2);
        assert!(errs[1].origin.is_none());
        assert_eq!((errs[1].end_line, errs[1].end_col), (2, 1));
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
    fn a_path_containing_a_colon_is_not_truncated() {
        let errs = parse("error: boom\n --> /project/odd:name/one.typ:7:2");
        assert_eq!(errs[0].file, PathBuf::from("/project/odd:name/one.typ"));
        assert_eq!(errs[0].line, 7);
    }

    #[test]
    fn quick_fixes_still_match_after_the_message_is_rewritten() {
        // The kind is decided from Typst's phrasing ("expected closing brace").
        // Once the headline became plain language it no longer contained those
        // words, so matching on it would have quietly removed every Fix button.
        let errs = parse("error: expected closing brace\n --> /project/main.typ:4:1");
        assert!(
            is_quick_fixable(&errs[0], None),
            "should still offer a fix; headline is now {:?}",
            errs[0].message
        );
    }

    #[test]
    fn the_text_at_the_fault_decides_which_mistake_it_was() {
        let errs = parse(
            "error: unclosed delimiter\n --> /project/main.typ:1:7\n   = range: 1:8\n   = at: $",
        );
        assert_eq!(errs[0].kind, Kind::UnclosedDollar);
        assert!(errs[0].advice.contains("\\$"), "got: {}", errs[0].advice);
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
