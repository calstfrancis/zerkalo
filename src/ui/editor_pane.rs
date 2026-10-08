use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, CssProvider, DrawingArea, DropTarget, Entry, EventControllerFocus,
    EventControllerKey, EventControllerMotion, GestureClick, Label, Orientation, Popover,
    PropagationPhase, ScrolledWindow, Separator, TextSearchFlags, TextTag, TextWindowType,
    ToggleButton,
};
use libadwaita as adw;
use sourceview5::prelude::*;
use sourceview5::{Buffer, LanguageManager, MarkAttributes, StyleSchemeManager, View};

use super::bib_popup::{BibPopup, PopupEntry, PopupSource};
use super::diagnostics_model::DiagMark;
use super::find_bar::FindBar;
use super::font_manager::FontManager;
use super::lsp_popup::LspPopup;
use super::tab_host::{TabHost, TabMark};
use crate::bibliography::BibEntry;
use crate::lsp::CompletionItem;
use crate::ui::autopair;

mod completion;
mod context_menu;
mod cursor;
mod diagnostics;
mod find;
mod input;
mod navigation;
mod reload;
mod save;
mod simple_mode;
mod spell;
mod tabs;
mod wordcount;
mod writing;

use self::completion::*;
use self::cursor::*;
use self::diagnostics::*;
use self::input::*;
use self::reload::*;
use self::simple_mode::*;
use self::spell::*;
use self::wordcount::*;

// Package names/descriptions matching EXTRA_PACKAGES in template_dialog.rs
const IMPORT_PACKAGE_TOOLTIPS: &[(&str, &str)] = &[
    ("droplet", "Large decorative first-letter (dropcap)"),
    ("codly", "Beautiful code listings with syntax highlighting"),
    ("showybox", "Coloured callout and theorem boxes"),
    (
        "gentle-clues",
        "Admonition blocks: note, tip, warning, important",
    ),
    ("tablex", "Advanced tables with merged cells and styling"),
    ("drafting", "Margin notes and annotation tools"),
];

// Minimal Typst language definition for GtkSourceView
const TYPST_LANG: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<language id="typst" name="Typst" version="2.0" _section="Markup">
  <metadata>
    <property name="mimetypes">text/x-typst</property>
    <property name="globs">*.typ</property>
    <property name="line-comment-start">//</property>
    <property name="block-comment-start">/*</property>
    <property name="block-comment-end">*/</property>
  </metadata>
  <styles>
    <style id="comment"  name="Comment"  map-to="def:comment"/>
    <style id="string"   name="String"   map-to="def:string"/>
    <style id="function" name="Function" map-to="def:identifier"/>
    <style id="heading"  name="Heading"  map-to="def:type"/>
    <style id="markup"   name="Markup"   map-to="def:preprocessor"/>
    <style id="math"     name="Math"     map-to="def:number"/>
  </styles>
  <definitions>
    <context id="typst">
      <include>
        <context id="line-comment" style-ref="comment" end-at-line-end="true">
          <start>//</start>
        </context>
        <context id="block-comment" style-ref="comment">
          <start>/\*</start>
          <end>\*/</end>
        </context>
        <context id="heading" style-ref="heading" end-at-line-end="true">
          <start>^=+\s</start>
        </context>
        <context id="string" style-ref="string" end-at-line-end="false">
          <start>"</start>
          <end>"</end>
        </context>
        <context id="math-inline" style-ref="math">
          <start>\$</start>
          <end>\$</end>
        </context>
        <context id="function-call" style-ref="function">
          <match>#[a-zA-Z][a-zA-Z0-9_-]*</match>
        </context>
        <context id="citation" style-ref="markup">
          <match>@[a-zA-Z][a-zA-Z0-9:_-]*</match>
        </context>
        <context id="label-def" style-ref="markup">
          <match>&lt;[a-zA-Z][a-zA-Z0-9:_-]*&gt;</match>
        </context>
      </include>
    </context>
  </definitions>
</language>
"#;

// ── Internal types ────────────────────────────────────────────────────────────

struct EditorTab {
    buffer: Buffer,
    view: View,
    // The actual notebook page widget (an Overlay wrapping the ScrolledWindow,
    // for the empty-buffer placeholder — see where it's built): notebook.page_num/
    // remove_page/etc. need whatever was actually passed to append_page.
    notebook_page: gtk4::Overlay,
    modified: bool,
    diag_dot: TabMark,
    dot_label: TabMark,
    display_name: String,
    lsp_popup: LspPopup,
    ghost_label: Label,
    ghost_item: Rc<RefCell<Option<CompletionItem>>>,
    session_start_words: u32,
}

struct EditorState {
    tabs: HashMap<PathBuf, EditorTab>,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// What happened when a one-click fix was applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixOutcome {
    Fixed,
    /// The edit went in, but the file still has as many syntax problems as before.
    DidNotHelp,
    /// The fix had nothing to do at that spot (the text has moved on).
    NotApplicable,
}

#[derive(Clone)]
pub struct EditorPane {
    outer: GtkBox,
    notebook: TabHost,
    typewriter_crosshair: DrawingArea,
    typewriter_crosshair_timer: Rc<RefCell<Option<glib::SourceId>>>,
    state: Rc<RefCell<EditorState>>,
    on_change: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_modified_changed: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    on_file_dirty: Rc<RefCell<Option<Box<dyn Fn(PathBuf, bool)>>>>,
    save_problems: Rc<RefCell<Vec<(PathBuf, String)>>>,
    on_save_problems: Rc<RefCell<Option<Box<dyn Fn(&[(PathBuf, String)])>>>>,
    on_image_drop: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>,
    on_document_drop: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>,
    on_delete_file: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>,
    on_page_switch: Rc<RefCell<Option<Box<dyn Fn(String, PathBuf)>>>>,
    on_file_opened: Rc<RefCell<Option<Box<dyn Fn(PathBuf, String)>>>>,
    on_completion_needed: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32, u32)>>>>,
    on_cursor_heading: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32)>>>>,
    on_cursor_moved: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32, u32)>>>>,
    bib_entries: Rc<RefCell<Vec<BibEntry>>>,
    /// Keys the open document cites; shared with every file's `@` popup.
    cited_keys: Rc<RefCell<std::collections::HashSet<String>>>,
    cv_entries: Rc<RefCell<Vec<skrizhal_core::CvEntry>>>,
    font_provider: Rc<CssProvider>,
    font_size: Rc<RefCell<u32>>,
    font_family: Rc<RefCell<String>>,
    show_whitespace: Rc<RefCell<bool>>,
    tab_width: Rc<RefCell<u32>>,
    find_bar: FindBar,
    undo_btn: Button,
    redo_btn: Button,
    word_count_label: Label,
    on_word_count_click: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    session_delta_label: Label,
    goal_ring: DrawingArea,
    goal_fraction: Rc<Cell<f64>>,
    goal_celebrating: Rc<Cell<bool>>,
    lsp_status_label: Label,
    safe_label: Label,
    diag_label: Label,
    diag_btn: Button,
    on_diag_click: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_fix_request: Rc<RefCell<Option<Box<dyn Fn(DiagMark)>>>>,
    last_edit: Rc<Cell<Option<std::time::Instant>>>,
    last_diagnostics: Rc<RefCell<Vec<DiagMark>>>,
    cursor_label: Label,
    section_wc_label: Label,
    breadcrumb_label: Label,
    breadcrumb_bar: GtkBox,
    simple_mode: Rc<RefCell<bool>>,
    simple_mode_label: Label,
    on_simple_mode_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    spell_checker: Rc<RefCell<crate::spellcheck::SpellChecker>>,
    line_spacing: Rc<RefCell<u32>>,
    typewriter_scroll: Rc<RefCell<bool>>,
    /// Set around a jump's own `grab_focus`, so the focus-enter restore doesn't
    /// throw the view back to where it was before the jump.
    jumping: Rc<Cell<bool>>,
    zen_on: Rc<Cell<bool>>,
    /// Highlights every match of the current Find, not just the one the cursor is on.
    search_context: Rc<RefCell<Option<sourceview5::SearchContext>>>,
    on_show_in_preview: Rc<RefCell<Option<Box<dyn Fn(PathBuf, usize)>>>>,
    word_count_goal: Rc<RefCell<u32>>,
    /// The Settings → Editor goal, applied to any document that doesn't carry
    /// its own `// @zerkalo-goal:` comment. Kept separate from
    /// `word_count_goal` so opening a document with a goal comment and then one
    /// without doesn't leave the first document's goal on screen.
    default_word_count_goal: Rc<RefCell<u32>>,
    last_wc_text: Rc<RefCell<String>>,
    project_root: Rc<RefCell<Option<PathBuf>>>,
    status_bar: GtkBox,
    simple_mode_btn: Button,
    /// Shown once per session, the first time raw front-matter becomes
    /// visible (Simple Mode turned off, or a document with no body marker) —
    /// a tooltip alone is easy to never see, and a wall of Typst setup code
    /// with no explanation is the scariest thing in the editor for someone
    /// who's never seen it before.
    frontmatter_banner: adw::Banner,
    shown_frontmatter_banner: Rc<Cell<bool>>,
    focus_toggle_btn: Button,
    gost_btn: Button,
    /// Whether a language server is answering. Drives the "built-in snippets
    /// only" note on the completion hint.
    lsp_ready: Rc<Cell<bool>>,
    /// prefix → name last chosen for it, remembered per project.
    completion_picks: Rc<RefCell<std::collections::HashMap<String, String>>>,
    autocorrect_label: Label,
    autocorrect_btn: Button,
    on_autocorrect_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    gost_label: Label,
    gost_enabled: Rc<RefCell<bool>>,
    /// True only while `set_gost_enabled` replays the saved state at startup,
    /// so the toggle callback can tell a restore from a real click and skip
    /// the "font isn't installed" toast on every launch.
    gost_restoring: Rc<Cell<bool>>,
    on_gost_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    on_version_click: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    bib_active: Rc<RefCell<bool>>,
    format_bar_container: GtkBox,
    format_bar_label: Label,
    format_bar_toggle_btn: Button,
    on_format_bar_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    autosave_label: Label,
    autosave_toggle_btn: Button,
    autosave_on: Rc<Cell<bool>>,
    on_autosave_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    user_dismissed_format_bar: Rc<RefCell<bool>>,
    focus_label: Label,
    on_focus_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    on_doc_font: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    on_doc_font_size: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    font_bar_label: Label,
    size_bar_label: Label,
    line_numbers_override: Rc<Cell<bool>>,
    line_numbers_btn: ToggleButton,
    cv_mode: Rc<Cell<bool>>,
    cv_format_section: GtkBox,
    cv_style_label: Label,
}

/// Title Case, unlike the lowercase status-bar toggles: this label lives in the
/// hamburger menu, between "Font Management…" and "Settings".
fn set_autocorrect_label(label: &Label, enabled: bool) {
    set_toggle_label(label, "Autocorrect", enabled);
}

fn set_toggle_label(label: &Label, text: &str, enabled: bool) {
    if enabled {
        label.set_markup(&format!("<b>{text}</b>"));
    } else {
        label.set_text(text);
    }
}

fn set_status_toggle(btn: &Button, label: &Label, text: &str, active: bool) {
    set_toggle_label(label, text, active);
    let pressed = if active {
        gtk4::AccessibleTristate::True
    } else {
        gtk4::AccessibleTristate::False
    };
    btn.update_state(&[gtk4::accessible::State::Pressed(pressed)]);
}

/// The widgets belonging to one open tab. `open_file` was 2,730 lines largely
/// because every wiring section closed over these same few values plus `self`;
/// bundling them is what lets a section become a method instead of a closure
/// with a dozen captures.
struct TabContext {
    path: PathBuf,
    buffer: Buffer,
    view: View,
    scroll: ScrolledWindow,
    dot_label: TabMark,
}

impl EditorPane {
    pub fn new() -> Self {
        let notebook = TabHost::new();

        let state = Rc::new(RefCell::new(EditorState {
            tabs: HashMap::new(),
        }));

        let font_provider = CssProvider::new();
        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &font_provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }

        // Install Typst language definition
        let lang_dir = crate::config::zerkalo_data_dir().join("language-specs");
        let lang_file = lang_dir.join("typst.lang");
        if !lang_file.exists() && std::fs::create_dir_all(&lang_dir).is_ok() {
            let _ = std::fs::write(&lang_file, TYPST_LANG);
        }
        let lang_manager = LanguageManager::default();
        let dir_str = lang_dir.to_string_lossy().to_string();
        let existing: Vec<String> = lang_manager
            .search_path()
            .iter()
            .map(|s| s.to_string())
            .collect();
        if !existing.contains(&dir_str) {
            let mut paths: Vec<&str> = vec![dir_str.as_str()];
            paths.extend(existing.iter().map(|s| s.as_str()));
            lang_manager.set_search_path(&paths);
        }

        let find_bar = FindBar::new();

        let status_bar = GtkBox::new(Orientation::Horizontal, 0);
        status_bar.set_hexpand(true);
        status_bar.add_css_class("fond-chrome");
        status_bar.add_css_class("fond-statusbar");
        status_bar.add_css_class("fond-edge-top");

        // Lives in the hamburger menu — a whole-UI font switch is a setting you
        // change once, not something to keep a status-bar chip for.
        let gost_label = Label::new(Some("GOST Type B font"));
        gost_label.set_use_markup(true);
        gost_label.set_halign(gtk4::Align::Start);
        gost_label.set_hexpand(true);

        // Same row padding as make_menu_item() in app_window, so it lines up
        // with the menu items either side of it.
        let gost_row = GtkBox::new(Orientation::Horizontal, 0);
        gost_row.set_margin_start(4);
        gost_row.set_margin_end(6);
        gost_row.append(&gost_label);

        let gost_btn = Button::new();
        gost_btn.set_child(Some(&gost_row));
        gost_btn.add_css_class("flat");
        gost_btn.set_tooltip_text(Some("Toggle GOST type B engineering font for the whole UI"));

        // Matches gost_label above and make_menu_item()'s rows: this button is
        // packed into the hamburger, not the status bar, so it drops the
        // dim/caption status-toggle styling that made it read as a stray.
        let autocorrect_label = Label::new(Some("Autocorrect"));
        autocorrect_label.set_use_markup(true);
        autocorrect_label.set_halign(gtk4::Align::Start);
        autocorrect_label.set_hexpand(true);

        let autocorrect_row = GtkBox::new(Orientation::Horizontal, 0);
        autocorrect_row.set_margin_start(4);
        autocorrect_row.set_margin_end(6);
        autocorrect_row.append(&autocorrect_label);

        let autocorrect_btn = Button::new();
        autocorrect_btn.set_child(Some(&autocorrect_row));
        autocorrect_btn.add_css_class("flat");
        autocorrect_btn.set_tooltip_text(Some("Toggle autocorrect (fixes spelling as you type)"));
        autocorrect_btn.update_property(&[gtk4::accessible::Property::Label("Toggle autocorrect")]);

        let search_label = Label::new(Some("search"));
        search_label.add_css_class("dim-label");
        search_label.add_css_class("caption");
        search_label.set_use_markup(true);
        search_label.set_margin_top(3);
        search_label.set_margin_bottom(3);
        let search_btn = Button::new();
        search_btn.set_child(Some(&search_label));
        search_btn.add_css_class("flat");
        search_btn.add_css_class("status-toggle");
        search_btn.set_tooltip_text(Some("Find & Replace (Ctrl+F)"));
        search_btn.set_margin_start(4);
        search_btn.set_margin_end(4);
        let focus_label = Label::new(Some("focus"));
        focus_label.add_css_class("dim-label");
        focus_label.add_css_class("caption");
        focus_label.set_use_markup(true);
        focus_label.set_margin_top(3);
        focus_label.set_margin_bottom(3);

        let focus_toggle_btn = Button::new();
        focus_toggle_btn.set_child(Some(&focus_label));
        focus_toggle_btn.add_css_class("flat");
        focus_toggle_btn.add_css_class("status-toggle");
        focus_toggle_btn.set_tooltip_text(Some("Focus mode — hide sidebar and preview"));
        focus_toggle_btn.set_margin_end(4);
        focus_toggle_btn.update_property(&[gtk4::accessible::Property::Label("Toggle focus mode")]);

        let autosave_label = Label::new(Some("autosave"));
        autosave_label.add_css_class("dim-label");
        autosave_label.add_css_class("caption");
        autosave_label.set_use_markup(true);
        autosave_label.set_margin_top(3);
        autosave_label.set_margin_bottom(3);
        let autosave_toggle_btn = Button::new();
        autosave_toggle_btn.set_child(Some(&autosave_label));
        autosave_toggle_btn.add_css_class("flat");
        autosave_toggle_btn.add_css_class("status-toggle");
        autosave_toggle_btn.set_tooltip_text(Some(
            "Autosave — save the document a few seconds after you stop typing, and \
             whenever you switch away, compile, export or quit",
        ));
        autosave_toggle_btn.set_margin_end(4);
        autosave_toggle_btn
            .update_property(&[gtk4::accessible::Property::Label("Toggle autosave")]);

        let format_bar_label = Label::new(Some("format bar"));
        format_bar_label.add_css_class("dim-label");
        format_bar_label.add_css_class("caption");
        format_bar_label.set_use_markup(true);
        format_bar_label.set_margin_top(3);
        format_bar_label.set_margin_bottom(3);
        set_toggle_label(&format_bar_label, "format bar", true);

        let format_bar_toggle_btn = Button::new();
        format_bar_toggle_btn.set_child(Some(&format_bar_label));
        format_bar_toggle_btn.add_css_class("flat");
        format_bar_toggle_btn.add_css_class("status-toggle");
        format_bar_toggle_btn.set_tooltip_text(Some("Toggle the formatting toolbar"));
        format_bar_toggle_btn.set_margin_end(4);
        format_bar_toggle_btn
            .update_property(&[gtk4::accessible::Property::Label("Toggle format bar")]);

        let sb_sep1 = gtk4::Separator::new(Orientation::Vertical);
        sb_sep1.add_css_class("statusbar-sep");
        sb_sep1.set_margin_start(6);
        sb_sep1.set_margin_end(6);
        sb_sep1.set_margin_top(6);
        sb_sep1.set_margin_bottom(6);

        let undo_btn = Button::from_icon_name("edit-undo-symbolic");
        undo_btn.add_css_class("flat");
        undo_btn.set_tooltip_text(Some("Undo (Ctrl+Z)"));
        undo_btn.set_sensitive(false);
        undo_btn.update_property(&[gtk4::accessible::Property::Label("Undo")]);

        let redo_btn = Button::from_icon_name("edit-redo-symbolic");
        redo_btn.add_css_class("flat");
        redo_btn.set_tooltip_text(Some("Redo (Ctrl+Shift+Z)"));
        redo_btn.set_sensitive(false);
        redo_btn.update_property(&[gtk4::accessible::Property::Label("Redo")]);

        let cursor_label = Label::new(Some("L1:C1"));
        cursor_label.add_css_class("dim-label");
        cursor_label.add_css_class("caption");
        cursor_label.set_margin_start(12);
        cursor_label.set_margin_top(3);
        cursor_label.set_margin_bottom(3);
        cursor_label.set_tooltip_text(Some("Line 1, Column 1"));

