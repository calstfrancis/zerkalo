use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{
    AlertDialog, Box as GtkBox, Button, CssProvider, DropTarget, Entry, EventControllerFocus,
    EventControllerKey, EventControllerMotion, GestureClick, Label,
    Notebook, Orientation, Popover, ProgressBar, PropagationPhase, ScrolledWindow, Separator,
    TextSearchFlags, TextTag, TextWindowType, ToggleButton,
};
use libadwaita as adw;
use sourceview5::prelude::*;
use sourceview5::{Buffer, LanguageManager, MarkAttributes, StyleSchemeManager, View};

use crate::bibliography::BibEntry;
use crate::lsp::CompletionItem;
use super::bib_popup::BibPopup;
use super::font_manager::FontManager;
use super::find_bar::FindBar;
use super::lsp_popup::LspPopup;

// Package names/descriptions matching EXTRA_PACKAGES in template_dialog.rs
const IMPORT_PACKAGE_TOOLTIPS: &[(&str, &str)] = &[
    ("droplet", "Large decorative first-letter (dropcap)"),
    ("codly", "Beautiful code listings with syntax highlighting"),
    ("showybox", "Coloured callout and theorem boxes"),
    ("gentle-clues", "Admonition blocks: note, tip, warning, important"),
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
    <style id="keyword"  name="Keyword"  map-to="def:keyword"/>
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
        <context id="keywords" style-ref="keyword">
          <keyword>let</keyword>
          <keyword>set</keyword>
          <keyword>show</keyword>
          <keyword>if</keyword>
          <keyword>else</keyword>
          <keyword>for</keyword>
          <keyword>in</keyword>
          <keyword>while</keyword>
          <keyword>break</keyword>
          <keyword>continue</keyword>
          <keyword>return</keyword>
          <keyword>import</keyword>
          <keyword>include</keyword>
          <keyword>none</keyword>
          <keyword>auto</keyword>
          <keyword>true</keyword>
          <keyword>false</keyword>
        </context>
      </include>
    </context>
  </definitions>
</language>
"#;

// ── Built-in academic snippets ────────────────────────────────────────────────
// (match_key, display_label, insert_text_with_leading_#)
// (match_key, display_label, description, insert_text)
const ACADEMIC_SNIPPETS: &[(&str, &str, &str, &str)] = &[
    ("figure", "Figure",
     "Image with caption and cross-reference label",
     "#figure(\n  image(\"\", width: 80%),\n  caption: [Caption text],\n) <fig:label>"),
    ("table", "Table",
     "Table with a header row and cross-reference label",
     "#figure(\n  table(\n    columns: (auto, auto),\n    table.header([*Column 1*], [*Column 2*]),\n    [Cell 1], [Cell 2],\n  ),\n  caption: [Table title],\n) <tab:label>"),
    ("footnote", "Footnote",
     "Inline footnote — appears at the bottom of the page",
     "#footnote[Note text]"),
    ("bibliography", "Bibliography",
     "Bibliography section from a .bib file",
     "#bibliography(\"refs.bib\")"),
    ("pagebreak", "Page break",
     "Force content to start on a new page",
     "#pagebreak()"),
    ("outline", "Table of Contents",
     "Auto-generated table of contents (headings up to depth 3)",
     "#outline(title: [Contents], depth: 3)"),
    ("lorem", "Lorem ipsum",
     "100 words of placeholder text",
     "#lorem(100)"),
    ("set", "Set rule",
     "Change text size and font for the rest of the document",
     "#set text(size: 11pt, font: \"Liberation Serif\")"),
    ("show", "Show rule",
     "Transform how an element is displayed (example: bold headings)",
     "#show heading: it => strong(it)"),
    ("block", "Block / quote",
     "Indented block — use for block quotations",
     "#block(inset: (left: 2em))[\n  Quoted text\n]"),
    ("dropcap", "Drop cap",
     "Large decorative first letter. Requires Droplet enabled in template settings → Packages.",
     "#dropcap[\n  First paragraph text here.\n]"),
];

// ── Internal types ────────────────────────────────────────────────────────────

struct EditorTab {
    buffer: Buffer,
    view: View,
    scroll_window: ScrolledWindow,
    modified: bool,
    dot_label: Label,
    diag_dot: Label,
    tab_box: GtkBox,
    display_name: String,
    lsp_popup: LspPopup,
    session_start_words: u32,
}

struct EditorState {
    tabs: HashMap<PathBuf, EditorTab>,
}

// ── Public API ────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct EditorPane {
    outer: GtkBox,
    notebook: Notebook,
    state: Rc<RefCell<EditorState>>,
    on_change: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_modified_changed: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    on_file_dirty: Rc<RefCell<Option<Box<dyn Fn(PathBuf, bool)>>>>,
    on_image_drop: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>,
    on_delete_file: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>,
    on_page_switch: Rc<RefCell<Option<Box<dyn Fn(String, PathBuf)>>>>,
    on_file_opened: Rc<RefCell<Option<Box<dyn Fn(PathBuf, String)>>>>,
    on_completion_needed: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32, u32)>>>>,
    on_cursor_heading: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32)>>>>,
    on_cursor_moved: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32, u32)>>>>,
    bib_entries: Rc<RefCell<Vec<BibEntry>>>,
    font_provider: Rc<CssProvider>,
    font_size: Rc<RefCell<u32>>,
    font_family: Rc<RefCell<String>>,
    word_wrap: Rc<RefCell<bool>>,
    show_whitespace: Rc<RefCell<bool>>,
    tab_width: Rc<RefCell<u32>>,
    find_bar: FindBar,
    undo_btn: Button,
    redo_btn: Button,
    word_count_label: Label,
    on_word_count_click: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    session_delta_label: Label,
    goal_bar: ProgressBar,
    lsp_status_label: Label,
    diag_label: Label,
    last_diagnostics: Rc<RefCell<Vec<(PathBuf, u32, bool)>>>,
    cursor_label: Label,
    section_wc_label: Label,
    breadcrumb_label: Label,
    breadcrumb_bar: GtkBox,
    word_wrap_btn: ToggleButton,
    simple_mode: Rc<RefCell<bool>>,
    simple_mode_label: Label,
    on_simple_mode_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    spell_checker: Rc<RefCell<crate::spellcheck::SpellChecker>>,
    line_spacing: Rc<RefCell<u32>>,
    typewriter_scroll: Rc<RefCell<bool>>,
    word_count_goal: Rc<RefCell<u32>>,
    last_wc_text: Rc<RefCell<String>>,
    project_root: Rc<RefCell<Option<PathBuf>>>,
    status_bar: GtkBox,
    status_bar_left_spacer: GtkBox,
    autocorrect_label: Label,
    on_autocorrect_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    gost_label: Label,
    on_gost_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    on_version_click: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    bib_active: Rc<RefCell<bool>>,
    format_bar_container: GtkBox,
    format_bar_label: Label,
    format_bar_toggle_btn: Button,
    on_format_bar_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    user_dismissed_format_bar: Rc<RefCell<bool>>,
    focus_label: Label,
    on_focus_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    on_doc_font: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    on_doc_font_size: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    font_bar_label: Label,
    size_bar_label: Label,
}

fn set_autocorrect_label(label: &Label, enabled: bool) {
    if enabled {
        label.set_markup("<b>autocorrect</b>");
    } else {
        label.set_text("autocorrect");
    }
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


impl EditorPane {
    pub fn new() -> Self {
        let notebook = Notebook::new();
        notebook.set_scrollable(true);
        notebook.set_hexpand(true);
        notebook.set_vexpand(true);
        notebook.set_show_tabs(false);

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
        let lang_dir = glib::user_data_dir().join("zerkalo/language-specs");
        let lang_file = lang_dir.join("typst.lang");
        if !lang_file.exists() {
            if std::fs::create_dir_all(&lang_dir).is_ok() {
                let _ = std::fs::write(&lang_file, TYPST_LANG);
            }
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

        let gost_label = Label::new(Some("gost type b"));
        gost_label.add_css_class("dim-label");
        gost_label.add_css_class("caption");
        gost_label.set_use_markup(true);
        gost_label.set_margin_top(3);
        gost_label.set_margin_bottom(3);

        let gost_btn = Button::new();
        gost_btn.set_child(Some(&gost_label));
        gost_btn.add_css_class("flat");
        gost_btn.set_tooltip_text(Some("Toggle GOST type B engineering font for the whole UI"));
        gost_btn.set_margin_end(4);

        let autocorrect_label = Label::new(Some("autocorrect"));
        autocorrect_label.add_css_class("dim-label");
        autocorrect_label.add_css_class("caption");
        autocorrect_label.set_use_markup(true);
        autocorrect_label.set_margin_top(3);
        autocorrect_label.set_margin_bottom(3);

        let autocorrect_btn = Button::new();
        autocorrect_btn.set_child(Some(&autocorrect_label));
        autocorrect_btn.add_css_class("flat");
        autocorrect_btn.add_css_class("status-toggle");
        autocorrect_btn.set_tooltip_text(Some("Toggle autocorrect (fixes spelling as you type)"));
        autocorrect_btn.set_margin_end(4);
        autocorrect_btn.update_property(&[gtk4::accessible::Property::Label("Toggle autocorrect")]);

        let search_label = Label::new(Some("search"));
        search_label.add_css_class("dim-label");
        search_label.add_css_class("caption");
        search_label.set_use_markup(true);
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
        format_bar_toggle_btn.update_property(&[gtk4::accessible::Property::Label("Toggle format bar")]);

        status_bar.append(&focus_toggle_btn);
        status_bar.append(&format_bar_toggle_btn);
        status_bar.append(&autocorrect_btn);
        status_bar.append(&gost_btn);
        status_bar.append(&search_btn);

        let sb_sep1 = gtk4::Separator::new(Orientation::Vertical);
        sb_sep1.add_css_class("statusbar-sep");
        sb_sep1.set_margin_start(6);
        sb_sep1.set_margin_end(6);
        sb_sep1.set_margin_top(6);
        sb_sep1.set_margin_bottom(6);
        status_bar.append(&sb_sep1);

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
        status_bar.append(&cursor_label);

        let lsp_status_label = Label::new(None);
        lsp_status_label.add_css_class("dim-label");
        lsp_status_label.add_css_class("caption");
        lsp_status_label.set_use_markup(true);
        lsp_status_label.set_margin_start(8);
        lsp_status_label.set_margin_top(3);
        lsp_status_label.set_margin_bottom(3);
        status_bar.append(&lsp_status_label);

        let diag_label = Label::new(None);
        diag_label.add_css_class("dim-label");
        diag_label.add_css_class("caption");
        diag_label.set_margin_start(8);
        diag_label.set_margin_top(3);
        diag_label.set_margin_bottom(3);
        status_bar.append(&diag_label);

        // Left spacer so SIMPLE is centered
        let left_spacer = GtkBox::new(Orientation::Horizontal, 0);
        left_spacer.set_hexpand(true);
        status_bar.append(&left_spacer);

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
            "Simple Mode: hides Typst front-matter above the document body.\nEdit it via the Update Template button.",
        ));
        simple_mode_btn.update_property(&[gtk4::accessible::Property::Label("Toggle simple mode")]);
        status_bar.append(&simple_mode_btn);

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
        status_bar.append(&wc_btn);

        let session_delta_label = Label::new(None);
        session_delta_label.add_css_class("dim-label");
        session_delta_label.add_css_class("caption");
        session_delta_label.set_margin_end(8);
        session_delta_label.set_margin_top(3);
        session_delta_label.set_margin_bottom(3);
        session_delta_label.set_visible(false);
        status_bar.append(&session_delta_label);

        let goal_bar = ProgressBar::new();
        goal_bar.set_visible(false);
        goal_bar.set_valign(gtk4::Align::Center);
        goal_bar.set_size_request(80, -1);
        goal_bar.set_margin_end(8);
        goal_bar.set_tooltip_text(Some("Word count progress toward goal"));
        status_bar.append(&goal_bar);

        let version_btn = Button::with_label(concat!("v", env!("CARGO_PKG_VERSION")));
        version_btn.add_css_class("flat");
        version_btn.add_css_class("dim-label");
        version_btn.add_css_class("caption");
        version_btn.set_margin_end(4);
        version_btn.set_tooltip_text(Some("View changelog"));
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

        let tab_dropdown_btn = Button::from_icon_name("pan-down-symbolic");
        tab_dropdown_btn.add_css_class("flat");
        tab_dropdown_btn.set_tooltip_text(Some("Open tabs"));
        tab_dropdown_btn.set_margin_end(4);
        tab_dropdown_btn.set_valign(gtk4::Align::Center);

        let word_wrap_btn = ToggleButton::new();
        word_wrap_btn.set_icon_name("format-justify-left-symbolic");
        word_wrap_btn.add_css_class("flat");
        word_wrap_btn.set_tooltip_text(Some("Toggle word wrap"));
        word_wrap_btn.set_valign(gtk4::Align::Center);
        word_wrap_btn.update_property(&[gtk4::accessible::Property::Label("Toggle word wrap")]);

        let breadcrumb_bar = GtkBox::new(Orientation::Horizontal, 0);
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
        breadcrumb_bar.append(&tab_dropdown_btn);
        let _ = &word_wrap_btn; // kept for settings sync; not shown in toolbar

        let editor_row = GtkBox::new(Orientation::Horizontal, 0);
        editor_row.set_hexpand(true);
        editor_row.set_vexpand(true);
        editor_row.append(&notebook);

        // ── Formatting toolbar ────────────────────────────────────────────────
        let format_bar = GtkBox::new(Orientation::Horizontal, 0);
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
        fb_sep1.set_margin_top(6); fb_sep1.set_margin_bottom(6);
        fb_sep1.set_margin_start(4); fb_sep1.set_margin_end(4);
        format_bar.append(&fb_sep1);

        let h1_btn = Button::with_label("H1");
        h1_btn.add_css_class("flat"); h1_btn.add_css_class("caption");
        h1_btn.set_tooltip_text(Some("Heading 1  (= Heading text)"));
        h1_btn.update_property(&[gtk4::accessible::Property::Label("Heading 1")]);
        let h2_btn = Button::with_label("H2");
        h2_btn.add_css_class("flat"); h2_btn.add_css_class("caption");
        h2_btn.set_tooltip_text(Some("Heading 2  (== Heading text)"));
        h2_btn.update_property(&[gtk4::accessible::Property::Label("Heading 2")]);
        let h3_btn = Button::with_label("H3");
        h3_btn.add_css_class("flat"); h3_btn.add_css_class("caption");
        h3_btn.set_tooltip_text(Some("Heading 3  (=== Heading text)"));
        h3_btn.update_property(&[gtk4::accessible::Property::Label("Heading 3")]);

        format_bar.append(&h1_btn);
        format_bar.append(&h2_btn);
        format_bar.append(&h3_btn);

        let fb_sep2 = Separator::new(Orientation::Vertical);
        fb_sep2.set_margin_top(6); fb_sep2.set_margin_bottom(6);
        fb_sep2.set_margin_start(4); fb_sep2.set_margin_end(4);
        format_bar.append(&fb_sep2);

        let pb_btn = Button::with_label("¶");
        pb_btn.add_css_class("flat"); pb_btn.add_css_class("caption");
        pb_btn.set_tooltip_text(Some("Insert page break  (#pagebreak())"));
        pb_btn.update_property(&[gtk4::accessible::Property::Label("Insert page break")]);
        format_bar.append(&pb_btn);

        let fb_sep3 = Separator::new(Orientation::Vertical);
        fb_sep3.set_margin_top(6); fb_sep3.set_margin_bottom(6);
        fb_sep3.set_margin_start(4); fb_sep3.set_margin_end(4);
        format_bar.append(&fb_sep3);

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
                cell.update_property(&[gtk4::accessible::Property::Label(
                    &format!("{}×{} table", r + 1, c + 1)
                )]);
                row_btns.push(cell.clone());
                row_box.append(&cell);
            }
            grid_btns.push(row_btns);
            table_grid_box.append(&row_box);
        }
        // Wrap grid_btns in Rc so hover handlers can update all cells
        let grid_rc: Rc<Vec<Vec<Button>>> = Rc::new(
            grid_btns.iter().map(|row| row.clone()).collect()
        );
        // Wire hover handlers (separate pass so all cells are available)
        for r in 0..GRID_MAX {
            for c in 0..GRID_MAX {
                let cell = grid_btns[r][c].clone();
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
                if tp.is_visible() { tp.popdown(); } else { tp.popup(); tp.grab_focus(); }
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

        // ── Spacer ────────────────────────────────────────────────────────────
        let fb_spacer = GtkBox::new(Orientation::Horizontal, 0);
        fb_spacer.set_hexpand(true);
        format_bar.append(&fb_spacer);

        // ── Font dropdown (right-aligned) ────────────────────────────────────
        let enabled_fonts = FontManager::enabled_fonts();
        let font_popover = Popover::new();
        let font_popover_box = GtkBox::new(Orientation::Vertical, 2);
        font_popover_box.set_margin_top(4); font_popover_box.set_margin_bottom(4);
        font_popover_box.set_margin_start(4); font_popover_box.set_margin_end(4);
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
                if fp.is_visible() { fp.popdown(); } else { fp.popup(); fp.grab_focus(); }
            });
        }
        format_bar.append(&font_bar_btn);

        // ── Font size dropdown (right-aligned) ────────────────────────────────
        const DOC_SIZES: &[&str] = &["10pt", "11pt", "12pt", "14pt", "16pt", "18pt", "20pt", "24pt"];
        let size_popover = Popover::new();
        let size_popover_box = GtkBox::new(Orientation::Vertical, 2);
        size_popover_box.set_margin_top(4); size_popover_box.set_margin_bottom(4);
        size_popover_box.set_margin_start(4); size_popover_box.set_margin_end(4);
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
                if sp.is_visible() { sp.popdown(); } else { sp.popup(); sp.grab_focus(); }
            });
        }
        format_bar.append(&size_bar_btn);

        let format_bar_container = GtkBox::new(Orientation::Vertical, 0);
        format_bar_container.append(&format_bar);
        format_bar_container.append(&Separator::new(Orientation::Horizontal));

        let outer = GtkBox::new(Orientation::Vertical, 0);
        outer.set_hexpand(true);
        outer.set_vexpand(true);
        outer.append(&breadcrumb_bar);
        outer.append(&Separator::new(Orientation::Horizontal));
        outer.append(&format_bar_container);
        outer.append(&editor_row);
        outer.append(find_bar.widget());
        // Note: status_bar is intentionally NOT appended here.
        // app_window places it below inner_paned so it spans the full window width.

        let on_change: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_modified_changed: Rc<RefCell<Option<Box<dyn Fn(bool)>>>> = Rc::new(RefCell::new(None));
        let on_file_dirty: Rc<RefCell<Option<Box<dyn Fn(PathBuf, bool)>>>> = Rc::new(RefCell::new(None));
        let on_image_drop: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>> = Rc::new(RefCell::new(None));
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
        let on_gost_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>> =
            Rc::new(RefCell::new(None));
        let on_version_click: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_word_count_click: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_simple_mode_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>> = Rc::new(RefCell::new(None));

        let font_size: Rc<RefCell<u32>> = Rc::new(RefCell::new(13));
        let font_family: Rc<RefCell<String>> = Rc::new(RefCell::new("Monospace".to_string()));
        let word_wrap: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let show_whitespace: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let tab_width: Rc<RefCell<u32>> = Rc::new(RefCell::new(2));
        let line_spacing: Rc<RefCell<u32>> = Rc::new(RefCell::new(2));
        let typewriter_scroll: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let word_count_goal: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let last_wc_text: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
        let project_root: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));

        {
            let state2 = state.clone();
            let wc = word_count_label.clone();
            let ps = on_page_switch.clone();
            let ub = undo_btn.clone();
            let rb = redo_btn.clone();
            notebook.connect_switch_page(move |nb, _, page_num| {
                // Extract content/path and release the state borrow before calling the
                // page-switch callback, which may call all_tab_texts() → double-borrow panic.
                let page_data = {
                    let bstate = state2.borrow();
                    let mut found = None;
                    for (path, tab) in &bstate.tabs {
                        if nb.page_num(&tab.scroll_window) == Some(page_num) {
                            let (s, e) = tab.buffer.bounds();
                            let content = tab.buffer.text(&s, &e, true).to_string();
                            let can_undo = tab.buffer.can_undo();
                            let can_redo = tab.buffer.can_redo();
                            let session_start = tab.session_start_words;
                            found = Some((path.clone(), content, can_undo, can_redo, session_start));
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
            state,
            on_change,
            on_modified_changed,
            on_file_dirty,
            on_image_drop,
            on_delete_file,
            on_page_switch,
            on_file_opened,
            on_completion_needed,
            on_cursor_heading,
            on_cursor_moved,
            bib_entries: Rc::new(RefCell::new(Vec::new())),
            font_provider: Rc::new(font_provider),
            font_size,
            font_family,
            word_wrap,
            show_whitespace,
            tab_width,
            find_bar,
            undo_btn,
            redo_btn,
            word_count_label,
            session_delta_label,
            goal_bar,
            lsp_status_label,
            diag_label,
            last_diagnostics: Rc::new(RefCell::new(Vec::new())),
            cursor_label,
            section_wc_label,
            breadcrumb_label,
            breadcrumb_bar,
            word_wrap_btn,
            simple_mode: Rc::new(RefCell::new(true)),
            simple_mode_label: simple_mode_label.clone(),
            on_simple_mode_toggle,
            spell_checker: Rc::new(RefCell::new(crate::spellcheck::SpellChecker::new(vec!["en_US".to_string()]))),
            line_spacing,
            typewriter_scroll,
            word_count_goal,
            last_wc_text,
            project_root,
            status_bar,
            status_bar_left_spacer: left_spacer.clone(),
            autocorrect_label,
            on_autocorrect_toggle,
            gost_label,
            on_gost_toggle,
            on_version_click,
            on_word_count_click,
            bib_active: Rc::new(RefCell::new(false)),
            format_bar_container,
            format_bar_label,
            format_bar_toggle_btn: format_bar_toggle_btn.clone(),
            on_format_bar_toggle: Rc::new(RefCell::new(None)),
            user_dismissed_format_bar: Rc::new(RefCell::new(false)),
            focus_label,
            on_focus_toggle: Rc::new(RefCell::new(None)),
            on_doc_font: Rc::new(RefCell::new(None)),
            on_doc_font_size: Rc::new(RefCell::new(None)),
            font_bar_label,
            size_bar_label,
        };

        {
            let cb = ep.on_version_click.clone();
            version_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() { f(); }
            });
        }
        {
            let cb = ep.on_word_count_click.clone();
            wc_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() { f(); }
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
            ep.find_bar.set_on_reveal_changed(move |revealed| {
                set_toggle_label(&sl, "search", revealed);
            });
        }
        {
            let lbl_g = ep.gost_label.clone();
            let cb_g = ep.on_gost_toggle.clone();
            let gost_on: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
            gost_btn.connect_clicked(move |_| {
                let new_val = !*gost_on.borrow();
                *gost_on.borrow_mut() = new_val;
                set_toggle_label(&lbl_g, "gost type b", new_val);
                if let Some(f) = cb_g.borrow().as_ref() { f(new_val); }
            });
        }
        {
            let ep_fb = ep.clone();
            format_bar_toggle_btn.connect_clicked(move |_| {
                let new_val = !ep_fb.format_bar_visible();
                ep_fb.set_format_bar_visible(new_val);
                *ep_fb.user_dismissed_format_bar.borrow_mut() = !new_val;
                if let Some(f) = ep_fb.on_format_bar_toggle.borrow().as_ref() { f(new_val); }
            });
        }
        {
            // Tab switcher dropdown — shows all open tabs as clickable rows.
            let ep_tabs = ep.clone();
            tab_dropdown_btn.connect_clicked(move |btn| {
                let popover = Popover::new();
                popover.set_parent(btn);
                popover.set_has_arrow(true);

                let vbox = GtkBox::new(Orientation::Vertical, 0);
                vbox.set_margin_top(4);
                vbox.set_margin_bottom(4);

                let state = ep_tabs.state.borrow();
                let current = ep_tabs.notebook.current_page();

                // Build ordered list: current tab first, then rest in notebook order.
                let mut entries: Vec<(u32, String, PathBuf)> = state.tabs.iter()
                    .filter_map(|(path, tab)| {
                        let page = ep_tabs.notebook.page_num(&tab.scroll_window)?;
                        let name = path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("untitled")
                            .to_string();
                        Some((page, name, path.clone()))
                    })
                    .collect();
                entries.sort_by_key(|(page, _, _)| *page);
                drop(state);

                if entries.is_empty() {
                    let lbl = Label::new(Some("No open files"));
                    lbl.add_css_class("dim-label");
                    lbl.set_margin_top(4);
                    lbl.set_margin_bottom(4);
                    lbl.set_margin_start(8);
                    lbl.set_margin_end(8);
                    vbox.append(&lbl);
                } else {
                    for (page, name, _path) in entries {
                        let row = Button::with_label(&name);
                        row.add_css_class("flat");
                        if Some(page) == current {
                            row.add_css_class("accent");
                        }
                        let nb = ep_tabs.notebook.clone();
                        let pop = popover.clone();
                        row.connect_clicked(move |_| {
                            nb.set_current_page(Some(page));
                            pop.popdown();
                        });
                        vbox.append(&row);
                    }
                }

                popover.set_child(Some(&vbox));
                let pop_close = popover.clone();
                popover.connect_closed(move |_| pop_close.unparent());
                popover.popup();
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
                if let Some(f) = ep_focus.on_focus_toggle.borrow().as_ref() { f(new_val); }
            });
        }

        // Restore focus to editor when format bar popovers close (item 2)
        for pop in [&table_popover, &font_popover, &size_popover] {
            let ep_fc = ep.clone();
            pop.connect_closed(move |_| { ep_fc.grab_focus(); });
        }

        // Wire font dropdown rows
        for (font_name, btn) in &font_buttons {
            let fn2 = font_name.clone();
            let ep_f = ep.clone();
            let fp = font_popover.clone();
            let fbl = ep.font_bar_label.clone();
            btn.connect_clicked(move |_| {
                fp.popdown();
                fbl.set_text(&fn2);
                if let Some(f) = ep_f.on_doc_font.borrow().as_ref() { f(fn2.clone()); }
            });
        }
        // Wire size dropdown rows
        for (size_name, btn) in &size_buttons {
            let sn2 = size_name.clone();
            let ep_s = ep.clone();
            let sp = size_popover.clone();
            let sbl = ep.size_bar_label.clone();
            btn.connect_clicked(move |_| {
                sp.popdown();
                sbl.set_text(&sn2);
                if let Some(f) = ep_s.on_doc_font_size.borrow().as_ref() { f(sn2.clone()); }
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
                let ep2 = ep_img.clone();
                dialog.open(gtk4::Window::NONE, gtk4::gio::Cancellable::NONE, move |result| {
                    if let Ok(file) = result {
                        if let Some(path) = file.path() {
                            if let Some((_, buf)) = ep2.active_view_buffer() {
                                let name = path.file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("image.png");
                                let snippet = format!(
                                    "#figure(\n  image(\"{name}\", width: 80%),\n  caption: [Caption],\n) <fig:label>\n"
                                );
                                buf.insert_at_cursor(&snippet);
                            }
                        }
                    }
                });
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
                if let Some(f) = cb_ac.borrow().as_ref() { f(new_val); }
            });
        }

        {
            let ep_b = ep.clone();
            bold_btn.connect_clicked(move |_| { ep_b.toggle_active_markup("*"); });
        }
        {
            let ep_i = ep.clone();
            italic_btn.connect_clicked(move |_| { ep_i.toggle_active_markup("_"); });
        }
        {
            let ep_h1 = ep.clone();
            h1_btn.connect_clicked(move |_| { ep_h1.set_active_heading(1); });
        }
        {
            let ep_h2 = ep.clone();
            h2_btn.connect_clicked(move |_| { ep_h2.set_active_heading(2); });
        }
        {
            let ep_h3 = ep.clone();
            h3_btn.connect_clicked(move |_| { ep_h3.set_active_heading(3); });
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
            let state_u = ep.state.clone();
            let nb_u = ep.notebook.clone();
            ep.undo_btn.connect_clicked(move |_| {
                let current = nb_u.current_page().unwrap_or(0);
                let buffer = {
                    let state = state_u.borrow();
                    state.tabs.values()
                        .find(|tab| nb_u.page_num(&tab.scroll_window) == Some(current))
                        .map(|tab| tab.buffer.clone())
                };
                if let Some(buf) = buffer { buf.undo(); }
            });
        }
        {
            let state_r = ep.state.clone();
            let nb_r = ep.notebook.clone();
            ep.redo_btn.connect_clicked(move |_| {
                let current = nb_r.current_page().unwrap_or(0);
                let buffer = {
                    let state = state_r.borrow();
                    state.tabs.values()
                        .find(|tab| nb_r.page_num(&tab.scroll_window) == Some(current))
                        .map(|tab| tab.buffer.clone())
                };
                if let Some(buf) = buffer { buf.redo(); }
            });
        }
        {
            let ep2 = ep.clone();
            ep.find_bar.set_on_search(move |text, forward| ep2.do_find(text, forward));
        }
        {
            let ep2 = ep.clone();
            ep.find_bar.set_on_replace_one(move |find, replace| ep2.do_replace_one(find, replace));
        }
        {
            let ep2 = ep.clone();
            ep.find_bar.set_on_replace_all(move |find, replace| ep2.do_replace_all(find, replace));
        }

        // Word wrap toggle button
        {
            let ep2 = ep.clone();
            ep.word_wrap_btn.connect_toggled(move |btn| {
                ep2.apply_word_wrap(btn.is_active());
            });
        }

        // SIMPLE mode button
        {
            let ep2 = ep.clone();
            simple_mode_btn.connect_clicked(move |_| {
                let new_val = !*ep2.simple_mode.borrow();
                ep2.apply_simple_mode(new_val);
                if let Some(f) = ep2.on_simple_mode_toggle.borrow().as_ref() { f(new_val); }
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
        self.status_bar.insert_child_after(w, Some(&self.goal_bar));
    }

    /// Insert a widget into the status bar just before the SIMPLE toggle
    /// (i.e. between the hexpand spacer and SIMPLE).
    pub fn status_bar_insert_before_simple(&self, w: &impl gtk4::prelude::IsA<gtk4::Widget>) {
        self.status_bar.insert_child_after(w, Some(&self.status_bar_left_spacer));
    }

    // ── Settings ──────────────────────────────────────────────────────────────

    pub fn set_bib_entries(&self, entries: Vec<BibEntry>) {
        *self.bib_entries.borrow_mut() = entries;
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

    pub fn set_word_wrap_btn(&self, active: bool) {
        self.word_wrap_btn.set_active(active);
    }

    #[allow(dead_code)]
    pub fn set_word_wrap_btn_visible(&self, v: bool) {
        self.word_wrap_btn.set_visible(v);
    }

    #[allow(dead_code)]
    pub fn get_simple_mode(&self) -> bool {
        *self.simple_mode.borrow()
    }

    /// Apply simple mode to the current active buffer and update button label.
    pub fn apply_simple_mode(&self, on: bool) {
        *self.simple_mode.borrow_mut() = on;
        set_toggle_label(&self.simple_mode_label, "SIMPLE", on);
        self.apply_simple_mode_to_buffer(on);
        if on && !self.format_bar_visible() && !*self.user_dismissed_format_bar.borrow() {
            self.set_format_bar_visible(true);
            if let Some(f) = self.on_format_bar_toggle.borrow().as_ref() { f(true); }
        }
    }

    fn apply_simple_mode_to_buffer(&self, on: bool) {
        let left_margin = if on { 40 } else { 8 };
        let tabs: Vec<_> = {
            let state = self.state.borrow();
            state.tabs.values().map(|t| (t.buffer.clone(), t.view.clone())).collect()
        };
        for (buffer, view) in &tabs {
            apply_simple_mode_tag(buffer, on);
            view.set_show_line_numbers(!on);
            view.set_left_margin(left_margin);
        }
    }

    pub fn set_on_simple_mode_toggle(&self, f: impl Fn(bool) + 'static) {
        *self.on_simple_mode_toggle.borrow_mut() = Some(Box::new(f));
    }

    pub fn breadcrumb_bar_append(&self, w: &impl gtk4::prelude::IsA<gtk4::Widget>) {
        self.breadcrumb_bar.append(w);
    }

    #[allow(dead_code)]
    pub fn set_lsp_label_visible(&self, v: bool) {
        self.lsp_status_label.set_visible(v);
    }

    pub fn apply_word_wrap(&self, enabled: bool) {
        *self.word_wrap.borrow_mut() = enabled;
        let mode = if enabled { gtk4::WrapMode::Word } else { gtk4::WrapMode::None };
        let h_policy = if enabled { gtk4::PolicyType::Never } else { gtk4::PolicyType::Automatic };
        let tabs: Vec<_> = {
            let state = self.state.borrow();
            state.tabs.values().map(|t| (t.view.clone(), t.scroll_window.clone())).collect()
        };
        for (view, scroll) in &tabs {
            view.set_wrap_mode(mode);
            scroll.set_policy(h_policy, gtk4::PolicyType::Automatic);
        }
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
    }

    /// Constrain editor to a comfortable reading width when zen/focus mode is on.
    pub fn set_zen_width(&self, enabled: bool) {
        if enabled {
            self.outer.set_halign(gtk4::Align::Center);
            self.outer.set_size_request(720, -1);
        } else {
            self.outer.set_halign(gtk4::Align::Fill);
            self.outer.set_size_request(-1, -1);
        }
    }

    pub fn grab_focus(&self) {
        if let Some((view, _)) = self.active_view_buffer() {
            view.grab_focus();
        }
    }

    #[allow(dead_code)]
    pub fn apply_word_count_goal(&self, goal: u32) {
        *self.word_count_goal.borrow_mut() = goal;
        if goal == 0 {
            self.goal_bar.set_visible(false);
        } else {
            if let Some(text) = self.get_active_content() {
                update_goal_bar(&self.goal_bar, &text, goal);
            }
        }
    }

    pub fn apply_style_scheme(&self, is_dark: bool) {
        let candidates: &[&str] = if is_dark {
            &["monokai-extended", "solarized-dark", "oblivion", "Adwaita-dark", "classic-dark"]
        } else {
            &["kate", "tango", "Adwaita", "classic"]
        };
        let mgr = StyleSchemeManager::default();
        let scheme = candidates.iter().find_map(|id| mgr.scheme(id));
        let buffers: Vec<_> = {
            let state = self.state.borrow();
            state.tabs.values().map(|t| t.buffer.clone()).collect()
        };
        for buffer in &buffers {
            buffer.set_style_scheme(scheme.as_ref());
        }
    }

    // ── Find & Replace ────────────────────────────────────────────────────────

    pub fn toggle_find(&self) {
        self.find_bar.toggle();
    }

    pub fn do_find(&self, text: &str, forward: bool) {
        if text.is_empty() {
            self.find_bar.set_result("");
            return;
        }
        let Some((view, buffer)) = self.active_view_buffer() else { return };
        let case_sensitive = self.find_bar.is_case_sensitive();
        let whole_word = self.find_bar.is_whole_word();
        let regex_mode = self.find_bar.is_regex_mode();
        let cursor_pos = buffer.cursor_position();

        let matches: Vec<(i32, i32)> = if regex_mode || whole_word {
            let full_text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string();
            let pattern = if whole_word {
                format!("\\b{}\\b", regex::escape(text))
            } else {
                text.to_string()
            };
            let re_result = if case_sensitive {
                regex::Regex::new(&pattern)
            } else {
                regex::Regex::new(&format!("(?i){}", pattern))
            };
            match re_result {
                Err(_) => {
                    self.find_bar.set_entry_error(true);
                    self.find_bar.set_result("Invalid regex");
                    return;
                }
                Ok(re) => {
                    self.find_bar.set_entry_error(false);
                    re.find_iter(&full_text)
                        .map(|m| {
                            let char_start = full_text[..m.start()].chars().count() as i32;
                            let char_end = full_text[..m.end()].chars().count() as i32;
                            (char_start, char_end)
                        })
                        .collect()
                }
            }
        } else {
            let flags = if case_sensitive {
                TextSearchFlags::TEXT_ONLY
            } else {
                TextSearchFlags::TEXT_ONLY | TextSearchFlags::CASE_INSENSITIVE
            };
            let mut v = Vec::new();
            let mut it = buffer.start_iter();
            while let Some((s, e)) = it.forward_search(text, flags, None) {
                let advance = e.clone();
                v.push((s.offset(), e.offset()));
                it = advance;
            }
            self.find_bar.set_entry_error(false);
            v
        };

        if matches.is_empty() {
            self.find_bar.set_result("No results");
            return;
        }

        // Pick the next (or previous) match relative to cursor, with wrap-around
        let idx = if forward {
            matches.iter().position(|(s, _)| *s > cursor_pos)
                .unwrap_or(0)
        } else {
            matches.iter().rposition(|(_, e)| *e < cursor_pos)
                .unwrap_or(matches.len() - 1)
        };

        let (start_off, end_off) = matches[idx];
        let start = buffer.iter_at_offset(start_off);
        let end = buffer.iter_at_offset(end_off);
        // Place insertion cursor at start so next forward search skips past current match
        buffer.select_range(&end, &start);
        view.scroll_to_iter(&mut start.clone(), 0.1, false, 0.0, 0.5);
        self.find_bar.set_result(&format!("{} of {}", idx + 1, matches.len()));
    }

    pub fn do_replace_one(&self, find: &str, replace: &str) {
        if find.is_empty() {
            return;
        }
        let Some((_view, buffer)) = self.active_view_buffer() else { return };
        let case_sensitive = self.find_bar.is_case_sensitive();
        if let Some((sel_start, sel_end)) = buffer.selection_bounds() {
            let selected = buffer.text(&sel_start, &sel_end, false).to_string();
            let matches = if case_sensitive {
                selected == find
            } else {
                selected.to_lowercase() == find.to_lowercase()
            };
            if matches {
                let offset = sel_start.offset();
                let mut s = sel_start;
                let mut e = sel_end;
                buffer.begin_user_action();
                buffer.delete(&mut s, &mut e);
                let mut ins = buffer.iter_at_offset(offset);
                buffer.insert(&mut ins, replace);
                buffer.end_user_action();
            }
        }
        self.do_find(find, true);
    }

    pub fn do_replace_all(&self, find: &str, replace: &str) {
        if find.is_empty() {
            return;
        }
        let Some((_view, buffer)) = self.active_view_buffer() else { return };
        let case_sensitive = self.find_bar.is_case_sensitive();
        let whole_word = self.find_bar.is_whole_word();
        let regex_mode = self.find_bar.is_regex_mode();
        let mut count: usize = 0;

        if regex_mode || whole_word {
            let full_text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string();
            let pattern = if whole_word {
                format!("\\b{}\\b", regex::escape(find))
            } else {
                find.to_string()
            };
            let re_result = if case_sensitive {
                regex::Regex::new(&pattern)
            } else {
                regex::Regex::new(&format!("(?i){}", pattern))
            };
            match re_result {
                Err(_) => {
                    self.find_bar.set_entry_error(true);
                    self.find_bar.set_result("Invalid regex");
                    return;
                }
                Ok(re) => {
                    self.find_bar.set_entry_error(false);
                    let new_text = re.replace_all(&full_text, replace);
                    count = re.find_iter(&full_text).count();
                    let mut start = buffer.start_iter();
                    let mut end = buffer.end_iter();
                    buffer.begin_user_action();
                    buffer.delete(&mut start, &mut end);
                    let mut ins = buffer.start_iter();
                    buffer.insert(&mut ins, &new_text);
                    buffer.end_user_action();
                }
            }
        } else {
            let flags = if case_sensitive {
                TextSearchFlags::TEXT_ONLY
            } else {
                TextSearchFlags::TEXT_ONLY | TextSearchFlags::CASE_INSENSITIVE
            };
            self.find_bar.set_entry_error(false);
            buffer.begin_user_action();
            let mut iter = buffer.start_iter();
            loop {
                match iter.forward_search(find, flags, None) {
                    Some((mut start, mut end)) => {
                        let offset = start.offset();
                        buffer.delete(&mut start, &mut end);
                        let mut ins = buffer.iter_at_offset(offset);
                        buffer.insert(&mut ins, replace);
                        iter = buffer.iter_at_offset(offset + replace.chars().count() as i32);
                        count += 1;
                    }
                    None => break,
                }
            }
            buffer.end_user_action();
        }
        self.find_bar.set_result(&format!("Replaced {count}"));
    }

    // ── LSP completions ───────────────────────────────────────────────────────

    /// Called from app_window when a completion response arrives. Shows the
    /// popup on the currently-active tab's view.
    pub fn show_lsp_completions(&self, items: Vec<CompletionItem>) {
        let current = match self.notebook.current_page() {
            Some(p) => p,
            None => return,
        };
        // Collect everything we need from state, then drop the borrow before any
        // GTK widget ops — popup.popup() / show_items can cascade through GTK and
        // fire signals that re-enter Zerkalo callbacks trying borrow_mut on state.
        struct TabInfo {
            view: sourceview5::View,
            buffer: sourceview5::Buffer,
            lsp_popup: crate::ui::lsp_popup::LspPopup,
            popup_visible: bool,
        }
        let tab_info: Option<TabInfo> = {
            let state = self.state.borrow();
            state.tabs.values()
                .find(|tab| self.notebook.page_num(&tab.scroll_window) == Some(current))
                .map(|tab| TabInfo {
                    view: tab.view.clone(),
                    buffer: tab.buffer.clone(),
                    lsp_popup: tab.lsp_popup.clone(),
                    popup_visible: tab.lsp_popup.is_visible(),
                })
        };
        let Some(ti) = tab_info else { return };

        let prefix = lsp_hash_prefix(&ti.buffer);
        let cursor = ti.buffer.iter_at_offset(ti.buffer.cursor_position());
        let loc = ti.view.iter_location(&cursor);
        let (wx, wy_bottom) = ti.view.buffer_to_window_coords(
            TextWindowType::Widget, loc.x(), loc.y() + loc.height());
        let (_, wy_top) = ti.view.buffer_to_window_coords(
            TextWindowType::Widget, loc.x(), loc.y());
        let view_h = ti.view.allocated_height() as i32;
        let above = wy_bottom > view_h / 2;
        let wy = if above { wy_top } else { wy_bottom };

        if !ti.popup_visible {
            let mut all_items: Vec<CompletionItem> = ACADEMIC_SNIPPETS
                .iter()
                .map(|(_, label, desc, body)| CompletionItem {
                    label: label.to_string(),
                    kind: 15,
                    detail: Some(desc.to_string()),
                    insert_text: Some(body.to_string()),
                })
                .collect();
            all_items.extend(items);
            ti.lsp_popup.show_items(all_items, wx, wy, above);
        } else {
            ti.lsp_popup.merge_items(items);
        }
        ti.lsp_popup.apply_filter(&prefix);
    }

    // ── Inline diagnostic marks ───────────────────────────────────────────────

    /// Apply underline squiggles for the given diagnostics. Each entry is
    /// (file, 1-based line, is_error). Call after compile or LSP diagnostics.
    pub fn mark_diagnostics(&self, diagnostics: &[(PathBuf, u32, bool)]) {
        *self.last_diagnostics.borrow_mut() = diagnostics.to_vec();
        // Collect buffer/widget refs while holding borrow, then drop it before GTK ops.
        // GTK buffer ops (apply_tag, create_source_mark) fire synchronous signals that
        // can cascade back into Zerkalo callbacks that try borrow_mut — holding borrow
        // across them causes a BorrowError → SIGABRT.
        let tabs: Vec<(PathBuf, Buffer, Label)> = {
            let state = self.state.borrow();
            state.tabs.iter().map(|(p, t)| (p.clone(), t.buffer.clone(), t.diag_dot.clone())).collect()
        };
        for (path, buffer, diag_dot) in &tabs {
            let (buf_start, buf_end) = buffer.bounds();
            ensure_diag_tags(buffer);
            buffer.remove_tag_by_name("zerkalo-diag-error", &buf_start, &buf_end);
            buffer.remove_tag_by_name("zerkalo-diag-warning", &buf_start, &buf_end);
            buffer.remove_source_marks(&buf_start, &buf_end, Some("zerkalo-error"));
            buffer.remove_source_marks(&buf_start, &buf_end, Some("zerkalo-warning"));
            let has_errors = diagnostics.iter().any(|(f, _, is_err)| f == path && *is_err);
            diag_dot.set_visible(has_errors);
            for (err_file, err_line, is_error) in diagnostics {
                if err_file != path {
                    continue;
                }
                let line_idx = err_line.saturating_sub(1) as i32;
                if let Some(line_start) = buffer.iter_at_line(line_idx) {
                    let mut line_end = line_start.clone();
                    line_end.forward_to_line_end();
                    let tag = if *is_error { "zerkalo-diag-error" } else { "zerkalo-diag-warning" };
                    buffer.apply_tag_by_name(tag, &line_start, &line_end);
                    let category = if *is_error { "zerkalo-error" } else { "zerkalo-warning" };
                    buffer.create_source_mark(None, category, &line_start);
                }
            }
        }
    }

    pub fn clear_diagnostic_marks(&self) {
        self.last_diagnostics.borrow_mut().clear();
        let tabs: Vec<(Buffer, Label)> = {
            let state = self.state.borrow();
            state.tabs.values().map(|t| (t.buffer.clone(), t.diag_dot.clone())).collect()
        };
        for (buffer, diag_dot) in &tabs {
            let (start, end) = buffer.bounds();
            ensure_diag_tags(buffer);
            buffer.remove_tag_by_name("zerkalo-diag-error", &start, &end);
            buffer.remove_tag_by_name("zerkalo-diag-warning", &start, &end);
            buffer.remove_source_marks(&start, &end, Some("zerkalo-error"));
            buffer.remove_source_marks(&start, &end, Some("zerkalo-warning"));
            diag_dot.set_visible(false);
        }
    }

    pub fn is_bib_active(&self) -> bool {
        *self.bib_active.borrow()
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

    pub fn set_on_cursor_moved(&self, f: impl Fn(PathBuf, u32, u32) + 'static) {
        *self.on_cursor_moved.borrow_mut() = Some(Box::new(f));
    }

    pub fn apply_style(&self, style_code: &str, bib_style: &str, bib_title: &str, style_key: &str) {
        let Some(path) = self.get_active_path() else { return };
        let Some(content) = self.get_active_content() else { return };

        let new_content = if crate::styles::has_template_block(&content) {
            // Template document: update heading styles within the TEMPLATE block,
            // then regenerate the title page layout for the new style.
            let with_headings = super::template_dialog::replace_heading_styles_in_template(
                &content, style_key,
            );
            let with_title = super::template_dialog::rebuild_title_page_for_style(
                &with_headings, style_key,
            );
            crate::styles::update_bibliography_only(&with_title, bib_style, bib_title)
        } else {
            crate::styles::apply_to(&content, style_code, bib_style, bib_title)
        };

        if new_content != content {
            // Clone the buffer before dropping the borrow; set_text fires
            // connect_changed which calls borrow_mut — holding the borrow here
            // causes a RefCell double-borrow panic.
            let buffer_opt = {
                let state = self.state.borrow();
                state.tabs.get(&path).map(|tab| tab.buffer.clone())
            };
            if let Some(buffer) = buffer_opt {
                buffer.set_text(&new_content);
                { let sm = *self.simple_mode.borrow(); apply_simple_mode_tag(&buffer, sm); }
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

    pub fn set_session_delta(&self, delta: i32) {
        if delta > 0 {
            self.session_delta_label.set_text(&format!("↑ {delta}"));
            self.session_delta_label.set_visible(true);
        } else {
            self.session_delta_label.set_visible(false);
        }
    }

    pub fn get_active_session_delta(&self) -> i32 {
        let current = match self.notebook.current_page() {
            Some(p) => p,
            None => return 0,
        };
        let state = self.state.borrow();
        for tab in state.tabs.values() {
            if self.notebook.page_num(&tab.scroll_window) == Some(current) {
                let (s, e) = tab.buffer.bounds();
                let text = tab.buffer.text(&s, &e, false);
                let current_words = count_words(&text) as i32;
                return current_words - tab.session_start_words as i32;
            }
        }
        0
    }

    pub fn set_lsp_status(&self, status: &str) {
        let is_dark = adw::StyleManager::default().is_dark();
        let green = if is_dark { "#57e389" } else { "#26a269" };
        let red   = if is_dark { "#ff7b63" } else { "#c01c28" };
        let markup = status
            .replace('●', &format!("<span color=\"{green}\">●</span>"))
            .replace('✗', &format!("<span color=\"{red}\">✗</span>"));
        self.lsp_status_label.set_markup(&markup);
    }

    pub fn set_diag_summary(&self, errors: u32, warnings: u32) {
        let text = match (errors, warnings) {
            (0, 0) => String::new(),
            (e, 0) => format!("{e} error{}", if e == 1 { "" } else { "s" }),
            (0, w) => format!("{w} warning{}", if w == 1 { "" } else { "s" }),
            (e, w) => format!(
                "{e} error{} · {w} warning{}",
                if e == 1 { "" } else { "s" },
                if w == 1 { "" } else { "s" },
            ),
        };
        self.diag_label.set_text(&text);
    }

    pub fn set_spell_enabled(&self, enabled: bool) {
        self.spell_checker.borrow_mut().enabled = enabled;
        if !enabled {
            // Clone buffers out of the borrow before GTK tag ops — remove_tag_by_name
            // can cascade through GtkSourceView signals and re-enter code that tries
            // a conflicting borrow on state, causing a BorrowError panic.
            let buffers: Vec<_> = {
                let state = self.state.borrow();
                state.tabs.values().map(|t| t.buffer.clone()).collect()
            };
            for buffer in &buffers {
                clear_spell_tags(buffer);
            }
        } else {
            self.recheck_all_buffers();
        }
    }

    pub fn set_spell_autocorrect(&self, enabled: bool) {
        self.spell_checker.borrow_mut().autocorrect = enabled;
        set_autocorrect_label(&self.autocorrect_label, enabled);
    }

    pub fn set_on_autocorrect_toggle(&self, f: impl Fn(bool) + 'static) {
        *self.on_autocorrect_toggle.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_gost_toggle(&self, f: impl Fn(bool) + 'static) {
        *self.on_gost_toggle.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_format_bar_toggle(&self, f: impl Fn(bool) + 'static) {
        *self.on_format_bar_toggle.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_format_bar_visible(&self, visible: bool) {
        self.format_bar_container.set_visible(visible);
        set_status_toggle(&self.format_bar_toggle_btn, &self.format_bar_label, "format bar", visible);
    }

    pub fn format_bar_visible(&self) -> bool {
        self.format_bar_container.is_visible()
    }

    pub fn set_on_focus_toggle(&self, f: impl Fn(bool) + 'static) {
        *self.on_focus_toggle.borrow_mut() = Some(Box::new(f));
    }

    #[allow(dead_code)]
    pub fn set_focus_active(&self, active: bool) {
        set_toggle_label(&self.focus_label, "focus", active);
        // aria-pressed for focus button updated in the click handler directly
    }

    pub fn set_on_doc_font(&self, f: impl Fn(String) + 'static) {
        *self.on_doc_font.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_doc_font_size(&self, f: impl Fn(String) + 'static) {
        *self.on_doc_font_size.borrow_mut() = Some(Box::new(f));
    }

    #[allow(dead_code)]
    pub fn set_doc_font_label(&self, name: &str) {
        self.font_bar_label.set_text(name);
    }

    #[allow(dead_code)]
    pub fn set_doc_size_label(&self, size: &str) {
        self.size_bar_label.set_text(size);
    }

    pub fn set_on_version_click(&self, f: impl Fn() + 'static) {
        *self.on_version_click.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_word_count_click(&self, f: impl Fn() + 'static) {
        *self.on_word_count_click.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_spell_languages(&self, langs: Vec<String>) {
        self.spell_checker.borrow_mut().languages = langs;
        self.recheck_all_buffers();
    }

    fn recheck_all_buffers(&self) {
        let sc = self.spell_checker.borrow();
        if !sc.enabled { return; }
        let languages = sc.languages.clone();
        let ignored = sc.ignored().clone();
        drop(sc);

        let state = self.state.borrow();
        for tab in state.tabs.values() {
            let (s, e) = tab.buffer.bounds();
            let text = tab.buffer.text(&s, &e, true).to_string();
            let buffer = tab.buffer.clone();
            let langs = languages.clone();
            let ig = ignored.clone();

            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            std::thread::spawn(move || {
                let words = crate::spellcheck::extract_words(&text);
                let unique: Vec<String> = {
                    let mut seen = HashSet::new();
                    words.iter()
                        .filter(|(_, _, w)| !ig.contains(&w.to_lowercase()) && seen.insert(w.to_lowercase()))
                        .map(|(_, _, w)| w.clone())
                        .collect()
                };
                let unique_refs: Vec<&str> = unique.iter().map(|s| s.as_str()).collect();
                let misspelled = crate::spellcheck::check_words_batch(&unique_refs, &langs);
                let _ = tx.send((words, misspelled));
            });

            glib::timeout_add_local(Duration::from_millis(50), move || {
                match rx.try_recv() {
                    Ok((words, misspelled)) => {
                        apply_spell_tags(&buffer, &words, &misspelled);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                }
            });
        }
    }

    // ── File management ───────────────────────────────────────────────────────

    /// Like `open_file` but forces a buffer refresh if the file is already open.
    pub fn reload_file(&self, path: PathBuf, content: &str) {
        // Clone the buffer out before releasing the borrow — set_text fires
        // connect_changed which re-borrows state, causing a double-borrow panic.
        let existing = {
            let state = self.state.borrow();
            state.tabs.get(&path).map(|tab| (tab.buffer.clone(), tab.scroll_window.clone()))
        };
        if let Some((buffer, scroll)) = existing {
            buffer.set_text(content);
            { let sm = *self.simple_mode.borrow(); apply_simple_mode_tag(&buffer, sm); }
            if let Some(n) = self.notebook.page_num(&scroll) {
                self.notebook.set_current_page(Some(n));
            }
            return;
        }
        self.open_file(path, content);
    }

    /// Replace only the preamble region of an already-open file, preserving the
    /// undo stack for everything below the body marker.
    pub fn splice_preamble(&self, path: PathBuf, full_new_content: &str) {
        const BODY_MARKERS: &[&str] = &["// \u{2500}\u{2500} Document body", "// \u{2500}\u{2500} Chapters"];
        let existing = {
            let state = self.state.borrow();
            state.tabs.get(&path).map(|tab| (tab.buffer.clone(), tab.scroll_window.clone()))
        };
        let Some((buffer, scroll)) = existing else {
            self.open_file(path, full_new_content);
            return;
        };
        if let Some(n) = self.notebook.page_num(&scroll) {
            self.notebook.set_current_page(Some(n));
        }
        let current_text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string();
        let marker = BODY_MARKERS.iter().find(|m| current_text.contains(*m));
        match marker {
            Some(m) => {
                let body_byte = current_text.find(m).unwrap();
                let body_char = current_text[..body_byte].chars().count() as i32;
                let new_preamble = full_new_content.find(m)
                    .map(|pos| &full_new_content[..pos])
                    .unwrap_or(full_new_content);
                let mut preamble_end = buffer.iter_at_offset(body_char);
                let mut preamble_start = buffer.start_iter();
                buffer.begin_user_action();
                buffer.delete(&mut preamble_start, &mut preamble_end);
                let mut ins = buffer.start_iter();
                buffer.insert(&mut ins, new_preamble);
                buffer.end_user_action();
                { let sm = *self.simple_mode.borrow(); apply_simple_mode_tag(&buffer, sm); }
            }
            None => {
                buffer.set_text(full_new_content);
                { let sm = *self.simple_mode.borrow(); apply_simple_mode_tag(&buffer, sm); }
            }
        }
    }

    pub fn open_file(&self, path: PathBuf, content: &str) {
        {
            let state = self.state.borrow();
            if let Some(tab) = state.tabs.get(&path) {
                if let Some(n) = self.notebook.page_num(&tab.scroll_window) {
                    self.notebook.set_current_page(Some(n));
                }
                return;
            }
        }

        let display_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("untitled")
            .to_string();

        let buffer = Buffer::new(None::<&gtk4::TextTagTable>);

        let lang_manager = LanguageManager::default();
        if let Some(path_str) = path.to_str() {
            if let Some(lang) = lang_manager.guess_language(Some(path_str), None) {
                buffer.set_language(Some(&lang));
                buffer.set_highlight_syntax(true);
            }
        }
        let scheme_id = if adw::StyleManager::default().is_dark() {
            "Adwaita-dark"
        } else {
            "Adwaita"
        };
        if let Some(scheme) = StyleSchemeManager::default().scheme(scheme_id) {
            buffer.set_style_scheme(Some(&scheme));
        }

        let migrated;
        let content = if content.contains(
            "#if it.numbering != none [#context counter(heading).display(it.numbering)"
        ) {
            migrated = migrate_template_it_numbering(content);
            migrated.as_str()
        } else {
            content
        };
        buffer.set_text(content);
        apply_comment_highlights(&buffer);
        { let sm = *self.simple_mode.borrow(); apply_simple_mode_tag(&buffer, sm); }

        let view = View::with_buffer(&buffer);
        view.update_property(&[
            gtk4::accessible::Property::Label("Document editor"),
            gtk4::accessible::Property::MultiLine(true),
        ]);
        view.set_show_line_numbers(!*self.simple_mode.borrow());
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
        view.set_highlight_current_line(true);
        let wrap_mode = if *self.word_wrap.borrow() { gtk4::WrapMode::Word } else { gtk4::WrapMode::None };
        view.set_wrap_mode(wrap_mode);
        apply_space_drawer(&view, *self.show_whitespace.borrow());
        set_view_line_spacing(&view, *self.line_spacing.borrow());
        // Comfortable content padding. In simple mode the gutter is hidden so
        // add extra left padding to keep the text away from the window edge.
        let left_margin = if *self.simple_mode.borrow() { 40 } else { 8 };
        view.set_left_margin(left_margin);
        view.set_right_margin(8);

        // ── Image drag-and-drop ───────────────────────────────────────────────
        {
            let drop = DropTarget::new(
                gtk4::gdk::FileList::static_type(),
                gtk4::gdk::DragAction::COPY,
            );
            let on_drop_cb = self.on_image_drop.clone();
            drop.connect_drop(move |_, value, _, _| {
                if let Ok(file_list) = value.get::<gtk4::gdk::FileList>() {
                    for file in file_list.files() {
                        if let Some(p) = file.path() {
                            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                            if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "svg" | "gif" | "webp") {
                                if let Some(f) = on_drop_cb.borrow().as_ref() { f(p); }
                                return true;
                            }
                        }
                    }
                }
                false
            });
            view.add_controller(drop);
        }

        // Ghost-text placeholder — shown when the buffer is empty.
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

        let view_overlay = gtk4::Overlay::new();
        view_overlay.set_child(Some(&view));
        view_overlay.add_overlay(&placeholder_lbl);
        view_overlay.set_hexpand(true);
        view_overlay.set_vexpand(true);

        let ph_lbl_for_buf = placeholder_lbl.clone();
        buffer.connect_changed(move |buf| {
            ph_lbl_for_buf.set_visible(buf.char_count() == 0);
        });

        let scroll = ScrolledWindow::new();
        scroll.set_child(Some(&view_overlay));
        scroll.set_hexpand(true);
        scroll.set_vexpand(true);
        // Horizontal scroll is permanently disabled — all wrapping is done in the
        // text view itself. Kinetic scrolling is disabled to prevent the view from
        // "coasting" past where the user clicked.
        let h_policy = if *self.word_wrap.borrow() {
            gtk4::PolicyType::Never
        } else {
            gtk4::PolicyType::Automatic
        };
        scroll.set_policy(h_policy, gtk4::PolicyType::Automatic);
        scroll.set_kinetic_scrolling(false);

        // ── Tab label ─────────────────────────────────────────────────────────

        let tab_box = GtkBox::new(Orientation::Horizontal, 4);
        tab_box.set_margin_start(2);
        tab_box.set_margin_end(2);
        let name_label = Label::new(Some(&display_name));
        name_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        name_label.set_max_width_chars(24);
        let diag_dot = Label::new(Some("⬤"));
        diag_dot.add_css_class("error");
        diag_dot.set_visible(false);
        let dot_label = Label::new(Some("●"));
        dot_label.add_css_class("modified-dot");
        dot_label.set_visible(false);
        let close_btn = Button::from_icon_name("window-close-symbolic");
        close_btn.add_css_class("flat");
        close_btn.add_css_class("circular");
        close_btn.set_valign(gtk4::Align::Center);
        close_btn.set_tooltip_text(Some("Close tab"));

        tab_box.append(&name_label);
        tab_box.append(&diag_dot);
        tab_box.append(&dot_label);
        tab_box.append(&close_btn);

        let state_for_close = self.state.clone();
        let notebook_for_close = self.notebook.clone();
        let path_for_close = path.clone();
        let scroll_for_close = scroll.clone();
        close_btn.connect_clicked(move |_| {
            if let Some(n) = notebook_for_close.page_num(&scroll_for_close) {
                notebook_for_close.remove_page(Some(n));
            }
            state_for_close.borrow_mut().tabs.remove(&path_for_close);
        });

        // Middle-click anywhere on the tab label also closes the tab
        {
            let nb_mc = self.notebook.clone();
            let sc_mc = scroll.clone();
            let st_mc = self.state.clone();
            let p_mc  = path.clone();
            let mc = gtk4::GestureClick::new();
            mc.set_button(2); // middle button
            mc.connect_pressed(move |_, _, _, _| {
                if let Some(n) = nb_mc.page_num(&sc_mc) {
                    nb_mc.remove_page(Some(n));
                }
                st_mc.borrow_mut().tabs.remove(&p_mc);
            });
            tab_box.add_controller(mc);
        }

        // Right-click context menu on tab: close tab, delete file
        {
            let nb_rc = self.notebook.clone();
            let sc_rc = scroll.clone();
            let st_rc = self.state.clone();
            let path_rc = path.clone();
            let del_cb = self.on_delete_file.clone();
            let filename_rc = display_name.clone();

            let popover = Popover::new();
            popover.set_has_arrow(false);
            let menu_box = GtkBox::new(Orientation::Vertical, 2);
            menu_box.set_margin_top(4);
            menu_box.set_margin_bottom(4);
            menu_box.set_margin_start(4);
            menu_box.set_margin_end(4);

            let close_item = Button::with_label("Close tab");
            close_item.add_css_class("flat");
            let del_item = Button::with_label("Delete file…");
            del_item.add_css_class("flat");
            del_item.add_css_class("destructive-action");
            menu_box.append(&close_item);
            menu_box.append(&del_item);
            popover.set_child(Some(&menu_box));
            popover.set_parent(&tab_box);

            // Close tab
            let nb_ci = nb_rc.clone();
            let sc_ci = sc_rc.clone();
            let st_ci = st_rc.clone();
            let path_ci = path_rc.clone();
            let pop_ci = popover.clone();
            close_item.connect_clicked(move |_| {
                pop_ci.popdown();
                if let Some(n) = nb_ci.page_num(&sc_ci) {
                    nb_ci.remove_page(Some(n));
                }
                st_ci.borrow_mut().tabs.remove(&path_ci);
            });

            // Delete file
            let nb_di = nb_rc.clone();
            let sc_di = sc_rc.clone();
            let st_di = st_rc.clone();
            let path_di = path_rc.clone();
            let name_di = filename_rc.clone();
            let pop_di = popover.clone();
            del_item.connect_clicked(move |_| {
                pop_di.popdown();
                let alert = AlertDialog::builder()
                    .modal(true)
                    .message("Delete this file?")
                    .detail(&format!("'{}' will be permanently deleted.", name_di))
                    .buttons(["Cancel", "Delete"])
                    .cancel_button(0)
                    .default_button(0)
                    .build();
                let path_confirm = path_di.clone();
                let nb_confirm = nb_di.clone();
                let sc_confirm = sc_di.clone();
                let st_confirm = st_di.clone();
                let cb_confirm = del_cb.clone();
                alert.choose(
                    None::<&gtk4::Window>,
                    None::<&gtk4::gio::Cancellable>,
                    move |result| {
                        if result == Ok(1) {
                            let _ = std::fs::remove_file(&path_confirm);
                            if let Some(n) = nb_confirm.page_num(&sc_confirm) {
                                nb_confirm.remove_page(Some(n));
                            }
                            st_confirm.borrow_mut().tabs.remove(&path_confirm);
                            if let Some(f) = cb_confirm.borrow().as_ref() {
                                f(path_confirm.clone());
                            }
                        }
                    },
                );
            });

            let rc_for_gesture = GestureClick::new();
            rc_for_gesture.set_button(3);
            let pop_for_rc = popover.clone();
            rc_for_gesture.connect_pressed(move |_, _, x, y| {
                pop_for_rc.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                pop_for_rc.popup();
            });
            tab_box.add_controller(rc_for_gesture);
        }

        // ── Modified flag + word count ────────────────────────────────────────

        let state_for_change = self.state.clone();
        let path_for_change = path.clone();
        let dot_for_change = dot_label.clone();
        let tab_box_for_change = tab_box.clone();
        let tab_name_for_change = display_name.clone();
        let on_change_cb = self.on_change.clone();
        let on_modified_cb = self.on_modified_changed.clone();
        let on_file_dirty_cb = self.on_file_dirty.clone();
        let wc_for_change = self.word_count_label.clone();
        let goal_for_change = self.goal_bar.clone();
        let goal_val_for_change = self.word_count_goal.clone();
        let last_wc_for_change = self.last_wc_text.clone();
        let project_root_for_wc = self.project_root.clone();
        let session_start_for_change: Rc<std::cell::Cell<u32>> = Rc::new(std::cell::Cell::new(count_words(content)));
        // SourceId-based debounce timers. Each keystroke cancels the previous
        // pending timer before scheduling a new one, so timers never accumulate
        // in the event loop regardless of typing speed.
        let wc_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        let comment_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        let proj_wc_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        buffer.connect_changed(move |buf| {
            let newly_modified = {
                let mut state = state_for_change.borrow_mut();
                if let Some(tab) = state.tabs.get_mut(&path_for_change) {
                    if !tab.modified { tab.modified = true; true } else { false }
                } else { false }
            };
            if newly_modified {
                // GTK widget ops must happen after borrow_mut is released — doing
                // them inside the borrow can cause reentrant signal dispatch that
                // tries to borrow state again, triggering a BorrowMutError panic.
                dot_for_change.set_visible(true);
                tab_box_for_change.update_property(&[gtk4::accessible::Property::Label(
                    &format!("{} — unsaved", tab_name_for_change)
                )]);
                if let Some(f) = on_modified_cb.borrow().as_ref() { f(true); }
                if let Some(f) = on_file_dirty_cb.borrow().as_ref() { f(path_for_change.clone(), true); }
            }
            if let Some(f) = on_change_cb.borrow().as_ref() { f(); }

            // ── Debounced word count (300 ms) ─────────────────────────────────
            if let Some(id) = wc_timer.borrow_mut().take() { id.remove(); }
            {
                let wc2 = wc_for_change.clone();
                let goal2 = goal_for_change.clone();
                let goal_val2 = goal_val_for_change.clone();
                let last_wc2 = last_wc_for_change.clone();
                let ss2 = session_start_for_change.clone();
                let buf2 = buf.clone();
                let t = wc_timer.clone();
                *wc_timer.borrow_mut() = Some(glib::timeout_add_local_once(
                    Duration::from_millis(300),
                    move || {
                        *t.borrow_mut() = None;
                        let (s, e) = buf2.bounds();
                        let text = buf2.text(&s, &e, false);
                        let goal = *goal_val2.borrow();
                        if goal > 0 { update_goal_bar(&goal2, &text, goal); }
                        let wc_str = wc_str_with_delta(&text, ss2.get());
                        *last_wc2.borrow_mut() = wc_str.clone();
                        wc2.set_text(&wc_str);
                    },
                ));
            }

            // ── Debounced project word count tooltip (5 s) ────────────────────
            if let Some(id) = proj_wc_timer.borrow_mut().take() { id.remove(); }
            {
                let wc_lbl_proj = wc_for_change.clone();
                let root_proj = project_root_for_wc.clone();
                let t = proj_wc_timer.clone();
                *proj_wc_timer.borrow_mut() = Some(glib::timeout_add_local_once(
                    Duration::from_millis(5000),
                    move || {
                        *t.borrow_mut() = None;
                        if let Some(root) = root_proj.borrow().as_ref() {
                            let total = count_project_words(root);
                            wc_lbl_proj.set_tooltip_text(Some(&format!("Project total: {total} words")));
                        }
                    },
                ));
            }

            // ── Debounced comment highlights (500 ms) ─────────────────────────
            if let Some(id) = comment_timer.borrow_mut().take() { id.remove(); }
            {
                let buf_comment = buf.clone();
                let t = comment_timer.clone();
                *comment_timer.borrow_mut() = Some(glib::timeout_add_local_once(
                    Duration::from_millis(500),
                    move || {
                        *t.borrow_mut() = None;
                        apply_comment_highlights(&buf_comment);
                    },
                ));
            }
        });

        // ── Cursor position tracking + heading detection ──────────────────────

        let cursor_lbl = self.cursor_label.clone();
        let section_wc_lbl = self.section_wc_label.clone();
        let last_section_line: Rc<std::cell::Cell<i32>> = Rc::new(std::cell::Cell::new(-1));
        let wc_lbl_for_sel = self.word_count_label.clone();
        let last_wc_for_mark = self.last_wc_text.clone();
        // Extra clones for the selection_bound handler below.
        let wc_lbl_for_sel_bound = wc_lbl_for_sel.clone();
        let last_wc_for_sel_bound = last_wc_for_mark.clone();
        let breadcrumb_lbl = self.breadcrumb_label.clone();
        let lsp_lbl_for_pkg = self.lsp_status_label.clone();
        let on_heading_cb = self.on_cursor_heading.clone();
        let on_moved_cb = self.on_cursor_moved.clone();
        let cursor_moved_gen: Rc<std::cell::Cell<u64>> = Rc::new(std::cell::Cell::new(0));
        let path_for_heading = path.clone();
        let path_for_moved = path.clone();
        let last_heading_line: Rc<RefCell<u32>> = Rc::new(RefCell::new(u32::MAX));
        let typewriter_for_mark = self.typewriter_scroll.clone();
        let view_for_typewriter = view.clone();
        let scroll_for_typewriter = scroll.clone();
        let view_for_scroll_margin = view.clone();
        // Track last line the typewriter scroll recentered on, so we only fire
        // when the cursor crosses a line boundary (not every column move).
        let last_tw_line: Rc<std::cell::Cell<i32>> = Rc::new(std::cell::Cell::new(-1));
        // Only do typewriter scroll when typing, not on mouse click.
        // connect_changed fires before connect_mark_set on keyboard input.
        let typing_flag: Rc<std::cell::Cell<bool>> = Rc::new(std::cell::Cell::new(false));
        let typing_flag_set = typing_flag.clone();
        buffer.connect_changed(move |_| {
            typing_flag_set.set(true);
        });
        buffer.connect_mark_set(move |buf, _iter, mark| {
            if mark.name().as_deref() == Some("insert") {
                let cursor = buf.iter_at_mark(mark);
                let line = cursor.line() + 1;
                let col = cursor.line_offset() + 1;
                cursor_lbl.set_text(&format!("L{line}:C{col}"));
                cursor_lbl.set_tooltip_text(Some(&format!("Line {line}, Column {col}")));

                // Section word count — only recompute when the line changes
                let cur_line = cursor.line();
                if cur_line != last_section_line.get() {
                    last_section_line.set(cur_line);
                    if let Some(wc) = section_word_count_for_line(buf, cur_line) {
                        section_wc_lbl.set_text(&format!("§ {wc}"));
                        section_wc_lbl.set_tooltip_text(Some("Words in this section"));
                    } else {
                        section_wc_lbl.set_text("");
                    }
                }

                // Selection word/sentence stats — use cached wc to avoid reading entire buffer
                if let Some((sel_s, sel_e)) = buf.selection_bounds() {
                    let sel_text = buf.text(&sel_s, &sel_e, false).to_string();
                    let word_count = sel_text.split_whitespace().count();
                    let sentence_count = sel_text
                        .split(|c: char| matches!(c, '.' | '!' | '?'))
                        .filter(|s| !s.trim().is_empty())
                        .count();
                    wc_lbl_for_sel.set_text(&format!(
                        "{word_count} words, {sentence_count} sentences selected"
                    ));
                } else {
                    // Restore cached word count — no full buffer read needed
                    let cached = last_wc_for_mark.borrow().clone();
                    if !cached.is_empty() {
                        wc_lbl_for_sel.set_text(&cached);
                    }
                }

                // Read typing flag before any scroll decisions. connect_changed fires
                // before connect_mark_set on keyboard input, so was_typing is true for
                // keystrokes and false for mouse clicks.
                let was_typing = typing_flag.get();
                typing_flag.set(false);

                // Scroll margin: keep the cursor at least ~5 lines from the viewport
                // edges while typing (within_margin=0.15). Only queued on keyboard
                // input — mouse press must NOT queue this or the idle fires mid-drag
                // and breaks selection.
                //
                // Inside the idle we re-check two conditions before scrolling:
                //  1. No selection active (drag started while idle was pending).
                //  2. Cursor is within ±1 viewport height of the visible area.
                //     If it's further away the user intentionally scrolled; snapping
                //     back would be disorienting. ±1vh covers the normal case of
                //     typing one or two lines past the edge.
                if was_typing && !buf.has_selection() {
                    let vs = view_for_scroll_margin.clone();
                    let insert_mark = buf.get_insert();
                    let buf_s = buf.clone();
                    glib::idle_add_local_once(move || {
                        if buf_s.has_selection() { return; }
                        let cursor = buf_s.iter_at_mark(&insert_mark);
                        let loc = vs.iter_location(&cursor);
                        let (_, wy) = vs.buffer_to_window_coords(
                            TextWindowType::Widget, loc.x(), loc.y(),
                        );
                        let view_h = vs.allocated_height();
                        if wy > -view_h && wy < 2 * view_h {
                            vs.scroll_to_mark(&insert_mark, 0.15, false, 0.0, 0.5);
                        }
                    });
                }

                // Typewriter scroll: only recenter when typing (not on mouse click).
                if *typewriter_for_mark.borrow()
                    && was_typing
                    && !buf.has_selection()
                    && cursor.line() != last_tw_line.get()
                {
                    last_tw_line.set(cursor.line());
                    let mut c = cursor.clone();
                    let vt = view_for_typewriter.clone();
                    let sc_tw = scroll_for_typewriter.clone();
                    glib::idle_add_local_once(move || {
                        // Preserve horizontal scroll — scroll_to_iter with xalign=0.0 would
                        // snap the view left, hiding text behind the left margin/line numbers.
                        let h = sc_tw.hadjustment().value();
                        vt.scroll_to_iter(&mut c, 0.0, true, 0.0, 0.45);
                        sc_tw.hadjustment().set_value(h);
                    });
                }

                // #import "@preview/pkg:ver" tooltip
                {
                    let line_start = buf.iter_at_line(cursor.line()).unwrap_or_else(|| buf.start_iter());
                    let line_end = {
                        let mut e = line_start.clone();
                        if !e.ends_line() { e.forward_to_line_end(); }
                        e
                    };
                    let line_text = buf.text(&line_start, &line_end, false).to_string();
                    let trimmed = line_text.trim();
                    if let Some(rest) = trimmed.strip_prefix("#import \"@preview/") {
                        let pkg_name: String = rest.chars()
                            .take_while(|c| *c != ':' && *c != '"')
                            .collect();
                        // Strip version suffix (e.g. "codly" from "codly:1.0.0")
                        let base_name = pkg_name.split(':').next().unwrap_or(&pkg_name);
                        if let Some((_, desc)) = IMPORT_PACKAGE_TOOLTIPS.iter()
                            .find(|(n, _)| *n == base_name)
                        {
                            let lbl = lsp_lbl_for_pkg.clone();
                            let desc_s = desc.to_string();
                            let pkg_s = base_name.to_string();
                            lbl.set_text(&format!("{pkg_s}: {desc_s}"));
                            glib::timeout_add_local_once(
                                std::time::Duration::from_secs(3),
                                move || { lbl.set_text(""); },
                            );
                        }
                    }
                }

                // Update breadcrumb heading path
                let heading_path = build_heading_path(buf, cursor.line());
                breadcrumb_lbl.set_text(&heading_path);

                // Scan backward for a heading; only scroll preview on keyboard nav (not mouse click).
                if was_typing {
                    if let Some(cb) = on_heading_cb.borrow().as_ref() {
                        let heading_line = find_heading_line_for(buf, cursor.line());
                        if heading_line != *last_heading_line.borrow() {
                            *last_heading_line.borrow_mut() = heading_line;
                            if heading_line != u32::MAX {
                                cb(path_for_heading.clone(), heading_line);
                            }
                        }
                    }
                }

                // Debounced reverse sync: notify app_window of cursor position 300ms after it
                // settles. Only fire on keyboard movement (was_typing), not mouse clicks —
                // otherwise a click in the editor jumps the preview to match the clicked line.
                // Uses a generation counter rather than SourceId::remove() — glib 0.18 panics
                // when remove() is called on a source that timeout_add_local_once already removed.
                if was_typing {
                    let line = cursor.line() as u32;
                    let total = buf.line_count() as u32;
                    let gen = cursor_moved_gen.get().wrapping_add(1);
                    cursor_moved_gen.set(gen);
                    let cb = on_moved_cb.clone();
                    let path_m = path_for_moved.clone();
                    let gen_rc = cursor_moved_gen.clone();
                    glib::timeout_add_local_once(
                        std::time::Duration::from_millis(300),
                        move || {
                            if gen_rc.get() == gen {
                                if let Some(f) = cb.borrow().as_ref() {
                                    f(path_m.clone(), line, total);
                                }
                            }
                        },
                    );
                }
            }
        });

        // When the user clicks to deselect, GTK moves the `insert` mark first and
        // `selection_bound` second. The `insert` handler above fires while
        // `selection_bound` is still at the old anchor, making
        // `selection_bounds()` return Some — so it prints "N selected" even
        // though nothing is selected. This second handler fires when
        // `selection_bound` arrives and clears the ghost label.
        buffer.connect_mark_set(move |buf, _iter, mark| {
            if mark.name().as_deref() != Some("selection_bound") { return; }
            if !buf.has_selection() {
                let cached = last_wc_for_sel_bound.borrow().clone();
                if !cached.is_empty() {
                    wc_lbl_for_sel_bound.set_text(&cached);
                }
            }
        });

        // ── Undo / Redo sensitivity ───────────────────────────────────────────
        {
            let ub = self.undo_btn.clone();
            buffer.connect_can_undo_notify(move |buf| {
                ub.set_sensitive(buf.can_undo());
            });
            let rb = self.redo_btn.clone();
            buffer.connect_can_redo_notify(move |buf| {
                rb.set_sensitive(buf.can_redo());
            });
        }

        // ── @-citation autocomplete ───────────────────────────────────────────

        let bib_popup = BibPopup::new(&view, self.bib_entries.clone());
        let ac_mark: Rc<RefCell<Option<gtk4::TextMark>>> = Rc::new(RefCell::new(None));
        let completing: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let bib_active_for_open = self.bib_active.clone();

        let buf_complete = buffer.clone();
        let view_complete = view.clone();
        let mark_complete = ac_mark.clone();
        let completing_complete = completing.clone();
        let popup_complete = bib_popup.clone();
        bib_popup.set_on_complete(move |key| {
            *completing_complete.borrow_mut() = true;
            let mark_opt = mark_complete.borrow().clone();
            if let Some(ref m) = mark_opt {
                let mut start = buf_complete.iter_at_mark(m);
                let mut end = buf_complete.iter_at_offset(buf_complete.cursor_position());
                buf_complete.begin_user_action();
                buf_complete.delete(&mut start, &mut end);
                buf_complete.insert_at_cursor(&format!("@{key}"));
                buf_complete.end_user_action();
                buf_complete.delete_mark(m);
            }
            *mark_complete.borrow_mut() = None;
            popup_complete.hide();
            view_complete.grab_focus();
            *completing_complete.borrow_mut() = false;
        });

        let view_ac = view.clone();
        let popup_ac = bib_popup.clone();
        let mark_ac = ac_mark.clone();
        let completing_ac = completing.clone();
        let bib_active_ac = bib_active_for_open.clone();
        buffer.connect_changed(move |buf| {
            if *completing_ac.borrow() {
                return;
            }
            let cursor_pos = buf.cursor_position();
            let cursor_iter = buf.iter_at_offset(cursor_pos);
            let mut temp = cursor_iter.clone();
            let mut found_at = false;
            let mut at_iter = cursor_iter.clone();
            loop {
                if !temp.backward_char() {
                    break;
                }
                let ch = temp.char();
                if ch == '@' {
                    found_at = true;
                    at_iter = temp.clone();
                    break;
                }
                if !(ch.is_alphanumeric() || ch == '-' || ch == '_' || ch == ':') {
                    break;
                }
            }
            if !found_at {
                *bib_active_ac.borrow_mut() = false;
                dismiss_popup(buf, &popup_ac, &mark_ac);
                return;
            }
            let prev_is_word = {
                let mut prev = at_iter.clone();
                if prev.backward_char() {
                    let ch = prev.char();
                    ch.is_alphanumeric() || ch == '_'
                } else {
                    false
                }
            };
            if prev_is_word {
                *bib_active_ac.borrow_mut() = false;
                dismiss_popup(buf, &popup_ac, &mark_ac);
                return;
            }
            let query = buf.text(&at_iter, &cursor_iter, false);
            let query = query.trim_start_matches('@');
            {
                let mut mark_ref = mark_ac.borrow_mut();
                match mark_ref.as_ref() {
                    Some(m) => buf.move_mark(m, &at_iter),
                    None => *mark_ref = Some(buf.create_mark(None::<&str>, &at_iter, true)),
                }
            }
            // Position popup below cursor when in upper half of view,
            // above cursor when in lower half — so it never lands on the cursor line.
            let loc = view_ac.iter_location(&cursor_iter);
            let (wx, wy_bottom) = view_ac.buffer_to_window_coords(
                TextWindowType::Widget, loc.x(), loc.y() + loc.height());
            let (_, wy_top) = view_ac.buffer_to_window_coords(
                TextWindowType::Widget, loc.x(), loc.y());
            let view_h = view_ac.allocated_height() as i32;
            // above=true: popup uses PositionType::Top, its bottom lands at wy_top (cursor top)
            // above=false: popup uses PositionType::Bottom, its top lands at wy_bottom (cursor bottom)
            let above = wy_bottom > view_h / 2;
            let wy = if above { wy_top } else { wy_bottom };
            popup_ac.show_filtered(query, wx, wy, above);
            *bib_active_ac.borrow_mut() = popup_ac.is_visible();
        });

        // ── #-function LSP autocomplete ───────────────────────────────────────

        let lsp_popup = LspPopup::new(&view);
        let lsp_mark: Rc<RefCell<Option<gtk4::TextMark>>> = Rc::new(RefCell::new(None));
        let lsp_completing: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let lsp_comp_gen: Rc<RefCell<u64>> = Rc::new(RefCell::new(0));

        // LSP on_complete: replace #prefix with the chosen insertion text
        {
            let buf2 = buffer.clone();
            let view2 = view.clone();
            let mark2 = lsp_mark.clone();
            let comp2 = lsp_completing.clone();
            let popup2 = lsp_popup.clone();
            lsp_popup.set_on_complete(move |item| {
                *comp2.borrow_mut() = true;
                let mark_opt = mark2.borrow().clone();
                if let Some(ref m) = mark_opt {
                    let mut start = buf2.iter_at_mark(m); // position of '#'
                    let mut end = buf2.iter_at_offset(buf2.cursor_position());
                    let insert_text = item
                        .insert_text
                        .as_deref()
                        .unwrap_or(&item.label);
                    let insert_text = strip_snippets(insert_text);
                    let final_text = if insert_text.starts_with('#') {
                        insert_text
                    } else {
                        format!("#{insert_text}")
                    };
                    buf2.begin_user_action();
                    buf2.delete(&mut start, &mut end);
                    buf2.insert_at_cursor(&final_text);
                    buf2.end_user_action();
                    buf2.delete_mark(m);
                }
                *mark2.borrow_mut() = None;
                popup2.hide();
                view2.grab_focus();
                *comp2.borrow_mut() = false;
            });
        }

        // Detect #word context and fire on_completion_needed
        {
            let lsp_mark3 = lsp_mark.clone();
            let lsp_popup3 = lsp_popup.clone();
            let lsp_completing3 = lsp_completing.clone();
            let lsp_gen3 = lsp_comp_gen.clone();
            let on_comp_cb = self.on_completion_needed.clone();
            let path_for_lsp = path.clone();
            let view_lsp = view.clone();
            buffer.connect_changed(move |buf| {
                if *lsp_completing3.borrow() {
                    return;
                }
                let cursor_pos = buf.cursor_position();
                let cursor_iter = buf.iter_at_offset(cursor_pos);
                let mut temp = cursor_iter.clone();
                let mut found_hash = false;
                let mut hash_iter = cursor_iter.clone();

                loop {
                    if !temp.backward_char() {
                        break;
                    }
                    let ch = temp.char();
                    if ch == '#' {
                        found_hash = true;
                        hash_iter = temp.clone();
                        break;
                    }
                    if !(ch.is_alphanumeric() || ch == '_' || ch == '-') {
                        break;
                    }
                }

                if found_hash {
                    // Track the '#' position
                    {
                        let mut mark_ref = lsp_mark3.borrow_mut();
                        match mark_ref.as_ref() {
                            Some(m) => buf.move_mark(m, &hash_iter),
                            None => {
                                *mark_ref =
                                    Some(buf.create_mark(None::<&str>, &hash_iter, true))
                            }
                        }
                    }

                    // Show built-in snippets immediately without waiting for LSP
                    if !lsp_popup3.is_visible() {
                        let loc = view_lsp.iter_location(&cursor_iter);
                        let (wx, wy_bottom) = view_lsp.buffer_to_window_coords(
                            TextWindowType::Widget, loc.x(), loc.y() + loc.height());
                        let (_, wy_top) = view_lsp.buffer_to_window_coords(
                            TextWindowType::Widget, loc.x(), loc.y());
                        let view_h = view_lsp.allocated_height() as i32;
                        let above = wy_bottom > view_h / 2;
                        let wy = if above { wy_top } else { wy_bottom };
                        let snippets: Vec<CompletionItem> = ACADEMIC_SNIPPETS
                            .iter()
                            .map(|(_, label, desc, body)| CompletionItem {
                                label: label.to_string(),
                                kind: 15,
                                detail: Some(desc.to_string()),
                                insert_text: Some(body.to_string()),
                            })
                            .collect();
                        lsp_popup3.show_items(snippets, wx, wy, above);
                        lsp_popup3.apply_filter(&lsp_hash_prefix(buf));
                    }

                    let line = cursor_iter.line() as u32 + 1;
                    let col = cursor_iter.line_offset() as u32 + 1;

                    *lsp_gen3.borrow_mut() += 1;
                    let my_gen = *lsp_gen3.borrow();
                    let gen4 = lsp_gen3.clone();
                    let ocb = on_comp_cb.clone();
                    let p = path_for_lsp.clone();

                    glib::timeout_add_local(Duration::from_millis(150), move || {
                        if *gen4.borrow() == my_gen {
                            if let Some(f) = ocb.borrow().as_ref() {
                                f(p.clone(), line, col);
                            }
                        }
                        glib::ControlFlow::Break
                    });
                } else {
                    // No longer in # context — clear mark and hide popup
                    if let Some(m) = lsp_mark3.borrow_mut().take() {
                        buf.delete_mark(&m);
                    }
                    lsp_popup3.hide();
                }
            });
        }

        // ── Key controller ────────────────────────────────────────────────────

        let bib_popup_key = bib_popup.clone();
        let lsp_popup_key = lsp_popup.clone();
        let buf_key = buffer.clone();
        let mark_key = ac_mark.clone();
        let lsp_mark_key = lsp_mark.clone();
        let completing_key = completing.clone();
        let lsp_completing_key = lsp_completing.clone();
        let view_key = view.clone();
        let bib_active_key = bib_active_for_open.clone();

        let key_ctrl = EventControllerKey::new();
        key_ctrl.set_propagation_phase(PropagationPhase::Capture);
        key_ctrl.connect_key_pressed(move |_, key, _, _mods| {
            use gtk4::gdk::Key;

            // LSP popup takes priority
            if lsp_popup_key.is_visible() {
                return match key {
                    Key::Escape => {
                        let mark_opt = lsp_mark_key.borrow_mut().take();
                        if let Some(m) = mark_opt {
                            let mut start = buf_key.iter_at_mark(&m);
                            let mut end = buf_key.iter_at_offset(buf_key.cursor_position());
                            buf_key.begin_user_action();
                            buf_key.delete(&mut start, &mut end);
                            buf_key.end_user_action();
                            buf_key.delete_mark(&m);
                        }
                        lsp_popup_key.hide();
                        glib::Propagation::Stop
                    }
                    Key::Tab => {
                        let item = lsp_popup_key
                            .selected_item()
                            .or_else(|| lsp_popup_key.first_item());
                        if let Some(i) = item {
                            do_lsp_complete(
                                &buf_key,
                                &lsp_mark_key,
                                &lsp_completing_key,
                                &lsp_popup_key,
                                &view_key,
                                i,
                            );
                        }
                        glib::Propagation::Stop
                    }
                    Key::Return => {
                        if let Some(i) = lsp_popup_key.selected_item() {
                            do_lsp_complete(
                                &buf_key,
                                &lsp_mark_key,
                                &lsp_completing_key,
                                &lsp_popup_key,
                                &view_key,
                                i,
                            );
                            glib::Propagation::Stop
                        } else {
                            glib::Propagation::Proceed
                        }
                    }
                    Key::Down => {
                        lsp_popup_key.move_selection(1);
                        glib::Propagation::Stop
                    }
                    Key::Up => {
                        lsp_popup_key.move_selection(-1);
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                };
            }

            // Bib popup
            if !bib_popup_key.is_visible() {
                return glib::Propagation::Proceed;
            }
            match key {
                Key::Escape => {
                    *bib_active_key.borrow_mut() = false;
                    dismiss_popup_only(&bib_popup_key, &buf_key, &mark_key);
                    glib::Propagation::Stop
                }
                Key::Tab => {
                    let chosen = bib_popup_key
                        .selected_key()
                        .or_else(|| bib_popup_key.first_filtered_key());
                    if let Some(k) = chosen {
                        *bib_active_key.borrow_mut() = false;
                        do_bib_complete(
                            &buf_key, &mark_key, &completing_key, &bib_popup_key, &view_key, &k,
                        );
                    }
                    glib::Propagation::Stop
                }
                Key::Return => {
                    if let Some(k) = bib_popup_key.selected_key() {
                        *bib_active_key.borrow_mut() = false;
                        do_bib_complete(
                            &buf_key, &mark_key, &completing_key, &bib_popup_key, &view_key, &k,
                        );
                        glib::Propagation::Stop
                    } else {
                        glib::Propagation::Proceed
                    }
                }
                Key::Down => {
                    bib_popup_key.move_selection(1);
                    glib::Propagation::Stop
                }
                Key::Up => {
                    bib_popup_key.move_selection(-1);
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        view.add_controller(key_ctrl);

        // ── Auto-pair brackets and quotes ─────────────────────────────────────
        let last_was_autopair: Rc<std::cell::Cell<bool>> = Rc::new(std::cell::Cell::new(false));
        {
            let buf_pair = buffer.clone();
            let pair_ctrl = EventControllerKey::new();
            pair_ctrl.set_propagation_phase(PropagationPhase::Capture);
            let last_ap = last_was_autopair.clone();
            pair_ctrl.connect_key_pressed(move |_, key, _, mods| {
                use gtk4::gdk::Key;
                // Don't interfere when modifier keys are held (shortcuts)
                if mods.intersects(
                    gtk4::gdk::ModifierType::CONTROL_MASK | gtk4::gdk::ModifierType::ALT_MASK,
                ) {
                    last_ap.set(false);
                    return glib::Propagation::Proceed;
                }
                // Don't auto-pair when there is a selection
                if buf_pair.has_selection() {
                    last_ap.set(false);
                    return glib::Propagation::Proceed;
                }

                // Skip-forward if the closing char is already there from a prior autopair
                let skip_char: Option<char> = match key {
                    Key::parenright   => Some(')'),
                    Key::bracketright => Some(']'),
                    Key::braceright   => Some('}'),
                    // Only skip " when we know it was auto-inserted
                    Key::quotedbl if last_ap.get() => Some('"'),
                    _ => None,
                };
                if let Some(expected) = skip_char {
                    let pos = buf_pair.cursor_position();
                    let next = buf_pair.iter_at_offset(pos);
                    if next.char() == expected {
                        let ahead = buf_pair.iter_at_offset(pos + 1);
                        buf_pair.place_cursor(&ahead);
                        last_ap.set(false);
                        return glib::Propagation::Stop;
                    }
                }

                let pair = match key {
                    Key::parenleft      => Some(("(", ")")),
                    Key::bracketleft    => Some(("[", "]")),
                    Key::braceleft      => Some(("{", "}")),
                    Key::quotedbl       => Some(("\"", "\"")),
                    Key::dollar         => Some(("$", "$")),
                    _ => None,
                };
                if let Some((open, close)) = pair {
                    buf_pair.begin_user_action();
                    buf_pair.insert_at_cursor(open);
                    buf_pair.insert_at_cursor(close);
                    // Move cursor back one character to sit between the pair
                    let pos = buf_pair.cursor_position();
                    let iter = buf_pair.iter_at_offset(pos - 1);
                    buf_pair.place_cursor(&iter);
                    buf_pair.end_user_action();
                    last_ap.set(true);
                    return glib::Propagation::Stop;
                }
                last_ap.set(false);
                glib::Propagation::Proceed
            });
            view.add_controller(pair_ctrl);
        }

        // ── Comment toggle (Ctrl+/) ───────────────────────────────────────────
        {
            let buf_cmt = buffer.clone();
            let cmt_ctrl = EventControllerKey::new();
            cmt_ctrl.set_propagation_phase(PropagationPhase::Capture);
            cmt_ctrl.connect_key_pressed(move |_, key, _, mods| {
                use gtk4::gdk::Key;
                let ctrl = mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
                if !ctrl || key != Key::slash {
                    return glib::Propagation::Proceed;
                }

                let (first_line, last_line) = if let Some((s, e)) = buf_cmt.selection_bounds() {
                    let end_line = if e.line_offset() == 0 && e.line() > s.line() {
                        e.line() - 1
                    } else {
                        e.line()
                    };
                    (s.line(), end_line)
                } else {
                    let line = buf_cmt.iter_at_offset(buf_cmt.cursor_position()).line();
                    (line, line)
                };

                // Determine whether all non-empty lines start with "//"
                let all_commented = (first_line..=last_line).all(|ln| {
                    if let Some(it) = buf_cmt.iter_at_line(ln) {
                        let mut end = it.clone();
                        end.forward_to_line_end();
                        let line_text = buf_cmt.text(&it, &end, false).to_string();
                        line_text.trim_start().is_empty() || line_text.trim_start().starts_with("//")
                    } else {
                        true
                    }
                });

                buf_cmt.begin_user_action();
                for ln in (first_line..=last_line).rev() {
                    let Some(line_start) = buf_cmt.iter_at_line(ln) else { continue };
                    let mut line_end = line_start.clone();
                    line_end.forward_to_line_end();
                    let line_text = buf_cmt.text(&line_start, &line_end, false).to_string();
                    if line_text.trim_start().is_empty() { continue; }

                    if all_commented {
                        // Remove "// " or "//" prefix
                        let stripped = line_text.trim_start();
                        let indent_len = (line_text.len() - stripped.len()) as i32;
                        if let Some(mut del_start) = buf_cmt.iter_at_line_offset(ln, indent_len) {
                            let remove = if stripped.starts_with("// ") { 3 } else { 2 };
                            let mut del_end = del_start.clone();
                            del_end.forward_chars(remove);
                            buf_cmt.delete(&mut del_start, &mut del_end);
                        }
                    } else {
                        // Insert "// " at indent level
                        let stripped = line_text.trim_start();
                        let indent_len = (line_text.len() - stripped.len()) as i32;
                        if let Some(mut ins) = buf_cmt.iter_at_line_offset(ln, indent_len) {
                            buf_cmt.insert(&mut ins, "// ");
                        }
                    }
                }
                buf_cmt.end_user_action();
                glib::Propagation::Stop
            });
            view.add_controller(cmt_ctrl);
        }

        // ── Bold (Ctrl+B) / Italic (Ctrl+I) ─────────────────────────────────
        {
            let buf_bi = buffer.clone();
            let bi_ctrl = EventControllerKey::new();
            bi_ctrl.set_propagation_phase(PropagationPhase::Capture);
            bi_ctrl.connect_key_pressed(move |_, key, _, mods| {
                use gtk4::gdk::Key;
                let ctrl = mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
                let shift = mods.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
                if !ctrl || shift { return glib::Propagation::Proceed; }
                let marker = match key {
                    Key::b => "*",
                    Key::i => "_",
                    _ => return glib::Propagation::Proceed,
                };
                let mlen = marker.len() as i32;
                if let Some((sel_s, sel_e)) = buf_bi.selection_bounds() {
                    let start_off = sel_s.offset();
                    let end_off = sel_e.offset();
                    let text = buf_bi.text(&sel_s, &sel_e, false).to_string();
                    buf_bi.begin_user_action();
                    if text.starts_with(marker) && text.ends_with(marker)
                        && text.len() > 2 * marker.len()
                    {
                        let inner = text[marker.len()..text.len() - marker.len()].to_string();
                        let inner_len = inner.len() as i32;
                        let mut s = buf_bi.iter_at_offset(start_off);
                        let mut e = buf_bi.iter_at_offset(end_off);
                        buf_bi.delete(&mut s, &mut e);
                        let mut ins = buf_bi.iter_at_offset(start_off);
                        buf_bi.insert(&mut ins, &inner);
                        let ns = buf_bi.iter_at_offset(start_off);
                        let ne = buf_bi.iter_at_offset(start_off + inner_len);
                        buf_bi.select_range(&ns, &ne);
                    } else {
                        let tlen = text.len() as i32;
                        let mut s = buf_bi.iter_at_offset(start_off);
                        let mut e = buf_bi.iter_at_offset(end_off);
                        buf_bi.delete(&mut s, &mut e);
                        let mut ins = buf_bi.iter_at_offset(start_off);
                        buf_bi.insert(&mut ins, &format!("{marker}{text}{marker}"));
                        let ns = buf_bi.iter_at_offset(start_off + mlen);
                        let ne = buf_bi.iter_at_offset(start_off + mlen + tlen);
                        buf_bi.select_range(&ns, &ne);
                    }
                    buf_bi.end_user_action();
                } else {
                    buf_bi.begin_user_action();
                    let pos = buf_bi.cursor_position();
                    let mut ins = buf_bi.iter_at_offset(pos);
                    buf_bi.insert(&mut ins, &format!("{marker}{marker}"));
                    let cursor = buf_bi.iter_at_offset(pos + mlen);
                    buf_bi.place_cursor(&cursor);
                    buf_bi.end_user_action();
                }
                glib::Propagation::Stop
            });
            view.add_controller(bi_ctrl);
        }

        // ── Duplicate line / selection (Ctrl+D) ──────────────────────────────
        {
            let buf_dup = buffer.clone();
            let dup_ctrl = EventControllerKey::new();
            dup_ctrl.set_propagation_phase(PropagationPhase::Capture);
            dup_ctrl.connect_key_pressed(move |_, key, _, mods| {
                use gtk4::gdk::Key;
                let ctrl = mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
                let shift = mods.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
                if !ctrl || shift || key != Key::d {
                    return glib::Propagation::Proceed;
                }
                buf_dup.begin_user_action();
                if let Some((sel_s, sel_e)) = buf_dup.selection_bounds() {
                    let text = buf_dup.text(&sel_s, &sel_e, false).to_string();
                    let mut ins = sel_e.clone();
                    buf_dup.insert(&mut ins, &text);
                } else {
                    let cursor_pos = buf_dup.cursor_position();
                    let cursor = buf_dup.iter_at_offset(cursor_pos);
                    let ln = cursor.line();
                    let Some(line_start) = buf_dup.iter_at_line(ln) else {
                        buf_dup.end_user_action();
                        return glib::Propagation::Stop;
                    };
                    let mut line_end = line_start.clone();
                    if !line_end.ends_line() {
                        line_end.forward_to_line_end();
                    }
                    let text = buf_dup.text(&line_start, &line_end, false).to_string();
                    let mut ins = line_end.clone();
                    buf_dup.insert(&mut ins, &format!("\n{text}"));
                }
                buf_dup.end_user_action();
                glib::Propagation::Stop
            });
            view.add_controller(dup_ctrl);
        }

        // ── Page break (Ctrl+Enter) ───────────────────────────────────────────
        {
            let buf_pb = buffer.clone();
            let pb_ctrl = EventControllerKey::new();
            pb_ctrl.set_propagation_phase(PropagationPhase::Capture);
            pb_ctrl.connect_key_pressed(move |_, key, _, mods| {
                use gtk4::gdk::Key;
                let ctrl = mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
                if !ctrl || key != Key::Return {
                    return glib::Propagation::Proceed;
                }
                buf_pb.begin_user_action();
                buf_pb.insert_at_cursor("\n#pagebreak()\n");
                buf_pb.end_user_action();
                glib::Propagation::Stop
            });
            view.add_controller(pb_ctrl);
        }

        // ── Undo / Redo keyboard shortcuts ───────────────────────────────────
        // GTK4 GtkTextView has built-in Ctrl+Z / Ctrl+Shift+Z bindings, but we add
        // explicit handling here so our nav_ctrl (Capture phase) can also update the
        // button sensitivity immediately rather than waiting for the next idle cycle.
        {
            let buf_undo = buffer.clone();
            let undo_ctrl = EventControllerKey::new();
            undo_ctrl.set_propagation_phase(PropagationPhase::Capture);
            undo_ctrl.connect_key_pressed(move |_, key, _, mods| {
                use gtk4::gdk::Key;
                let ctrl  = mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
                let shift = mods.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
                let alt   = mods.contains(gtk4::gdk::ModifierType::ALT_MASK);
                if !ctrl || alt || key != Key::z { return glib::Propagation::Proceed; }
                if shift {
                    if buf_undo.can_redo() { buf_undo.redo(); }
                } else {
                    if buf_undo.can_undo() { buf_undo.undo(); }
                }
                glib::Propagation::Stop
            });
            view.add_controller(undo_ctrl);
        }

        // ── Typst-aware word navigation (Ctrl+Left/Right) ────────────────────
        // GTK's default word boundaries stop at '#' and '@', forcing two presses to
        // skip past `#set`, `@citation`, etc.  This controller intercepts Ctrl+arrow
        // and moves to the true end of the token (including the sigil character).
        {
            let buf_nav = buffer.clone();
            let view_nav = view.clone();
            let nav_ctrl = EventControllerKey::new();
            nav_ctrl.set_propagation_phase(PropagationPhase::Capture);
            nav_ctrl.connect_key_pressed(move |_, key, _, mods| {
                use gtk4::gdk::Key;
                let ctrl  = mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
                let shift = mods.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
                let alt   = mods.contains(gtk4::gdk::ModifierType::ALT_MASK);
                if !ctrl || alt { return glib::Propagation::Proceed; }

                // ── Heading jump: Ctrl+Shift+Up / Ctrl+Shift+Down ────────────
                if shift && (key == Key::Up || key == Key::Down) {
                    let pos = buf_nav.cursor_position();
                    let cur_line = buf_nav.iter_at_offset(pos).line();
                    let line_count = buf_nav.line_count();
                    let target_line = if key == Key::Up {
                        (0..cur_line).rev().find(|&ln| is_heading_line(&buf_nav, ln))
                    } else {
                        (cur_line + 1..line_count).find(|&ln| is_heading_line(&buf_nav, ln))
                    };
                    if let Some(ln) = target_line {
                        if let Some(it) = buf_nav.iter_at_line(ln) {
                            buf_nav.place_cursor(&it);
                            let mut it2 = it.clone();
                            view_nav.scroll_to_iter(&mut it2, 0.1, false, 0.0, 0.3);
                        }
                    }
                    return glib::Propagation::Stop;
                }

                // ── Typst-aware word movement: Ctrl+Left / Ctrl+Right ────────
                if shift { return glib::Propagation::Proceed; } // let Ctrl+Shift+Left/Right select
                if key != Key::Left && key != Key::Right { return glib::Propagation::Proceed; }

                let pos = buf_nav.cursor_position();
                let mut it = buf_nav.iter_at_offset(pos);
                let forward = key == Key::Right;

                if forward {
                    // Skip leading whitespace first (mirrors GtkTextView default)
                    while !it.is_end() && it.char().is_whitespace() {
                        it.forward_char();
                    }
                    // If we're now on '#' or '@' (Typst sigils), absorb the sigil so
                    // the next word_end lands after the whole `#keyword` or `@key`.
                    if matches!(it.char(), '#' | '@') {
                        it.forward_char();
                    }
                    it.forward_word_end();
                } else {
                    // Skip trailing whitespace
                    while !it.is_start() && it.char().is_whitespace() {
                        it.backward_char();
                    }
                    it.backward_word_start();
                    // If the character just before the new position is '#' or '@', absorb it
                    let mut probe = it.clone();
                    if probe.backward_char() && matches!(probe.char(), '#' | '@') {
                        it = probe;
                    }
                }

                buf_nav.place_cursor(&it);
                let mut sc = it.clone();
                view_nav.scroll_to_iter(&mut sc, 0.07, false, 0.0, 0.5);
                glib::Propagation::Stop
            });
            view.add_controller(nav_ctrl);
        }

        // ── Alt+Enter: open spell suggestions for word under cursor ─────────────
        {
            let spell_ae = self.spell_checker.clone();
            let buf_ae   = buffer.clone();
            let view_ae  = view.clone();
            let ae_ctrl  = EventControllerKey::new();
            ae_ctrl.connect_key_pressed(move |_, key, _, mods| {
                use gtk4::gdk::{Key, ModifierType};
                if key != Key::Return && key != Key::KP_Enter { return glib::Propagation::Proceed; }
                if !mods.contains(ModifierType::ALT_MASK) { return glib::Propagation::Proceed; }

                let sc = spell_ae.borrow();
                if !sc.enabled { return glib::Propagation::Proceed; }

                let buf = &buf_ae;
                let pos = buf.cursor_position();
                let iter = buf.iter_at_offset(pos);
                let table = buf.tag_table();
                let Some(tag) = table.lookup("zerkalo-spell") else { return glib::Propagation::Proceed; };
                if !iter.has_tag(&tag) { return glib::Propagation::Proceed; }

                let mut word_start = iter.clone();
                loop {
                    let mut prev = word_start.clone();
                    if !prev.backward_char() { break; }
                    if !prev.char().is_alphabetic() { break; }
                    word_start = prev;
                }
                let mut word_end = iter.clone();
                while word_end.char().is_alphabetic() {
                    if !word_end.forward_char() { break; }
                }
                let word = buf.text(&word_start, &word_end, false).to_string();
                if word.is_empty() { return glib::Propagation::Proceed; }

                let suggestions = sc.suggestions_for(&word);
                drop(sc);

                // Position popover at cursor
                let (cx, cy) = {
                    let iter2 = buf.iter_at_offset(pos);
                    let rect = view_ae.iter_location(&iter2);
                    (rect.x() + rect.width() / 2, rect.y() + rect.height())
                };
                let popover = Popover::new();
                popover.set_parent(&view_ae);
                popover.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(cx, cy, 1, 1)));
                popover.set_has_arrow(true);
                popover.set_autohide(true);

                let vbox = GtkBox::new(Orientation::Vertical, 2);
                vbox.set_margin_top(6); vbox.set_margin_bottom(6);
                vbox.set_margin_start(4); vbox.set_margin_end(4);

                if suggestions.is_empty() {
                    let lbl = Label::new(Some("No suggestions"));
                    lbl.add_css_class("dim-label");
                    lbl.set_margin_top(4); lbl.set_margin_bottom(4);
                    vbox.append(&lbl);
                } else {
                    for sugg in suggestions.iter().take(6) {
                        let btn = Button::with_label(sugg);
                        btn.add_css_class("flat");
                        let buf2 = buf_ae.clone();
                        let ws = word_start.clone();
                        let we = word_end.clone();
                        let s = sugg.clone();
                        let pop2 = popover.clone();
                        btn.connect_clicked(move |_| {
                            let mut a = ws.clone();
                            let mut b = we.clone();
                            buf2.begin_user_action();
                            buf2.delete(&mut a, &mut b);
                            buf2.insert(&mut a, &s);
                            buf2.end_user_action();
                            pop2.popdown();
                        });
                        vbox.append(&btn);
                    }
                }

                let pop_close = popover.clone();
                popover.connect_closed(move |_| { pop_close.unparent(); });
                popover.set_child(Some(&vbox));
                popover.popup();
                popover.grab_focus();
                glib::Propagation::Stop
            });
            view.add_controller(ae_ctrl);
        }

        // ── Spell check: debounced buffer check ───────────────────────────────

        {
            let spell_c = self.spell_checker.clone();
            let spell_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
            let spell_poll_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
            let buf_spell = buffer.clone();

            buffer.connect_changed(move |buf| {
                let enabled = {
                    let sc = spell_c.borrow();
                    sc.enabled
                };
                if !enabled {
                    // Release spell_checker borrow before GTK tag ops — remove_tag_by_name
                    // can cascade through GtkSourceView signals and re-enter this closure
                    // (or another that borrows spell_checker), causing a BorrowError panic.
                    clear_spell_tags(&buf_spell);
                    return;
                }

                // Cancel any pending debounce and in-flight poll timer.
                if let Some(id) = spell_timer.borrow_mut().take() { id.remove(); }
                if let Some(id) = spell_poll_timer.borrow_mut().take() { id.remove(); }

                let buf2 = buf.clone();
                let sc2 = spell_c.clone();
                let t = spell_timer.clone();
                let pt = spell_poll_timer.clone();
                let pt2 = spell_poll_timer.clone();

                *spell_timer.borrow_mut() = Some(glib::timeout_add_local_once(
                    Duration::from_millis(700),
                    move || {
                        *t.borrow_mut() = None;

                        let sc = sc2.borrow();
                        if !sc.enabled {
                            clear_spell_tags(&buf2);
                            return;
                        }
                        let langs = sc.languages.clone();
                        let ignored = sc.ignored().clone();
                        drop(sc);

                        let (s, e) = buf2.bounds();
                        let text = buf2.text(&s, &e, true).to_string();
                        let buf3 = buf2.clone();

                        let (tx, rx) = std::sync::mpsc::sync_channel(1);
                        std::thread::spawn(move || {
                            let words = crate::spellcheck::extract_words(&text);
                            let unique: Vec<String> = {
                                let mut seen = HashSet::new();
                                words.iter()
                                    .filter(|(_, _, w)| !ignored.contains(&w.to_lowercase()) && seen.insert(w.to_lowercase()))
                                    .map(|(_, _, w)| w.clone())
                                    .collect()
                            };
                            let unique_refs: Vec<&str> = unique.iter().map(|s| s.as_str()).collect();
                            let misspelled = crate::spellcheck::check_words_batch(&unique_refs, &langs);
                            let _ = tx.send((words, misspelled));
                        });

                        let poll_id = glib::timeout_add_local(Duration::from_millis(50), move || {
                            match rx.try_recv() {
                                Ok((words, misspelled)) => {
                                    // Clear the RefCell before returning Break. GLib auto-removes
                                    // the source after the callback, but the RefCell still holds
                                    // the now-dead SourceId. A subsequent connect_changed would call
                                    // id.remove() on it and panic with "Failed to remove source".
                                    *pt2.borrow_mut() = None;
                                    apply_spell_tags(&buf3, &words, &misspelled);
                                    glib::ControlFlow::Break
                                }
                                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                    *pt2.borrow_mut() = None;
                                    glib::ControlFlow::Break
                                }
                            }
                        });
                        *pt.borrow_mut() = Some(poll_id);
                    },
                ));
            });
        }

        // ── Spell check: autocorrect on word boundary ─────────────────────────

        {
            let spell_ac = self.spell_checker.clone();
            let buf_ac = buffer.clone();

            buffer.connect_changed(move |buf| {
                let sc = spell_ac.borrow();
                if !sc.enabled || !sc.autocorrect {
                    return;
                }

                let cursor = buf.cursor_position();
                if cursor < 2 {
                    return;
                }

                let just_typed = buf.iter_at_offset(cursor - 1);
                let ch = just_typed.char();
                // Only autocorrect when a word-terminating character is typed
                if !matches!(ch, ' ' | '\t' | '\n' | '.' | ',' | ';' | ':' | '!' | '?') {
                    return;
                }

                // Scan backward to find the preceding word
                let word_end = buf.iter_at_offset(cursor - 1);
                let mut word_start = word_end.clone();
                loop {
                    let mut prev = word_start.clone();
                    if !prev.backward_char() { break; }
                    if !prev.char().is_alphabetic() { break; }
                    word_start = prev;
                }
                if word_start == word_end { return; }

                let word = buf.text(&word_start, &word_end, false).to_string();
                if word.len() < 3 || sc.is_ignored(&word) { return; }

                // Don't autocorrect proper nouns or words already starting with upper
                if word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    return;
                }

                let suggestions = sc.suggestions_for(&word);
                drop(sc);

                if let Some(best) = suggestions.first() {
                    // Only apply if edit distance is 1 (very confident replacement)
                    if crate::spellcheck::levenshtein(&word.to_lowercase(), &best.to_lowercase()) <= 1 {
                        let ws = word_start.clone();
                        let we = word_end.clone();
                        let best_c = best.clone();
                        let buf_c = buf_ac.clone();
                        glib::idle_add_local_once(move || {
                            let mut s = ws;
                            let mut e = we;
                            buf_c.begin_user_action();
                            buf_c.delete(&mut s, &mut e);
                            buf_c.insert(&mut s, &best_c);
                            buf_c.end_user_action();
                        });
                    }
                }
            });
        }

        // ── Right-click context menu (spell suggestions + ignore) ─────────────
        //
        // saved_scroll is defined here (not in the focus-snap block below) so
        // the right-click gesture can also update it. If we don't, the sequence:
        //   right-click → GTK snaps scroll → focus_leave saves snapped value
        //   → idle restores real value → dismiss popover → focus_enter restores
        //   wrong (snapped) value → visible jump.
        let saved_scroll: Rc<Cell<f64>> = Rc::new(Cell::new(-1.0));
        let saved_hscroll: Rc<Cell<f64>> = Rc::new(Cell::new(-1.0));

        {
            let spell_rc = self.spell_checker.clone();
            let buf_rc = buffer.clone();
            let view_rc = view.clone();
            let scroll_rc = scroll.clone();
            let saved_rc = saved_scroll.clone();
            let saved_hrc = saved_hscroll.clone();

            // Use connect_pressed, not connect_released. GtkSourceView processes
            // button-3 internally and may grab the pointer before the release
            // event reaches our gesture, so connect_released is unreliable.
            // connect_pressed fires before any widget-level handling.
            let gesture = GestureClick::new();
            gesture.set_button(3); // right button

            gesture.connect_pressed(move |_, _, x, y| {
                // Suppress the focus-snap that right-click can trigger even
                // when the view already has focus. Also update saved_scroll so
                // that focus_ctrl.connect_enter (which fires on popover dismiss)
                // restores to the correct position, not the GTK-snapped one.
                // focus_ctrl.connect_leave fires during event processing (before
                // idle), so it would otherwise capture the wrong snapped value.
                let scroll_val = scroll_rc.vadjustment().value();
                let hscroll_val = scroll_rc.hadjustment().value();
                {
                    let sc = scroll_rc.clone();
                    let sv = saved_rc.clone();
                    let sh = saved_hrc.clone();
                    glib::timeout_add_local_once(Duration::ZERO, move || {
                        sc.vadjustment().set_value(scroll_val);
                        sc.hadjustment().set_value(hscroll_val);
                        sv.set(scroll_val);
                        sh.set(hscroll_val);
                    });
                }

                let sc = spell_rc.borrow();
                if !sc.enabled { return; }

                let (bx, by) = view_rc.window_to_buffer_coords(
                    TextWindowType::Widget, x as i32, y as i32,
                );
                let Some(iter) = view_rc.iter_at_location(bx, by) else { return };

                let table = buf_rc.tag_table();
                let Some(tag) = table.lookup("zerkalo-spell") else { return };
                if !iter.has_tag(&tag) { return; }

                // Find word boundaries
                let mut word_start = iter.clone();
                loop {
                    let mut prev = word_start.clone();
                    if !prev.backward_char() { break; }
                    if !prev.char().is_alphabetic() { break; }
                    word_start = prev;
                }
                let mut word_end = iter.clone();
                while word_end.char().is_alphabetic() {
                    if !word_end.forward_char() { break; }
                }
                let word = buf_rc.text(&word_start, &word_end, false).to_string();
                if word.is_empty() { return; }

                let suggestions = sc.suggestions_for(&word);
                drop(sc);

                let popover = Popover::new();
                popover.set_parent(&view_rc);
                let rect = gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
                popover.set_pointing_to(Some(&rect));
                popover.set_has_arrow(true);

                let vbox = GtkBox::new(Orientation::Vertical, 2);
                vbox.set_margin_top(6);
                vbox.set_margin_bottom(6);
                vbox.set_margin_start(4);
                vbox.set_margin_end(4);

                if suggestions.is_empty() {
                    let lbl = Label::new(Some("No suggestions"));
                    lbl.add_css_class("dim-label");
                    lbl.set_margin_top(4);
                    lbl.set_margin_bottom(4);
                    vbox.append(&lbl);
                } else {
                    for sugg in suggestions.iter().take(6) {
                        let btn = Button::with_label(sugg);
                        btn.add_css_class("flat");
                        let buf2 = buf_rc.clone();
                        let ws = word_start.clone();
                        let we = word_end.clone();
                        let s = sugg.clone();
                        let pop2 = popover.clone();
                        btn.connect_clicked(move |_| {
                            let mut a = ws.clone();
                            let mut b = we.clone();
                            buf2.begin_user_action();
                            buf2.delete(&mut a, &mut b);
                            buf2.insert(&mut a, &s);
                            buf2.end_user_action();
                            pop2.popdown();
                        });
                        vbox.append(&btn);
                    }
                }

                vbox.append(&Separator::new(Orientation::Horizontal));

                let ignore_btn = Button::with_label("Ignore All");
                ignore_btn.add_css_class("flat");
                let spell_ign = spell_rc.clone();
                let buf_ign = buf_rc.clone();
                let word_ign = word.clone();
                let pop_ign = popover.clone();
                ignore_btn.connect_clicked(move |_| {
                    spell_ign.borrow_mut().ignore(&word_ign);
                    let tag_table = buf_ign.tag_table();
                    if let Some(t) = tag_table.lookup("zerkalo-spell") {
                        remove_spell_word_tags(&buf_ign, &t, &word_ign);
                    }
                    pop_ign.popdown();
                });
                vbox.append(&ignore_btn);

                let add_dict_btn = Button::with_label("Add to Dictionary");
                add_dict_btn.add_css_class("flat");
                let spell_dict = spell_rc.clone();
                let buf_dict = buf_rc.clone();
                let word_dict = word.clone();
                let pop_dict = popover.clone();
                add_dict_btn.connect_clicked(move |_| {
                    spell_dict.borrow_mut().add_to_user_dict(&word_dict);
                    let tag_table = buf_dict.tag_table();
                    if let Some(t) = tag_table.lookup("zerkalo-spell") {
                        remove_spell_word_tags(&buf_dict, &t, &word_dict);
                    }
                    pop_dict.popdown();
                });
                vbox.append(&add_dict_btn);

                if spell_rc.borrow().has_project_dict() {
                    let add_proj_btn = Button::with_label("Add to Project Dictionary");
                    add_proj_btn.add_css_class("flat");
                    let spell_proj = spell_rc.clone();
                    let buf_proj = buf_rc.clone();
                    let word_proj = word.clone();
                    let pop_proj = popover.clone();
                    add_proj_btn.connect_clicked(move |_| {
                        spell_proj.borrow_mut().add_to_project_dict(&word_proj);
                        let tag_table = buf_proj.tag_table();
                        if let Some(t) = tag_table.lookup("zerkalo-spell") {
                            remove_spell_word_tags(&buf_proj, &t, &word_proj);
                        }
                        pop_proj.popdown();
                    });
                    vbox.append(&add_proj_btn);
                }

                popover.set_child(Some(&vbox));

                let pop_close = popover.clone();
                popover.connect_closed(move |_| {
                    pop_close.unparent();
                    // Do NOT restore scroll here. The idle in popup() already
                    // anchored the view. Restoring on close fights with whatever
                    // the user clicked to dismiss the popover.
                });

                popover.popup();
            });
            view.add_controller(gesture);
        }

        // ── Inline error assistant — hover over error-tagged line ─────────────
        {
            let last_diags = self.last_diagnostics.clone();
            let view_hover = view.clone();
            let buf_hover = buffer.clone();
            let active_popup: Rc<RefCell<Option<Popover>>> = Rc::new(RefCell::new(None));

            let motion = EventControllerMotion::new();
            let active_popup_c = active_popup.clone();
            motion.connect_motion(move |_, x, y| {
                let diags = last_diags.borrow();
                if diags.is_empty() { return; }

                let (bx, by) = view_hover.window_to_buffer_coords(
                    TextWindowType::Widget, x as i32, y as i32,
                );
                let Some(iter) = view_hover.iter_at_location(bx, by) else { return };
                let line_1based = iter.line() as u32 + 1;

                let tag_table = buf_hover.tag_table();
                let has_error_tag = tag_table.lookup("zerkalo-diag-error")
                    .map(|t| iter.has_tag(&t))
                    .unwrap_or(false);
                if !has_error_tag { return; }

                let error_msg: Option<String> = diags.iter()
                    .find(|(_, ln, _)| *ln == line_1based)
                    .map(|(path, ln, _)| format!("Error on line {ln} in {}", path.file_name().and_then(|n| n.to_str()).unwrap_or("?")));
                let Some(msg) = error_msg else { return };

                // Only create a new popup if none is showing (avoid flicker)
                if active_popup_c.borrow().is_some() { return; }

                let fix = crate::error_patterns::match_fix(&msg);

                let popover = Popover::new();
                popover.set_parent(&view_hover);
                popover.set_has_arrow(true);
                popover.set_autohide(true);
                popover.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));

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

                if let Some(fx) = fix {
                    vbox.append(&Separator::new(Orientation::Horizontal));
                    let fix_row = GtkBox::new(Orientation::Horizontal, 8);
                    let fix_desc = Label::new(Some(fx.description));
                    fix_desc.add_css_class("dim-label");
                    fix_desc.set_xalign(0.0);
                    fix_desc.set_wrap(true);
                    fix_desc.set_max_width_chars(40);
                    fix_desc.set_hexpand(true);
                    fix_row.append(&fix_desc);

                    if let Some(fix_fn) = fx.fix_fn {
                        let fix_btn = Button::with_label("Fix It");
                        fix_btn.add_css_class("suggested-action");
                        let buf_fix = buf_hover.clone();
                        let line_fix = iter.line() as usize;
                        let pop_fix = popover.clone();
                        fix_btn.connect_clicked(move |_| {
                            let (s, e) = buf_fix.bounds();
                            let text = buf_fix.text(&s, &e, true).to_string();
                            if let Some(patched) = fix_fn(&text, line_fix) {
                                buf_fix.begin_user_action();
                                let mut start = buf_fix.start_iter();
                                let mut end = buf_fix.end_iter();
                                buf_fix.delete(&mut start, &mut end);
                                buf_fix.insert(&mut start, &patched);
                                buf_fix.end_user_action();
                            }
                            pop_fix.popdown();
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
            // Per-frame tick: undo GTK's left-margin snap before this frame renders.
            // GtkSourceView5's scroll-to-cursor sets the view's own hadjustment (a
            // separate object from the ScrolledWindow's hadjustment) to exactly
            // left_margin, hiding the margin. Correcting in the tick — which fires
            // before layout+draw — prevents any visible artifact.
            view.add_tick_callback(|v, _| {
                let x = v.visible_rect().x();
                let lm = v.left_margin();
                if x > 0 && x == lm {
                    if let Some(hadj) = v.hadjustment() {
                        hadj.set_value(0.0);
                    }
                }
                glib::ControlFlow::Continue
            });

            // Save on pointer-enter as a fallback for the very first enter.
            let ptr_ctrl = EventControllerMotion::new();
            {
                let sc = scroll.clone();
                let sv = saved_scroll.clone();
                let sh = saved_hscroll.clone();
                ptr_ctrl.connect_enter(move |_, _, _| {
                    let hval = sc.hadjustment().value();
                    sv.set(sc.vadjustment().value());
                    sh.set(hval);
                });
            }
            view.add_controller(ptr_ctrl);

            // On left-click, suppress the focus-snap only when the view is
            // actually gaining focus. If it already has focus the click is
            // intentional navigation and should scroll to the cursor normally.
            // button=1 only — button=0 would steal the right-click spell gesture's sequence.
            let any_click = GestureClick::new();
            any_click.set_button(1);
            {
                let sc = scroll.clone();
                let sv = saved_scroll.clone();
                let sh = saved_hscroll.clone();
                let view_fc = view.clone();
                any_click.connect_pressed(move |_, _, _, _| {
                    let val = sc.vadjustment().value();
                    let hval = sc.hadjustment().value();
                    sv.set(val);
                    sh.set(hval);
                    if !view_fc.has_focus() {
                        // View is gaining focus → GTK will snap to insert mark → restore both axes.
                        // Use a 0ms timeout (not idle_add) so we fire AFTER the entire idle queue
                        // drains, including GTK's own focus-snap scroll_mark_onscreen idle.
                        let sc2 = sc.clone();
                        glib::timeout_add_local_once(Duration::ZERO, move || {
                            sc2.vadjustment().set_value(val);
                            sc2.hadjustment().set_value(hval);
                        });
                    }
                });
            }
            view.add_controller(any_click);

            // Also refresh the saved value whenever focus leaves the view, so
            // that the stored position stays current for re-entry.
            let focus_ctrl = EventControllerFocus::new();
            {
                let sc_leave = scroll.clone();
                let sv_leave = saved_scroll.clone();
                let sh_leave = saved_hscroll.clone();
                focus_ctrl.connect_leave(move |_| {
                    let hval = sc_leave.hadjustment().value();
                    sv_leave.set(sc_leave.vadjustment().value());
                    sh_leave.set(hval);
                });
            }
            {
                let sc_enter = scroll.clone();
                let sv_enter = saved_scroll.clone();
                let sh_enter = saved_hscroll.clone();
                focus_ctrl.connect_enter(move |_| {
                    // Use the value saved when focus left (or at right-click time) rather
                    // than the current scroll. GTK can snap the view to the cursor
                    // synchronously before this signal fires (e.g. on context-menu
                    // dismiss), so reading the current value would restore the snapped
                    // position rather than where the user actually was.
                    let val = sv_enter.get();
                    let hval = sh_enter.get();
                    if val < 0.0 { return; }
                    let sc = sc_enter.clone();
                    glib::timeout_add_local_once(Duration::ZERO, move || {
                        sc.vadjustment().set_value(val);
                        sc.hadjustment().set_value(hval);
                    });
                });
            }
            view.add_controller(focus_ctrl);
        }

        // Re-apply squiggles after undo restores old text
        {
            let last_diags = self.last_diagnostics.clone();
            let ep_rem = self.clone();
            let remarking: Rc<std::cell::Cell<bool>> = Rc::new(std::cell::Cell::new(false));
            buffer.connect_changed(move |_| {
                if remarking.get() { return; }
                let diags = last_diags.borrow().clone();
                if diags.is_empty() { return; }
                remarking.set(true);
                let ep = ep_rem.clone();
                let rem = remarking.clone();
                glib::idle_add_local_once(move || {
                    ep.mark_diagnostics(&diags);
                    rem.set(false);
                });
            });
        }

        // ── Insert into notebook ──────────────────────────────────────────────

        let page_index = self.notebook.append_page(&scroll, Some(&tab_box));
        self.notebook.set_tab_reorderable(&scroll, true);

        let path_for_callback = path.clone();
        let content_for_callback = content.to_string();


        let session_start_words = count_words(content);
        self.state.borrow_mut().tabs.insert(
            path,
            EditorTab {
                buffer,
                view,
                scroll_window: scroll,
                modified: false,
                dot_label,
                diag_dot,
                tab_box,
                display_name: display_name.clone(),
                lsp_popup,
                session_start_words,
            },
        );

        self.notebook.set_current_page(Some(page_index));
        set_wc_text_with_session(&self.word_count_label, content, session_start_words);

        // Parse per-document goal from `// @zerkalo-goal: N`
        if let Some(goal) = parse_goal_comment(content) {
            *self.word_count_goal.borrow_mut() = goal;
            update_goal_bar(&self.goal_bar, content, goal);
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

    #[allow(dead_code)]
    pub fn close_file_if_open(&self, path: &PathBuf) {
        if self.state.borrow().tabs.contains_key(path) {
            self.close_file(path);
        }
    }

    pub fn close_file(&self, path: &PathBuf) {
        // Extract page number and drop the borrow before remove_page, which fires
        // switch_page → connect_switch_page tries state.borrow() → double-borrow panic.
        let page_num = {
            let state = self.state.borrow();
            state.tabs.get(path)
                .and_then(|t| self.notebook.page_num(&t.scroll_window))
        };
        self.state.borrow_mut().tabs.remove(path);
        if let Some(n) = page_num {
            self.notebook.remove_page(Some(n));
        }
    }

    pub fn active_line_count(&self) -> u32 {
        let current = match self.notebook.current_page() {
            Some(p) => p,
            None => return 1,
        };
        let state = self.state.borrow();
        for tab in state.tabs.values() {
            if self.notebook.page_num(&tab.scroll_window) == Some(current) {
                return tab.buffer.line_count() as u32;
            }
        }
        1
    }

    pub fn get_active_content(&self) -> Option<String> {
        let current = self.notebook.current_page()?;
        let state = self.state.borrow();
        for tab in state.tabs.values() {
            if let Some(n) = self.notebook.page_num(&tab.scroll_window) {
                if n == current {
                    let (start, end) = tab.buffer.bounds();
                    return Some(tab.buffer.text(&start, &end, true).to_string());
                }
            }
        }
        None
    }

    #[allow(dead_code)]
    pub fn set_active_content(&self, text: &str) {
        let current = match self.notebook.current_page() {
            Some(p) => p,
            None => return,
        };
        let buf = {
            let state = self.state.borrow();
            state.tabs.values()
                .find(|t| self.notebook.page_num(&t.scroll_window) == Some(current))
                .map(|t| t.buffer.clone())
        };
        if let Some(buffer) = buf {
            buffer.set_text(text);
            { let sm = *self.simple_mode.borrow(); apply_simple_mode_tag(&buffer, sm); }
        }
    }

    /// Replace the active buffer's entire content as a single undoable user action.
    pub fn set_active_content_undoable(&self, text: &str) {
        let current = match self.notebook.current_page() {
            Some(p) => p,
            None => return,
        };
        let buf = {
            let state = self.state.borrow();
            state.tabs.values()
                .find(|t| self.notebook.page_num(&t.scroll_window) == Some(current))
                .map(|t| t.buffer.clone())
        };
        if let Some(buffer) = buf {
            buffer.begin_user_action();
            let (start, end) = buffer.bounds();
            buffer.delete(&mut start.clone(), &mut end.clone());
            buffer.insert(&mut buffer.end_iter(), text);
            buffer.end_user_action();
            { let sm = *self.simple_mode.borrow(); apply_simple_mode_tag(&buffer, sm); }
        }
    }

    pub fn state_has_file(&self, path: &std::path::Path) -> bool {
        self.state.borrow().tabs.contains_key(path)
    }

    pub fn set_content(&self, path: &std::path::Path, text: &str) {
        let buf = self.state.borrow().tabs.get(path).map(|t| t.buffer.clone());
        if let Some(buffer) = buf {
            buffer.set_text(text);
            { let sm = *self.simple_mode.borrow(); apply_simple_mode_tag(&buffer, sm); }
        }
    }

    pub fn switch_to_file(&self, path: &PathBuf) {
        let state = self.state.borrow();
        if let Some(tab) = state.tabs.get(path) {
            if let Some(n) = self.notebook.page_num(&tab.scroll_window) {
                self.notebook.set_current_page(Some(n));
            }
        }
    }

    pub fn mark_saved(&self, path: &PathBuf) {
        let widgets = {
            let mut state = self.state.borrow_mut();
            state.tabs.get_mut(path).map(|tab| {
                tab.modified = false;
                (tab.dot_label.clone(), tab.tab_box.clone(), tab.display_name.clone())
            })
        };
        if let Some((dot_label, tab_box, display_name)) = widgets {
            dot_label.set_visible(false);
            tab_box.update_property(&[gtk4::accessible::Property::Label(&display_name)]);
        }
        if let Some(f) = self.on_modified_changed.borrow().as_ref() { f(false); }
        if let Some(f) = self.on_file_dirty.borrow().as_ref() { f(path.clone(), false); }
    }

    pub fn set_on_file_dirty(&self, f: impl Fn(PathBuf, bool) + 'static) {
        *self.on_file_dirty.borrow_mut() = Some(Box::new(f));
    }

    pub fn get_open_paths_ordered(&self) -> Vec<PathBuf> {
        let state = self.state.borrow();
        let mut pages: Vec<(u32, PathBuf)> = state
            .tabs
            .iter()
            .filter_map(|(path, tab)| {
                self.notebook.page_num(&tab.scroll_window).map(|n| (n, path.clone()))
            })
            .collect();
        pages.sort_by_key(|(n, _)| *n);
        pages.into_iter().map(|(_, p)| p).collect()
    }

    pub fn get_cursor_positions(&self) -> std::collections::HashMap<PathBuf, i32> {
        let state = self.state.borrow();
        state.tabs.iter().map(|(path, tab)| {
            (path.clone(), tab.buffer.cursor_position())
        }).collect()
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
            if let Some(n) = self.notebook.page_num(&tab.scroll_window) {
                if n == current {
                    return Some(path.clone());
                }
            }
        }
        None
    }

    /// Returns (path, content) for every tab that has unsaved modifications.
    pub fn modified_buffers(&self) -> Vec<(PathBuf, String)> {
        let state = self.state.borrow();
        state.tabs.iter()
            .filter(|(_, tab)| tab.modified)
            .map(|(path, tab)| {
                let (s, e) = tab.buffer.bounds();
                (path.clone(), tab.buffer.text(&s, &e, true).to_string())
            })
            .collect()
    }

    pub fn save_all_modified(&self) {
        let saved: Vec<(Label, GtkBox, String)> = {
            let mut state = self.state.borrow_mut();
            let mut out = Vec::new();
            for (path, tab) in state.tabs.iter_mut() {
                if !tab.modified { continue; }
                let (start, end) = tab.buffer.bounds();
                let content = tab.buffer.text(&start, &end, true);
                if std::fs::write(path, content.as_bytes()).is_ok() {
                    tab.modified = false;
                    out.push((tab.dot_label.clone(), tab.tab_box.clone(), tab.display_name.clone()));
                }
            }
            out
        };
        for (dot_label, tab_box, display_name) in saved {
            dot_label.set_visible(false);
            tab_box.update_property(&[gtk4::accessible::Property::Label(&display_name)]);
        }
    }

    pub fn save_current(&self) -> Option<PathBuf> {
        let path = self.get_active_path()?;
        let content = self.get_active_content()?;
        std::fs::write(&path, content.as_bytes()).ok()?;
        self.mark_saved(&path);
        Some(path)
    }

    pub fn next_tab(&self) {
        let n = self.notebook.n_pages();
        if n < 2 {
            return;
        }
        let current = self.notebook.current_page().unwrap_or(0);
        self.notebook.set_current_page(Some((current + 1) % n));
    }

    pub fn prev_tab(&self) {
        let n = self.notebook.n_pages();
        if n < 2 {
            return;
        }
        let current = self.notebook.current_page().unwrap_or(0);
        let prev = if current == 0 { n - 1 } else { current - 1 };
        self.notebook.set_current_page(Some(prev));
    }

    /// Jump to the first occurrence of `text` in the active buffer, select it, and scroll to centre.
    pub fn jump_to_text(&self, text: &str) {
        let Some((view, buffer)) = self.active_view_buffer() else { return };
        let flags = TextSearchFlags::TEXT_ONLY | TextSearchFlags::CASE_INSENSITIVE;
        let start_iter = buffer.start_iter();
        if let Some((s, e)) = start_iter.forward_search(text, flags, None) {
            buffer.select_range(&s, &e);
            view.scroll_to_iter(&mut s.clone(), 0.0, true, 0.0, 0.5);
            view.grab_focus();
        }
    }

    pub fn jump_to_line(&self, path: &PathBuf, line: u32) {
        self.switch_to_file(path);
        let state = self.state.borrow();
        if let Some(tab) = state.tabs.get(path) {
            let line_idx = line.saturating_sub(1) as i32;
            let line_start = tab.buffer.iter_at_line(line_idx).unwrap_or_else(|| {
                let (_, end) = tab.buffer.bounds();
                end
            });
            let mut line_end = line_start;
            line_end.forward_to_line_end();
            // Select the heading text so it's visually highlighted
            tab.buffer.select_range(&line_start, &line_end);
            // Scroll so the heading is vertically centered
            let mut scroll_iter = line_start;
            tab.view.scroll_to_iter(&mut scroll_iter, 0.0, true, 0.0, 0.5);
        }
    }

    pub fn active_text(&self) -> Option<String> {
        let (_, buf) = self.active_view_buffer()?;
        let (s, e) = buf.bounds();
        Some(buf.text(&s, &e, true).to_string())
    }

    #[allow(dead_code)]
    pub fn all_tab_texts(&self) -> Vec<(PathBuf, String)> {
        self.state.borrow().tabs.iter()
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
            if self.notebook.page_num(&tab.scroll_window) == Some(current) {
                return tab.session_start_words;
            }
        }
        0
    }

    fn toggle_active_markup(&self, marker: &str) {
        let Some((_view, buf)) = self.active_view_buffer() else { return };
        let mlen = marker.len() as i32;
        buf.begin_user_action();
        if let Some((sel_s, sel_e)) = buf.selection_bounds() {
            let start_off = sel_s.offset();
            let end_off = sel_e.offset();
            let text = buf.text(&sel_s, &sel_e, false).to_string();
            if text.starts_with(marker) && text.ends_with(marker)
                && text.len() > 2 * marker.len()
            {
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
        let Some((_view, buf)) = self.active_view_buffer() else { return };
        let cursor = buf.iter_at_mark(&buf.get_insert());
        let line = cursor.line();
        let line_start = buf.iter_at_line(line).unwrap_or(cursor.clone());
        let mut line_end = line_start.clone();
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
            if self.notebook.page_num(&tab.scroll_window) == Some(current) {
                return Some((tab.view.clone(), tab.buffer.clone()));
            }
        }
        None
    }
}

// ── Free helpers ──────────────────────────────────────────────────────────────

const SIMPLE_TAG: &str = "zk-simple-hidden";
const BODY_SEPARATOR: &str = "// ── Document body";

fn apply_simple_mode_tag(buffer: &Buffer, on: bool) {
    let table = buffer.tag_table();
    let tag = match table.lookup(SIMPLE_TAG) {
        Some(t) => t,
        None => {
            let t = TextTag::new(Some(SIMPLE_TAG));
            t.set_invisible(on);
            table.add(&t);
            t
        }
    };
    tag.set_invisible(on);

    // Always clear any existing span first.
    let (start, end) = buffer.bounds();
    buffer.remove_tag(&tag, &start, &end);

    if !on {
        return;
    }

    // Find the "// ── Document body" separator line.
    let text = buffer.text(&start, &end, false);
    let body_line = text.lines().position(|l| l.starts_with(BODY_SEPARATOR));
    let Some(body_line_idx) = body_line else { return };

    // Count consecutive separator lines (typically 2: the "DO NOT DELETE"
    // warning and the decorative rule beneath it) so we hide those too.
    let sep_count = text.lines()
        .skip(body_line_idx)
        .take_while(|l| l.starts_with(BODY_SEPARATOR))
        .count();
    let hide_to = body_line_idx + sep_count;

    if hide_to == 0 {
        return;
    }
    let hide_end = match buffer.iter_at_line(hide_to as i32) {
        Some(it) => it,
        None => { let (_, e) = buffer.bounds(); e },
    };
    buffer.apply_tag(&tag, &start, &hide_end);
}

/// Apply a background fill to all comment lines (// runs and /* */ blocks).
/// Adjacent // lines are merged into one contiguous tag span for a "box" look.
fn apply_comment_highlights(buffer: &Buffer) {
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
    let base = dummy.style_context()
        .lookup_color("accent_color")
        .unwrap_or(gtk4::gdk::RGBA::new(0.2, 0.4, 0.9, 1.0));
    let color = gtk4::gdk::RGBA::new(base.red(), base.green(), base.blue(), alpha);
    tag.set_paragraph_background_rgba(Some(&color));

    // Remove old highlights
    let (buf_start, buf_end) = buffer.bounds();
    buffer.remove_tag(&tag, &buf_start, &buf_end);

    let text = buffer.text(&buf_start, &buf_end, false).to_string();
    let lines: Vec<&str> = text.lines().collect();
    let n = lines.len();
    let mut i = 0;
    while i < n {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("//") {
            // Merge consecutive // lines into one span
            let run_start = i;
            while i < n && lines[i].trim().starts_with("//") { i += 1; }
            if let (Some(ts), Some(mut te)) = (
                buffer.iter_at_line(run_start as i32),
                buffer.iter_at_line((i - 1) as i32),
            ) {
                te.forward_to_line_end();
                buffer.apply_tag(&tag, &ts, &te);
            }
        } else if trimmed.contains("/*") {
            // Block comment: scan for closing */
            let block_start = i;
            while i < n && !lines[i].contains("*/") { i += 1; }
            if i < n { i += 1; } // include closing line
            let last = (i - 1).min(n.saturating_sub(1));
            if let (Some(ts), Some(mut te)) = (
                buffer.iter_at_line(block_start as i32),
                buffer.iter_at_line(last as i32),
            ) {
                te.forward_to_line_end();
                buffer.apply_tag(&tag, &ts, &te);
            }
        } else {
            i += 1;
        }
    }
}

fn ensure_diag_tags(buffer: &Buffer) {
    let table = buffer.tag_table();
    if table.lookup("zerkalo-diag-error").is_none() {
        let tag = TextTag::new(Some("zerkalo-diag-error"));
        tag.set_underline(gtk4::pango::Underline::Error);
        tag.set_underline_rgba(Some(&gtk4::gdk::RGBA::new(0.9, 0.2, 0.2, 1.0)));
        table.add(&tag);
    }
    if table.lookup("zerkalo-diag-warning").is_none() {
        let tag = TextTag::new(Some("zerkalo-diag-warning"));
        tag.set_underline(gtk4::pango::Underline::SingleLine);
        tag.set_underline_rgba(Some(&gtk4::gdk::RGBA::new(0.85, 0.72, 0.1, 1.0)));
        table.add(&tag);
    }
}

fn ensure_spell_tag(buffer: &Buffer) {
    let table = buffer.tag_table();
    if table.lookup("zerkalo-spell").is_none() {
        let tag = TextTag::new(Some("zerkalo-spell"));
        tag.set_underline(gtk4::pango::Underline::Error);
        tag.set_underline_rgba(Some(&gtk4::gdk::RGBA::new(0.22, 0.55, 0.97, 1.0)));
        table.add(&tag);
    }
}

fn clear_spell_tags(buffer: &Buffer) {
    ensure_spell_tag(buffer);
    let (s, e) = buffer.bounds();
    buffer.remove_tag_by_name("zerkalo-spell", &s, &e);
}

// Remove the spell-error tag from every occurrence of `word` in the buffer.
// Uses forward_to_tag_toggle to skip directly between tagged ranges — O(k) in
// the number of misspelled-word ranges, not O(N) in buffer length.
fn remove_spell_word_tags(buffer: &Buffer, tag: &gtk4::TextTag, word: &str) {
    let target = word.to_lowercase();
    let (mut it, e) = buffer.bounds();
    loop {
        if !it.has_tag(tag) {
            if !it.forward_to_tag_toggle(Some(tag)) { break; }
            if it >= e { break; }
        }
        let ws = it.clone();
        let mut we = it.clone();
        if !we.forward_to_tag_toggle(Some(tag)) { we = e.clone(); }
        let w = buffer.text(&ws, &we, false).to_string();
        if w.to_lowercase() == target {
            buffer.remove_tag(tag, &ws, &we);
        }
        it = we;
        if it >= e { break; }
    }
}

fn apply_spell_tags(
    buffer: &Buffer,
    words: &[(usize, usize, String)],
    misspelled: &HashMap<String, Vec<String>>,
) {
    ensure_spell_tag(buffer);
    let (s, e) = buffer.bounds();
    buffer.remove_tag_by_name("zerkalo-spell", &s, &e);

    for (start, end, word) in words {
        if misspelled.contains_key(&word.to_lowercase()) {
            let iter_start = buffer.iter_at_offset(*start as i32);
            let iter_end = buffer.iter_at_offset(*end as i32);
            buffer.apply_tag_by_name("zerkalo-spell", &iter_start, &iter_end);
        }
    }
}

fn lsp_hash_prefix(buffer: &Buffer) -> String {
    let cursor = buffer.iter_at_offset(buffer.cursor_position());
    let mut temp = cursor.clone();
    loop {
        if !temp.backward_char() {
            break;
        }
        let ch = temp.char();
        if ch == '#' {
            return buffer
                .text(&temp, &cursor, false)
                .to_string()
                .trim_start_matches('#')
                .to_lowercase();
        }
        if !(ch.is_alphanumeric() || ch == '_' || ch == '-') {
            break;
        }
    }
    String::new()
}

fn count_words(text: &str) -> u32 {
    count_content_words(text) as u32
}

fn count_project_words(root: &std::path::Path) -> u32 {
    crate::project::collect_typ_files(root)
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .map(|c| count_content_words(&c) as u32)
        .sum()
}

fn wc_str_with_delta(text: &str, session_start: u32) -> String {
    let words = count_content_words(text) as u32;
    let reading = if words < 200 { "< 1 min".to_string() } else { format!("{} min", words / 200) };
    if words > session_start {
        let delta = words - session_start;
        format!("{words} words (+{delta}) · {reading} read")
    } else {
        format!("{words} words · {reading} read")
    }
}

fn set_wc_text_with_session(label: &Label, text: &str, session_start: u32) {
    label.set_text(&wc_str_with_delta(text, session_start));
}

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
        let f = if num_fmt.is_empty() { "1.".to_string() } else { num_fmt };
        format!("#context counter(heading).display(\"{f}\")#h(0.3em)")
    } else {
        String::new()
    };

    content.replace(OLD, &new_prefix)
}

fn count_content_words(text: &str) -> usize {
    strip_typst_markup(&strip_zerkalo_blocks(text)).split_whitespace().count()
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
            while i < n && chars[i] != '\n' { i += 1; }
            continue;
        }

        // Line comment //
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < n && chars[i] != '\n' { i += 1; }
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
            while i < n && chars[i] != '\n' { i += 1; }
            continue;
        }

        // Inline raw `...`
        if c == '`' {
            i += 1;
            while i < n && chars[i] != '`' && chars[i] != '\n' { i += 1; }
            if i < n && chars[i] == '`' { i += 1; }
            out.push(' ');
            continue;
        }

        // Math $...$
        if c == '$' {
            i += 1;
            while i < n && chars[i] != '$' { i += 1; }
            if i < n { i += 1; }
            out.push(' ');
            continue;
        }

        // Citation reference @key
        if c == '@' {
            i += 1;
            while i < n && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '-' || chars[i] == ':') {
                i += 1;
            }
            continue;
        }

        // Hash function calls: skip #ident and (...){...} args, but KEEP text in [...] args.
        // Structural directives (#set, #show, #let, #import, #include, etc.) have a space
        // between the keyword and the element name — skip the whole line for those.
        if c == '#' {
            i += 1;
            while i < n && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '-' || chars[i] == '.') {
                i += 1;
            }
            if i < n && chars[i] == ' ' {
                while i < n && chars[i] != '\n' { i += 1; }
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
            while i < n && chars[i] != '>' && chars[i] != '\n' { i += 1; }
            if i < n && chars[i] == '>' { i += 1; }
            continue;
        }

        out.push(c);
        i += 1;
    }

    out
}

fn skip_balanced_typst(chars: &[char], start: usize, n: usize) -> usize {
    let open = chars[start];
    let close = match open { '[' => ']', '(' => ')', '{' => '}', _ => return start + 1 };
    let mut i = start + 1;
    let mut depth = 1usize;
    while i < n && depth > 0 {
        if chars[i] == open { depth += 1; }
        else if chars[i] == close { depth -= 1; }
        i += 1;
    }
    i
}

fn strip_snippets(s: &str) -> String {
    // Remove LSP snippet placeholders: $0, $1, ${1:...}, etc.
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '$' {
            match chars.peek() {
                Some('{') => {
                    chars.next(); // consume '{'
                    // skip until matching '}'
                    for c in chars.by_ref() {
                        if c == '}' {
                            break;
                        }
                    }
                }
                Some(c) if c.is_ascii_digit() => {
                    // consume digits
                    while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                        chars.next();
                    }
                }
                _ => out.push(ch),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

// Builds a breadcrumb path string for the cursor position, e.g. "Intro › Methods".
// Scans backward collecting the first heading at each level encountered.
fn build_heading_path(buf: &sourceview5::Buffer, line_idx: i32) -> String {
    let mut path: Vec<(u32, String)> = Vec::new();
    let mut min_level: u32 = u32::MAX;
    let mut check = line_idx;
    while check >= 0 {
        if let Some(iter) = buf.iter_at_line(check) {
            let mut end = iter.clone();
            end.forward_to_line_end();
            let text = buf.text(&iter, &end, false).to_string();
            if text.starts_with('=') {
                let level = text.chars().take_while(|&c| c == '=').count() as u32;
                let content = text[level as usize..].trim().to_string();
                if path.is_empty() || level < min_level {
                    path.push((level, content));
                    min_level = level;
                    if level == 1 {
                        break;
                    }
                }
            }
        }
        check -= 1;
    }
    path.reverse();
    path.into_iter().map(|(_, t)| t).collect::<Vec<_>>().join(" / ")
}

// Returns the 1-based line number of the nearest heading at or above `line_idx`,
// or u32::MAX if none found.
fn find_heading_line_for(buf: &sourceview5::Buffer, line_idx: i32) -> u32 {
    let mut check = line_idx;
    while check >= 0 {
        if let Some(iter) = buf.iter_at_line(check) {
            let mut end = iter.clone();
            end.forward_to_line_end();
            let text = buf.text(&iter, &end, false);
            if text.starts_with('=') {
                return (check + 1) as u32;
            }
        }
        check -= 1;
    }
    u32::MAX
}

/// True if `ln` is a Typst heading line (starts with `=`).
fn is_heading_line(buf: &sourceview5::Buffer, ln: i32) -> bool {
    if let Some(it) = buf.iter_at_line(ln) {
        let mut end = it.clone();
        end.forward_to_line_end();
        let text = buf.text(&it, &end, false);
        return text.starts_with('=');
    }
    false
}

fn dismiss_popup(
    buf: &Buffer,
    popup: &BibPopup,
    mark: &Rc<RefCell<Option<gtk4::TextMark>>>,
) {
    if let Some(m) = mark.borrow_mut().take() {
        buf.delete_mark(&m);
    }
    popup.hide();
}

fn dismiss_popup_only(
    popup: &BibPopup,
    buf: &Buffer,
    mark: &Rc<RefCell<Option<gtk4::TextMark>>>,
) {
    if let Some(m) = mark.borrow_mut().take() {
        buf.delete_mark(&m);
    }
    popup.hide();
}

fn do_bib_complete(
    buf: &Buffer,
    mark: &Rc<RefCell<Option<gtk4::TextMark>>>,
    completing: &Rc<RefCell<bool>>,
    popup: &BibPopup,
    view: &View,
    key: &str,
) {
    *completing.borrow_mut() = true;
    let mark_opt = mark.borrow().clone();
    if let Some(ref m) = mark_opt {
        let mut start = buf.iter_at_mark(m);
        let mut end = buf.iter_at_offset(buf.cursor_position());
        buf.begin_user_action();
        buf.delete(&mut start, &mut end);
        buf.insert_at_cursor(&format!("@{key}"));
        buf.end_user_action();
        buf.delete_mark(m);
    }
    *mark.borrow_mut() = None;
    popup.hide();
    view.grab_focus();
    *completing.borrow_mut() = false;
}

fn set_view_line_spacing(view: &View, spacing: u32) {
    view.set_pixels_above_lines(spacing as i32);
    view.set_pixels_below_lines(spacing as i32);
}

fn update_goal_bar(bar: &ProgressBar, text: &str, goal: u32) {
    if goal == 0 {
        bar.set_visible(false);
        return;
    }
    let words = count_content_words(text);
    let fraction = (words as f64 / goal as f64).min(1.0);
    bar.set_fraction(fraction);
    bar.set_visible(true);
    bar.set_tooltip_text(Some(&format!("{words} / {goal} words ({:.0}%)", fraction * 100.0)));
}

fn parse_goal_comment(content: &str) -> Option<u32> {
    for line in content.lines().take(20) {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("// @zerkalo-goal:") {
            if let Ok(n) = rest.trim().parse::<u32>() {
                return Some(n);
            }
        }
    }
    None
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

fn do_lsp_complete(
    buf: &Buffer,
    mark: &Rc<RefCell<Option<gtk4::TextMark>>>,
    completing: &Rc<RefCell<bool>>,
    popup: &LspPopup,
    view: &View,
    item: CompletionItem,
) {
    *completing.borrow_mut() = true;
    let mark_opt = mark.borrow().clone();
    if let Some(ref m) = mark_opt {
        let mut start = buf.iter_at_mark(m);
        let mut end = buf.iter_at_offset(buf.cursor_position());
        let raw = item.insert_text.as_deref().unwrap_or(&item.label);
        let cleaned = strip_snippets(raw);
        let final_text = if cleaned.starts_with('#') {
            cleaned
        } else {
            format!("#{cleaned}")
        };
        buf.begin_user_action();
        buf.delete(&mut start, &mut end);
        buf.insert_at_cursor(&final_text);
        buf.end_user_action();
        buf.delete_mark(m);
    }
    *mark.borrow_mut() = None;
    popup.hide();
    view.grab_focus();
    *completing.borrow_mut() = false;
}

fn section_heading_level(text: &str) -> Option<usize> {
    let trimmed = text.trim_start();
    let lvl = trimmed.chars().take_while(|c| *c == '=').count();
    if lvl > 0 && trimmed[lvl..].starts_with(' ') { Some(lvl) } else { None }
}

/// Count words in a line of Typst text, treating `#lorem(N)` as N words.
fn count_words_typst(text: &str) -> u32 {
    let mut count = 0u32;
    let mut remaining = text;
    while !remaining.is_empty() {
        if let Some(pos) = remaining.find("#lorem(") {
            // Count words before #lorem
            count += remaining[..pos].split_whitespace().count() as u32;
            let after = &remaining[pos + 7..];
            if let Some(end) = after.find(')') {
                if let Ok(n) = after[..end].trim().parse::<u32>() {
                    count += n;
                }
                remaining = &after[end + 1..];
            } else {
                break;
            }
        } else {
            count += remaining.split_whitespace().count() as u32;
            break;
        }
    }
    count
}

fn section_word_count_for_line(buf: &sourceview5::Buffer, cursor_line: i32) -> Option<u32> {
    let total = buf.line_count();
    let mut sec_start = -1i32;
    let mut sec_level = 0usize;
    for ln in (0..=cursor_line).rev() {
        let start = buf.iter_at_line(ln)?;
        let mut end = start.clone();
        if !end.ends_line() { end.forward_to_line_end(); }
        let text = buf.text(&start, &end, false).to_string();
        if let Some(lvl) = section_heading_level(&text) {
            sec_start = ln;
            sec_level = lvl;
            break;
        }
    }
    if sec_start < 0 { return None; }
    let mut sec_end = total;
    for ln in (sec_start + 1)..total {
        let start = buf.iter_at_line(ln)?;
        let mut end = start.clone();
        if !end.ends_line() { end.forward_to_line_end(); }
        let text = buf.text(&start, &end, false).to_string();
        if let Some(lvl) = section_heading_level(&text) {
            if lvl <= sec_level { sec_end = ln; break; }
        }
    }
    let mut words = 0u32;
    for ln in sec_start..sec_end {
        let start = buf.iter_at_line(ln)?;
        let mut end = start.clone();
        if !end.ends_line() { end.forward_to_line_end(); }
        let text = buf.text(&start, &end, false).to_string();
        words += count_words_typst(&text);
    }
    Some(words)
}