        // The one steady, quiet line: "Saved · backed up 12 min ago".
        let safe_label = Label::new(None);
        safe_label.add_css_class("dim-label");
        safe_label.add_css_class("caption");
        safe_label.set_margin_start(8);
        safe_label.set_margin_top(3);
        safe_label.set_margin_bottom(3);
        safe_label.set_halign(gtk4::Align::Start);
        let lsp_status_label = Label::new(None);
        lsp_status_label.add_css_class("dim-label");
        lsp_status_label.add_css_class("caption");
        lsp_status_label.set_use_markup(true);
        lsp_status_label.set_margin_start(8);
        // First in the bar, so it takes its natural width before anything else
        // is measured — no width needs reserving. The ceiling and ellipsis are
        // there for the rare very long LSP description.
        lsp_status_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        lsp_status_label.set_max_width_chars(86);
        lsp_status_label.set_margin_top(3);
        lsp_status_label.set_margin_bottom(3);

        let diag_label = Label::new(None);
        diag_label.add_css_class("dim-label");
        diag_label.add_css_class("caption");
        let diag_btn = Button::new();
        diag_btn.set_child(Some(&diag_label));
        diag_btn.add_css_class("flat");
        diag_btn.add_css_class("status-toggle");
        diag_btn.set_margin_start(8);
        diag_btn.set_tooltip_text(Some("Show what Zerkalo found"));
        diag_btn.set_visible(false);
        let on_diag_click: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        {
            let cb = on_diag_click.clone();
            diag_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() {
                    f();
                }
            });
        }

        let left_spacer = GtkBox::new(Orientation::Horizontal, 0);
        left_spacer.set_hexpand(true);

        let simple_mode_label = Label::new(None);
        simple_mode_label.add_css_class("caption");
        simple_mode_label.set_use_markup(true);
        simple_mode_label.set_margin_top(3);
        simple_mode_label.set_margin_bottom(3);

        let simple_mode_btn = Button::new();
        simple_mode_btn.set_child(Some(&simple_mode_label));
        simple_mode_btn.add_css_class("flat");
        simple_mode_btn.add_css_class("status-toggle");
        simple_mode_btn.set_tooltip_text(Some(
            "Show Template: reveals the Typst front-matter above the document body.\nEdit it via the Update Template button.",
        ));
        simple_mode_btn.update_property(&[gtk4::accessible::Property::Label(
            "Toggle template visibility",
        )]);

        let sep1 = gtk4::Separator::new(Orientation::Vertical);
        sep1.add_css_class("statusbar-sep");
        sep1.set_margin_start(6);
        sep1.set_margin_end(6);
        sep1.set_margin_top(6);
        sep1.set_margin_bottom(6);

        let section_wc_label = Label::new(None);
        section_wc_label.add_css_class("dim-label");
        section_wc_label.add_css_class("caption");
        section_wc_label.set_margin_start(8);
        section_wc_label.set_margin_end(4);
        section_wc_label.set_margin_top(3);
        section_wc_label.set_margin_bottom(3);

        let word_count_label = Label::new(Some(""));
        word_count_label.add_css_class("dim-label");
        word_count_label.add_css_class("caption");
        word_count_label.set_xalign(1.0);

        let wc_btn = Button::new();
        wc_btn.set_child(Some(&word_count_label));
        wc_btn.add_css_class("flat");
        wc_btn.set_margin_end(4);
        wc_btn.set_margin_top(1);
        wc_btn.set_margin_bottom(1);
        wc_btn.set_tooltip_text(Some("Document statistics"));

        let sep2 = gtk4::Separator::new(Orientation::Vertical);
        sep2.add_css_class("statusbar-sep");
        sep2.set_margin_start(6);
        sep2.set_margin_end(6);
        sep2.set_margin_top(6);
        sep2.set_margin_bottom(6);

        let session_delta_label = Label::new(None);
        session_delta_label.add_css_class("dim-label");
        session_delta_label.add_css_class("caption");
        session_delta_label.set_margin_end(8);
        session_delta_label.set_margin_top(3);
        session_delta_label.set_margin_bottom(3);
        session_delta_label.set_visible(false);

        let goal_fraction: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));
        let goal_celebrating: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let goal_ring = DrawingArea::new();
        goal_ring.set_visible(false);
        goal_ring.set_valign(gtk4::Align::Center);
        goal_ring.set_size_request(22, 22);
        goal_ring.set_margin_end(6);
        goal_ring.set_tooltip_text(Some("Word count progress toward goal"));
        goal_ring.add_css_class("goal-ring");
        {
            let frac_rc = goal_fraction.clone();
            let cel_rc = goal_celebrating.clone();
            let ring_widget = goal_ring.clone();
            goal_ring.set_draw_func(move |_da, cr, w, h| {
                let cx = w as f64 / 2.0;
                let cy = h as f64 / 2.0;
                let radius = (w.min(h) as f64 / 2.0) - 2.0;
                let celebrating = cel_rc.get();

                // Query theme colors on every draw so a theme/accent switch is
                // reflected immediately, matching the pattern in apply_comment_highlights.
                #[allow(deprecated)]
                let ctx = ring_widget.style_context();
                #[allow(deprecated)]
                let track = ctx
                    .lookup_color("window_fg_color")
                    .unwrap_or(gtk4::gdk::RGBA::new(0.5, 0.5, 0.5, 1.0));
                #[allow(deprecated)]
                let accent = ctx
                    .lookup_color("accent_color")
                    .unwrap_or(gtk4::gdk::RGBA::new(0.2, 0.4, 0.9, 1.0));
                #[allow(deprecated)]
                let success = ctx
                    .lookup_color("success_color")
                    .unwrap_or(gtk4::gdk::RGBA::new(0.2, 0.8, 0.2, 1.0));

                cr.set_line_width(if celebrating { 3.5 } else { 2.5 });
                cr.set_source_rgba(
                    track.red() as f64,
                    track.green() as f64,
                    track.blue() as f64,
                    0.2,
                );
                cr.arc(cx, cy, radius, 0.0, 2.0 * std::f64::consts::PI);
                let _ = cr.stroke();
                let frac = frac_rc.get();
                if frac > 0.0 {
                    let end_angle =
                        -std::f64::consts::FRAC_PI_2 + frac * 2.0 * std::f64::consts::PI;
                    let progress = if frac >= 1.0 || celebrating {
                        &success
                    } else {
                        &accent
                    };
                    if celebrating {
                        cr.set_line_width(3.5);
                    }
                    cr.set_source_rgba(
                        progress.red() as f64,
                        progress.green() as f64,
                        progress.blue() as f64,
                        0.9,
                    );
                    cr.arc(cx, cy, radius, -std::f64::consts::FRAC_PI_2, end_angle);
                    let _ = cr.stroke();
                }
            });
        }

        let version_btn = Button::with_label(concat!("v", env!("CARGO_PKG_VERSION")));
        version_btn.add_css_class("flat");
        version_btn.add_css_class("dim-label");
        version_btn.add_css_class("caption");
        version_btn.set_margin_end(4);
        version_btn.set_tooltip_text(Some("View changelog"));

        // ── Status bar assembly ───────────────────────────────────────────────
        //
        // The completion hint leads, alone, with every standing control packed
        // to the far right behind an expanding spacer. The hint is the only
        // thing here that changes with what you're doing rather than how the
        // app is set up, and it needs room for a name, a description and its
        // keys — so it gets the whole left half of the window and the settings
        // queue up out of its way.
        status_bar.append(&safe_label);
        status_bar.append(&lsp_status_label);
        status_bar.append(&left_spacer);
        status_bar.append(&autosave_toggle_btn);
        status_bar.append(&format_bar_toggle_btn);
        status_bar.append(&search_btn);
        status_bar.append(&sb_sep1);
        status_bar.append(&cursor_label);
        status_bar.append(&diag_btn);
        status_bar.append(&sep1);
        status_bar.append(&wc_btn);
        status_bar.append(&sep2);
        status_bar.append(&session_delta_label);
        status_bar.append(&goal_ring);
        status_bar.append(&version_btn);

        let breadcrumb_label = Label::new(Some(""));
        breadcrumb_label.add_css_class("dim-label");
        breadcrumb_label.add_css_class("caption");
        breadcrumb_label.set_margin_start(12);
        breadcrumb_label.set_margin_top(3);
        breadcrumb_label.set_margin_bottom(3);
        breadcrumb_label.set_hexpand(true);
        breadcrumb_label.set_xalign(0.0);
        breadcrumb_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);

        let breadcrumb_bar = GtkBox::new(Orientation::Horizontal, 0);
        breadcrumb_bar.add_css_class("breadcrumb-bar");
        // Undo/redo at top-left of the editor panel
        breadcrumb_bar.append(&undo_btn);
        breadcrumb_bar.append(&redo_btn);
        let sep = Separator::new(Orientation::Vertical);
        sep.set_margin_top(6);
        sep.set_margin_bottom(6);
        sep.set_margin_start(2);
        sep.set_margin_end(2);
        breadcrumb_bar.append(&sep);
        breadcrumb_bar.append(&breadcrumb_label);
        breadcrumb_bar.append(&section_wc_label);

        let editor_row = GtkBox::new(Orientation::Horizontal, 0);
        editor_row.set_hexpand(true);
        editor_row.set_vexpand(true);
        editor_row.append(&notebook.view);

        let typewriter_crosshair = DrawingArea::new();
        typewriter_crosshair.set_can_target(false);
        typewriter_crosshair.set_visible(false);
        typewriter_crosshair.set_hexpand(true);
        typewriter_crosshair.set_vexpand(true);
        typewriter_crosshair.set_draw_func(move |_da, cr, w, h| {
            let y = h as f64 * 0.45;
            cr.set_source_rgba(0.5, 0.5, 0.5, 0.13);
            cr.set_line_width(1.0);
            cr.move_to(0.0, y);
            cr.line_to(w as f64, y);
            let _ = cr.stroke();
        });

        let editor_overlay = gtk4::Overlay::new();
        editor_overlay.set_child(Some(&editor_row));
        editor_overlay.add_overlay(&typewriter_crosshair);

        // ── Formatting toolbar ────────────────────────────────────────────────
        let format_bar = GtkBox::new(Orientation::Horizontal, 0);
        format_bar.add_css_class("format-bar");
        format_bar.set_hexpand(true);
        format_bar.set_margin_start(4);
        format_bar.set_margin_end(4);
        format_bar.set_margin_top(1);
        format_bar.set_margin_bottom(1);

        let bold_btn = Button::from_icon_name("format-text-bold-symbolic");
        bold_btn.add_css_class("flat");
        bold_btn.set_tooltip_text(Some("Bold — wraps selection in *…*  (Ctrl+B)"));
        bold_btn.update_property(&[gtk4::accessible::Property::Label("Bold")]);

        let italic_btn = Button::from_icon_name("format-text-italic-symbolic");
        italic_btn.add_css_class("flat");
        italic_btn.set_tooltip_text(Some("Italic — wraps selection in _…_  (Ctrl+I)"));
        italic_btn.update_property(&[gtk4::accessible::Property::Label("Italic")]);

        format_bar.append(&bold_btn);
        format_bar.append(&italic_btn);

        let fb_sep1 = Separator::new(Orientation::Vertical);
        fb_sep1.set_margin_top(6);
        fb_sep1.set_margin_bottom(6);
        fb_sep1.set_margin_start(4);
        fb_sep1.set_margin_end(4);
        format_bar.append(&fb_sep1);

        let h1_btn = Button::with_label("H1");
        h1_btn.add_css_class("flat");
        h1_btn.add_css_class("caption");
        h1_btn.set_tooltip_text(Some("Heading 1  (= Heading text)"));
        h1_btn.update_property(&[gtk4::accessible::Property::Label("Heading 1")]);
        let h2_btn = Button::with_label("H2");
        h2_btn.add_css_class("flat");
        h2_btn.add_css_class("caption");
        h2_btn.set_tooltip_text(Some("Heading 2  (== Heading text)"));
        h2_btn.update_property(&[gtk4::accessible::Property::Label("Heading 2")]);
        let h3_btn = Button::with_label("H3");
        h3_btn.add_css_class("flat");
        h3_btn.add_css_class("caption");
        h3_btn.set_tooltip_text(Some("Heading 3  (=== Heading text)"));
        h3_btn.update_property(&[gtk4::accessible::Property::Label("Heading 3")]);

        format_bar.append(&h1_btn);
        format_bar.append(&h2_btn);
        format_bar.append(&h3_btn);

        let fb_sep2 = Separator::new(Orientation::Vertical);
        fb_sep2.set_margin_top(6);
        fb_sep2.set_margin_bottom(6);
        fb_sep2.set_margin_start(4);
        fb_sep2.set_margin_end(4);
        format_bar.append(&fb_sep2);

        let pb_btn = Button::with_label("¶");
        pb_btn.add_css_class("flat");
        pb_btn.add_css_class("caption");
        pb_btn.set_tooltip_text(Some("Insert page break  (#pagebreak())"));
        pb_btn.update_property(&[gtk4::accessible::Property::Label("Insert page break")]);
        format_bar.append(&pb_btn);

        let hr_btn = Button::with_label("―");
        hr_btn.add_css_class("flat");
        hr_btn.add_css_class("caption");
        hr_btn.set_tooltip_text(Some("Insert horizontal rule  (#line(length: 100%))"));
        hr_btn.update_property(&[gtk4::accessible::Property::Label("Insert horizontal rule")]);
        format_bar.append(&hr_btn);

        let fb_sep3 = Separator::new(Orientation::Vertical);
        fb_sep3.set_margin_top(6);
        fb_sep3.set_margin_bottom(6);
        fb_sep3.set_margin_start(4);
        fb_sep3.set_margin_end(4);
        format_bar.append(&fb_sep3);

        let line_numbers_btn = ToggleButton::with_label("#");
        line_numbers_btn.add_css_class("flat");
        line_numbers_btn.add_css_class("caption");
        line_numbers_btn.set_tooltip_text(Some("Toggle line numbers"));
        line_numbers_btn
            .update_property(&[gtk4::accessible::Property::Label("Toggle line numbers")]);
        format_bar.append(&line_numbers_btn);

        let fb_sep3b = Separator::new(Orientation::Vertical);
        fb_sep3b.set_margin_top(6);
        fb_sep3b.set_margin_bottom(6);
        fb_sep3b.set_margin_start(4);
        fb_sep3b.set_margin_end(4);
        format_bar.append(&fb_sep3b);

        // ── Insert table (grid picker) ──────────────────────────────────────
        let table_popover = Popover::new();
        let table_grid_box = GtkBox::new(Orientation::Vertical, 2);
        table_grid_box.set_margin_top(6);
        table_grid_box.set_margin_bottom(6);
        table_grid_box.set_margin_start(6);
        table_grid_box.set_margin_end(6);
        let table_size_lbl = Label::new(Some("Insert table"));
        table_size_lbl.add_css_class("caption");
        table_size_lbl.add_css_class("dim-label");
        table_grid_box.append(&table_size_lbl);

        // 8×8 grid of cells; hover to highlight, click to insert
        const GRID_MAX: usize = 8;
        let selected_rows: Rc<std::cell::Cell<i32>> = Rc::new(std::cell::Cell::new(0));
        let selected_cols: Rc<std::cell::Cell<i32>> = Rc::new(std::cell::Cell::new(0));
        let mut grid_btns: Vec<Vec<Button>> = Vec::new();
        for r in 0..GRID_MAX {
            let row_box = GtkBox::new(Orientation::Horizontal, 1);
            let mut row_btns: Vec<Button> = Vec::new();
            for c in 0..GRID_MAX {
                let cell = Button::new();
                cell.set_size_request(22, 20);
                cell.add_css_class("table-grid-cell");
                cell.update_property(&[gtk4::accessible::Property::Label(&format!(
                    "{}×{} table",
                    r + 1,
                    c + 1
                ))]);
                row_btns.push(cell.clone());
                row_box.append(&cell);
            }
            grid_btns.push(row_btns);
            table_grid_box.append(&row_box);
        }
        // Wrap grid_btns in Rc so hover handlers can update all cells
        let grid_rc: Rc<Vec<Vec<Button>>> = Rc::new(grid_btns.to_vec());
        // Wire hover handlers (separate pass so all cells are available)
        for (r, row) in grid_btns.iter().enumerate().take(GRID_MAX) {
            for (c, cell) in row.iter().enumerate().take(GRID_MAX) {
                let cell = cell.clone();
                let sr = selected_rows.clone();
                let sc = selected_cols.clone();
                let lbl = table_size_lbl.clone();
                let gc = grid_rc.clone();
                let mc = EventControllerMotion::new();
                mc.connect_enter(move |_, _, _| {
                    sr.set(r as i32 + 1);
                    sc.set(c as i32 + 1);
                    lbl.set_text(&format!("{}×{} table", r + 1, c + 1));
                    for (ri, row) in gc.iter().enumerate() {
                        for (ci, btn) in row.iter().enumerate() {
                            if ri <= r && ci <= c {
                                btn.add_css_class("table-grid-cell-selected");
                            } else {
                                btn.remove_css_class("table-grid-cell-selected");
                            }
                        }
                    }
                });
                cell.add_controller(mc);
            }
        }
        // Clear highlights when pointer leaves the grid
        {
            let gc = grid_rc.clone();
            let lbl = table_size_lbl.clone();
            let mc_leave = EventControllerMotion::new();
            mc_leave.connect_leave(move |_| {
                lbl.set_text("Insert table");
                for row in gc.iter() {
                    for btn in row.iter() {
                        btn.remove_css_class("table-grid-cell-selected");
                    }
                }
            });
            table_grid_box.add_controller(mc_leave);
        }
        // Custom rows × cols entry below the grid
        let custom_sep = Separator::new(Orientation::Horizontal);
        custom_sep.set_margin_top(4);
        custom_sep.set_margin_bottom(2);
        table_grid_box.append(&custom_sep);

        let custom_row_box = GtkBox::new(Orientation::Horizontal, 4);
        custom_row_box.set_margin_top(2);

        let table_rows_entry = Entry::new();
        table_rows_entry.set_placeholder_text(Some("Rows"));
        table_rows_entry.set_input_purpose(gtk4::InputPurpose::Digits);
        table_rows_entry.set_width_chars(4);
        table_rows_entry.set_max_length(2);

        let table_x_lbl = Label::new(Some("×"));
        table_x_lbl.add_css_class("dim-label");

        let table_cols_entry = Entry::new();
        table_cols_entry.set_placeholder_text(Some("Cols"));
        table_cols_entry.set_input_purpose(gtk4::InputPurpose::Digits);
        table_cols_entry.set_width_chars(4);
        table_cols_entry.set_max_length(2);

        let table_custom_insert_btn = Button::with_label("Insert");
        table_custom_insert_btn.add_css_class("suggested-action");

        custom_row_box.append(&table_rows_entry);
        custom_row_box.append(&table_x_lbl);
        custom_row_box.append(&table_cols_entry);
        custom_row_box.append(&table_custom_insert_btn);
        table_grid_box.append(&custom_row_box);
        table_popover.set_child(Some(&table_grid_box));

        let table_btn = Button::new();
        table_btn.set_icon_name("x-office-spreadsheet-symbolic");
        table_btn.add_css_class("flat");
        table_btn.set_tooltip_text(Some("Insert table"));
        table_btn.update_property(&[gtk4::accessible::Property::Label("Insert table")]);
        table_popover.set_autohide(true);
        {
            let tp = table_popover.clone();
            let tb = table_btn.clone();
            table_btn.connect_clicked(move |_| {
                tp.set_parent(&tb);
                if tp.is_visible() {
                    tp.popdown();
                } else {
                    tp.popup();
                    tp.grab_focus();
                }
            });
        }
        format_bar.append(&table_btn);

        // ── Insert figure (file dialog) ──────────────────────────────────────
        let figure_btn = Button::new();
        figure_btn.set_icon_name("insert-image-symbolic");
        figure_btn.add_css_class("flat");
        figure_btn.set_tooltip_text(Some("Insert figure / image"));
        figure_btn.update_property(&[gtk4::accessible::Property::Label("Insert figure or image")]);
        format_bar.append(&figure_btn);

        // ── CV style switcher (shown only when editing a CV) ─────────────────
        let cv_sep = Separator::new(Orientation::Vertical);
        cv_sep.set_margin_top(6);
        cv_sep.set_margin_bottom(6);
        cv_sep.set_margin_start(4);
        cv_sep.set_margin_end(4);

        let cv_style_label = Label::new(Some("Modern"));
        cv_style_label.add_css_class("dim-label");
        cv_style_label.add_css_class("caption");

        let cv_style_popover = Popover::new();
        let cv_style_popover_box = GtkBox::new(Orientation::Vertical, 2);
        cv_style_popover_box.set_margin_top(4);
        cv_style_popover_box.set_margin_bottom(4);
        cv_style_popover_box.set_margin_start(4);
        cv_style_popover_box.set_margin_end(4);
        // Descriptions mirror the CV presets in the "New from Template" gallery
        // (see TEMPLATE_PRESETS in template_dialog.rs) so switching style here
        // carries the same explanation as picking it there.
        const CV_STYLE_DESCRIPTIONS: &[(&str, &str)] = &[
            ("Modern", "Clean résumé with colour accents, compact margins"),
            ("Academic", "Traditional academic CV with ruled section headers"),
            ("Classic", "Minimal timeless résumé, clean lines, no colour"),
            ("Two-Column", "Profile summary above a sidebar (Education, Skills & Awards) beside a main Experience column"),
        ];
        for (label, desc) in CV_STYLE_DESCRIPTIONS {
            let row = Button::new();
            row.add_css_class("flat");
            row.set_halign(gtk4::Align::Start);
            row.set_size_request(240, -1);
            let row_box = GtkBox::new(Orientation::Vertical, 1);
            row_box.set_margin_top(3);
            row_box.set_margin_bottom(3);
            let name_lbl = Label::new(Some(label));
            name_lbl.set_halign(gtk4::Align::Start);
            let desc_lbl = Label::new(Some(desc));
            desc_lbl.add_css_class("dim-label");
            desc_lbl.add_css_class("caption");
            desc_lbl.set_halign(gtk4::Align::Start);
            desc_lbl.set_wrap(true);
            desc_lbl.set_max_width_chars(30);
            row_box.append(&name_lbl);
            row_box.append(&desc_lbl);
            row.set_child(Some(&row_box));
            cv_style_popover_box.append(&row);
        }
        cv_style_popover.set_child(Some(&cv_style_popover_box));
        cv_style_popover.set_autohide(true);

        let cv_style_btn = Button::new();
        cv_style_btn.set_child(Some(&cv_style_label));
        cv_style_btn.add_css_class("flat");
        cv_style_btn.set_tooltip_text(Some("Switch CV visual style"));
        cv_style_btn.set_margin_start(4);
        {
            let sp = cv_style_popover.clone();
            let sb = cv_style_btn.clone();
            cv_style_btn.connect_clicked(move |_| {
                sp.set_parent(&sb);
                if sp.is_visible() {
                    sp.popdown();
                } else {
                    sp.popup();
                    sp.grab_focus();
                }
            });
        }

        let cv_format_section = GtkBox::new(Orientation::Horizontal, 0);
        cv_format_section.append(&cv_sep);
        cv_format_section.append(&cv_style_btn);
        cv_format_section.set_visible(false);
        format_bar.append(&cv_format_section);

        // ── Spacer ────────────────────────────────────────────────────────────
        let fb_spacer = GtkBox::new(Orientation::Horizontal, 0);
        fb_spacer.set_hexpand(true);
        format_bar.append(&fb_spacer);

        // ── Font dropdown (right-aligned) ────────────────────────────────────
        let enabled_fonts = FontManager::enabled_fonts();
        let font_popover = Popover::new();
        let font_popover_box = GtkBox::new(Orientation::Vertical, 2);
        font_popover_box.set_margin_top(4);
        font_popover_box.set_margin_bottom(4);
        font_popover_box.set_margin_start(4);
        font_popover_box.set_margin_end(4);
        let mut font_buttons: Vec<(String, Button)> = Vec::new();
        for font_name in &enabled_fonts {
            let row = Button::with_label(font_name);
            row.add_css_class("flat");
            row.set_halign(gtk4::Align::Start);
            row.set_size_request(260, -1);
            font_popover_box.append(&row);
            font_buttons.push((font_name.clone(), row));
        }
        let font_scroll = ScrolledWindow::new();
        font_scroll.set_child(Some(&font_popover_box));
        font_scroll.set_min_content_width(260);
        font_scroll.set_max_content_height(320);
        font_scroll.set_propagate_natural_height(true);
        font_scroll.set_propagate_natural_width(true);
        font_popover.set_child(Some(&font_scroll));
        font_popover.set_autohide(true);

        let font_bar_label = Label::new(Some("font"));
        font_bar_label.add_css_class("dim-label");
        font_bar_label.add_css_class("caption");
        let font_bar_btn = Button::new();
        font_bar_btn.set_child(Some(&font_bar_label));
        font_bar_btn.add_css_class("flat");
        font_bar_btn.set_tooltip_text(Some("Document body font"));
        font_bar_btn.set_margin_start(4);
        {
            let fp = font_popover.clone();
            let fb = font_bar_btn.clone();
            font_bar_btn.connect_clicked(move |_| {
                fp.set_parent(&fb);
                if fp.is_visible() {
                    fp.popdown();
                } else {
                    fp.popup();
                    fp.grab_focus();
                }
            });
        }
        format_bar.append(&font_bar_btn);

        // ── Font size dropdown (right-aligned) ────────────────────────────────
        const DOC_SIZES: &[&str] = &[
            "10pt", "11pt", "12pt", "14pt", "16pt", "18pt", "20pt", "24pt",
        ];
        let size_popover = Popover::new();
        let size_popover_box = GtkBox::new(Orientation::Vertical, 2);
        size_popover_box.set_margin_top(4);
        size_popover_box.set_margin_bottom(4);
        size_popover_box.set_margin_start(4);
        size_popover_box.set_margin_end(4);
        let mut size_buttons: Vec<(String, Button)> = Vec::new();
        for size_name in DOC_SIZES {
            let row = Button::with_label(size_name);
            row.add_css_class("flat");
            row.add_css_class("caption");
            row.set_halign(gtk4::Align::Start);
            row.set_size_request(80, -1);
            size_popover_box.append(&row);
            size_buttons.push((size_name.to_string(), row));
        }
        size_popover.set_child(Some(&size_popover_box));
        size_popover.set_autohide(true);

        let size_bar_label = Label::new(Some("size"));
        size_bar_label.add_css_class("dim-label");
        size_bar_label.add_css_class("caption");
        let size_bar_btn = Button::new();
        size_bar_btn.set_child(Some(&size_bar_label));
        size_bar_btn.add_css_class("flat");
        size_bar_btn.set_tooltip_text(Some("Document font size"));
        size_bar_btn.set_margin_start(2);
        {
            let sp = size_popover.clone();
            let sb = size_bar_btn.clone();
            size_bar_btn.connect_clicked(move |_| {
                sp.set_parent(&sb);
                if sp.is_visible() {
                    sp.popdown();
                } else {
                    sp.popup();
                    sp.grab_focus();
                }
            });
        }
        format_bar.append(&size_bar_btn);

        // ── Overflow menu ─────────────────────────────────────────────────────
        // The bar above can't shrink below the combined minimum width of every
        // button it holds, which used to force the editor pane to overflow
        // underneath the sidebar on narrow windows/splits. An AdwBreakpointBin
        // (which — unlike a plain GtkBox — is allowed to be allocated smaller
        // than its child's minimum size once it has breakpoints) wraps the bar
        // and, as space runs low, moves lower-priority controls out of the bar
        // and into this popover behind a trailing "more" button instead.
        let overflow_box = GtkBox::new(Orientation::Vertical, 2);
        overflow_box.set_margin_top(4);
        overflow_box.set_margin_bottom(4);
        overflow_box.set_margin_start(4);
        overflow_box.set_margin_end(4);

        let overflow_popover = Popover::new();
        overflow_popover.set_child(Some(&overflow_box));
        overflow_popover.set_autohide(true);

        let overflow_btn = Button::from_icon_name("pan-down-symbolic");
        overflow_btn.add_css_class("flat");
        overflow_btn.set_tooltip_text(Some("More formatting options"));
        overflow_btn.set_visible(false);
        overflow_btn
            .update_property(&[gtk4::accessible::Property::Label("More formatting options")]);
        {
            let op = overflow_popover.clone();
            let ob = overflow_btn.clone();
            overflow_btn.connect_clicked(move |_| {
                op.set_parent(&ob);
                if op.is_visible() {
                    op.popdown();
                } else {
                    op.popup();
                    op.grab_focus();
                }
            });
        }
        format_bar.append(&overflow_btn);

        // A plain GTK Button grabs keyboard focus on click by default, which
        // moved it off the document — clicking Bold left the cursor blinking
        // in the format bar, not the text, so the next keystroke went
        // nowhere until the user clicked back into the document. Formatting
        // actions like these are meant to apply to a selection and hand
        // control straight back, not become the new focus target — so every
        // widget in the bar is marked non-focusable. This naturally stops at
        // popover boundaries (a Popover attaches via set_parent, not as a
        // child in the anchor's own child chain), so the table/figure/font
        // pickers' own keyboard navigation inside their popovers is
        // unaffected.
        fn disable_focus_recursive(widget: &impl IsA<gtk4::Widget>) {
            let widget = widget.as_ref();
            widget.set_can_focus(false);
            let mut child = widget.first_child();
            while let Some(c) = child {
                disable_focus_recursive(&c);
                child = c.next_sibling();
            }
        }
        disable_focus_recursive(&format_bar);

        struct OverflowGroup {
            lead_separator: Option<gtk4::Widget>,
            controls: Vec<gtk4::Widget>,
            zone_b: bool,
        }

        // Collapse priority, least-important-first (mirrors visual order:
        // zone B — font/size — collapses right-to-left, then zone A —
        // headings/pagebreak/line-numbers/table/figure/CV style — also
        // collapses right-to-left).
        let overflow_groups: Rc<Vec<OverflowGroup>> = Rc::new(vec![
            OverflowGroup {
                lead_separator: None,
                controls: vec![size_bar_btn.clone().upcast()],
                zone_b: true,
            },
            OverflowGroup {
                lead_separator: None,
                controls: vec![font_bar_btn.clone().upcast()],
                zone_b: true,
            },
            OverflowGroup {
                lead_separator: None,
                controls: vec![cv_format_section.clone().upcast()],
                zone_b: false,
            },
            OverflowGroup {
                lead_separator: None,
                controls: vec![figure_btn.clone().upcast()],
                zone_b: false,
            },
            OverflowGroup {
                lead_separator: Some(fb_sep3b.clone().upcast()),
                controls: vec![table_btn.clone().upcast()],
                zone_b: false,
            },
            OverflowGroup {
                lead_separator: Some(fb_sep3.clone().upcast()),
                controls: vec![line_numbers_btn.clone().upcast()],
                zone_b: false,
            },
            OverflowGroup {
                lead_separator: Some(fb_sep2.clone().upcast()),
                controls: vec![pb_btn.clone().upcast(), hr_btn.clone().upcast()],
                zone_b: false,
            },
            OverflowGroup {
                lead_separator: Some(fb_sep1.clone().upcast()),
                controls: vec![
                    h1_btn.clone().upcast(),
                    h2_btn.clone().upcast(),
                    h3_btn.clone().upcast(),
                ],
                zone_b: false,
            },
        ]);

        // Rolling "insert after" anchors: the current rightmost visible widget
        // in each zone. Restoring a group pushes its last widget as the new
        // anchor; collapsing a group pops back to the previous one. So while
        // the bar is fully expanded each stack must already hold the whole
        // chain, bottom-to-top: the zone's fixed base widget, then the last
        // control of every group in reverse collapse order. Seeding only the
        // base left the stack empty after the first collapse, and restoring
        // then unwrapped a None and aborted the app.
        let zone_a_anchor_stack: Rc<RefCell<Vec<gtk4::Widget>>> = Rc::new(RefCell::new(vec![
            italic_btn.clone().upcast(),
            h3_btn.clone().upcast(),
            hr_btn.clone().upcast(),
            line_numbers_btn.clone().upcast(),
            table_btn.clone().upcast(),
            figure_btn.clone().upcast(),
            cv_format_section.clone().upcast(),
        ]));
        let zone_b_anchor_stack: Rc<RefCell<Vec<gtk4::Widget>>> = Rc::new(RefCell::new(vec![
            fb_spacer.clone().upcast(),
            font_bar_btn.clone().upcast(),
            size_bar_btn.clone().upcast(),
        ]));
        let zone_a_base: gtk4::Widget = italic_btn.clone().upcast();
        let zone_b_base: gtk4::Widget = fb_spacer.clone().upcast();
        let overflow_stage: Rc<Cell<usize>> = Rc::new(Cell::new(0));

        let set_overflow_stage = {
            let overflow_groups = overflow_groups.clone();
            let zone_a_anchor_stack = zone_a_anchor_stack.clone();
            let zone_b_anchor_stack = zone_b_anchor_stack.clone();
            let zone_a_base = zone_a_base.clone();
            let zone_b_base = zone_b_base.clone();
            let overflow_stage = overflow_stage.clone();
            let overflow_box = overflow_box.clone();
            let overflow_btn = overflow_btn.clone();
            let format_bar = format_bar.clone();
            move |target: usize| {
                let target = target.min(overflow_groups.len());
                let mut stage = overflow_stage.get();
                while stage < target {
                    let group = &overflow_groups[stage];
                    for control in &group.controls {
                        control.unparent();
                        overflow_box.append(control);
                    }
                    if let Some(sep) = &group.lead_separator {
                        sep.unparent();
                    }
                    let stack = if group.zone_b {
                        &zone_b_anchor_stack
                    } else {
                        &zone_a_anchor_stack
                    };
                    stack.borrow_mut().pop();
                    stage += 1;
                }
                while stage > target {
                    stage -= 1;
                    let group = &overflow_groups[stage];
                    let stack = if group.zone_b {
                        &zone_b_anchor_stack
                    } else {
                        &zone_a_anchor_stack
                    };
                    let base = if group.zone_b {
                        &zone_b_base
                    } else {
                        &zone_a_base
                    };
                    // Never unwrap here: an unbalanced stack must degrade to a
                    // slightly odd button order, not abort the process (a panic
                    // in a GTK callback can't unwind and takes the app down).
                    let mut anchor = stack
                        .borrow()
                        .last()
                        .cloned()
                        .unwrap_or_else(|| base.clone());
                    if let Some(sep) = &group.lead_separator {
                        format_bar.insert_child_after(sep, Some(&anchor));
                        anchor = sep.clone();
                    }
                    for control in &group.controls {
                        control.unparent();
                        format_bar.insert_child_after(control, Some(&anchor));
                        anchor = control.clone();
                    }
                    stack.borrow_mut().push(anchor);
                }
                overflow_stage.set(stage);
                overflow_btn.set_visible(stage > 0);
            }
        };

        let format_bar_bin = adw::BreakpointBin::new();
        format_bar_bin.set_width_request(190);
        format_bar_bin.set_height_request(38);
        format_bar_bin.set_hexpand(true);
        format_bar_bin.set_child(Some(&format_bar));

        // Thresholds are generous on purpose (better to collapse a control
        // slightly before it's strictly necessary than to risk the bar
        // overflowing its own bin). Added widest-to-narrowest: AdwBreakpointBin
        // picks "the last added breakpoint whose condition matches", so at any
        // given width the narrowest still-matching one — i.e. the deepest
        // applicable collapse stage — always wins.
        const OVERFLOW_THRESHOLDS: &[f64] =
            &[760.0, 700.0, 650.0, 600.0, 550.0, 480.0, 420.0, 360.0];
        for (i, px) in OVERFLOW_THRESHOLDS.iter().enumerate() {
            let condition = adw::BreakpointCondition::new_length(
                adw::BreakpointConditionLengthType::MaxWidth,
                *px,
                adw::LengthUnit::Px,
            );
            let bp = adw::Breakpoint::new(condition);
            {
                let set_stage = set_overflow_stage.clone();
                bp.connect_apply(move |_| set_stage(i + 1));
            }
            {
                let set_stage = set_overflow_stage.clone();
                bp.connect_unapply(move |_| set_stage(i));
            }
            format_bar_bin.add_breakpoint(bp);
        }

        let format_bar_container = GtkBox::new(Orientation::Vertical, 0);
        format_bar_container.append(&format_bar_bin);
        format_bar_container.append(&Separator::new(Orientation::Horizontal));

        // Two rows, deliberately. Merging them into one was tried and reverted:
        // the formatting bar is an AdwBreakpointBin and needs most of the
        // editor's width to show its buttons, so sharing a row with undo/redo
        // and the citation style collapsed it to its smallest overflow stage —
        // hiding the very buttons the row exists for. One row only works at a
        // pane width this layout does not have.
        let frontmatter_banner = adw::Banner::new(
            "This is your document's technical setup — most people don't need to touch it. \
             Change it from the Template button instead of editing it directly.",
        );
        frontmatter_banner.set_button_label(Some("Got it"));
        frontmatter_banner.set_revealed(false);
        let shown_frontmatter_banner: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        {
            let banner = frontmatter_banner.clone();
            frontmatter_banner.connect_button_clicked(move |_| banner.set_revealed(false));
        }

        let outer = GtkBox::new(Orientation::Vertical, 0);
        outer.set_hexpand(true);
        outer.set_vexpand(true);
        outer.append(&breadcrumb_bar);
        outer.append(&Separator::new(Orientation::Horizontal));
        outer.append(&format_bar_container);
        outer.append(&frontmatter_banner);
        outer.append(&notebook.bar);
        outer.append(&editor_overlay);
        outer.append(find_bar.widget());
        // Note: status_bar is intentionally NOT appended here.
        // app_window places it below inner_paned so it spans the full window width.

        let on_change: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_modified_changed: Rc<RefCell<Option<Box<dyn Fn(bool)>>>> =
            Rc::new(RefCell::new(None));
        let on_file_dirty: Rc<RefCell<Option<Box<dyn Fn(PathBuf, bool)>>>> =
            Rc::new(RefCell::new(None));
        let on_image_drop: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>> = Rc::new(RefCell::new(None));
        let on_document_drop: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>> =
            Rc::new(RefCell::new(None));
        let on_delete_file: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>> = Rc::new(RefCell::new(None));
        let on_page_switch: Rc<RefCell<Option<Box<dyn Fn(String, PathBuf)>>>> =
            Rc::new(RefCell::new(None));
        let on_file_opened: Rc<RefCell<Option<Box<dyn Fn(PathBuf, String)>>>> =
            Rc::new(RefCell::new(None));
        let on_completion_needed: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32, u32)>>>> =
            Rc::new(RefCell::new(None));
        let on_cursor_heading: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32)>>>> =
            Rc::new(RefCell::new(None));
        let on_cursor_moved: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32, u32)>>>> =
            Rc::new(RefCell::new(None));
        let on_autocorrect_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>> =
            Rc::new(RefCell::new(None));
        let on_gost_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>> = Rc::new(RefCell::new(None));
        let on_version_click: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_word_count_click: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_simple_mode_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>> =
            Rc::new(RefCell::new(None));

        let font_size: Rc<RefCell<u32>> = Rc::new(RefCell::new(13));
        let font_family: Rc<RefCell<String>> = Rc::new(RefCell::new("Monospace".to_string()));
        let show_whitespace: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let tab_width: Rc<RefCell<u32>> = Rc::new(RefCell::new(2));
        let line_spacing: Rc<RefCell<u32>> = Rc::new(RefCell::new(2));
        let typewriter_scroll: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let word_count_goal: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let default_word_count_goal: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let last_wc_text: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
        let project_root: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));

        {
            let state2 = state.clone();
            let wc = word_count_label.clone();
            let ps = on_page_switch.clone();
            let ub = undo_btn.clone();
            let rb = redo_btn.clone();
            let nb = notebook.clone();
            notebook.view.connect_selected_page_notify(move |_| {
                let Some(page_num) = nb.current_page() else {
                    return;
                };
                // Extract content/path and release the state borrow before calling the
                // page-switch callback, which may call all_tab_texts() → double-borrow panic.
                let page_data = {
                    let bstate = state2.borrow();
                    let mut found = None;
                    for (path, tab) in &bstate.tabs {
                        if nb.page_num(&tab.notebook_page) == Some(page_num) {
                            let (s, e) = tab.buffer.bounds();
                            let content = tab.buffer.text(&s, &e, true).to_string();
                            let can_undo = tab.buffer.can_undo();
                            let can_redo = tab.buffer.can_redo();
                            let session_start = tab.session_start_words;
                            found =
                                Some((path.clone(), content, can_undo, can_redo, session_start));
                            break;
                        }
                    }
                    found
                };
                if let Some((path, content, can_undo, can_redo, session_start)) = page_data {
                    wc.set_text(&wc_str_with_delta(&content, session_start));
                    ub.set_sensitive(can_undo);
                    rb.set_sensitive(can_redo);
                    if let Some(f) = ps.borrow().as_ref() {
                        f(content, path);
                    }
                }
            });
        }

        let ep = Self {
            outer,
            notebook,
            typewriter_crosshair,
            typewriter_crosshair_timer: Rc::new(RefCell::new(None)),
            state,
            on_change,
            on_modified_changed,
            on_file_dirty,
            save_problems: Rc::new(RefCell::new(Vec::new())),
            on_save_problems: Rc::new(RefCell::new(None)),
            on_image_drop,
            on_document_drop,
            on_delete_file,
            on_page_switch,
            on_file_opened,
            on_completion_needed,
            on_cursor_heading,
            on_cursor_moved,
            bib_entries: Rc::new(RefCell::new(Vec::new())),
            cited_keys: Rc::new(RefCell::new(std::collections::HashSet::new())),
            cv_entries: Rc::new(RefCell::new(Vec::new())),
            font_provider: Rc::new(font_provider),
            font_size,
            font_family,
            show_whitespace,
            tab_width,
            find_bar,
            undo_btn,
            redo_btn,
            word_count_label,
            session_delta_label,
            goal_ring,
            goal_fraction,
            goal_celebrating,
            lsp_status_label,
            diag_label,
            diag_btn,
            on_diag_click,
            on_fix_request: Rc::new(RefCell::new(None)),
            last_edit: Rc::new(Cell::new(None)),
            last_diagnostics: Rc::new(RefCell::new(Vec::new())),
            cursor_label,
            section_wc_label,
            breadcrumb_label,
            breadcrumb_bar,
            simple_mode: Rc::new(RefCell::new(true)),
            simple_mode_label: simple_mode_label.clone(),
            on_simple_mode_toggle,
            spell_checker: Rc::new(RefCell::new(crate::spellcheck::SpellChecker::new(vec![
                "en_US".to_string(),
            ]))),
            line_spacing,
            typewriter_scroll,
            jumping: Rc::new(Cell::new(false)),
            zen_on: Rc::new(Cell::new(false)),
            search_context: Rc::new(RefCell::new(None)),
            on_show_in_preview: Rc::new(RefCell::new(None)),
            word_count_goal,
            default_word_count_goal,
            last_wc_text,
            project_root,
            status_bar,
            simple_mode_btn: simple_mode_btn.clone(),
            frontmatter_banner: frontmatter_banner.clone(),
            shown_frontmatter_banner: shown_frontmatter_banner.clone(),
            focus_toggle_btn: focus_toggle_btn.clone(),
            gost_btn: gost_btn.clone(),
            lsp_ready: Rc::new(Cell::new(false)),
            completion_picks: Rc::new(RefCell::new(std::collections::HashMap::new())),
            autocorrect_label,
            autocorrect_btn: autocorrect_btn.clone(),
            on_autocorrect_toggle,
            gost_label,
            gost_enabled: Rc::new(RefCell::new(false)),
            gost_restoring: Rc::new(Cell::new(false)),
            on_gost_toggle,
            on_version_click,
            on_word_count_click,
            bib_active: Rc::new(RefCell::new(false)),
            format_bar_container,
            format_bar_label,
            format_bar_toggle_btn: format_bar_toggle_btn.clone(),
            on_format_bar_toggle: Rc::new(RefCell::new(None)),
            autosave_label,
            safe_label,
            autosave_toggle_btn: autosave_toggle_btn.clone(),
            autosave_on: Rc::new(Cell::new(false)),
            on_autosave_toggle: Rc::new(RefCell::new(None)),
            user_dismissed_format_bar: Rc::new(RefCell::new(false)),
            focus_label,
            on_focus_toggle: Rc::new(RefCell::new(None)),
            on_doc_font: Rc::new(RefCell::new(None)),
            on_doc_font_size: Rc::new(RefCell::new(None)),
            font_bar_label,
            size_bar_label,
            line_numbers_override: Rc::new(Cell::new(false)),
            line_numbers_btn,
            cv_mode: Rc::new(Cell::new(false)),
            cv_format_section,
            cv_style_label,
        };

        // Wire CV style buttons
        {
            let mut child_opt = cv_style_popover_box.first_child();
            for style in &["modern", "academic", "classic", "sidebar"] {
                let Some(child) = child_opt else { break };
                let next = child.next_sibling();
                let Some(btn) = child.downcast_ref::<Button>() else {
                    child_opt = next;
                    continue;
                };
                let ep_cv = ep.clone();
                let style_s = style.to_string();
                let pop = cv_style_popover.clone();
                btn.connect_clicked(move |_| {
                    pop.popdown();
                    ep_cv.apply_cv_style(&style_s);
                });
                child_opt = next;
            }
        }

        {
            let ep_ln = ep.clone();
            ep.line_numbers_btn.connect_toggled(move |btn| {
                let on = btn.is_active();
                ep_ln.line_numbers_override.set(on);
                let simple = *ep_ln.simple_mode.borrow();
                let show = on || !simple;
                let views: Vec<_> = {
                    let state = ep_ln.state.borrow();
                    state.tabs.values().map(|t| t.view.clone()).collect()
                };
                for v in &views {
                    v.set_show_line_numbers(show);
                }
            });
        }
        {
            let cb = ep.on_version_click.clone();
            version_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() {
                    f();
                }
            });
        }
        {
            let cb = ep.on_word_count_click.clone();
            wc_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() {
                    f();
                }
            });
        }
        {
            let fb = ep.find_bar.clone();
            search_btn.connect_clicked(move |_| {
                fb.toggle();
            });
        }
        {
            let sl = search_label.clone();
            let ep_focus = ep.clone();
            ep.find_bar.set_on_reveal_changed(move |revealed| {
                set_toggle_label(&sl, "search", revealed);
                if !revealed {
                    ep_focus.clear_search_highlight();
                    ep_focus.grab_focus();
                }
            });
        }
        {
            let lbl_g = ep.gost_label.clone();
            let cb_g = ep.on_gost_toggle.clone();
            let gost_on = ep.gost_enabled.clone();
            gost_btn.connect_clicked(move |_| {
                let new_val = !*gost_on.borrow();
                *gost_on.borrow_mut() = new_val;
                set_toggle_label(&lbl_g, "GOST Type B font", new_val);
                if let Some(f) = cb_g.borrow().as_ref() {
                    f(new_val);
                }
            });
        }
        {
            let ep_as = ep.clone();
            autosave_toggle_btn.connect_clicked(move |_| {
                let new_val = !ep_as.autosave_enabled();
                ep_as.set_autosave(new_val);
                if let Some(f) = ep_as.on_autosave_toggle.borrow().as_ref() {
                    f(new_val);
                }
            });
        }
        {
            let ep_fb = ep.clone();
            format_bar_toggle_btn.connect_clicked(move |_| {
                let new_val = !ep_fb.format_bar_visible();
                ep_fb.set_format_bar_visible(new_val);
                *ep_fb.user_dismissed_format_bar.borrow_mut() = !new_val;
                if let Some(f) = ep_fb.on_format_bar_toggle.borrow().as_ref() {
                    f(new_val);
                }
            });
        }
        {
            let ep_focus = ep.clone();
            let focus_active: Rc<std::cell::Cell<bool>> = Rc::new(std::cell::Cell::new(false));
            let ftb = focus_toggle_btn.clone();
            focus_toggle_btn.connect_clicked(move |_| {
                let new_val = !focus_active.get();
                focus_active.set(new_val);
                set_status_toggle(&ftb, &ep_focus.focus_label, "focus", new_val);
                if let Some(f) = ep_focus.on_focus_toggle.borrow().as_ref() {
                    f(new_val);
                }
            });
        }

        ep.wire_tab_view();

        // Restore focus to editor when format bar popovers close (item 2)
        for pop in [&table_popover, &font_popover, &size_popover] {
            let ep_fc = ep.clone();
            pop.connect_closed(move |_| {
                ep_fc.grab_focus();
            });
        }

        // Wire font dropdown rows
        for (font_name, btn) in &font_buttons {
            let fn2 = font_name.clone();
            let ep_f = ep.clone();
            let fp = font_popover.clone();
            // The label is set by the handler, not here: an edit the document
            // can't take (no template block) used to leave the bar claiming a
            // font the file never got.
            btn.connect_clicked(move |_| {
                fp.popdown();
                if let Some(f) = ep_f.on_doc_font.borrow().as_ref() {
                    f(fn2.clone());
                }
            });
        }
        // Wire size dropdown rows
        for (size_name, btn) in &size_buttons {
            let sn2 = size_name.clone();
            let ep_s = ep.clone();
            let sp = size_popover.clone();
            btn.connect_clicked(move |_| {
                sp.popdown();
                if let Some(f) = ep_s.on_doc_font_size.borrow().as_ref() {
                    f(sn2.clone());
                }
            });
        }
        // Wire table grid cell clicks (insert Typst table)
        for (ri, row_btns) in grid_btns.iter().enumerate() {
            for (ci, cell) in row_btns.iter().enumerate() {
                let ep_t = ep.clone();
                let rows = ri + 1;
                let cols = ci + 1;
                let tp2 = table_popover.clone();
                let sr2 = selected_rows.clone();
                let sc2 = selected_cols.clone();
                cell.connect_clicked(move |_| {
                    tp2.popdown();
                    let r = if sr2.get() > 0 { sr2.get() as usize } else { rows };
                    let c = if sc2.get() > 0 { sc2.get() as usize } else { cols };
                    sr2.set(0); sc2.set(0);
                    if let Some((_, buf)) = ep_t.active_view_buffer() {
                        let header_cols: String = (1..=c).map(|j| format!("[*Col {j}*]")).collect::<Vec<_>>().join(", ");
                        let data_cols: String = (1..=c).map(|_| "[ ]".to_string()).collect::<Vec<_>>().join(", ");
                        let data_rows: String = (1..=r).map(|_| format!("    {data_cols},")).collect::<Vec<_>>().join("\n");
                        let snippet = format!(
                            "#figure(\n  table(\n    columns: {c},\n    table.header({header_cols}),\n{data_rows}\n  ),\n  caption: [Caption],\n) <tab:label>\n"
                        );
                        buf.insert_at_cursor(&snippet);
                    }
                });
            }
        }
        // Wire custom table size insert button
        {
            let ep_ci = ep.clone();
            let tp_ci = table_popover.clone();
            let re = table_rows_entry.clone();
            let ce = table_cols_entry.clone();
            table_custom_insert_btn.connect_clicked(move |_| {
                let r: usize = re.text().parse().unwrap_or(0);
                let c: usize = ce.text().parse().unwrap_or(0);
                if r == 0 || c == 0 { return; }
                tp_ci.popdown();
                if let Some((_, buf)) = ep_ci.active_view_buffer() {
                    let header_cols: String = (1..=c).map(|j| format!("[*Col {j}*]")).collect::<Vec<_>>().join(", ");
                    let data_cols: String = (1..=c).map(|_| "[ ]".to_string()).collect::<Vec<_>>().join(", ");
                    let data_rows: String = (1..=r).map(|_| format!("    {data_cols},")).collect::<Vec<_>>().join("\n");
                    let snippet = format!(
                        "#figure(\n  table(\n    columns: {c},\n    table.header({header_cols}),\n{data_rows}\n  ),\n  caption: [Caption],\n) <tab:label>\n"
                    );
                    buf.insert_at_cursor(&snippet);
                }
            });
        }
        // Wire figure/image button (file dialog)
        {
            let ep_img = ep.clone();
            figure_btn.connect_clicked(move |_| {
                let dialog = gtk4::FileDialog::new();
                let filter = gtk4::FileFilter::new();
                filter.set_name(Some("Images"));
                filter.add_pattern("*.png");
                filter.add_pattern("*.jpg");
                filter.add_pattern("*.jpeg");
                filter.add_pattern("*.svg");
                filter.add_pattern("*.webp");
                let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
                filters.append(&filter);
                dialog.set_filters(Some(&filters));
                let on_image = ep_img.on_image_drop.clone();
                dialog.open(
                    gtk4::Window::NONE,
                    gtk4::gio::Cancellable::NONE,
                    move |result| {
                        if let Some(path) = result.ok().and_then(|f| f.path()) {
                            if let Some(f) = on_image.borrow().as_ref() {
                                f(path);
                            }
                        }
                    },
                );
            });
        }
        {
            let sc_ac = ep.spell_checker.clone();
            let lbl_ac = ep.autocorrect_label.clone();
            let cb_ac = ep.on_autocorrect_toggle.clone();
            autocorrect_btn.connect_clicked(move |_| {
                let new_val = !sc_ac.borrow().autocorrect;
                sc_ac.borrow_mut().autocorrect = new_val;
                set_autocorrect_label(&lbl_ac, new_val);
                if let Some(f) = cb_ac.borrow().as_ref() {
                    f(new_val);
                }
            });
        }

        {
            let ep_b = ep.clone();
            bold_btn.connect_clicked(move |_| {
                ep_b.toggle_active_markup("*");
            });
        }
        {
            let ep_i = ep.clone();
            italic_btn.connect_clicked(move |_| {
                ep_i.toggle_active_markup("_");
            });
        }
        {
            let ep_h1 = ep.clone();
            h1_btn.connect_clicked(move |_| {
                ep_h1.set_active_heading(1);
            });
        }
        {
            let ep_h2 = ep.clone();
            h2_btn.connect_clicked(move |_| {
                ep_h2.set_active_heading(2);
            });
        }
        {
            let ep_h3 = ep.clone();
            h3_btn.connect_clicked(move |_| {
                ep_h3.set_active_heading(3);
            });
        }
        {
            let ep_pb = ep.clone();
            pb_btn.connect_clicked(move |_| {
                if let Some((_v, buf)) = ep_pb.active_view_buffer() {
                    buf.insert_at_cursor("\n#pagebreak()\n");
                }
            });
        }
        {
            let ep_hr = ep.clone();
            hr_btn.connect_clicked(move |_| {
                if let Some((_v, buf)) = ep_hr.active_view_buffer() {
                    buf.insert_at_cursor("\n#line(length: 100%)\n");
                }
            });
        }

        {
            let state_u = ep.state.clone();
            let nb_u = ep.notebook.clone();
            ep.undo_btn.connect_clicked(move |_| {
                let current = nb_u.current_page().unwrap_or(0);
                let buffer = {
                    let state = state_u.borrow();
                    state
                        .tabs
                        .values()
                        .find(|tab| nb_u.page_num(&tab.notebook_page) == Some(current))
                        .map(|tab| tab.buffer.clone())
                };
                if let Some(buf) = buffer {
                    buf.undo();
                }
            });
        }
        {
            let state_r = ep.state.clone();
            let nb_r = ep.notebook.clone();
            ep.redo_btn.connect_clicked(move |_| {
                let current = nb_r.current_page().unwrap_or(0);
                let buffer = {
                    let state = state_r.borrow();
                    state
                        .tabs
                        .values()
                        .find(|tab| nb_r.page_num(&tab.notebook_page) == Some(current))
                        .map(|tab| tab.buffer.clone())
                };
                if let Some(buf) = buffer {
                    buf.redo();
                }
            });
        }
        {
            let ep2 = ep.clone();
            ep.find_bar
                .set_on_search(move |text, forward| ep2.do_find(text, forward));
        }
        {
            let ep2 = ep.clone();
            ep.find_bar
                .set_on_replace_one(move |find, replace| ep2.do_replace_one(find, replace));
        }
        {
            let ep2 = ep.clone();
            ep.find_bar
                .set_on_replace_all(move |find, replace| ep2.do_replace_all(find, replace));
        }

        // SIMPLE mode button
        {
            let ep2 = ep.clone();
            simple_mode_btn.connect_clicked(move |_| {
                let new_val = !*ep2.simple_mode.borrow();
                ep2.apply_simple_mode(new_val);
                if let Some(f) = ep2.on_simple_mode_toggle.borrow().as_ref() {
                    f(new_val);
                }
            });
        }

        ep
    }

    pub fn widget(&self) -> &GtkBox {
        &self.outer
    }

    /// The status bar widget — placed by app_window below the full-width inner_paned.
    pub fn status_bar_widget(&self) -> &GtkBox {
        &self.status_bar
    }

    pub fn status_bar_insert_after_goal(&self, w: &impl gtk4::prelude::IsA<gtk4::Widget>) {
        self.status_bar.insert_child_after(w, Some(&self.goal_ring));
    }

    /// Buttons built here but placed by the caller: Simple Mode and Focus sit
    /// in the header beside Library, and the GOST font switch in the hamburger
    /// menu. They keep all their wiring — only their parent differs.
    pub fn simple_mode_button_for_header(&self) -> Button {
        self.simple_mode_btn.clone()
    }

    pub fn focus_button_for_header(&self) -> Button {
        self.focus_toggle_btn.clone()
    }

    pub fn gost_button_for_menu(&self) -> Button {
        self.gost_btn.clone()
    }

    /// Autocorrect is a setting you change once, not a status to keep on
    /// screen, so it sits in the menu beside the font switch.
    pub fn autocorrect_button_for_menu(&self) -> Button {
        self.autocorrect_btn.clone()
    }

    /// Told by app_window once it knows whether tinymist actually started.
    pub fn set_lsp_available(&self, ready: bool) {
        self.lsp_ready.set(ready);
    }

    /// Seed the remembered prefix → name picks when a project is opened.
    pub fn set_completion_picks(&self, picks: std::collections::HashMap<String, String>) {
        *self.completion_picks.borrow_mut() = picks;
    }

    // ── Settings ──────────────────────────────────────────────────────────────

    /// Keys the open document cites, so the `@` popup lists them first.
    pub fn set_cited_keys(&self, keys: std::collections::HashSet<String>) {
        *self.cited_keys.borrow_mut() = keys;
    }

    pub fn set_bib_entries(&self, entries: Vec<BibEntry>) {
        *self.bib_entries.borrow_mut() = entries;
    }

    pub fn set_cv_entries(&self, entries: Vec<skrizhal_core::CvEntry>) {
        *self.cv_entries.borrow_mut() = entries;
    }

    pub fn apply_font_size(&self, size: u32) {
        *self.font_size.borrow_mut() = size;
        self.rebuild_font_css();
    }

    pub fn apply_font_family(&self, family: &str) {
        *self.font_family.borrow_mut() = family.to_string();
        self.rebuild_font_css();
    }

    fn rebuild_font_css(&self) {
        let size = *self.font_size.borrow();
        let family = self.font_family.borrow().clone();
        let css = if size > 0 {
            format!("textview {{ font-family: '{family}'; font-size: {size}pt; }}")
        } else {
            format!("textview {{ font-family: '{family}'; }}")
        };
        self.font_provider.load_from_data(&css);
        if self.zen_on.get() {
            self.set_zen_width(true);
        }
        // Force a redraw on all open views so the font change is immediately visible
        // on every tab, not just the active one.
        for tab in self.state.borrow().tabs.values() {
            tab.view.queue_draw();
        }
    }

    pub fn set_project_root(&self, path: PathBuf) {
        self.spell_checker.borrow_mut().set_project_root(&path);
        *self.project_root.borrow_mut() = Some(path);
    }

    /// Put a widget in the status bar's left group, after the mode toggles.
    /// Used to move header controls that report state rather than act on the
    /// document — the status bar is a line of plain words, and a toggle reads
    /// better there than as one more button in a crowded header.
    pub fn status_bar_append_left(&self, w: &impl gtk4::prelude::IsA<gtk4::Widget>) {
        self.status_bar
            .insert_child_after(w, Some(&self.format_bar_toggle_btn));
    }

    pub fn breadcrumb_bar_append(&self, w: &impl gtk4::prelude::IsA<gtk4::Widget>) {
        self.breadcrumb_bar.append(w);
    }

    pub fn apply_show_whitespace(&self, enabled: bool) {
        *self.show_whitespace.borrow_mut() = enabled;
        let views: Vec<_> = {
            let state = self.state.borrow();
            state.tabs.values().map(|t| t.view.clone()).collect()
        };
        for view in &views {
            apply_space_drawer(view, enabled);
        }
    }

    pub fn apply_tab_width(&self, width: u32) {
        *self.tab_width.borrow_mut() = width;
        let w = width.max(1);
        let views: Vec<_> = {
            let state = self.state.borrow();
            state.tabs.values().map(|t| t.view.clone()).collect()
        };
        for view in &views {
            view.set_tab_width(w);
            view.set_indent_width(w as i32);
        }
    }

    pub fn apply_line_spacing(&self, spacing: u32) {
        *self.line_spacing.borrow_mut() = spacing;
        let views: Vec<_> = {
            let state = self.state.borrow();
            state.tabs.values().map(|t| t.view.clone()).collect()
        };
        for view in &views {
            set_view_line_spacing(view, spacing);
        }
    }

    pub fn apply_typewriter_scroll(&self, enabled: bool) {
        *self.typewriter_scroll.borrow_mut() = enabled;
        for tab in self.state.borrow().tabs.values() {
            let pad = if enabled { tab.view.height() / 2 } else { 0 };
            tab.view.set_bottom_margin(pad);
        }
    }

    /// Constrain editor to a comfortable reading width when zen/focus mode is on.
    pub fn set_zen_width(&self, enabled: bool) {
        self.zen_on.set(enabled);
        if enabled {
            self.outer.set_halign(gtk4::Align::Center);
            self.outer.set_size_request(self.zen_pixel_width(), -1);
        } else {
            self.outer.set_halign(gtk4::Align::Fill);
            self.outer.set_size_request(-1, -1);
        }
    }

    /// About 70 characters of the current editor font, plus the margins, so the
    /// line length stays comfortable at any font size.
    fn zen_pixel_width(&self) -> i32 {
        const MEASURE_CHARS: i32 = 70;
        const FALLBACK: i32 = 720;
        let Some((view, _)) = self.active_view_buffer() else {
            return FALLBACK;
        };
        let metrics = view.pango_context().metrics(None, None);
        let char_px = metrics.approximate_char_width() / gtk4::pango::SCALE;
        if char_px <= 0 {
            return FALLBACK;
        }
        (char_px * MEASURE_CHARS + view.left_margin() + view.right_margin() + 24).clamp(480, 1400)
    }

    pub fn grab_focus(&self) {
        if let Some((view, _)) = self.active_view_buffer() {
            view.grab_focus();
        }
    }

    pub fn apply_style_scheme(&self, is_dark: bool) {
        let scheme = StyleSchemeManager::default().scheme(scheme_id_for(is_dark));
        let buffers: Vec<_> = {
            let state = self.state.borrow();
            state.tabs.values().map(|t| t.buffer.clone()).collect()
        };
        for buffer in &buffers {
            buffer.set_style_scheme(scheme.as_ref());
        }
    }

    // ── Find & Replace ────────────────────────────────────────────────────────

    // ── LSP completions ───────────────────────────────────────────────────────

    // ── Inline diagnostic marks ───────────────────────────────────────────────

    pub fn is_bib_active(&self) -> bool {
        *self.bib_active.borrow()
    }

    /// Renames citation-key occurrences (`@key`, `#cite(<key>)`, `#cite("key")`)
    /// in every currently open tab. Returns the paths of tabs that changed.
    pub fn replace_citation_key_in_open_tabs(&self, old_key: &str, new_key: &str) -> Vec<PathBuf> {
        let tabs: Vec<(PathBuf, Buffer)> = self
            .state
            .borrow()
            .tabs
            .iter()
            .map(|(p, t)| (p.clone(), t.buffer.clone()))
            .collect();

        let mut changed_paths = Vec::new();
        for (path, buf) in tabs {
            let (s, e) = buf.bounds();
            let text = buf.text(&s, &e, true).to_string();
            let (new_text, changed) =
                crate::bibliography::rename_key_in_text(&text, old_key, new_key);
            if !changed {
                continue;
            }
            replace_text_minimally(&buf, &new_text);
            changed_paths.push(path);
        }
        changed_paths
    }

    /// Paths of every tab currently open in this editor.
    pub fn open_tab_paths(&self) -> Vec<PathBuf> {
        self.state.borrow().tabs.keys().cloned().collect()
    }

    // ── Callbacks ─────────────────────────────────────────────────────────────

    pub fn set_on_change(&self, f: impl Fn() + 'static) {
        *self.on_change.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_modified_changed(&self, f: impl Fn(bool) + 'static) {
        *self.on_modified_changed.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_image_drop(&self, f: impl Fn(PathBuf) + 'static) {
        *self.on_image_drop.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_document_drop(&self, f: impl Fn(PathBuf) + 'static) {
        *self.on_document_drop.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_delete_file(&self, f: impl Fn(PathBuf) + 'static) {
        *self.on_delete_file.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_page_switch(&self, f: impl Fn(String, PathBuf) + 'static) {
        *self.on_page_switch.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_file_opened(&self, f: impl Fn(PathBuf, String) + 'static) {
        *self.on_file_opened.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_completion_needed(&self, f: impl Fn(PathBuf, u32, u32) + 'static) {
        *self.on_completion_needed.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_cursor_heading(&self, f: impl Fn(PathBuf, u32) + 'static) {
        *self.on_cursor_heading.borrow_mut() = Some(Box::new(f));
    }

    #[allow(dead_code)] // cursor-position readout, wired by callers not yet built
    pub fn set_on_cursor_moved(&self, f: impl Fn(PathBuf, u32, u32) + 'static) {
        *self.on_cursor_moved.borrow_mut() = Some(Box::new(f));
    }

    /// See `styles::combines_serial_citations`.
    pub fn set_combine_serial_citations(&self, combine: bool) {
        let (Some(path), Some(content)) = (self.get_active_path(), self.get_active_content())
        else {
            return;
        };
        let new_content = crate::styles::set_combine_serial_citations(&content, combine);
        if new_content != content {
            self.replace_buffer_content(&path, &new_content);
        }
    }

    pub fn apply_style(&self, style_code: &str, bib_style: &str, bib_title: &str, style_key: &str) {
        let Some(path) = self.get_active_path() else {
            return;
        };
        let Some(content) = self.get_active_content() else {
            return;
        };

        let new_content = if crate::styles::has_template_block(&content) {
            // Template document: update heading styles within the TEMPLATE block,
            // then regenerate the title page layout for the new style.
            let with_headings =
                super::template_dialog::replace_heading_styles_in_template(&content, style_key);
            let with_title =
                super::template_dialog::rebuild_title_page_for_style(&with_headings, style_key);
            crate::styles::update_bibliography_only(&with_title, bib_style, bib_title)
        } else {
            crate::styles::apply_to(&content, style_code, bib_style, bib_title)
        };

        if new_content != content {
            self.replace_buffer_content(&path, &new_content);
        }

        // Keep the sidecar's `style` in step with the document, same as
        // `apply_doc_font_edit` already does for the format bar's font/size
        // pickers. Without this, the header Style dropdown changed the
        // document's headings/title page/@zerkalo-style annotation directly
        // but left the `.zerkalo.toml` sidecar on whatever style was active
        // before — so "Update Template Settings" would reopen showing the
        // *old* style, and Apply would silently regenerate the document back
        // onto it, discarding the choice made here.
        if crate::styles::has_template_block(&content) {
            if let Some(mut sc) = super::template_dialog::load_sidecar(&path) {
                if sc.style != style_key {
                    sc.style = style_key.to_string();
                    super::template_dialog::save_sidecar(&path, &sc);
                }
            }
        }
    }

    pub fn insert_at_cursor(&self, text: &str) {
        if let Some((view, buffer)) = self.active_view_buffer() {
            buffer.begin_user_action();
            buffer.insert_at_cursor(text);
            buffer.end_user_action();
            view.grab_focus();
        }
    }

    // ── Spell check API ───────────────────────────────────────────────────────

    pub fn set_lsp_status(&self, status: &str) {
        if status.is_empty() {
            self.lsp_status_label.set_markup("");
            return;
        }
        let lower = status.to_lowercase();
        let dot_color = if status.contains('✗')
            || lower.contains("error")
            || lower.contains("failed")
        {
            crate::ui::theme::muted_fg_hex(&self.lsp_status_label)
        } else if status.contains('↻')
            || lower.contains("loading")
            || lower.contains("indexing")
            || lower.contains("starting")
            || lower.contains("connecting")
        {
            crate::ui::theme::lookup_color_hex(&self.lsp_status_label, "warning_color", "#e5a50a")
        } else if status.contains('●') || lower.contains("ready") || lower.contains("connected") {
            crate::ui::theme::lookup_color_hex(&self.lsp_status_label, "success_color", "#26a269")
        } else {
            crate::ui::theme::muted_fg_hex(&self.lsp_status_label)
        };
        let plain: String = status
            .chars()
            .filter(|c| !matches!(*c, '●' | '✗' | '↻'))
            .collect::<String>()
            .trim()
            .to_string();
        let text = if plain.is_empty() {
            "Suggestions".to_string()
        } else {
            plain
        };
        let markup = format!("<span color=\"{dot_color}\">●</span> {text}");
        self.lsp_status_label.set_markup(&markup);
    }

    pub fn set_diag_summary(&self, errors: u32, notes: u32) {
        let problems = |n: u32| format!("{n} thing{} to look at", if n == 1 { "" } else { "s" });
        let note = |n: u32| format!("{n} note{}", if n == 1 { "" } else { "s" });
        let text = match (errors, notes) {
            (0, 0) => String::new(),
            (e, 0) => problems(e),
            (0, n) => note(n),
            (e, n) => format!("{} · {}", problems(e), note(n)),
        };
        self.diag_label.set_text(&text);
        self.diag_btn.set_visible(!text.is_empty());
    }

    pub fn set_on_diag_click(&self, f: impl Fn() + 'static) {
        *self.on_diag_click.borrow_mut() = Some(Box::new(f));
    }

    /// Time since the last keystroke in any tab, or None if nothing was typed yet.
    pub fn since_last_edit(&self) -> Option<Duration> {
        self.last_edit.get().map(|t| t.elapsed())
    }

    pub fn set_on_gost_toggle(&self, f: impl Fn(bool) + 'static) {
        *self.on_gost_toggle.borrow_mut() = Some(Box::new(f));
    }

    /// Restores the saved GOST state at startup and fires the toggle callback
    /// so the CSS is applied, matching what a click would have done.
    pub fn set_gost_enabled(&self, enabled: bool) {
        *self.gost_enabled.borrow_mut() = enabled;
        set_toggle_label(&self.gost_label, "GOST Type B font", enabled);
        self.gost_restoring.set(true);
        if let Some(f) = self.on_gost_toggle.borrow().as_ref() {
            f(enabled);
        }
        self.gost_restoring.set(false);
    }

    pub fn is_gost_restoring(&self) -> bool {
        self.gost_restoring.get()
    }

    /// Whether "GOST type B" is actually installed. The toggle silently did
    /// nothing when it wasn't, so callers use this to explain instead.
    pub fn gost_font_available(&self) -> bool {
        self.gost_btn
            .pango_context()
            .list_families()
            .iter()
            .any(|f| f.name().eq_ignore_ascii_case("GOST type B"))
    }

    pub fn set_on_autosave_toggle(&self, f: impl Fn(bool) + 'static) {
        *self.on_autosave_toggle.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_autosave(&self, enabled: bool) {
        self.autosave_on.set(enabled);
        set_status_toggle(
            &self.autosave_toggle_btn,
            &self.autosave_label,
            "autosave",
            enabled,
        );
    }

    /// Replaces the calm "is my work safe" line in the status bar.
    pub fn set_safety_line(&self, text: &str) {
        if self.safe_label.text() != text {
            self.safe_label.set_text(text);
        }
    }

    pub fn autosave_enabled(&self) -> bool {
        self.autosave_on.get()
    }

    pub fn set_on_format_bar_toggle(&self, f: impl Fn(bool) + 'static) {
        *self.on_format_bar_toggle.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_format_bar_visible(&self, visible: bool) {
        self.format_bar_container.set_visible(visible);
        set_status_toggle(
            &self.format_bar_toggle_btn,
            &self.format_bar_label,
            "format bar",
            visible,
        );
    }

    pub fn format_bar_visible(&self) -> bool {
        self.format_bar_container.is_visible()
    }

    pub fn is_cv_mode(&self) -> bool {
        self.cv_mode.get()
    }

    pub fn set_cv_mode(&self, cv: bool) {
        self.cv_mode.set(cv);
        self.cv_format_section.set_visible(cv);
    }

    pub fn update_cv_style_label(&self, content: &str) {
        let style =
            super::template_dialog::parse_cv_style(content).unwrap_or_else(|| "modern".to_string());
        let display = match style.as_str() {
            "academic" => "Academic",
            "classic" => "Classic",
            "sidebar" => "Two-Column",
            _ => "Modern",
        };
        self.cv_style_label.set_text(display);
    }

    pub fn apply_cv_style(&self, style: &str) {
        let Some((_view, buf)) = self.active_view_buffer() else {
            return;
        };
        let (start, end) = buf.bounds();
        // include_hidden_chars=true: simple mode marks the preamble invisible; without
        // this flag buf.text() silently drops it and the full-buffer replace wipes it.
        let text = buf.text(&start, &end, true).to_string();
        let new_text: String = text
            .lines()
            .map(|line| {
                let t = line.trim_start();
                if t.starts_with("#let CV_STYLE =") {
                    format!("#let CV_STYLE = \"{style}\"")
                } else if t.starts_with("// @zerkalo-cv-style:") {
                    format!("// @zerkalo-cv-style: {style}")
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let new_text = if text.ends_with('\n') {
            format!("{new_text}\n")
        } else {
            new_text
        };

        // "Two-Column" (sidebar) is the only style with a structurally different
        // body (a #grid columns split, written once at document-creation time —
        // see generate_cv_sidebar_body). The other three styles re-color/re-font
        // from the same flat single-column body just by flipping CV_STYLE above,
        // no regeneration needed. But crossing sidebar<->non-sidebar needs the
        // body itself rebuilt, or switching *out* of Two-Column would leave the
        // old two-column grid in place — the body would keep rendering columnar
        // even though the header now says "Modern"/"Academic"/"Classic".
        let old_style = super::template_dialog::parse_cv_style(&text);
        let old_is_sidebar = old_style.as_deref() == Some("sidebar");
        let new_is_sidebar = style == "sidebar";
        let new_text = if old_is_sidebar != new_is_sidebar {
            match new_text.find("// ── Document body") {
                Some(pos) => {
                    let mut spliced = new_text[..pos].to_string();
                    spliced.push_str(&super::template_dialog::generate_cv_body(style));
                    spliced
                }
                None => new_text,
            }
        } else {
            new_text
        };

        buf.begin_user_action();
        let (mut s, mut e) = buf.bounds();
        buf.delete(&mut s, &mut e);
        buf.insert(&mut buf.end_iter(), &new_text);
        buf.end_user_action();
        let sm = *self.simple_mode.borrow();
        apply_simple_mode_tag(&buf, sm);
        let display = match style {
            "academic" => "Academic",
            "classic" => "Classic",
            "sidebar" => "Two-Column",
            _ => "Modern",
        };
        self.cv_style_label.set_text(display);
    }

    pub fn set_on_focus_toggle(&self, f: impl Fn(bool) + 'static) {
        *self.on_focus_toggle.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_doc_font(&self, f: impl Fn(String) + 'static) {
        *self.on_doc_font.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_doc_font_size(&self, f: impl Fn(String) + 'static) {
        *self.on_doc_font_size.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_doc_font_label(&self, name: &str) {
        self.font_bar_label.set_text(name);
    }

    pub fn set_doc_size_label(&self, size: &str) {
        self.size_bar_label.set_text(size);
    }

    pub fn set_on_version_click(&self, f: impl Fn() + 'static) {
        *self.on_version_click.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_word_count_click(&self, f: impl Fn() + 'static) {
        *self.on_word_count_click.borrow_mut() = Some(Box::new(f));
    }

    // ── File management ───────────────────────────────────────────────────────

    pub fn open_file(&self, path: PathBuf, content: &str) {
        // Release the borrow before set_current_page: the switch-page callback
        // autosaves, which needs state.borrow_mut().
        let existing = self
            .state
            .borrow()
            .tabs
            .get(&path)
            .map(|tab| tab.notebook_page.clone());
        if let Some(page) = existing {
            if let Some(n) = self.notebook.page_num(&page) {
                self.notebook.set_current_page(Some(n));
            }
            return;
        }

        let display_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("untitled")
            .to_string();

        let buffer = Buffer::new(None::<&gtk4::TextTagTable>);
        // GTK4 defaults to 200 undo steps; raise to effectively unlimited so
        // users can always undo back through an entire editing session.
        gtk4::prelude::TextBufferExt::set_max_undo_levels(&buffer, u32::MAX);

        let lang_manager = LanguageManager::default();
        if let Some(path_str) = path.to_str() {
            if let Some(lang) = lang_manager.guess_language(Some(path_str), None) {
                buffer.set_language(Some(&lang));
                buffer.set_highlight_syntax(true);
            }
        }
        if let Some(scheme) = StyleSchemeManager::default()
            .scheme(scheme_id_for(adw::StyleManager::default().is_dark()))
        {
            buffer.set_style_scheme(Some(&scheme));
        }

        let migrated;
        let content = if content
            .contains("#if it.numbering != none [#context counter(heading).display(it.numbering)")
        {
            migrated = migrate_template_it_numbering(content);
            migrated.as_str()
        } else {
            content
        };
        buffer.set_text(content);
        apply_comment_highlights(&buffer, None);
        {
            let sm = *self.simple_mode.borrow();
            apply_simple_mode_tag(&buffer, sm);
        }

        let view = View::with_buffer(&buffer);
        view.update_property(&[
            gtk4::accessible::Property::Label("Document editor"),
            gtk4::accessible::Property::MultiLine(true),
        ]);
        view.set_show_line_numbers(!*self.simple_mode.borrow() || self.line_numbers_override.get());
        // Soft right-margin guide at 90 characters — useful even with word wrap
        // as a visual rhythm reference for longer code lines.
        view.set_show_right_margin(true);
        view.set_right_margin_position(90);

        // Gutter icons for error and warning marks
        let err_attrs = MarkAttributes::new();
        err_attrs.set_icon_name("dialog-error-symbolic");
        view.set_mark_attributes("zerkalo-error", &err_attrs, 1);
        let warn_attrs = MarkAttributes::new();
        warn_attrs.set_icon_name("dialog-warning-symbolic");
        view.set_mark_attributes("zerkalo-warning", &warn_attrs, 1);

        view.set_auto_indent(true);
        view.set_smart_backspace(true);
        view.set_insert_spaces_instead_of_tabs(true);
        let tw = *self.tab_width.borrow();
        view.set_tab_width(tw.max(1));
        view.set_indent_width(tw as i32);
        // Do NOT set_monospace — the editor font family is set explicitly via
        // apply_font_family; monospace mode only matters when no font is configured.
        // A buffer line is a whole paragraph in prose, so the band is a code-editing
        // aid only; Simple Mode (prose) leaves it off.
        view.set_highlight_current_line(!*self.simple_mode.borrow());
        // Text always wraps at word boundaries; there is no unwrapped mode. A
        // single word too long to fit (a URL, say) breaks across lines rather than
        // running off the edge, since the editor never scrolls sideways.
        view.set_wrap_mode(gtk4::WrapMode::WordChar);
        apply_space_drawer(&view, *self.show_whitespace.borrow());
        set_view_line_spacing(&view, *self.line_spacing.borrow());
        // Comfortable content padding. In simple mode the gutter is hidden so
        // add extra left padding to keep the text away from the window edge.
        let left_margin = if *self.simple_mode.borrow() { 40 } else { 8 };
        view.set_left_margin(left_margin);
        view.set_right_margin(8);

        self.wire_drag_and_drop(&view);

        let scroll = ScrolledWindow::new();
        // The view must be the ScrolledWindow's direct child — not wrapped in
        // an Overlay — so GTK wires up its real, ScrolledWindow-owned
        // adjustments the normal way. An Overlay doesn't implement
        // GtkScrollable, so the ScrolledWindow would auto-wrap it in its own
        // Viewport instead, and forcing the view's adjustments to match the
        // Viewport's afterward (tried previously) makes both the Viewport and
        // the view apply the same scroll offset independently — worse than
        // the original bug it was meant to fix: it doesn't just fail to
        // auto-scroll on jumps, it breaks scrolling entirely. With the view
        // as the direct child, scroll_to_mark/scroll_to_iter (search jump,
        // click-to-jump, heading navigation, ...) and ordinary wheel/keyboard
        // scrolling all go through the same, single, correctly-connected
        // adjustment.
        scroll.set_child(Some(&view));
        scroll.set_hexpand(true);
        scroll.set_vexpand(true);
        // Horizontal scroll is permanently disabled — all wrapping is done in the
        // text view itself. Kinetic scrolling is disabled to prevent the view from
        // "coasting" past where the user clicked.
        lock_horizontal_scroll(&scroll);
        scroll.set_kinetic_scrolling(false);

        // Ghost-text placeholder — shown when the buffer is empty. This wraps
        // the ScrolledWindow (not the view) in a plain gtk4::Overlay, so the
        // view keeps its direct-child adjustments above while the placeholder
        // is still free to take its natural wrap width instead of the view's
        // own (buffer-coordinate) overlay sizing, which allocates a child
        // only its minimum size — enough for one word per line for text this
        // long.
        let placeholder_lbl = Label::new(Some(
            "Start writing. Use = Heading for headings, *word* for bold, _word_ for italic, @key to cite."
        ));
        placeholder_lbl.add_css_class("dim-label");
        placeholder_lbl.set_halign(gtk4::Align::Start);
        placeholder_lbl.set_valign(gtk4::Align::Start);
        placeholder_lbl.set_margin_top(8);
        placeholder_lbl.set_margin_start(48); // aligns with view left-margin + gutter
        placeholder_lbl.set_wrap(true);
        placeholder_lbl.set_sensitive(false);
        placeholder_lbl.set_visible(buffer.char_count() == 0);

        let editor_overlay = gtk4::Overlay::new();
        editor_overlay.set_child(Some(&scroll));
        editor_overlay.add_overlay(&placeholder_lbl);
        editor_overlay.set_hexpand(true);
        editor_overlay.set_vexpand(true);

        let ph_lbl_for_buf = placeholder_lbl.clone();
        buffer.connect_changed(move |buf| {
            ph_lbl_for_buf.set_visible(buf.char_count() == 0);
        });

        let (dot_label, diag_dot) = self.attach_tab_page(&path, &display_name, &editor_overlay);

        let tab = TabContext {
            path: path.clone(),
            buffer: buffer.clone(),
            view: view.clone(),
            scroll: scroll.clone(),
            dot_label: dot_label.clone(),
        };

        self.wire_modified_and_word_count(&tab, content);
        self.wire_cursor_tracking(&tab);
        self.wire_undo_redo_sensitivity(&tab);
        let CitationAutocomplete {
            bib_popup,
            ac_mark,
            completing,
            ghost_label,
            ghost_item,
            completion_suppressed_at,
            ghost_bib_entry,
        } = self.wire_citation_autocomplete(&view, &buffer);

        let LspAutocomplete {
            lsp_popup,
            lsp_mark,
            lsp_completing,
        } = self.wire_lsp_autocomplete(
            &view,
            &buffer,
            &path,
            &ghost_label,
            &ghost_item,
            &completion_suppressed_at,
            &ghost_bib_entry,
        );

        let (hold_position, hold_until) = self.wire_key_controller(
            &view,
            &buffer,
            &bib_popup,
            &lsp_popup,
            &ac_mark,
            &lsp_mark,
            &completing,
            &lsp_completing,
            &ghost_item,
            &ghost_label,
            &completion_suppressed_at,
            &ghost_bib_entry,
        );

        self.wire_spell_suggestions(&tab, &hold_position, &hold_until);
        self.wire_spellcheck(&tab);
        self.wire_autocorrect(&tab);
        writing::wire(&view, &buffer);
        let (saved_scroll, saved_hscroll, pause_tracking) =
            self.wire_right_click_menu(&view, &buffer, &scroll, &hold_position, &hold_until);

        // ── Inline error assistant — hover over error-tagged line ─────────────
        // Shared with the any_click handler further down, which dismisses it
        // explicitly — see that handler's own comment for why.
        let active_error_popup: Rc<RefCell<Option<Popover>>> = Rc::new(RefCell::new(None));
        {
            let last_diags = self.last_diagnostics.clone();
            let on_fix_request = self.on_fix_request.clone();
            let view_hover = view.clone();
            let buf_hover = buffer.clone();
            let path_hover = path.clone();
            let active_popup = active_error_popup.clone();

            let motion = EventControllerMotion::new();
            let active_popup_c = active_popup.clone();
            let show_hover = Rc::new(move |x: f64, y: f64| {
                let diags = last_diags.borrow();
                if diags.is_empty() {
                    return;
                }

                let (bx, by) =
                    view_hover.window_to_buffer_coords(TextWindowType::Widget, x as i32, y as i32);
                let Some(iter) = view_hover.iter_at_location(bx, by) else {
                    // Past the end of a line, or below the text: no character is
                    // under the pointer, so the error pop-up has nothing to point at
                    // and closes like it does for any other spot off the error.
                    let taken = active_popup_c.borrow_mut().take();
                    if let Some(p) = taken {
                        p.popdown();
                    }
                    return;
                };
                let line_1based = iter.line() as u32 + 1;

                let tag_table = buf_hover.tag_table();
                let has_error_tag = tag_table
                    .lookup("zerkalo-diag-error")
                    .map(|t| iter.has_tag(&t))
                    .unwrap_or(false);
                if !has_error_tag {
                    // Moved off the error entirely — with autohide(false)
                    // (see why at the popover's own construction below)
                    // nothing else closes this, so it must be done here.
                    //
                    // The extracted Option is bound to its own `let` first,
                    // not matched directly on `.borrow_mut().take()` — Rust
                    // extends that temporary RefMut's lifetime to the end of
                    // the `if let` block otherwise, so it's still held while
                    // popdown() runs. popdown() synchronously fires this
                    // popover's own `connect_closed` handler below, which
                    // does `ap_closed.borrow_mut()` on the same RefCell —
                    // a real double-borrow panic, hit live (RefCell already
                    // borrowed, aborting the whole process, every time the
                    // mouse left an error line with the popup still open).
                    let taken = active_popup_c.borrow_mut().take();
                    if let Some(p) = taken {
                        p.popdown();
                    }
                    return;
                }

                let col_1based = iter.line_offset() as u32 + 1;
                let on_line = |m: &&DiagMark| m.line == line_1based && m.file == path_hover;
                let mark: Option<DiagMark> = diags
                    .iter()
                    .filter(on_line)
                    .find(|m| {
                        col_1based >= m.col
                            && (m.end_line != m.line || col_1based < m.end_col.max(m.col + 1))
                    })
                    .or_else(|| diags.iter().find(on_line))
                    .cloned();
                let Some(mark) = mark else { return };
                let msg = mark.headline.clone();

                // Only create a new popup if none is showing (avoid flicker)
                if active_popup_c.borrow().is_some() {
                    return;
                }

                let (line_start, line_end) = {
                    let mut a = buf_hover.iter_at_line(mark.line as i32 - 1).unwrap_or(iter);
                    let mut b = a;
                    if !b.ends_line() {
                        b.forward_to_line_end();
                    }
                    a.set_line_offset(0);
                    (a, b)
                };
                let line_text = buf_hover.text(&line_start, &line_end, true).to_string();
                let fix = crate::diagnostic_catalog::fix_for(mark.kind).filter(|_| {
                    !mark.in_package
                        && crate::diagnostic_catalog::fix_applies(
                            mark.kind,
                            Some(&line_text),
                            mark.col,
                        )
                });

                let popover = Popover::new();
                popover.set_parent(&view_hover);
                popover.set_has_arrow(true);
                // Not autohide(true): a click meant to place the cursor on
                // error-underlined text — the whole point of an inline error
                // popup is that the error is right where you're about to
                // edit — was instead being swallowed to dismiss the popover,
                // needing a second click to actually reach the text.
                // autohide(false) plus the explicit dismiss in any_click's
                // handler below (same pattern the LSP/citation popups already
                // use, and for the same reason) lets that first click do
                // both at once.
                popover.set_autohide(false);
                // Point at the hovered line's own rectangle and open below it, so the
                // popup never sits on top of the text you are about to click.
                let loc = view_hover.iter_location(&iter);
                let (wx, wy) =
                    view_hover.buffer_to_window_coords(TextWindowType::Widget, loc.x(), loc.y());
                popover.set_position(gtk4::PositionType::Bottom);
                popover.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(
                    wx.max(x as i32 - 1),
                    wy,
                    1,
                    loc.height().max(1),
                )));

                let vbox = GtkBox::new(Orientation::Vertical, 4);
                vbox.set_margin_top(8);
                vbox.set_margin_bottom(8);
                vbox.set_margin_start(10);
                vbox.set_margin_end(10);

                let msg_lbl = Label::new(Some(&msg));
                msg_lbl.set_xalign(0.0);
                msg_lbl.set_wrap(true);
                msg_lbl.set_max_width_chars(50);
                vbox.append(&msg_lbl);

                let explanation = match &fix {
                    Some(fx) => Some(fx.description.clone()),
                    None => Some(mark.advice.clone()).filter(|a| !a.is_empty()),
                };
                if let Some(text) = explanation {
                    vbox.append(&Separator::new(Orientation::Horizontal));
                    let fix_row = GtkBox::new(Orientation::Horizontal, 8);
                    let fix_desc = Label::new(Some(&text));
                    fix_desc.add_css_class("dim-label");
                    fix_desc.set_xalign(0.0);
                    fix_desc.set_wrap(true);
                    fix_desc.set_max_width_chars(40);
                    fix_desc.set_hexpand(true);
                    fix_row.append(&fix_desc);

                    if fix.is_some() {
                        let fix_btn = Button::with_label("Fix It");
                        fix_btn.add_css_class("suggested-action");
                        let request = on_fix_request.clone();
                        let mark_fix = mark.clone();
                        let pop_fix = popover.clone();
                        fix_btn.connect_clicked(move |_| {
                            pop_fix.popdown();
                            if let Some(f) = request.borrow().as_ref() {
                                f(mark_fix.clone());
                            }
                        });
                        fix_row.append(&fix_btn);
                    }
                    vbox.append(&fix_row);
                }

                popover.set_child(Some(&vbox));
                *active_popup_c.borrow_mut() = Some(popover.clone());
                let ap_closed = active_popup_c.clone();
                popover.connect_closed(move |_| {
                    *ap_closed.borrow_mut() = None;
                });
                popover.popup();
            });
            // A popup that opens the instant the pointer crosses an underline covers
            // text people are passing over on their way somewhere else, so it waits
            // for the pointer to rest. Moving while one is showing is handled at once
            // so it closes as soon as the pointer leaves the error.
            let last_pos = Rc::new(Cell::new((0.0f64, 0.0f64)));
            let hover_gen = Rc::new(Cell::new(0u64));
            {
                let showing = active_popup.clone();
                motion.connect_motion(move |_, x, y| {
                    last_pos.set((x, y));
                    let gen = hover_gen.get().wrapping_add(1);
                    hover_gen.set(gen);
                    if showing.borrow().is_some() {
                        show_hover(x, y);
                        return;
                    }
                    let show = show_hover.clone();
                    let gen_cell = hover_gen.clone();
                    let pos = last_pos.clone();
                    glib::timeout_add_local_once(HOVER_DELAY, move || {
                        if gen_cell.get() == gen {
                            let (x, y) = pos.get();
                            show(x, y);
                        }
                    });
                });
            }
            view.add_controller(motion);
        }

        // Suppress GTK's built-in focus-in cursor snap.
        // GtkTextView calls scroll_mark_onscreen(insert) when it gains keyboard
        // focus, which can violently snap the viewport to the cursor's OLD position
        // when the user has scrolled elsewhere. We save the scroll position just
        // before each click (GestureClick::pressed fires before GtkTextView's own
        // button-press handler, which is what triggers focus-in and the snap) and
        // restore it in idle after GTK's focus-in handler runs.
        // saved_scroll is shared with the right-click gesture above — see comment there.
        {
            // saved_scroll/saved_hscroll follow every scroll (see the adjustment
            // handlers above), so nothing here needs to sample the position.

            // On left-click, suppress the focus-snap only when the view is
            // actually gaining focus. If it already has focus the click is
            // intentional navigation and should scroll to the cursor normally.
            // button=1 only — button=0 would steal the right-click spell gesture's sequence.
            let any_click = GestureClick::new();
            any_click.set_button(1);
            {
                let sc = scroll.clone();
                let pause = pause_tracking.clone();
                let view_fc = view.clone();
                let lsp_popup_click = lsp_popup.clone();
                let bib_popup_click = bib_popup.clone();
                let ghost_click = ghost_label.clone();
                let ghost_item_click = ghost_item.clone();
                let ghost_bib_click = ghost_bib_entry.clone();
                let hint_click = self.lsp_status_label.clone();
                let active_error_popup_click = active_error_popup.clone();
                let hold_pos_click = hold_position.clone();
                let hold_until_click = hold_until.clone();
                any_click.connect_pressed(move |_, _, _, _| {
                    // Clicking anywhere in the text dismisses a suggestion —
                    // the popovers are autohide(false) (they must not steal the
                    // keyboard while you type), so they'd otherwise sit there.
                    lsp_popup_click.hide();
                    bib_popup_click.hide();
                    // See the matching comment on the hover handler above for why this
                    // can't be `if let Some(p) = ...borrow_mut().take() { p.popdown(); }`
                    // directly — same double-borrow panic, same fix.
                    let taken = active_error_popup_click.borrow_mut().take();
                    if let Some(p) = taken {
                        p.popdown();
                    }
                    clear_citation_ghost(&ghost_click, &ghost_bib_click, &hint_click);
                    clear_ghost(&ghost_click, &ghost_item_click, &hint_click);
                    if !view_fc.has_focus() {
                        // View is gaining focus → GTK will snap to insert mark. A
                        // single "capture now, restore once at the next zero-delay
                        // timeout" used to live here, but that only works if GTK's
                        // snap is strictly slower than our restore — and dismissing
                        // a popover/menu by clicking elsewhere in the view (rather
                        // than e.g. pressing Escape) fires this same focus-gain
                        // path, where the snap can already be synchronously done by
                        // the time this handler runs. Use the same continuous-hold
                        // mechanism as paste and spell-suggestion-accept instead:
                        // reassert the captured position on every adjustment change
                        // for a short window, which is correct regardless of when
                        // GTK's own snap actually lands.
                        let val = sc.vadjustment().value();
                        let hval = sc.hadjustment().value();
                        pause();
                        hold_pos_click.set(Some((val, hval)));
                        hold_until_click.set(Instant::now() + PASTE_HOLD);
                        let release = hold_pos_click.clone();
                        glib::timeout_add_local_once(PASTE_HOLD, move || release.set(None));
                    }
                });
            }
            view.add_controller(any_click);

            let focus_ctrl = EventControllerFocus::new();
            {
                // Pause tracking as focus leaves too: the snap can fire on the
                // way out (when a context menu takes focus), and recording it
                // would make the restore on the way back in aim at it. Focus
                // leaving the editor at all — a click in the sidebar, the
                // preview, another window — also means any suggestion on screen
                // is stale, so drop it.
                let pause = pause_tracking.clone();
                let lsp_popup_focus = lsp_popup.clone();
                let bib_popup_focus = bib_popup.clone();
                let ghost_focus = ghost_label.clone();
                let ghost_item_focus = ghost_item.clone();
                let ghost_bib_focus = ghost_bib_entry.clone();
                let hint_focus = self.lsp_status_label.clone();
                let active_error_popup_focus = active_error_popup.clone();
                focus_ctrl.connect_leave(move |_| {
                    pause();
                    lsp_popup_focus.hide();
                    bib_popup_focus.hide();
                    // Same double-borrow hazard and fix as the hover handler above.
                    let taken = active_error_popup_focus.borrow_mut().take();
                    if let Some(p) = taken {
                        p.popdown();
                    }
                    clear_citation_ghost(&ghost_focus, &ghost_bib_focus, &hint_focus);
                    clear_ghost(&ghost_focus, &ghost_item_focus, &hint_focus);
                });
            }
            {
                let sc_enter = scroll.clone();
                let sv_enter = saved_scroll.clone();
                let sh_enter = saved_hscroll.clone();
                let pause = pause_tracking.clone();
                let jumping = self.jumping.clone();
                focus_ctrl.connect_enter(move |_| {
                    if jumping.get() {
                        return;
                    }
                    // Use the tracked position rather than the current scroll. GTK can
                    // snap the view to the cursor synchronously before this signal fires
                    // (e.g. on context-menu dismiss), so reading the adjustment here
                    // would restore the snapped position rather than where the user was.
                    let val = sv_enter.get();
                    let hval = sh_enter.get();
                    if val < 0.0 {
                        return;
                    }
                    let sc = sc_enter.clone();
                    pause();
                    glib::timeout_add_local_once(Duration::ZERO, move || {
                        sc.vadjustment().set_value(val);
                        sc.hadjustment().set_value(hval);
                    });
                });
            }
            view.add_controller(focus_ctrl);
        }

        // Copying must never move the viewport. GtkTextView scrolls to the
        // insert mark after a clipboard action, and taking Copy from the
        // right-click menu adds a focus round-trip that can snap it as well —
        // together they threw the view to wherever the cursor happened to be
        // (usually the top of the file) on a plain copy. Pin the position
        // across both, for cut too, since a cut happens where the user already
        // is. Paste is deliberately left alone: scrolling to the insertion
        // point is the correct thing there, since that's where the text landed.
        {
            let pin = {
                let scroll = scroll.clone();
                let pause = pause_tracking.clone();
                move || {
                    let val = scroll.vadjustment().value();
                    let hval = scroll.hadjustment().value();
                    let sc = scroll.clone();
                    pause();
                    glib::timeout_add_local_once(Duration::ZERO, move || {
                        sc.vadjustment().set_value(val);
                        sc.hadjustment().set_value(hval);
                    });
                }
            };
            let pin_cut = pin.clone();
            view.connect_copy_clipboard(move |_| pin());
            view.connect_cut_clipboard(move |_| pin_cut());

            // Paste inserts at the cursor, so the right viewport is the one the
            // user is already looking at. Hold it while GTK's animation plays
            // out — unless the paste landed off-screen, where following it is
            // the correct behaviour.
            {
                let scroll = scroll.clone();
                let view_p = view.clone();
                let _buf_p = buffer.clone();
                let held = hold_position.clone();
                let held_until = hold_until.clone();
                let pause = pause_tracking.clone();
                buffer.connect_paste_done(move |buf, _| {
                    let cursor = buf.iter_at_offset(buf.cursor_position());
                    let loc = view_p.iter_location(&cursor);
                    let (_, wy) =
                        view_p.buffer_to_window_coords(TextWindowType::Widget, loc.x(), loc.y());
                    let on_screen = wy >= 0 && wy <= view_p.allocated_height();
                    if !on_screen {
                        return;
                    }
                    held.set(Some((
                        scroll.vadjustment().value(),
                        scroll.hadjustment().value(),
                    )));
                    held_until.set(Instant::now() + PASTE_HOLD);
                    pause();
                    // Release the hold once the animation is spent, so ordinary
                    // scrolling works again immediately afterwards.
                    let held_release = held.clone();
                    glib::timeout_add_local_once(PASTE_HOLD, move || held_release.set(None));
                });
            }
        }

        // Re-apply squiggles after undo restores old text. Debounced, and scoped
        // to this tab: the sweep is O(document length), so running it inline for
        // every open tab on every keystroke made typing lag on long documents.
        {
            let last_diags = self.last_diagnostics.clone();
            let path_rem = path.clone();
            let buf_rem = buffer.clone();
            let dot_rem = diag_dot.clone();
            let remarking: Rc<std::cell::Cell<bool>> = Rc::new(std::cell::Cell::new(false));
            let remark_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
            buffer.connect_changed(move |_| {
                if remarking.get() {
                    return;
                }
                if last_diags.borrow().is_empty() {
                    return;
                }
                if let Some(id) = remark_timer.borrow_mut().take() {
                    id.remove();
                }
                let diags_rc = last_diags.clone();
                let p = path_rem.clone();
                let b = buf_rem.clone();
                let d = dot_rem.clone();
                let rem = remarking.clone();
                let t = remark_timer.clone();
                *remark_timer.borrow_mut() = Some(glib::timeout_add_local_once(
                    DIAG_REMARK_DEBOUNCE,
                    move || {
                        *t.borrow_mut() = None;
                        let diags = diags_rc.borrow().clone();
                        if diags.is_empty() {
                            return;
                        }
                        rem.set(true);
                        mark_diagnostics_for_tab(&p, &b, &d, &diags);
                        rem.set(false);
                    },
                ));
            });
        }

        // ── Select the new tab ──────────────────────────────────────────────

        let page_index = self.notebook.page_num(&editor_overlay);

        let path_for_callback = tab.path.clone();
        let content_for_callback = content.to_string();

        // The loaded text is the saved state undo can return to.
        buffer.set_modified(false);
        let session_start_words = count_words(content);
        self.state.borrow_mut().tabs.insert(
            path,
            EditorTab {
                buffer,
                view,
                notebook_page: editor_overlay,
                modified: false,
                dot_label,
                diag_dot,
                display_name: display_name.clone(),
                lsp_popup,
                ghost_label,
                ghost_item,
                session_start_words,
            },
        );

        self.notebook.set_current_page(page_index);
        set_wc_text_with_session(&self.word_count_label, content, session_start_words);

        // Per-document `// @zerkalo-goal: N` wins; otherwise the Settings goal.
        let goal = parse_goal_comment(content).unwrap_or(*self.default_word_count_goal.borrow());
        *self.word_count_goal.borrow_mut() = goal;
        if goal == 0 {
            self.goal_ring.set_visible(false);
        } else {
            update_goal_ring(&self.goal_ring, &self.goal_fraction, content, goal);
        }

        // Explicitly fire page_switch so title/outline update even when this is
        // the first tab (connect_switch_page fires before the tab is in state.tabs).
        if let Some(f) = self.on_page_switch.borrow().as_ref() {
            f(content_for_callback.clone(), path_for_callback.clone());
        }

        if let Some(f) = self.on_file_opened.borrow().as_ref() {
            f(path_for_callback, content_for_callback);
        }
    }

    #[allow(dead_code)] // companion to the word-count stats
    pub fn active_line_count(&self) -> u32 {
        let current = match self.notebook.current_page() {
            Some(p) => p,
            None => return 1,
        };
        let state = self.state.borrow();
        for tab in state.tabs.values() {
            if self.notebook.page_num(&tab.notebook_page) == Some(current) {
                return tab.buffer.line_count() as u32;
            }
        }
        1
    }

    pub fn get_active_content(&self) -> Option<String> {
        let current = self.notebook.current_page()?;
        let state = self.state.borrow();
        for tab in state.tabs.values() {
            if let Some(n) = self.notebook.page_num(&tab.notebook_page) {
                if n == current {
                    let (start, end) = tab.buffer.bounds();
                    return Some(tab.buffer.text(&start, &end, true).to_string());
                }
            }
        }
        None
    }

    /// Replace the active buffer's entire content as a single undoable user action.
    pub fn set_active_content_undoable(&self, text: &str) {
        let current = match self.notebook.current_page() {
            Some(p) => p,
            None => return,
        };
        let buf = {
            let state = self.state.borrow();
            state
                .tabs
                .values()
                .find(|t| self.notebook.page_num(&t.notebook_page) == Some(current))
                .map(|t| t.buffer.clone())
        };
        if let Some(buffer) = buf {
            replace_text_minimally(&buffer, text);
            {
                let sm = *self.simple_mode.borrow();
                apply_simple_mode_tag(&buffer, sm);
            }
        }
    }

    /// Undoes the last edit in the buffer open for `path`.
    pub fn undo_in(&self, path: &std::path::Path) {
        let buffer = self.state.borrow().tabs.get(path).map(|t| t.buffer.clone());
        if let Some(buffer) = buffer {
            if buffer.can_undo() {
                buffer.undo();
            }
        }
    }

    /// Wired by the window: what to do when the hover popup's Fix It is pressed,
    /// so it goes through exactly the same path as the Problems panel's Fix.
    pub fn set_on_fix_request(&self, f: impl Fn(DiagMark) + 'static) {
        *self.on_fix_request.borrow_mut() = Some(Box::new(f));
    }

    pub fn state_has_file(&self, path: &std::path::Path) -> bool {
        self.state.borrow().tabs.contains_key(path)
    }

    pub fn set_content(&self, path: &std::path::Path, text: &str) {
        let buf = self.state.borrow().tabs.get(path).map(|t| t.buffer.clone());
        if let Some(buffer) = buf {
            replace_text_minimally(&buffer, text);
            {
                let sm = *self.simple_mode.borrow();
                apply_simple_mode_tag(&buffer, sm);
            }
        }
    }

    pub fn set_on_file_dirty(&self, f: impl Fn(PathBuf, bool) + 'static) {
        *self.on_file_dirty.borrow_mut() = Some(Box::new(f));
    }

    pub fn is_file_open(&self, path: &PathBuf) -> bool {
        self.state.borrow().tabs.contains_key(path)
    }

    pub fn get_open_paths_ordered(&self) -> Vec<PathBuf> {
        let state = self.state.borrow();
        let mut pages: Vec<(u32, PathBuf)> = state
            .tabs
            .iter()
            .filter_map(|(path, tab)| {
                self.notebook
                    .page_num(&tab.notebook_page)
                    .map(|n| (n, path.clone()))
            })
            .collect();
        pages.sort_by_key(|(n, _)| *n);
        pages.into_iter().map(|(_, p)| p).collect()
    }

    pub fn get_cursor_positions(&self) -> std::collections::HashMap<PathBuf, i32> {
        let state = self.state.borrow();
        state
            .tabs
            .iter()
            .map(|(path, tab)| (path.clone(), tab.buffer.cursor_position()))
            .collect()
    }

    pub fn restore_cursor(&self, path: &PathBuf, offset: i32) {
        let state = self.state.borrow();
        if let Some(tab) = state.tabs.get(path) {
            let clamped = offset.max(0).min(tab.buffer.char_count());
            let iter = tab.buffer.iter_at_offset(clamped);
            tab.buffer.place_cursor(&iter);
        }
    }

    pub fn get_active_path(&self) -> Option<PathBuf> {
        let current = self.notebook.current_page()?;
        let state = self.state.borrow();
        for (path, tab) in &state.tabs {
            if let Some(n) = self.notebook.page_num(&tab.notebook_page) {
                if n == current {
                    return Some(path.clone());
                }
            }
        }
        None
    }

    /// Cursor's vertical position in the active document as a 0.0–1.0
    /// fraction of total lines. Drives the preview's scroll-follow — coarse
    /// (line density isn't uniform with PDF page position) but needs no
    /// compiler-level source-span support, unlike click-to-jump.
    /// The live text of one line of an open file (1-based), or None if the file
    /// isn't open.
    ///
    /// The error panel shows the offending source line beside each diagnostic.
    /// Reading it from disk was wrong whenever the buffer was dirty: compiles
    /// run against the unsaved buffer, so the panel could quote a line the
    /// compiler never saw.
    pub fn line_text(&self, path: &std::path::Path, line: u32) -> Option<String> {
        let state = self.state.borrow();
        let tab = state.tabs.get(path)?;
        let (s, e) = tab.buffer.bounds();
        let text = tab.buffer.text(&s, &e, true);
        text.lines()
            .nth((line as usize).checked_sub(1)?)
            .map(str::to_string)
    }

    pub fn content_of(&self, path: &std::path::Path) -> Option<String> {
        let state = self.state.borrow();
        let tab = state.tabs.get(path)?;
        let (start, end) = tab.buffer.bounds();
        Some(tab.buffer.text(&start, &end, true).to_string())
    }

    pub fn active_text(&self) -> Option<String> {
        let (_, buf) = self.active_view_buffer()?;
        let (s, e) = buf.bounds();
        Some(buf.text(&s, &e, true).to_string())
    }

    pub fn all_tab_texts(&self) -> Vec<(PathBuf, String)> {
        self.state
            .borrow()
            .tabs
            .iter()
            .map(|(path, tab)| {
                let (s, e) = tab.buffer.bounds();
                let text = tab.buffer.text(&s, &e, true).to_string();
                (path.clone(), text)
            })
            .collect()
    }

    pub fn project_root(&self) -> Option<PathBuf> {
        self.project_root.borrow().clone()
    }

    pub fn session_start_words(&self) -> u32 {
        let current = self.notebook.current_page().unwrap_or(0);
        let state = self.state.borrow();
        for tab in state.tabs.values() {
            if self.notebook.page_num(&tab.notebook_page) == Some(current) {
                return tab.session_start_words;
            }
        }
        0
    }

    fn toggle_active_markup(&self, marker: &str) {
        let Some((_view, buf)) = self.active_view_buffer() else {
            return;
        };
        let mlen = marker.len() as i32;
        buf.begin_user_action();
        if let Some((sel_s, sel_e)) = buf.selection_bounds() {
            let start_off = sel_s.offset();
            let end_off = sel_e.offset();
            let text = buf.text(&sel_s, &sel_e, false).to_string();
            if text.starts_with(marker) && text.ends_with(marker) && text.len() > 2 * marker.len() {
                // strip markers, keep inner text selected
                let inner = text[marker.len()..text.len() - marker.len()].to_string();
                let inner_len = inner.len() as i32;
                let mut s = buf.iter_at_offset(start_off);
                let mut e = buf.iter_at_offset(end_off);
                buf.delete(&mut s, &mut e);
                let mut ins = buf.iter_at_offset(start_off);
                buf.insert(&mut ins, &inner);
                buf.select_range(
                    &buf.iter_at_offset(start_off),
                    &buf.iter_at_offset(start_off + inner_len),
                );
            } else {
                // wrap selection, keep inner text selected
                let tlen = text.len() as i32;
                let mut s = buf.iter_at_offset(start_off);
                let mut e = buf.iter_at_offset(end_off);
                buf.delete(&mut s, &mut e);
                let mut ins = buf.iter_at_offset(start_off);
                buf.insert(&mut ins, &format!("{marker}{text}{marker}"));
                buf.select_range(
                    &buf.iter_at_offset(start_off + mlen),
                    &buf.iter_at_offset(start_off + mlen + tlen),
                );
            }
        } else {
            // no selection: insert paired markers, place cursor between them
            let pos = buf.cursor_position();
            let mut ins = buf.iter_at_offset(pos);
            buf.insert(&mut ins, &format!("{marker}{marker}"));
            buf.place_cursor(&buf.iter_at_offset(pos + mlen));
        }
        buf.end_user_action();
    }

    fn set_active_heading(&self, level: usize) {
        let Some((_view, buf)) = self.active_view_buffer() else {
            return;
        };
        let cursor = buf.iter_at_mark(&buf.get_insert());
        let line = cursor.line();
        let line_start = buf.iter_at_line(line).unwrap_or(cursor);
        let mut line_end = line_start;
        line_end.forward_to_line_end();
        let line_text = buf.text(&line_start, &line_end, false).to_string();
        let raw = line_text.as_str();
        let current_level = raw.chars().take_while(|c| *c == '=').count();
        let body = raw.trim_start_matches('=').trim_start();
        let new_line = if current_level == level {
            // same level → remove heading (toggle off)
            body.to_string()
        } else {
            format!("{} {body}", "=".repeat(level))
        };
        let start_off = line_start.offset();
        let end_off = line_end.offset();
        buf.begin_user_action();
        let mut ls = buf.iter_at_offset(start_off);
        let mut le = buf.iter_at_offset(end_off);
        buf.delete(&mut ls, &mut le);
        let mut ins = buf.iter_at_offset(start_off);
        buf.insert(&mut ins, &new_line);
        buf.end_user_action();
    }

    fn active_view_buffer(&self) -> Option<(View, Buffer)> {
        let current = self.notebook.current_page()?;
        let state = self.state.borrow();
        for tab in state.tabs.values() {
            if self.notebook.page_num(&tab.notebook_page) == Some(current) {
                return Some((tab.view.clone(), tab.buffer.clone()));
            }
        }
        None
    }
}

// ── Free helpers ──────────────────────────────────────────────────────────────

/// The one place the editor's colour scheme is chosen, so a tab opened after a
/// theme switch looks the same as one already open.
fn scheme_id_for(is_dark: bool) -> &'static str {
    if is_dark {
        "Adwaita-dark"
    } else {
        "Adwaita"
    }
}

/// Makes the editor unable to scroll sideways at all: no horizontal scrollbar,
/// and an adjustment that snaps back to zero if anything — a stale restore, a
/// scroll-to-cursor, GTK's own focus handling — ever nudges it.
fn lock_horizontal_scroll(scroll: &ScrolledWindow) {
    scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    let adj = scroll.hadjustment();
    if adj.value() != 0.0 {
        adj.set_value(0.0);
    }
    // Once per scrolled window: this runs again whenever word wrap is toggled.
    const LOCKED: &str = "zerkalo-hscroll-locked";
    // SAFETY: only ever stores and reads a `bool` under this key.
    if unsafe { adj.data::<bool>(LOCKED) }.is_some() {
        return;
    }
    unsafe { adj.set_data(LOCKED, true) };
    adj.connect_value_changed(|adj| {
        if adj.value() != 0.0 {
            adj.set_value(0.0);
        }
    });
}

/// Apply a background fill to all comment lines (// runs and /* */ blocks).
/// Adjacent // lines are merged into one contiguous tag span for a "box" look.
/// Highlight comment blocks. `cache` holds the line spans from the last run:
/// when they are unchanged — the common case, since typing inside a paragraph
/// doesn't move a comment boundary — the tag sweep is skipped entirely. That
/// sweep costs O(document length) and forces a relayout, so on a long document
/// it was a visible hitch every time typing paused.
fn apply_comment_highlights(buffer: &Buffer, cache: Option<&RefCell<Vec<(i32, i32)>>>) {
    let tag_name = "zk-comment-bg";
    let table = buffer.tag_table();
    let tag = match table.lookup(tag_name) {
        Some(t) => t,
        None => {
            let t = TextTag::new(Some(tag_name));
            table.add(&t);
            t
        }
    };
    // Update colour every call so theme switches are reflected on next keystroke.
    // Use the user's accent colour rather than a hardcoded blue.
    let is_dark = adw::StyleManager::default().is_dark();
    let alpha = if is_dark { 0.10_f32 } else { 0.08_f32 };
    let dummy = gtk4::Label::new(None);
    #[allow(deprecated)]
    let base = dummy
        .style_context()
        .lookup_color("accent_color")
        .unwrap_or(gtk4::gdk::RGBA::new(0.2, 0.4, 0.9, 1.0));
    let color = gtk4::gdk::RGBA::new(base.red(), base.green(), base.blue(), alpha);
    tag.set_paragraph_background_rgba(Some(&color));

    let (buf_start, buf_end) = buffer.bounds();
    let text = buffer.text(&buf_start, &buf_end, false).to_string();
    let lines: Vec<&str> = text.lines().collect();
    let n = lines.len();

    let mut spans: Vec<(i32, i32)> = Vec::new();
    let mut i = 0;
    while i < n {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("//") {
            // Merge consecutive // lines into one span
            let run_start = i;
            while i < n && lines[i].trim().starts_with("//") {
                i += 1;
            }
            spans.push((run_start as i32, (i - 1) as i32));
        } else if trimmed.contains("/*") {
            // Block comment: scan for closing */
            let block_start = i;
            while i < n && !lines[i].contains("*/") {
                i += 1;
            }
            if i < n {
                i += 1;
            } // include closing line
            let last = (i - 1).min(n.saturating_sub(1));
            spans.push((block_start as i32, last as i32));
        } else {
            i += 1;
        }
    }

    if let Some(c) = cache {
        if *c.borrow() == spans {
            return;
        }
        *c.borrow_mut() = spans.clone();
    }

    buffer.remove_tag(&tag, &buf_start, &buf_end);
    for (start_line, end_line) in spans {
        if let (Some(ts), Some(mut te)) = (
            buffer.iter_at_line(start_line),
            buffer.iter_at_line(end_line),
        ) {
            te.forward_to_line_end();
            buffer.apply_tag(&tag, &ts, &te);
        }
    }
}

/// How long the viewport is held after a paste. Long enough to outlast GTK's
/// scroll animation (about a dozen frames), short enough that a deliberate
/// scroll right after pasting still feels immediate.
/// How long after a key press an edit or cursor move still counts as the
/// user's own typing or keyboard navigation.
/// Debug switch: `ZERKALO_TRACE_SCROLL=1` logs every vertical scroll to stderr,
/// so a jump can be attributed (GTK, a guard, or a jump request).
fn scroll_trace_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("ZERKALO_TRACE_SCROLL").is_some_and(|v| v != "0"))
}

/// How long the pointer must rest on an underlined error before its pop-up opens.
const HOVER_DELAY: Duration = Duration::from_millis(450);

const KEY_INTENT_WINDOW: Duration = Duration::from_millis(400);

const PASTE_HOLD: Duration = Duration::from_millis(600);

// Replace the legacy `it.numbering` heading pattern that Typst's non-PDF export
// pipeline cannot handle.  Called on file open so that saving the document
// will persist the fix to disk.
fn migrate_template_it_numbering(content: &str) -> String {
    const OLD: &str =
        "#if it.numbering != none [#context counter(heading).display(it.numbering)#h(0.3em)]";

    let template_range = content
        .find("// ZERKALO-TEMPLATE-BEGIN")
        .zip(content.find("// ZERKALO-TEMPLATE-END"));

    let (num_on, num_fmt) = if let Some((b, e)) = template_range {
        let block = &content[b..e];
        let mut on = false;
        let mut fmt = String::new();
        for line in block.lines() {
            if let Some(rest) = line.trim().strip_prefix("#set heading(numbering: \"") {
                if let Some(end) = rest.find('"') {
                    fmt = rest[..end].to_string();
                    on = true;
                    break;
                }
            }
        }
        (on, fmt)
    } else {
        (false, String::new())
    };

    let new_prefix = if num_on {
        let f = if num_fmt.is_empty() {
            "1.".to_string()
        } else {
            num_fmt
        };
        format!("#context counter(heading).display(\"{f}\")#h(0.3em)")
    } else {
        String::new()
    };

    content.replace(OLD, &new_prefix)
}

// Remove ZERKALO-STYLE and ZERKALO-TEMPLATE blocks before word counting.
// These contain raw Typst code that would otherwise inflate the count.
pub(super) fn strip_zerkalo_blocks(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_block = false;
    for line in input.lines() {
        let t = line.trim();
        if t == "// ZERKALO-STYLE-BEGIN" || t == "// ZERKALO-TEMPLATE-BEGIN" {
            in_block = true;
            continue;
        }
        if t == "// ZERKALO-STYLE-END" || t == "// ZERKALO-TEMPLATE-END" {
            in_block = false;
            continue;
        }
        if !in_block {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

pub(super) fn strip_typst_markup(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;
    let mut in_raw_block = false;
    let mut in_block_comment = false;

    while i < n {
        let c = chars[i];

        if in_raw_block {
            if c == '`' && chars.get(i + 1) == Some(&'`') && chars.get(i + 2) == Some(&'`') {
                in_raw_block = false;
                i += 3;
            } else {
                i += 1;
            }
            continue;
        }

        if in_block_comment {
            if c == '*' && chars.get(i + 1) == Some(&'/') {
                in_block_comment = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }

        // Open raw block ```
        if c == '`' && chars.get(i + 1) == Some(&'`') && chars.get(i + 2) == Some(&'`') {
            in_raw_block = true;
            i += 3;
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Line comment //
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Block comment /*
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            in_block_comment = true;
            i += 2;
            continue;
        }

        // Heading lines starting with =
        let at_line_start = out.is_empty() || out.ends_with('\n');
        if at_line_start && c == '=' {
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Inline raw `...`
        if c == '`' {
            i += 1;
            while i < n && chars[i] != '`' && chars[i] != '\n' {
                i += 1;
            }
            if i < n && chars[i] == '`' {
                i += 1;
            }
            out.push(' ');
            continue;
        }

        // Math $...$
        if c == '$' {
            i += 1;
            while i < n && chars[i] != '$' {
                i += 1;
            }
            if i < n {
                i += 1;
            }
            out.push(' ');
            continue;
        }

        // Citation reference @key
        if c == '@' {
            i += 1;
            while i < n
                && (chars[i].is_alphanumeric()
                    || chars[i] == '_'
                    || chars[i] == '-'
                    || chars[i] == ':')
            {
                i += 1;
            }
            continue;
        }

        // Hash function calls: skip #ident and (...){...} args, but KEEP text in [...] args.
        // Structural directives (#set, #show, #let, #import, #include, etc.) have a space
        // between the keyword and the element name — skip the whole line for those.
        if c == '#' {
            i += 1;
            while i < n
                && (chars[i].is_alphanumeric()
                    || chars[i] == '_'
                    || chars[i] == '-'
                    || chars[i] == '.')
            {
                i += 1;
            }
            if i < n && chars[i] == ' ' {
                while i < n && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            while i < n && matches!(chars[i], '[' | '(' | '{') {
                if chars[i] == '[' {
                    // Content block — recursively strip and keep the text
                    let end = skip_balanced_typst(&chars, i, n);
                    if end > i + 1 {
                        let inner: String = chars[i + 1..end - 1].iter().collect();
                        out.push_str(&strip_typst_markup(&inner));
                    }
                    out.push(' ');
                    i = end;
                } else {
                    i = skip_balanced_typst(&chars, i, n);
                }
            }
            continue;
        }

        // Label syntax <label>
        if c == '<' {
            while i < n && chars[i] != '>' && chars[i] != '\n' {
                i += 1;
            }
            if i < n && chars[i] == '>' {
                i += 1;
            }
            continue;
        }

        out.push(c);
        i += 1;
    }

    out
}

fn set_view_line_spacing(view: &View, spacing: u32) {
    view.set_pixels_above_lines(spacing as i32);
    view.set_pixels_below_lines(spacing as i32);
    // Above/below only separate paragraphs; in prose a paragraph is many wrapped
    // lines, so without this the setting barely showed.
    view.set_pixels_inside_wrap(spacing as i32);
}

fn apply_space_drawer(view: &View, enabled: bool) {
    let sd = view.space_drawer();
    sd.set_enable_matrix(enabled);
    if enabled {
        sd.set_types_for_locations(
            sourceview5::SpaceLocationFlags::ALL,
            sourceview5::SpaceTypeFlags::SPACE | sourceview5::SpaceTypeFlags::TAB,
        );
    } else {
        sd.set_types_for_locations(
            sourceview5::SpaceLocationFlags::ALL,
            sourceview5::SpaceTypeFlags::empty(),
        );
    }
}

impl EditorPane {}

#[cfg(test)]
mod tests {
    #[test]
    fn open_tabs_take_what_arrived_unless_they_hold_unsaved_typing() {
        let tabs = vec![
            (PathBuf::from("same.typ"), "same".to_string()),
            (PathBuf::from("arrived.typ"), "old words".to_string()),
            (PathBuf::from("typing.typ"), "my unsaved words".to_string()),
            (PathBuf::from("gone.typ"), "no file".to_string()),
        ];
        let disk = |p: &std::path::Path| match p.to_str().unwrap() {
            "same.typ" => Some("same".to_string()),
            "arrived.typ" => Some("new words from GitHub".to_string()),
            "typing.typ" => Some("their version".to_string()),
            _ => None,
        };
        let modified = |p: &std::path::Path| p.to_str() == Some("typing.typ");
        let (refresh, skipped) = tabs_to_refresh(&tabs, modified, disk);
        assert_eq!(
            refresh,
            vec![(
                PathBuf::from("arrived.typ"),
                "new words from GitHub".to_string()
            )]
        );
        assert_eq!(skipped, vec![PathBuf::from("typing.typ")]);
    }

    use super::*;

    // ── Minimal buffer rewrites ──────────────────────────────────────────────

    #[test]
    fn a_rewrite_touches_only_the_stretch_that_differs() {
        let (start, end, ins) = changed_span("the cat sat", "the dog sat").unwrap();
        assert_eq!((start, end, ins.as_str()), (4, 7, "dog"));
    }

    #[test]
    fn an_identical_rewrite_changes_nothing() {
        assert!(changed_span("same", "same").is_none());
    }

    #[test]
    fn a_pure_insertion_or_deletion_is_one_sided() {
        assert_eq!(changed_span("ab", "aXb"), Some((1, 1, "X".to_string())));
        assert_eq!(changed_span("aXb", "ab"), Some((1, 2, String::new())));
    }

    #[test]
    fn repeated_characters_are_not_counted_twice() {
        // Prefix and suffix must not overlap: "aa" -> "aaa" is one insertion.
        assert_eq!(changed_span("aa", "aaa"), Some((2, 2, "a".to_string())));
        assert_eq!(changed_span("aaa", "aa"), Some((2, 3, String::new())));
    }

    #[test]
    fn offsets_count_characters_not_bytes() {
        let (start, end, ins) = changed_span("héllo wörld", "héllo wurld").unwrap();
        assert_eq!((start, end, ins.as_str()), (7, 8, "u"));
    }

    #[test]
    fn replacing_everything_replaces_everything() {
        assert_eq!(changed_span("abc", "xyz"), Some((0, 3, "xyz".to_string())));
        assert_eq!(changed_span("", "xyz"), Some((0, 0, "xyz".to_string())));
        assert_eq!(changed_span("abc", ""), Some((0, 3, String::new())));
    }

    // ── Word counting ────────────────────────────────────────────────────────

    #[test]
    fn counts_plain_prose_words() {
        assert_eq!(count_content_words("one two three"), 3);
        assert_eq!(count_content_words(""), 0);
        assert_eq!(count_content_words("   \n\n  "), 0);
    }

    #[test]
    fn word_count_excludes_zerkalo_template_blocks() {
        let doc = "\
// ZERKALO-TEMPLATE-BEGIN
#set page(paper: \"a4\")
#set text(size: 12pt)
// ZERKALO-TEMPLATE-END
Real prose here.
";
        assert_eq!(
            count_content_words(doc),
            3,
            "only the prose line should count"
        );
    }

    #[test]
    fn lorem_counts_as_the_number_of_words_it_generates() {
        assert_eq!(count_words_typst("#lorem(50)"), 50);
        assert_eq!(count_words_typst("before #lorem(10) after"), 12);
        assert_eq!(count_words_typst("no lorem here"), 3);
    }

    #[test]
    fn an_unterminated_lorem_call_stops_the_count_rather_than_looping() {
        assert_eq!(count_words_typst("some words #lorem(30"), 2);
    }

    #[test]
    fn a_document_without_a_goal_comment_falls_back_to_the_settings_goal() {
        // The Settings goal was dead: never applied, and open_file only ever
        // set a goal when the document carried its own comment — so opening a
        // document with a comment then one without left the first one's goal on
        // screen. Both call sites now resolve the goal this way.
        let resolve = |content: &str, default: u32| parse_goal_comment(content).unwrap_or(default);
        assert_eq!(resolve("// @zerkalo-goal: 1500\n= Doc\n", 800), 1500);
        assert_eq!(resolve("= Doc\n", 800), 800);
        assert_eq!(resolve("= Doc\n", 0), 0);
    }

    #[test]
    fn word_count_label_reports_a_session_delta_only_when_words_were_added() {
        assert_eq!(
            wc_str_with_delta("one two three", 1),
            "3 words (+2) · < 1 min read"
        );
        assert_eq!(
            wc_str_with_delta("one two three", 3),
            "3 words · < 1 min read"
        );
        assert_eq!(
            wc_str_with_delta("one two three", 9),
            "3 words · < 1 min read"
        );
    }

    #[test]
    fn reading_time_switches_from_under_a_minute_at_two_hundred_words() {
        let just_under = "word ".repeat(199);
        let exactly = "word ".repeat(200);
        assert!(wc_str_with_delta(&just_under, 0).contains("< 1 min read"));
        assert!(wc_str_with_delta(&exactly, 0).contains("1 min read"));
        assert!(!wc_str_with_delta(&exactly, 0).contains("< 1 min"));
    }

    // ── Headings ─────────────────────────────────────────────────────────────

    #[test]
    fn heading_level_counts_leading_equals_signs() {
        assert_eq!(section_heading_level("= Top"), Some(1));
        assert_eq!(section_heading_level("== Second"), Some(2));
        assert_eq!(section_heading_level("===== Fifth"), Some(5));
        assert_eq!(section_heading_level("   == Indented"), Some(2));
    }

    /// Typst needs the space: `=text` is not a heading, and `==` alone is not
    /// one either. Getting this wrong would put junk in the outline panel.
    #[test]
    fn equals_without_a_following_space_is_not_a_heading() {
        assert_eq!(section_heading_level("=NoSpace"), None);
        assert_eq!(section_heading_level("=="), None);
        assert_eq!(section_heading_level("plain text"), None);
        assert_eq!(section_heading_level(""), None);
        assert_eq!(section_heading_level("a = b"), None);
    }

    // ── Goal comment ─────────────────────────────────────────────────────────

    #[test]
    fn reads_the_word_count_goal_from_a_zerkalo_comment() {
        assert_eq!(
            parse_goal_comment("// @zerkalo-goal: 1500\n= Doc\n"),
            Some(1500)
        );
        assert_eq!(
            parse_goal_comment("= Doc\n// @zerkalo-goal:800\n"),
            Some(800)
        );
    }

    #[test]
    fn a_missing_or_malformed_goal_comment_yields_none() {
        assert_eq!(parse_goal_comment("= Doc\n\nNo goal here.\n"), None);
        assert_eq!(parse_goal_comment("// @zerkalo-goal: not-a-number\n"), None);
        assert_eq!(parse_goal_comment(""), None);
    }

    /// Only the first 20 lines are scanned, so a goal further down is ignored.
    #[test]
    fn the_goal_comment_is_only_honoured_near_the_top_of_the_file() {
        let mut doc = "filler\n".repeat(25);
        doc.push_str("// @zerkalo-goal: 900\n");
        assert_eq!(parse_goal_comment(&doc), None);

        let mut near_top = "filler\n".repeat(5);
        near_top.push_str("// @zerkalo-goal: 900\n");
        assert_eq!(parse_goal_comment(&near_top), Some(900));
    }

    // ── LSP snippets ─────────────────────────────────────────────────────────

    #[test]
    fn strips_numbered_and_braced_snippet_placeholders() {
        assert_eq!(strip_snippets("figure($0)"), "figure()");
        assert_eq!(strip_snippets("figure(${1:body})"), "figure()");
        assert_eq!(
            strip_snippets("#table(columns: $1, $2)"),
            "#table(columns: , )"
        );
        assert_eq!(strip_snippets("no placeholders"), "no placeholders");
    }

    /// A bare `$` is Typst's math delimiter, not a placeholder, so it survives.
    #[test]
    fn a_lone_dollar_sign_is_preserved() {
        assert_eq!(strip_snippets("$x + y$"), "$x + y$");
        assert_eq!(strip_snippets("cost: $"), "cost: $");
    }

    // ── Balanced-delimiter scanning ──────────────────────────────────────────

    #[test]
    fn skips_to_just_past_the_matching_delimiter() {
        let c: Vec<char> = "(abc)rest".chars().collect();
        assert_eq!(skip_balanced_typst(&c, 0, c.len()), 5);
        let c: Vec<char> = "[a[b]c]tail".chars().collect();
        assert_eq!(
            skip_balanced_typst(&c, 0, c.len()),
            7,
            "nesting must be respected"
        );
        let c: Vec<char> = "{x}".chars().collect();
        assert_eq!(skip_balanced_typst(&c, 0, c.len()), 3);
    }

    #[test]
    fn a_non_delimiter_advances_by_one_and_an_unclosed_one_runs_to_the_end() {
        let c: Vec<char> = "abc".chars().collect();
        assert_eq!(skip_balanced_typst(&c, 0, c.len()), 1);
        let c: Vec<char> = "(never closed".chars().collect();
        assert_eq!(skip_balanced_typst(&c, 0, c.len()), c.len());
    }

    // ── Legacy template migration ────────────────────────────────────────────

    const LEGACY: &str =
        "#if it.numbering != none [#context counter(heading).display(it.numbering)#h(0.3em)]";

    /// The legacy `it.numbering` pattern breaks Typst's non-PDF export. When the
    /// template turns numbering on, it is replaced with the concrete format.
    #[test]
    fn legacy_numbering_is_rewritten_with_the_templates_format() {
        let doc = format!(
            "// ZERKALO-TEMPLATE-BEGIN\n#set heading(numbering: \"1.1\")\n// ZERKALO-TEMPLATE-END\n{LEGACY}\n"
        );
        let out = migrate_template_it_numbering(&doc);
        assert!(!out.contains("it.numbering"));
        assert!(out.contains("#context counter(heading).display(\"1.1\")#h(0.3em)"));
    }

    #[test]
    fn legacy_numbering_is_removed_when_the_template_has_no_numbering() {
        let doc = format!(
            "// ZERKALO-TEMPLATE-BEGIN\n#set page(paper: \"a4\")\n// ZERKALO-TEMPLATE-END\n{LEGACY}\n"
        );
        let out = migrate_template_it_numbering(&doc);
        assert!(!out.contains("it.numbering"));
        assert!(!out.contains("counter(heading).display"));
    }

    #[test]
    fn a_document_without_the_legacy_pattern_is_returned_unchanged() {
        let doc = "= Title\n\nOrdinary prose.\n";
        assert_eq!(migrate_template_it_numbering(doc), doc);
    }

    #[test]
    fn only_the_generated_separator_lines_count_as_the_separator() {
        assert!(is_separator_line(
            "// ── Document body — Zerkalo uses this exact line to find where your writing starts. Leave it in place; everything below it is yours to edit freely."
        ));
        assert!(is_separator_line(
            "// ── Document body ───────────────────────────────────────────────────"
        ));
        assert!(!is_separator_line(
            "// ── Document body ───────────────────────────────────────────────────My first paragraph"
        ));
    }

    #[test]
    fn migration_is_idempotent() {
        let doc = format!(
            "// ZERKALO-TEMPLATE-BEGIN\n#set heading(numbering: \"A.\")\n// ZERKALO-TEMPLATE-END\n{LEGACY}\n"
        );
        let once = migrate_template_it_numbering(&doc);
        assert_eq!(migrate_template_it_numbering(&once), once);
    }
}
