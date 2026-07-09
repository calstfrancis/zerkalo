use std::cell::RefCell;
use std::fmt::Write as FmtWrite;
use std::path::PathBuf;
use std::rc::Rc;

use chrono::Local;


use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Label, Notebook, Orientation, Overlay, Picture, PolicyType,
    PositionType, ScrolledWindow, Separator, Spinner, Switch,
};
use gtk4::glib;
use libadwaita as adw;
use adw::prelude::*;

type OnCreateCb = Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>;
type OnApplyCb  = Rc<RefCell<Option<Box<dyn Fn(String, SidecarSettings)>>>>;
type OnCvElementsCb = Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>;

// ── Static data tables ────────────────────────────────────────────────────────

const CITATION_STYLES: &[(&str, &str)] = &[
    ("SBL", "sbl"),
    ("Chicago (Notes-Bib)", "chicago-notes"),
    ("Chicago (Author-Date)", "chicago-author-date"),
    ("MLA", "mla"),
    ("APA 7th", "apa"),
    ("ASA", "asa"),
    ("Turabian", "turabian"),
    ("Harvard", "harvard"),
    ("IEEE", "ieee"),
    ("GOST R 7.0-5 (numeric)", "gost-r-705"),
    ("Vancouver", "vancouver"),
    ("LaTeX Look", "latex"),
];

const PAPER_SIZES: &[(&str, &str)] = &[
    ("US Letter", "us-letter"),
    ("A4", "a4"),
    ("A5", "a5"),
    ("Legal", "us-legal"),
    ("Executive", "executive"),
    ("Custom…", "custom"),
];

const MARGIN_PRESETS: &[&str] = &[
    "Normal (1\" / 1.25\")",
    "Narrow (0.5\" all)",
    "Wide (1\" / 2\")",
    "LaTeX (1.75\" all)",
    "Ross (1.25\" / 33% right)",
    "Custom…",
];

const PAGE_NUM_OPTIONS: &[&str] = &[
    "Bottom center",
    "Bottom right",
    "Top center",
    "Top right",
    "None",
];

const HEADER_OPTIONS: &[&str] = &[
    "None",
    "Title",
    "Author",
    "Current section",
    "Title · Author",
    "Title · Section",
    "Author · Section",
    "Author · Title",
];

const SPACING_OPTIONS: &[(&str, &str)] = &[
    ("Single", "0.65em"),
    ("1.5 Lines", "0.9em"),
    ("Double", "1.2em"),
];

// (display label, Typst color value used as the dropcap's `fill:` argument)
const DROPCAP_COLORS: &[(&str, &str)] = &[
    ("Ink Black (default)", ""),
    ("Vermilion Red", "rgb(\"#a3231f\")"),
    ("Lapis Blue", "rgb(\"#1e3a6e\")"),
    ("Illuminated Gold", "rgb(\"#b8860b\")"),
    ("Verdigris Green", "rgb(\"#2f6d5c\")"),
];

// (display label, Typst numbering pattern)
const NUMBERING_FORMATS: &[(&str, &str)] = &[
    ("Decimal  1.  1.1.  1.1.1.", "1."),
    ("IEEE Roman  I.  I.A.  I.A.1.", "I.A.1."),
    ("Alpha  a.  a.a.  a.a.a.", "a."),
];

const ACADEMIC_FONTS: &[&str] = &[
    "Times New Roman",
    "Libertinus Serif",
    "EB Garamond",
    "Palatino",
    "Linux Libertine O",
    "GOST type B",
    "Monospace",
    "New Computer Modern",
    "Other…",
];

const LANGUAGES: &[(&str, &str, &str)] = &[
    ("lang_ru", "Russian", "Cyrillic — sets lang:\"ru\", hyphenation, date locale"),
    ("lang_he", "Hebrew", "RTL — sets dir:rtl and lang:\"he\" for the whole document"),
    ("lang_el", "Ancient Greek", "Polytonic Greek — sets lang:\"el\"; needs a Unicode Greek font"),
    ("lang_ja", "Japanese", "CJK — needs Noto Serif CJK JP (install noto-serif-cjk or equivalent)"),
    ("lang_sa", "Sanskrit / Devanagari", "Devanagari — needs Noto Serif Devanagari"),
    ("lang_bo", "Tibetan", "Tibetan — needs Noto Serif Tibetan"),
    ("lang_zh", "Chinese", "CJK — needs Noto Serif CJK SC (install noto-serif-cjk or equivalent)"),
];

const EXTRA_PACKAGES: &[(&str, &str, &str)] = &[
    ("pkg_droplet", "Droplet", "Large decorative first-letter (dropcap)"),
    ("pkg_codly", "Codly",
        "Enhanced code-block presentation — line numbers, syntax highlighting, and inline \
         annotations. Enabled once with #show: codly-init.with(); every code block after that \
         is styled automatically, and #codly(...) lets you tweak numbering, radius, and colors."),
    ("pkg_showybox", "Showybox",
        "Coloured, bordered callout boxes with optional titles, footers, and shadows. \
         Call #showybox(title: \"...\")[content] anywhere to wrap content in a styled box — \
         useful for asides, examples, or highlighted notes."),
    ("pkg_gentle", "Gentle Clues",
        "Predefined admonition blocks — note, tip, warning, important, and more — each with \
         its own icon and colour. Use #note[...], #tip[...], #warning[...] directly, or pass \
         title: \"...\" to override the heading."),
    ("pkg_tablex", "Tablex",
        "Advanced tables with merged cells (colspan/rowspan), repeating headers across pages, \
         and per-cell/line styling via #tablex(...), used like Typst's built-in #table() but \
         with finer control. Most of this was upstreamed into native tables in Typst 0.11+, so \
         plain #table() may already suffice."),
    ("pkg_marginalia", "Marginalia",
        "Configurable margin notes with smart positioning, plus matching wide-blocks. After \
         #show: marginalia.setup.with(...), use #note[...] for an annotation placed in the \
         margin, #wideblock[...] to let content spill into the margin, and #notefigure(...) \
         for a captioned figure positioned there."),
];

// ── Template presets ──────────────────────────────────────────────────────────

struct TemplatePreset {
    name: &'static str,
    description: &'static str,
    style_idx: u32,
    paper_idx: u32,
    margin_idx: u32,
    spacing_idx: u32,
    page_num_pos: u32,
    header_idx: u32,
    include_toc: bool,
    include_abstract: bool,
    include_keywords: bool,
    body_kind: BodyKind,
}

// Indices reference CITATION_STYLES, PAPER_SIZES, MARGIN_PRESETS, SPACING_OPTIONS, PAGE_NUM_OPTIONS, HEADER_OPTIONS.
const TEMPLATE_PRESETS: &[TemplatePreset] = &[
    TemplatePreset {
        name: "Generic Academic",
        description: "Chicago Notes-Bib · US Letter · normal margins · 1.5-line spacing · page numbers bottom center",
        style_idx: 1,   // Chicago (Notes-Bib)
        paper_idx: 0,   // US Letter
        margin_idx: 0,  // Normal
        spacing_idx: 1, // 1.5em
        page_num_pos: 0, // bottom center
        header_idx: 0,  // None — the plain baseline preset
        include_toc: false,
        include_abstract: false,
        include_keywords: false,
        body_kind: BodyKind::Academic,
    },
    TemplatePreset {
        name: "Research Article (APA)",
        description: "APA 7th · US Letter · double-spaced · running head · abstract & keywords",
        style_idx: 4,   // APA 7th
        paper_idx: 0,
        margin_idx: 0,
        spacing_idx: 2, // Double (2.0em)
        page_num_pos: 3, // top right
        header_idx: 1,  // Title — APA's running head
        include_toc: false,
        include_abstract: true,
        include_keywords: true,
        body_kind: BodyKind::Academic,
    },
    TemplatePreset {
        name: "GOST R 7.0-5 Technical Report",
        description: "A4 · GOST margins · 1.5-line · section header · ToC included",
        style_idx: 9,   // GOST R 7.0-5
        paper_idx: 1,   // A4
        margin_idx: 0,
        spacing_idx: 1, // 1.5em
        page_num_pos: 0, // bottom center
        header_idx: 3,  // Current section — matches a technical report's running reference
        include_toc: true,
        include_abstract: false,
        include_keywords: false,
        body_kind: BodyKind::Academic,
    },
    TemplatePreset {
        name: "IEEE Conference Paper",
        description: "IEEE · US Letter · narrow margins · single-spaced · two columns · abstract",
        style_idx: 8,   // IEEE
        paper_idx: 0,
        margin_idx: 1,  // Narrow
        spacing_idx: 0, // Single
        page_num_pos: 0, // bottom center
        header_idx: 0,  // None — two-column layout carries the visual identity
        include_toc: false,
        include_abstract: true,
        include_keywords: true,
        body_kind: BodyKind::Academic,
    },
    TemplatePreset {
        name: "Academic Letter",
        description: "Actual letter layout — date, recipient, salutation, signature block · US Letter · single-spaced, no page numbers or header",
        style_idx: 0,   // SBL (minimal heading impact)
        paper_idx: 0,
        margin_idx: 0,
        spacing_idx: 0, // Single
        page_num_pos: 4, // None
        header_idx: 0,  // None — a letter doesn't carry a running header
        include_toc: false,
        include_abstract: false,
        include_keywords: false,
        body_kind: BodyKind::Letter,
    },
    TemplatePreset {
        name: "Book / Long-form",
        description: "Chapter structure · TOC · wide margins · chapter-title header · Chicago footnotes",
        style_idx: 1,   // Chicago (Notes-Bib) — footnotes suit prose
        paper_idx: 0,   // US Letter
        margin_idx: 2,  // Wide (1" / 2")
        spacing_idx: 1, // 1.5em
        page_num_pos: 0, // bottom center
        header_idx: 3,  // Current section — tracks the chapter title down the page
        include_toc: true,
        include_abstract: false,
        include_keywords: false,
        body_kind: BodyKind::Book,
    },
    TemplatePreset {
        name: "CV — Modern",
        description: "Clean résumé with colour accents · A4 · compact margins",
        style_idx: 0,   // 0 = modern in CV context
        paper_idx: 1,   // A4
        margin_idx: 1,  // Narrow
        spacing_idx: 0, // Single
        page_num_pos: 4, // None
        header_idx: 0,  // unused — CVs don't route through header_block
        include_toc: false,
        include_abstract: false,
        include_keywords: false,
        body_kind: BodyKind::Cv,
    },
    TemplatePreset {
        name: "CV — Academic",
        description: "Traditional academic CV with ruled section headers · A4",
        style_idx: 1,   // 1 = academic in CV context
        paper_idx: 1,   // A4
        margin_idx: 0,  // Normal
        spacing_idx: 0, // Single
        page_num_pos: 0, // bottom center
        header_idx: 0,  // unused — CVs don't route through header_block
        include_toc: false,
        include_abstract: false,
        include_keywords: false,
        body_kind: BodyKind::Cv,
    },
    TemplatePreset {
        name: "LaTeX Look",
        description: "Computer Modern typography · wide 1.75\" margins · tight leading · author/title header · US Letter",
        style_idx: 11,  // LaTeX Look
        paper_idx: 0,   // US Letter
        margin_idx: 3,  // LaTeX (1.75" all)
        spacing_idx: 0, // ignored — LaTeX Look sets its own leading/spacing
        page_num_pos: 0, // bottom center
        header_idx: 7,  // Author · Title — evokes a classic LaTeX book/report running head
        include_toc: false,
        include_abstract: false,
        include_keywords: false,
        body_kind: BodyKind::Academic,
    },
    TemplatePreset {
        name: "CV — Classic",
        description: "Minimal timeless résumé · clean lines, no colour · A4",
        style_idx: 2,   // 2 = classic in CV context
        paper_idx: 1,   // A4
        margin_idx: 0,  // Normal
        spacing_idx: 0, // Single
        page_num_pos: 0, // bottom center
        header_idx: 0,  // unused — CVs don't route through header_block
        include_toc: false,
        include_abstract: false,
        include_keywords: false,
        body_kind: BodyKind::Cv,
    },
    TemplatePreset {
        name: "CV — Two-Column",
        description: "Minimalist, rule-free résumé · sidebar (Education, Skills & Awards) beside a main Experience column · A4",
        style_idx: 3,   // 3 = sidebar in CV context
        paper_idx: 1,   // A4
        margin_idx: 0,  // Normal (1.5cm x/y for CVs)
        spacing_idx: 0, // Single
        page_num_pos: 4, // None
        header_idx: 0,  // unused — CVs don't route through header_block
        include_toc: false,
        include_abstract: false,
        include_keywords: false,
        body_kind: BodyKind::Cv,
    },
];

// ── Body kind ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Default, PartialEq, Debug)]
enum BodyKind {
    #[default]
    Academic,
    Book,
    Cv,
    Letter,
}

// ── Settings struct ───────────────────────────────────────────────────────────

pub(crate) struct TemplateSettings {
    title: String,
    subtitle: String,
    author: String,
    affiliation: String,
    course: String,
    professor: String,
    date: String,
    style_idx: usize,
    paper_idx: usize,
    custom_paper_w: String,
    custom_paper_h: String,
    margin_idx: usize,
    custom_margin: String,
    font: String,
    font_size: String,
    spacing: String,
    page_num_pos: u32,
    header_style: u32,
    include_toc: bool,
    toc_depth: u32,
    include_abstract: bool,
    abstract_text: String,
    include_keywords: bool,
    keywords: String,
    heading_numbering: bool,
    numbering_format: String,
    languages: Vec<String>,
    packages: Vec<String>,
    dropcap_font: String,
    dropcap_lines: u32,
    dropcap_color: String,
    body_kind: BodyKind,
    bib_path: Option<PathBuf>,
}

/// Canonical settings persisted as `<stem>.zerkalo.toml` alongside each `.typ` file.
/// This is the single source of truth for "Update Template Settings" pre-fill.
#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub struct SidecarSettings {
    pub title:              String,
    pub subtitle:           String,
    pub author:             String,
    pub affiliation:        String,
    pub course:             String,
    #[serde(default)]
    pub professor:          String,
    pub date:               String,
    pub style:              String,
    pub font:               String,
    pub font_size:          String,
    pub paper:              String,
    #[serde(default)]
    pub custom_paper_w:     String,
    #[serde(default)]
    pub custom_paper_h:     String,
    pub margin:             u32,
    #[serde(default)]
    pub custom_margin:      String,
    pub spacing:            String,
    pub page_numbers:       u32,
    #[serde(default)]
    pub header_style:       u32,
    pub toc:                bool,
    pub toc_depth:          u32,
    pub abstract_enabled:   bool,
    pub abstract_text:      String,
    pub keywords_enabled:   bool,
    pub keywords_text:      String,
    pub heading_numbering:  bool,
    pub numbering_format:   String,
    pub languages:          Vec<String>,
    pub packages:           Vec<String>,
    #[serde(default)]
    pub dropcap_font:       String,
    #[serde(default = "default_dropcap_lines")]
    pub dropcap_lines:      u32,
    #[serde(default)]
    pub dropcap_color:      String,
    pub bib_path:           Option<String>,
    pub body_kind:          String,
}

// ── Dialog ────────────────────────────────────────────────────────────────────

type OnLockCb = Rc<RefCell<Option<Box<dyn Fn(String, String)>>>>;
type OnAdvancedToggleCb = Rc<RefCell<Option<Box<dyn Fn(bool)>>>>;

pub struct TemplateDialog {
    window: adw::Window,
    on_create: OnCreateCb,
    on_apply: OnApplyCb,
    on_lock_identity: OnLockCb,
    on_advanced_toggle: OnAdvancedToggleCb,
    apply_btn: Button,
    style_row: adw::ComboRow,
    font_row: adw::ComboRow,
    paper_row: adw::ComboRow,
    custom_paper_w_row: adw::SpinRow,
    custom_paper_h_row: adw::SpinRow,
    margin_row: adw::ComboRow,
    custom_margin_row: adw::SpinRow,
    spacing_row: adw::ComboRow,
    toc_row: adw::SwitchRow,
    toc_depth_row: adw::ComboRow,
    abstract_row: adw::SwitchRow,
    abstract_text_row: adw::EntryRow,
    keywords_row: adw::SwitchRow,
    keywords_text_row: adw::EntryRow,
    heading_numbering_row: adw::SwitchRow,
    heading_format_row: adw::ComboRow,
    font_size_row: adw::ComboRow,
    custom_font_size_row: adw::SpinRow,
    // metadata fields
    title_row: adw::EntryRow,
    subtitle_row: adw::EntryRow,
    author_row: adw::EntryRow,
    affil_row: adw::EntryRow,
    course_row: adw::EntryRow,
    professor_row: adw::EntryRow,
    date_row: adw::EntryRow,
    bib_path: Rc<RefCell<Option<PathBuf>>>,
    pnum_row: adw::ComboRow,
    header_row: adw::ComboRow,
    lang_switches: Vec<(String, adw::SwitchRow)>,
    pkg_switches: Vec<(String, adw::SwitchRow)>,
    dropcap_expander: adw::ExpanderRow,
    dropcap_font_row: adw::ComboRow,
    dropcap_lines_row: adw::ComboRow,
    dropcap_color_row: adw::ComboRow,
    cv_switch: Switch,
    cv_elements_row: adw::EntryRow,
    cv_elements_path: Rc<RefCell<Option<PathBuf>>>,
    on_cv_elements_change: OnCvElementsCb,
}

impl TemplateDialog {
    pub fn new(parent: &impl IsA<gtk4::Window>, work_dir: &std::path::Path, _last_used_advanced: bool) -> Self {
        let window = adw::Window::builder()
            .title("New from Template")
            .transient_for(parent)
            .modal(true)
            .default_width(1240)
            .default_height(700)
            .build();

        let on_create: OnCreateCb = Rc::new(RefCell::new(None));
        let on_apply: OnApplyCb = Rc::new(RefCell::new(None));
        let on_lock_identity: OnLockCb = Rc::new(RefCell::new(None));
        let on_advanced_toggle: OnAdvancedToggleCb = Rc::new(RefCell::new(None));
        let on_cv_elements_change: OnCvElementsCb = Rc::new(RefCell::new(None));
        let cv_elements_path: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));

        let header = adw::HeaderBar::new();
        let cancel_btn = Button::with_label("Cancel");
        cancel_btn.add_css_class("flat");
        header.pack_start(&cancel_btn);
        let preview_code_btn = Button::with_label("Preview Code…");
        preview_code_btn.add_css_class("flat");
        preview_code_btn.set_tooltip_text(Some("Preview the Typst preamble that will be generated"));
        header.pack_start(&preview_code_btn);
        // pack_end calls apply in reverse visual order, so this ends up
        // leftmost of the end-aligned group: [CV toggle] [Apply/Create]
        let create_btn = Button::with_label("Create Document");
        create_btn.add_css_class("suggested-action");
        create_btn.add_css_class("pill");
        header.pack_end(&create_btn);
        let apply_btn = Button::with_label("Apply to Current");
        apply_btn.add_css_class("suggested-action");
        apply_btn.add_css_class("pill");
        apply_btn.set_visible(false);
        header.pack_end(&apply_btn);

        // Compact CV Mode toggle — just the label and switch side by side,
        // not a separate full-width bar with the two ends of the window apart.
        let cv_switch = Switch::new();
        cv_switch.set_valign(Align::Center);
        cv_switch.set_tooltip_text(Some("Show only CV templates and CV-relevant settings"));
        let cv_title_lbl = Label::new(Some("CV"));
        let cv_toggle_box = GtkBox::new(Orientation::Horizontal, 6);
        cv_toggle_box.set_valign(Align::Center);
        cv_toggle_box.append(&cv_title_lbl);
        cv_toggle_box.append(&cv_switch);
        header.pack_end(&cv_toggle_box);

        let notebook = Notebook::new();
        notebook.set_tab_pos(PositionType::Left);
        notebook.set_vexpand(true);

        // ── Tab 1: Document ──────────────────────────────────────────────────
        let meta_group = adw::PreferencesGroup::new();
        meta_group.set_title("Metadata");

        let title_row = adw::EntryRow::new();
        title_row.set_title("Title");
        meta_group.add(&title_row);

        let subtitle_row = adw::EntryRow::new();
        subtitle_row.set_title("Subtitle");
        meta_group.add(&subtitle_row);

        let author_row = adw::EntryRow::new();
        author_row.set_title("Author");
        let author_pin = Button::from_icon_name("changes-prevent-symbolic");
        author_pin.add_css_class("flat");
        author_pin.set_tooltip_text(Some("Save as default for new documents"));
        author_row.add_suffix(&author_pin);
        meta_group.add(&author_row);

        let affil_row = adw::EntryRow::new();
        affil_row.set_title("Affiliation");
        let affil_pin = Button::from_icon_name("changes-prevent-symbolic");
        affil_pin.add_css_class("flat");
        affil_pin.set_tooltip_text(Some("Save as default for new documents"));
        affil_row.add_suffix(&affil_pin);
        meta_group.add(&affil_row);

        let course_row = adw::EntryRow::new();
        course_row.set_title("Course / Context");
        meta_group.add(&course_row);

        let professor_row = adw::EntryRow::new();
        professor_row.set_title("Professor / Instructor");
        meta_group.add(&professor_row);

        let date_row = adw::EntryRow::new();
        date_row.set_title("Date");
        date_row.set_tooltip_text(Some("Leave blank to use today's date automatically"));
        meta_group.add(&date_row);

        let style_group = adw::PreferencesGroup::new();
        style_group.set_title("Citation & Heading Style");

        let style_labels: Vec<&str> = CITATION_STYLES.iter().map(|(n, _)| *n).collect();
        let style_model = gtk4::StringList::new(&style_labels);
        let style_row = adw::ComboRow::new();
        style_row.set_title("Style");
        style_row.set_subtitle("Sets heading formatting and bibliography output");
        style_row.set_model(Some(&style_model));
        style_row.set_selected(0);
        style_group.add(&style_row);

        let tab1_box = pref_tab_box();
        tab1_box.append(&meta_group);
        tab1_box.append(&style_group);
        notebook.append_page(&tab_scroll(tab1_box), Some(&tab_label("Document")));

        // ── Tab 2: Layout ────────────────────────────────────────────────────
        let page_group = adw::PreferencesGroup::new();
        page_group.set_title("Page");

        let paper_labels: Vec<&str> = PAPER_SIZES.iter().map(|(n, _)| *n).collect();
        let paper_model = gtk4::StringList::new(&paper_labels);
        let paper_row = adw::ComboRow::new();
        paper_row.set_title("Paper Size");
        paper_row.set_model(Some(&paper_model));
        paper_row.set_selected(0);
        page_group.add(&paper_row);

        let custom_paper_w_row = adw::SpinRow::with_range(50.0, 1200.0, 1.0);
        custom_paper_w_row.set_title("Custom Width (mm)");
        custom_paper_w_row.set_value(210.0);
        custom_paper_w_row.set_visible(false);
        page_group.add(&custom_paper_w_row);

        let custom_paper_h_row = adw::SpinRow::with_range(50.0, 1200.0, 1.0);
        custom_paper_h_row.set_title("Custom Height (mm)");
        custom_paper_h_row.set_value(297.0);
        custom_paper_h_row.set_visible(false);
        page_group.add(&custom_paper_h_row);

        let paper_row_c = paper_row.clone();
        let cpw = custom_paper_w_row.clone();
        let cph = custom_paper_h_row.clone();
        let custom_paper_idx = (PAPER_SIZES.len() - 1) as u32;
        paper_row_c.connect_selected_notify(move |r| {
            let is_custom = r.selected() == custom_paper_idx;
            cpw.set_visible(is_custom);
            cph.set_visible(is_custom);
        });

        let margin_model = gtk4::StringList::new(MARGIN_PRESETS);
        let margin_row = adw::ComboRow::new();
        margin_row.set_title("Margins");
        margin_row.set_model(Some(&margin_model));
        margin_row.set_selected(0);
        page_group.add(&margin_row);

        let custom_margin_row = adw::SpinRow::with_range(0.1, 5.0, 0.05);
        custom_margin_row.set_title("Custom Margin (in, all sides)");
        custom_margin_row.set_digits(2);
        custom_margin_row.set_value(1.0);
        custom_margin_row.set_visible(false);
        page_group.add(&custom_margin_row);

        let margin_row_c = margin_row.clone();
        let cmr = custom_margin_row.clone();
        let custom_margin_idx = (MARGIN_PRESETS.len() - 1) as u32;
        margin_row_c.connect_selected_notify(move |r| cmr.set_visible(r.selected() == custom_margin_idx));

        let pnum_model = gtk4::StringList::new(PAGE_NUM_OPTIONS);
        let pnum_row = adw::ComboRow::new();
        pnum_row.set_title("Page Numbers");
        pnum_row.set_model(Some(&pnum_model));
        pnum_row.set_selected(0);
        page_group.add(&pnum_row);

        let header_model = gtk4::StringList::new(HEADER_OPTIONS);
        let header_row = adw::ComboRow::new();
        header_row.set_title("Running Header");
        header_row.set_model(Some(&header_model));
        header_row.set_selected(0);
        page_group.add(&header_row);

        let typo_group = adw::PreferencesGroup::new();
        typo_group.set_title("Typography");

        let available_fonts = build_font_list();
        let font_labels: Vec<&str> = available_fonts.iter().map(|s| s.as_str()).collect();
        let font_model = gtk4::StringList::new(&font_labels);
        let font_row = adw::ComboRow::new();
        font_row.set_title("Body Font");
        font_row.set_model(Some(&font_model));
        let default_font_idx = available_fonts.iter().position(|f| f == "Times New Roman")
            .unwrap_or(0) as u32;
        font_row.set_selected(default_font_idx);
        typo_group.add(&font_row);

        let custom_font_row = adw::EntryRow::new();
        custom_font_row.set_title("Custom Font Name");
        custom_font_row.set_visible(false);
        typo_group.add(&custom_font_row);

        let font_row_c = font_row.clone();
        let cfr = custom_font_row.clone();
        let font_count = available_fonts.len();
        let other_idx = (font_count - 1) as u32;
        font_row_c.connect_selected_notify(move |r| cfr.set_visible(r.selected() == other_idx));

        let font_size_model = gtk4::StringList::new(&["10 pt", "11 pt", "12 pt", "14 pt", "Custom…"]);
        let font_size_row = adw::ComboRow::new();
        font_size_row.set_title("Font Size");
        font_size_row.set_model(Some(&font_size_model));
        font_size_row.set_selected(2); // 12pt default
        typo_group.add(&font_size_row);

        let custom_font_size_row = adw::SpinRow::with_range(6.0, 72.0, 1.0);
        custom_font_size_row.set_title("Custom Size (pt)");
        custom_font_size_row.set_value(12.0);
        custom_font_size_row.set_visible(false);
        typo_group.add(&custom_font_size_row);

        let font_size_row_c = font_size_row.clone();
        let cfs = custom_font_size_row.clone();
        const CUSTOM_FONT_SIZE_IDX: u32 = 4;
        font_size_row_c.connect_selected_notify(move |r| cfs.set_visible(r.selected() == CUSTOM_FONT_SIZE_IDX));

        let spacing_labels: Vec<&str> = SPACING_OPTIONS.iter().map(|(n, _)| *n).collect();
        let spacing_model = gtk4::StringList::new(&spacing_labels);
        let spacing_row = adw::ComboRow::new();
        spacing_row.set_title("Line Spacing");
        spacing_row.set_model(Some(&spacing_model));
        spacing_row.set_selected(1);
        typo_group.add(&spacing_row);

        let tab2_box = pref_tab_box();
        tab2_box.append(&page_group);
        tab2_box.append(&typo_group);
        notebook.append_page(&tab_scroll(tab2_box), Some(&tab_label("Layout")));

        // ── Tab 3: Sections ──────────────────────────────────────────────────
        let sec_group = adw::PreferencesGroup::new();
        sec_group.set_title("Document Sections");

        let toc_row = adw::SwitchRow::new();
        toc_row.set_title("Table of Contents");
        toc_row.set_active(false);
        sec_group.add(&toc_row);

        let toc_depth_labels = gtk4::StringList::new(&["1 level", "2 levels", "3 levels"]);
        let toc_depth_row = adw::ComboRow::new();
        toc_depth_row.set_title("ToC Depth");
        toc_depth_row.set_model(Some(&toc_depth_labels));
        toc_depth_row.set_selected(1);
        toc_depth_row.set_sensitive(false);
        sec_group.add(&toc_depth_row);

        {
            let dr = toc_depth_row.clone();
            toc_row.connect_active_notify(move |r| dr.set_sensitive(r.is_active()));
        }

        let abstract_row = adw::SwitchRow::new();
        abstract_row.set_title("Abstract");
        abstract_row.set_active(false);
        sec_group.add(&abstract_row);

        let abstract_text_row = adw::EntryRow::new();
        abstract_text_row.set_title("Abstract Text");
        abstract_text_row.set_visible(false);
        sec_group.add(&abstract_text_row);

        {
            let atr = abstract_text_row.clone();
            abstract_row.connect_active_notify(move |r| atr.set_visible(r.is_active()));
        }

        let keywords_row = adw::SwitchRow::new();
        keywords_row.set_title("Keywords Line");
        keywords_row.set_active(false);
        sec_group.add(&keywords_row);

        let keywords_text_row = adw::EntryRow::new();
        keywords_text_row.set_title("Keywords (comma-separated)");
        keywords_text_row.set_visible(false);
        sec_group.add(&keywords_text_row);

        {
            let ktr = keywords_text_row.clone();
            keywords_row.connect_active_notify(move |r| ktr.set_visible(r.is_active()));
        }

        let heading_numbering_row = adw::SwitchRow::new();
        heading_numbering_row.set_title("Numbered Headings");
        heading_numbering_row.set_subtitle("e.g. 1. Introduction, 1.1 Background");
        heading_numbering_row.set_active(false);
        sec_group.add(&heading_numbering_row);

        let heading_format_row = adw::ComboRow::new();
        heading_format_row.set_title("Numbering Format");
        heading_format_row.set_model(Some(&gtk4::StringList::new(
            &NUMBERING_FORMATS.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
        )));
        heading_format_row.set_visible(false);
        sec_group.add(&heading_format_row);

        // Show/hide format row when numbering is toggled
        {
            let hfr = heading_format_row.clone();
            heading_numbering_row.connect_active_notify(move |sw| {
                hfr.set_visible(sw.is_active());
            });
        }

        let tab3_box = pref_tab_box();
        tab3_box.append(&sec_group);
        let tab3_scroll = tab_scroll(tab3_box);
        notebook.append_page(&tab3_scroll, Some(&tab_label("Sections")));

        // ── Tab 4: Languages ─────────────────────────────────────────────────
        let lang_group = adw::PreferencesGroup::new();
        lang_group.set_title("Language Support");
        lang_group.set_description(Some(
            "Enable the scripts used in your document. Each loads the correct \
             font hint and text-direction setting.",
        ));

        let mut lang_switches: Vec<(String, adw::SwitchRow)> = Vec::new();
        for (key, name, desc) in LANGUAGES {
            let sw = adw::SwitchRow::new();
            sw.set_title(name);
            sw.set_subtitle(desc);
            sw.set_active(false);
            lang_group.add(&sw);
            lang_switches.push((key.to_string(), sw));
        }

        let tab4_box = pref_tab_box();
        tab4_box.append(&lang_group);
        notebook.append_page(&tab_scroll(tab4_box), Some(&tab_label("Languages")));

        // ── Tab 5: Packages ──────────────────────────────────────────────────
        let pkg_group = adw::PreferencesGroup::new();
        pkg_group.set_title("Extra Packages");
        pkg_group.set_description(Some(
            "Adds #import statements to the generated template. \
             You can add more packages manually at any time.",
        ));

        let mut pkg_switches: Vec<(String, adw::SwitchRow)> = Vec::new();

        // ── Droplet: ExpanderRow with font + lines children ───────────────────
        let dropcap_expander = adw::ExpanderRow::new();
        dropcap_expander.set_title("Droplet");
        dropcap_expander.set_subtitle(
            "Large decorative first-letter for an opening paragraph. Wraps it automatically \
             around the rest of the paragraph's text — no markup needed beyond enabling it here."
        );
        dropcap_expander.set_subtitle_lines(0);
        dropcap_expander.set_show_enable_switch(true);
        dropcap_expander.set_enable_expansion(false);
        pkg_group.add(&dropcap_expander);

        let droplet_hidden_sw = adw::SwitchRow::new();
        droplet_hidden_sw.set_active(false);
        {
            let sw_c = droplet_hidden_sw.clone();
            dropcap_expander.connect_notify_local(Some("enable-expansion"), move |exp, _| {
                sw_c.set_active(exp.enables_expansion());
            });
        }
        pkg_switches.push(("pkg_droplet".to_string(), droplet_hidden_sw));

        let dropcap_font_list: Vec<String> = {
            let mut v = vec!["(use body font)".to_string()];
            v.extend(build_font_list().into_iter().filter(|f| f != "Other…"));
            v
        };
        let dropcap_font_labels: Vec<&str> = dropcap_font_list.iter().map(|s| s.as_str()).collect();
        let dropcap_font_row = adw::ComboRow::new();
        dropcap_font_row.set_title("Font");
        dropcap_font_row.set_subtitle("Decorative font for the large first letter");
        dropcap_font_row.set_model(Some(&gtk4::StringList::new(&dropcap_font_labels)));
        dropcap_font_row.set_selected(0);
        dropcap_expander.add_row(&dropcap_font_row);

        let dropcap_lines_row = adw::ComboRow::new();
        dropcap_lines_row.set_title("Height");
        dropcap_lines_row.set_subtitle("How many lines tall the dropcap should be");
        dropcap_lines_row.set_model(Some(&gtk4::StringList::new(&["2 lines", "3 lines", "4 lines", "5 lines", "6 lines"])));
        dropcap_lines_row.set_selected(1); // 3 lines default
        dropcap_expander.add_row(&dropcap_lines_row);

        let dropcap_color_labels: Vec<&str> = DROPCAP_COLORS.iter().map(|(l, _)| *l).collect();
        let dropcap_color_row = adw::ComboRow::new();
        dropcap_color_row.set_title("Color");
        dropcap_color_row.set_subtitle("Ink color for the large first letter");
        dropcap_color_row.set_model(Some(&gtk4::StringList::new(&dropcap_color_labels)));
        dropcap_color_row.set_selected(0);
        dropcap_expander.add_row(&dropcap_color_row);

        // ── Other extra packages ──────────────────────────────────────────────
        for (key, name, desc) in EXTRA_PACKAGES.iter().filter(|(k, _, _)| *k != "pkg_droplet") {
            let sw = adw::SwitchRow::new();
            sw.set_title(name);
            sw.set_subtitle(desc);
            sw.set_subtitle_lines(0);
            sw.set_active(false);
            pkg_group.add(&sw);
            pkg_switches.push((key.to_string(), sw));
        }

        let tab5_box = pref_tab_box();
        tab5_box.append(&pkg_group);
        let tab5_scroll = tab_scroll(tab5_box);
        notebook.append_page(&tab5_scroll, Some(&tab_label("Packages")));

        // Tracks which body kind was most recently chosen via the gallery
        let body_kind_state: Rc<RefCell<BodyKind>> =
            Rc::new(RefCell::new(BodyKind::Academic));

        // Every gallery row alongside its preset's body kind, so CV Mode can
        // filter which rows are visible without rebuilding the gallery.
        let gallery_rows: Rc<RefCell<Vec<(adw::ActionRow, BodyKind)>>> =
            Rc::new(RefCell::new(Vec::new()));

        // ── Skrizhal CV Elements group — lives in the Template tab's left
        // column alongside the preset list, shown only in CV Mode, instead of
        // a separate bar that cramped the rest of the dialog.
        let cv_elements_group = adw::PreferencesGroup::new();
        cv_elements_group.set_title("Skrizhal CV Elements");
        cv_elements_group.set_description(Some(
            "A Skrizhal YAML file of jobs, degrees, awards, etc. — used to fill in this CV \
             instead of a bibliography.",
        ));
        cv_elements_group.set_visible(false);

        let cv_elements_row = adw::EntryRow::new();
        cv_elements_row.set_title("Skrizhal file");
        let cv_browse_btn = Button::from_icon_name("document-open-symbolic");
        cv_browse_btn.set_valign(Align::Center);
        cv_browse_btn.add_css_class("flat");
        cv_elements_row.add_suffix(&cv_browse_btn);
        cv_elements_group.add(&cv_elements_row);

        {
            let row_c = cv_elements_row.clone();
            let win_c = window.clone();
            let path_c = cv_elements_path.clone();
            let on_change_c = on_cv_elements_change.clone();
            cv_browse_btn.connect_clicked(move |_| {
                let row2 = row_c.clone();
                let path2 = path_c.clone();
                let on_change2 = on_change_c.clone();
                let fd = gtk4::FileDialog::new();
                let filter = gtk4::FileFilter::new();
                filter.set_name(Some("YAML files (*.yaml, *.yml)"));
                filter.add_pattern("*.yaml");
                filter.add_pattern("*.yml");
                let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
                filters.append(&filter);
                fd.set_filters(Some(&filters));
                fd.open(Some(&win_c), None::<&gtk4::gio::Cancellable>, move |result| {
                    if let Ok(file) = result {
                        if let Some(path) = file.path() {
                            row2.set_text(path.to_str().unwrap_or(""));
                            *path2.borrow_mut() = Some(path.clone());
                            if let Some(f) = on_change2.borrow().as_ref() { f(path); }
                        }
                    }
                });
            });
        }

        // ── Templates gallery ─────────────────────────────────────────────────
        let gallery_outer = {
            let gallery_outer = GtkBox::new(Orientation::Horizontal, 0);
            gallery_outer.set_hexpand(true);
            gallery_outer.set_vexpand(true);

            // Left: scrollable preset list
            let gallery_group = adw::PreferencesGroup::new();
            gallery_group.set_title("Starting Template");
            gallery_group.set_description(Some(
                "Click a preset to pre-fill the form and see a preview.",
            ));
            let left_box = pref_tab_box();
            left_box.append(&gallery_group);
            left_box.append(&cv_elements_group);
            let left_scroll = ScrolledWindow::new();
            left_scroll.set_width_request(300);
            left_scroll.set_vexpand(true);
            left_scroll.set_hscrollbar_policy(PolicyType::Never);
            left_scroll.set_child(Some(&left_box));
            gallery_outer.append(&left_scroll);
            gallery_outer.append(&Separator::new(Orientation::Vertical));

            // Right: preview pane
            let preview_overlay = Overlay::new();
            preview_overlay.set_hexpand(true);
            preview_overlay.set_vexpand(true);

            let preview_picture = Picture::new();
            preview_picture.set_hexpand(true);
            preview_picture.set_vexpand(true);
            preview_picture.set_can_shrink(true);
            preview_picture.set_content_fit(gtk4::ContentFit::Contain);
            preview_picture.set_margin_top(16);
            preview_picture.set_margin_bottom(16);
            preview_picture.set_margin_start(16);
            preview_picture.set_margin_end(16);
            preview_overlay.set_child(Some(&preview_picture));

            let preview_spinner = Spinner::new();
            preview_spinner.set_halign(Align::Center);
            preview_spinner.set_valign(Align::Center);
            preview_spinner.set_width_request(32);
            preview_spinner.set_height_request(32);
            preview_spinner.set_visible(false);
            preview_overlay.add_overlay(&preview_spinner);

            let hint_label = Label::new(Some("Select a template\nto preview it here"));
            hint_label.add_css_class("dim-label");
            hint_label.set_halign(Align::Center);
            hint_label.set_valign(Align::Center);
            hint_label.set_justify(gtk4::Justification::Center);
            preview_overlay.add_overlay(&hint_label);

            gallery_outer.append(&preview_overlay);

            // Form widget captures for preset application
            let g_style = style_row.clone();
            let g_paper = paper_row.clone();
            let g_margin = margin_row.clone();
            let g_spacing = spacing_row.clone();
            let g_pnum = pnum_row.clone();
            let g_header = header_row.clone();
            let g_toc = toc_row.clone();
            let g_abstract = abstract_row.clone();
            let g_keywords = keywords_row.clone();

            for (idx, preset) in TEMPLATE_PRESETS.iter().enumerate() {
                let row = adw::ActionRow::new();
                row.set_title(preset.name);
                row.set_subtitle(preset.description);
                row.set_activatable(true);
                row.add_suffix(&gtk4::Image::from_icon_name("go-next-symbolic"));

                let g_style_c = g_style.clone();
                let g_paper_c = g_paper.clone();
                let g_margin_c = g_margin.clone();
                let g_spacing_c = g_spacing.clone();
                let g_pnum_c = g_pnum.clone();
                let g_header_c = g_header.clone();
                let g_toc_c = g_toc.clone();
                let g_abstract_c = g_abstract.clone();
                let g_keywords_c = g_keywords.clone();
                let pic_c = preview_picture.clone();
                let spin_c = preview_spinner.clone();
                let hint_c = hint_label.clone();
                let bk_state_c = body_kind_state.clone();

                row.connect_activated(move |_| {
                    // Apply preset values to form
                    let p = &TEMPLATE_PRESETS[idx];
                    *bk_state_c.borrow_mut() = p.body_kind;
                    g_style_c.set_selected(p.style_idx);
                    g_paper_c.set_selected(p.paper_idx);
                    g_margin_c.set_selected(p.margin_idx);
                    g_spacing_c.set_selected(p.spacing_idx);
                    g_pnum_c.set_selected(p.page_num_pos);
                    g_header_c.set_selected(p.header_idx);
                    g_toc_c.set_active(p.include_toc);
                    g_abstract_c.set_active(p.include_abstract);
                    g_keywords_c.set_active(p.include_keywords);

                    // Kick off preview render
                    hint_c.set_visible(false);
                    pic_c.set_paintable(None::<&gtk4::gdk::Paintable>);
                    spin_c.set_visible(true);
                    spin_c.start();

                    let (tx, rx) = std::sync::mpsc::sync_channel::<Result<Vec<u8>, String>>(1);
                    std::thread::spawn(move || {
                        tx.send(generate_preset_preview(idx)).ok();
                    });

                    let rx = std::rc::Rc::new(rx);
                    let pic = pic_c.clone();
                    let spin = spin_c.clone();
                    glib::timeout_add_local(
                        std::time::Duration::from_millis(100),
                        move || {
                            use std::sync::mpsc::TryRecvError;
                            match rx.try_recv() {
                                Ok(Ok(png_bytes)) => {
                                    spin.stop();
                                    spin.set_visible(false);
                                    let bytes = glib::Bytes::from_owned(png_bytes);
                                    if let Ok(tex) = gtk4::gdk::Texture::from_bytes(&bytes) {
                                        pic.set_paintable(Some(
                                            tex.upcast_ref::<gtk4::gdk::Paintable>(),
                                        ));
                                    }
                                    glib::ControlFlow::Break
                                }
                                Ok(Err(_)) => {
                                    spin.stop();
                                    spin.set_visible(false);
                                    glib::ControlFlow::Break
                                }
                                Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
                                Err(TryRecvError::Disconnected) => {
                                    spin.stop();
                                    glib::ControlFlow::Break
                                }
                            }
                        },
                    );
                });

                gallery_group.add(&row);
                gallery_rows.borrow_mut().push((row, preset.body_kind));
            }

            // Auto-preview the first preset when the gallery opens
            if !TEMPLATE_PRESETS.is_empty() {
                let p = &TEMPLATE_PRESETS[0];
                *body_kind_state.borrow_mut() = p.body_kind;
                g_style.set_selected(p.style_idx);
                g_paper.set_selected(p.paper_idx);
                g_margin.set_selected(p.margin_idx);
                g_spacing.set_selected(p.spacing_idx);
                g_pnum.set_selected(p.page_num_pos);
                g_header.set_selected(p.header_idx);
                g_toc.set_active(p.include_toc);
                g_abstract.set_active(p.include_abstract);
                g_keywords.set_active(p.include_keywords);
                hint_label.set_visible(false);
                preview_spinner.set_visible(true);
                preview_spinner.start();
                let pic = preview_picture.clone();
                let spin = preview_spinner.clone();
                let (tx, rx) = std::sync::mpsc::sync_channel::<Result<Vec<u8>, String>>(1);
                std::thread::spawn(move || { tx.send(generate_preset_preview(0)).ok(); });
                let rx = std::rc::Rc::new(rx);
                glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                    use std::sync::mpsc::TryRecvError;
                    match rx.try_recv() {
                        Ok(Ok(png_bytes)) => {
                            spin.stop(); spin.set_visible(false);
                            let bytes = glib::Bytes::from_owned(png_bytes);
                            if let Ok(tex) = gtk4::gdk::Texture::from_bytes(&bytes) {
                                pic.set_paintable(Some(tex.upcast_ref::<gtk4::gdk::Paintable>()));
                            }
                            glib::ControlFlow::Break
                        }
                        Ok(Err(_)) => { spin.stop(); spin.set_visible(false); glib::ControlFlow::Break }
                        Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
                        Err(TryRecvError::Disconnected) => { spin.stop(); glib::ControlFlow::Break }
                    }
                });
            }

            gallery_outer
        };

        // ── Simple form group ────────────────────────────────────────────────
        // Gallery is Tab 0 — it fills the full window and has internal scrolling
        notebook.prepend_page(&gallery_outer, Some(&tab_label("Template")));
        notebook.set_hexpand(true);

        // ── Wire the toggle: filter gallery, hide Sections/Packages, reveal Skrizhal bar
        {
            let rows = gallery_rows.clone();
            let tab3 = tab3_scroll.clone();
            let tab5 = tab5_scroll.clone();
            let elements_group = cv_elements_group.clone();
            cv_switch.connect_active_notify(move |sw| {
                let cv_on = sw.is_active();
                for (row, kind) in rows.borrow().iter() {
                    row.set_visible(if cv_on { *kind == BodyKind::Cv } else { *kind != BodyKind::Cv });
                }
                tab3.set_visible(!cv_on);
                tab5.set_visible(!cv_on);
                elements_group.set_visible(cv_on);
            });
        }

        // ── Layout ───────────────────────────────────────────────────────────
        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&notebook));
        window.set_content(Some(&toolbar_view));

        // ── Button wiring ─────────────────────────────────────────────────────
        let win_cancel = window.clone();
        cancel_btn.connect_clicked(move |_| win_cancel.close());

        let bib_path: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));

        // Create: collect state → generate template → file dialog → write → callback
        let on_create_c = on_create.clone();
        let win_for_create = window.clone();
        let work_dir_for_create = work_dir.to_path_buf();

        // Capture all form widgets
        let w_title = title_row.clone();
        let w_subtitle = subtitle_row.clone();
        let w_author = author_row.clone();
        let w_affil = affil_row.clone();
        let w_course = course_row.clone();
        let w_professor = professor_row.clone();
        let w_date = date_row.clone();
        let w_style = style_row.clone();
        let w_paper = paper_row.clone();
        let w_custom_paper_w = custom_paper_w_row.clone();
        let w_custom_paper_h = custom_paper_h_row.clone();
        let w_margin = margin_row.clone();
        let w_custom_margin = custom_margin_row.clone();
        let w_font = font_row.clone();
        let w_custom_font = custom_font_row.clone();
        let w_font_size = font_size_row.clone();
        let w_custom_font_size = custom_font_size_row.clone();
        let w_spacing = spacing_row.clone();
        let w_pnum = pnum_row.clone();
        let w_header = header_row.clone();
        let w_toc = toc_row.clone();
        let w_toc_depth = toc_depth_row.clone();
        let w_abstract = abstract_row.clone();
        let w_abstract_text = abstract_text_row.clone();
        let w_keywords = keywords_row.clone();
        let w_keywords_text = keywords_text_row.clone();
        let w_heading_num = heading_numbering_row.clone();
        let w_heading_fmt = heading_format_row.clone();
        let w_langs = lang_switches.clone();
        let w_pkgs = pkg_switches.clone();
        let w_dropcap_font = dropcap_font_row.clone();
        let w_dropcap_lines = dropcap_lines_row.clone();
        let w_dropcap_color = dropcap_color_row.clone();
        let w_body_kind = body_kind_state.clone();
        let w_bib_path = bib_path.clone();

        create_btn.connect_clicked(move |_| {
            let font_idx = w_font.selected() as usize;
            let available_fonts_inner = build_font_list();
            let font = if font_idx >= available_fonts_inner.len().saturating_sub(1) {
                let s = w_custom_font.text().to_string();
                if s.is_empty() { "Times New Roman".to_string() } else { s }
            } else {
                available_fonts_inner.get(font_idx).cloned().unwrap_or_else(|| "Times New Roman".to_string())
            };

            let font_size = resolve_font_size(w_font_size.selected(), w_custom_font_size.value());

            let toc_depth = match w_toc_depth.selected() {
                0 => 1u32,
                2 => 3,
                _ => 2,
            };

            let settings = TemplateSettings {
                title: w_title.text().to_string(),
                subtitle: w_subtitle.text().to_string(),
                author: w_author.text().to_string(),
                affiliation: w_affil.text().to_string(),
                course: w_course.text().to_string(),
                professor: w_professor.text().to_string(),
                date: w_date.text().to_string(),
                style_idx: w_style.selected() as usize,
                paper_idx: w_paper.selected() as usize,
                custom_paper_w: (w_custom_paper_w.value() as i64).to_string(),
                custom_paper_h: (w_custom_paper_h.value() as i64).to_string(),
                margin_idx: w_margin.selected() as usize,
                custom_margin: format!("{:.2}", w_custom_margin.value()),
                font,
                font_size,
                spacing: SPACING_OPTIONS
                    .get(w_spacing.selected() as usize)
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_else(|| "1.5em".to_string()),
                page_num_pos: w_pnum.selected(),
                header_style: w_header.selected(),
                include_toc: w_toc.is_active(),
                toc_depth,
                include_abstract: w_abstract.is_active(),
                abstract_text: w_abstract_text.text().to_string(),
                include_keywords: w_keywords.is_active(),
                keywords: w_keywords_text.text().to_string(),
                heading_numbering: w_heading_num.is_active(),
                numbering_format: NUMBERING_FORMATS
                    .get(w_heading_fmt.selected() as usize)
                    .map(|(_, p)| p.to_string())
                    .unwrap_or_else(|| "1.".to_string()),
                languages: w_langs
                    .iter()
                    .filter(|(_, sw)| sw.is_active())
                    .map(|(k, _)| k.clone())
                    .collect(),
                packages: w_pkgs
                    .iter()
                    .filter(|(_, sw)| sw.is_active())
                    .map(|(k, _)| k.clone())
                    .collect(),
                dropcap_font: if w_pkgs.iter().any(|(k, sw)| k == "pkg_droplet" && sw.is_active()) {
                    let idx = w_dropcap_font.selected() as usize;
                    if idx == 0 { String::new() } else {
                        w_dropcap_font.model()
                            .and_then(|m| m.downcast::<gtk4::StringList>().ok())
                            .and_then(|sl| sl.string(idx as u32))
                            .map(|s| s.to_string())
                            .unwrap_or_default()
                    }
                } else {
                    String::new()
                },
                dropcap_lines: w_dropcap_lines.selected() + 2,
                dropcap_color: DROPCAP_COLORS
                    .get(w_dropcap_color.selected() as usize)
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_default(),
                body_kind: *w_body_kind.borrow(),
                bib_path: w_bib_path.borrow().clone(),
            };

            let content = generate_typst_template(&settings);
            let title_slug = slug(&settings.title);
            let sidecar = build_sidecar(&settings);

            let dialog = gtk4::FileDialog::new();
            dialog.set_title("Save New Document");
            dialog.set_initial_name(Some(&format!("{}.typ", title_slug)));
            dialog.set_initial_folder(Some(&gtk4::gio::File::for_path(&work_dir_for_create)));

            let win_c = win_for_create.clone();
            let cb = on_create_c.clone();
            dialog.save(
                Some(&win_for_create),
                None::<&gtk4::gio::Cancellable>,
                move |result| {
                    if let Ok(file) = result {
                        if let Some(path) = file.path() {
                            let _ = std::fs::write(&path, &content);
                            save_sidecar(&path, &sidecar);
                            if let Some(f) = cb.borrow().as_ref() {
                                f(path);
                            }
                        }
                    }
                    win_c.close();
                },
            );
        });

        // Apply: generate in-memory, fire on_apply(content) without file dialog
        let on_apply_c = on_apply.clone();
        let win_for_apply = window.clone();
        // Re-capture widget state (same set as create_btn, re-bound here)
        let a_title = title_row.clone();
        let a_subtitle = subtitle_row.clone();
        let a_author = author_row.clone();
        let a_affil = affil_row.clone();
        let a_course = course_row.clone();
        let a_professor = professor_row.clone();
        let a_date = date_row.clone();
        let a_style = style_row.clone();
        let a_paper = paper_row.clone();
        let a_custom_paper_w = custom_paper_w_row.clone();
        let a_custom_paper_h = custom_paper_h_row.clone();
        let a_margin = margin_row.clone();
        let a_custom_margin = custom_margin_row.clone();
        let a_font = font_row.clone();
        let a_custom_font = custom_font_row.clone();
        let a_font_size = font_size_row.clone();
        let a_custom_font_size = custom_font_size_row.clone();
        let a_spacing = spacing_row.clone();
        let a_pnum = pnum_row.clone();
        let a_header = header_row.clone();
        let a_toc = toc_row.clone();
        let a_toc_depth = toc_depth_row.clone();
        let a_abstract = abstract_row.clone();
        let a_abstract_text = abstract_text_row.clone();
        let a_keywords = keywords_row.clone();
        let a_keywords_text = keywords_text_row.clone();
        let a_heading_num = heading_numbering_row.clone();
        let a_heading_fmt = heading_format_row.clone();
        let a_langs = lang_switches.clone();
        let a_pkgs = pkg_switches.clone();
        let a_dropcap_font = dropcap_font_row.clone();
        let a_dropcap_lines = dropcap_lines_row.clone();
        let a_dropcap_color = dropcap_color_row.clone();
        let a_body_kind = body_kind_state.clone();
        let a_bib_path = bib_path.clone();
        apply_btn.connect_clicked(move |_| {
            let font_idx = a_font.selected() as usize;
            let available_fonts_inner = build_font_list();
            let font = if font_idx >= available_fonts_inner.len().saturating_sub(1) {
                let s = a_custom_font.text().to_string();
                if s.is_empty() { "Times New Roman".to_string() } else { s }
            } else {
                available_fonts_inner.get(font_idx).cloned().unwrap_or_else(|| "Times New Roman".to_string())
            };
            let font_size = resolve_font_size(a_font_size.selected(), a_custom_font_size.value());
            let toc_depth = match a_toc_depth.selected() { 0 => 1u32, 2 => 3, _ => 2 };
            let settings = TemplateSettings {
                title: a_title.text().to_string(),
                subtitle: a_subtitle.text().to_string(),
                author: a_author.text().to_string(),
                affiliation: a_affil.text().to_string(),
                course: a_course.text().to_string(),
                professor: a_professor.text().to_string(),
                date: a_date.text().to_string(),
                style_idx: a_style.selected() as usize,
                paper_idx: a_paper.selected() as usize,
                custom_paper_w: (a_custom_paper_w.value() as i64).to_string(),
                custom_paper_h: (a_custom_paper_h.value() as i64).to_string(),
                margin_idx: a_margin.selected() as usize,
                custom_margin: format!("{:.2}", a_custom_margin.value()),
                font,
                font_size,
                spacing: SPACING_OPTIONS
                    .get(a_spacing.selected() as usize)
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_else(|| "1.5em".to_string()),
                page_num_pos: a_pnum.selected(),
                header_style: a_header.selected(),
                include_toc: a_toc.is_active(),
                toc_depth,
                include_abstract: a_abstract.is_active(),
                abstract_text: a_abstract_text.text().to_string(),
                include_keywords: a_keywords.is_active(),
                keywords: a_keywords_text.text().to_string(),
                heading_numbering: a_heading_num.is_active(),
                numbering_format: NUMBERING_FORMATS
                    .get(a_heading_fmt.selected() as usize)
                    .map(|(_, p)| p.to_string())
                    .unwrap_or_else(|| "1.".to_string()),
                languages: a_langs.iter()
                    .filter(|(_, sw)| sw.is_active())
                    .map(|(k, _)| k.clone())
                    .collect(),
                packages: a_pkgs.iter()
                    .filter(|(_, sw)| sw.is_active())
                    .map(|(k, _)| k.clone())
                    .collect(),
                dropcap_font: if a_pkgs.iter().any(|(k, sw)| k == "pkg_droplet" && sw.is_active()) {
                    let idx = a_dropcap_font.selected() as usize;
                    if idx == 0 { String::new() } else {
                        a_dropcap_font.model()
                            .and_then(|m| m.downcast::<gtk4::StringList>().ok())
                            .and_then(|sl| sl.string(idx as u32))
                            .map(|s| s.to_string())
                            .unwrap_or_default()
                    }
                } else {
                    String::new()
                },
                dropcap_lines: a_dropcap_lines.selected() + 2,
                dropcap_color: DROPCAP_COLORS
                    .get(a_dropcap_color.selected() as usize)
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_default(),
                body_kind: *a_body_kind.borrow(),
                bib_path: a_bib_path.borrow().clone(),
            };
            let content = generate_typst_template(&settings);
            let sidecar = build_sidecar(&settings);
            if let Some(f) = on_apply_c.borrow().as_ref() {
                f(content, sidecar);
            }
            win_for_apply.close();
        });

        // ── Pin button wiring ─────────────────────────────────────────────────
        {
            let lock = on_lock_identity.clone();
            let ar = author_row.clone();
            let afr = affil_row.clone();
            author_pin.connect_clicked(move |_| {
                if let Some(f) = lock.borrow().as_ref() {
                    f(ar.text().to_string(), afr.text().to_string());
                }
            });
        }
        {
            let lock = on_lock_identity.clone();
            let ar = author_row.clone();
            let afr = affil_row.clone();
            affil_pin.connect_clicked(move |_| {
                if let Some(f) = lock.borrow().as_ref() {
                    f(ar.text().to_string(), afr.text().to_string());
                }
            });
        }

        // ── Preview Code button — generates the preamble and shows it read-only ─
        {
            let p_title = title_row.clone();
            let p_subtitle = subtitle_row.clone();
            let p_author = author_row.clone();
            let p_affil = affil_row.clone();
            let p_course = course_row.clone();
            let p_professor = professor_row.clone();
            let p_date = date_row.clone();
            let p_style = style_row.clone();
            let p_paper = paper_row.clone();
            let p_custom_paper_w = custom_paper_w_row.clone();
            let p_custom_paper_h = custom_paper_h_row.clone();
            let p_margin = margin_row.clone();
            let p_custom_margin = custom_margin_row.clone();
            let p_font = font_row.clone();
            let p_custom_font = custom_font_row.clone();
            let p_font_size = font_size_row.clone();
            let p_custom_font_size = custom_font_size_row.clone();
            let p_spacing = spacing_row.clone();
            let p_pnum = pnum_row.clone();
            let p_header = header_row.clone();
            let p_toc = toc_row.clone();
            let p_toc_depth = toc_depth_row.clone();
            let p_abstract = abstract_row.clone();
            let p_abstract_text = abstract_text_row.clone();
            let p_keywords = keywords_row.clone();
            let p_keywords_text = keywords_text_row.clone();
            let p_heading_num = heading_numbering_row.clone();
            let p_heading_fmt = heading_format_row.clone();
            let p_langs = lang_switches.clone();
            let p_pkgs = pkg_switches.clone();
            let p_dropcap_font = dropcap_font_row.clone();
            let p_dropcap_lines = dropcap_lines_row.clone();
            let p_dropcap_color = dropcap_color_row.clone();
            let p_body_kind = body_kind_state.clone();
            let p_bib_path = bib_path.clone();
            let p_win = window.clone();
            preview_code_btn.connect_clicked(move |_| {
                let font_idx = p_font.selected() as usize;
                let available_fonts_inner = build_font_list();
                let font = if font_idx >= available_fonts_inner.len().saturating_sub(1) {
                    let s = p_custom_font.text().to_string();
                    if s.is_empty() { "Times New Roman".to_string() } else { s }
                } else {
                    available_fonts_inner.get(font_idx).cloned().unwrap_or_else(|| "Times New Roman".to_string())
                };
                let font_size = resolve_font_size(p_font_size.selected(), p_custom_font_size.value());
                let toc_depth = match p_toc_depth.selected() { 0 => 1u32, 2 => 3, _ => 2 };
                let settings = TemplateSettings {
                    title: p_title.text().to_string(),
                    subtitle: p_subtitle.text().to_string(),
                    author: p_author.text().to_string(),
                    affiliation: p_affil.text().to_string(),
                    course: p_course.text().to_string(),
                    professor: p_professor.text().to_string(),
                    date: p_date.text().to_string(),
                    style_idx: p_style.selected() as usize,
                    paper_idx: p_paper.selected() as usize,
                    custom_paper_w: (p_custom_paper_w.value() as i64).to_string(),
                    custom_paper_h: (p_custom_paper_h.value() as i64).to_string(),
                    margin_idx: p_margin.selected() as usize,
                    custom_margin: format!("{:.2}", p_custom_margin.value()),
                    font,
                    font_size,
                    spacing: SPACING_OPTIONS
                        .get(p_spacing.selected() as usize)
                        .map(|(_, v)| v.to_string())
                        .unwrap_or_else(|| "1.5em".to_string()),
                    page_num_pos: p_pnum.selected(),
                    header_style: p_header.selected(),
                    include_toc: p_toc.is_active(),
                    toc_depth,
                    include_abstract: p_abstract.is_active(),
                    abstract_text: p_abstract_text.text().to_string(),
                    include_keywords: p_keywords.is_active(),
                    keywords: p_keywords_text.text().to_string(),
                    heading_numbering: p_heading_num.is_active(),
                    numbering_format: NUMBERING_FORMATS
                        .get(p_heading_fmt.selected() as usize)
                        .map(|(_, pat)| pat.to_string())
                        .unwrap_or_else(|| "1.".to_string()),
                    languages: p_langs.iter()
                        .filter(|(_, sw)| sw.is_active())
                        .map(|(k, _)| k.clone())
                        .collect(),
                    packages: p_pkgs.iter()
                        .filter(|(_, sw)| sw.is_active())
                        .map(|(k, _)| k.clone())
                        .collect(),
                    dropcap_font: if p_pkgs.iter().any(|(k, sw)| k == "pkg_droplet" && sw.is_active()) {
                        let idx = p_dropcap_font.selected() as usize;
                        if idx == 0 { String::new() } else {
                            p_dropcap_font.model()
                                .and_then(|m| m.downcast::<gtk4::StringList>().ok())
                                .and_then(|sl| sl.string(idx as u32))
                                .map(|s| s.to_string())
                                .unwrap_or_default()
                        }
                    } else {
                        String::new()
                    },
                    dropcap_lines: p_dropcap_lines.selected() + 2,
                    dropcap_color: DROPCAP_COLORS
                        .get(p_dropcap_color.selected() as usize)
                        .map(|(_, v)| v.to_string())
                        .unwrap_or_default(),
                    body_kind: *p_body_kind.borrow(),
                    bib_path: p_bib_path.borrow().clone(),
                };
                let code = generate_typst_template(&settings);

                // Show in a read-only window
                let pwin = adw::Window::new();
                pwin.set_title(Some("Generated Typst Code"));
                pwin.set_default_size(680, 560);
                pwin.set_transient_for(Some(&p_win));
                pwin.set_modal(false);

                let pheader = adw::HeaderBar::new();
                let close_btn = Button::with_label("Close");
                close_btn.add_css_class("flat");
                let pwin2 = pwin.clone();
                close_btn.connect_clicked(move |_| pwin2.close());
                pheader.pack_start(&close_btn);

                let tv = gtk4::TextView::new();
                tv.set_editable(false);
                tv.set_monospace(true);
                tv.set_left_margin(12);
                tv.set_right_margin(12);
                tv.set_top_margin(8);
                tv.set_bottom_margin(8);
                tv.buffer().set_text(&code);

                let scroll = ScrolledWindow::new();
                scroll.set_vexpand(true);
                scroll.set_hexpand(true);
                scroll.set_child(Some(&tv));

                let toolbar_view = adw::ToolbarView::new();
                toolbar_view.add_top_bar(&pheader);
                toolbar_view.set_content(Some(&scroll));
                pwin.set_content(Some(&toolbar_view));
                pwin.present();
            });
        }

        Self {
            window, on_create, on_apply, on_lock_identity, on_advanced_toggle, apply_btn,
            style_row, font_row, font_size_row, custom_font_size_row,
            paper_row, custom_paper_w_row, custom_paper_h_row,
            margin_row, custom_margin_row, spacing_row,
            toc_row, toc_depth_row, abstract_row, abstract_text_row,
            keywords_row, keywords_text_row, heading_numbering_row, heading_format_row,
            title_row, subtitle_row, author_row, affil_row, course_row, professor_row, date_row,
            bib_path, pnum_row, header_row, lang_switches, pkg_switches,
            dropcap_expander, dropcap_font_row, dropcap_lines_row, dropcap_color_row,
            cv_switch, cv_elements_row, cv_elements_path, on_cv_elements_change,
        }
    }

    pub fn set_bib_path(&self, path: Option<PathBuf>) {
        *self.bib_path.borrow_mut() = path;
    }

    /// Turns CV Mode on/off, which filters the gallery to CV-only (or
    /// non-CV-only) presets, hides the Sections/Packages tabs, and reveals
    /// the Skrizhal CV Elements selector.
    pub fn preselect_cv_mode(&self, active: bool) {
        self.cv_switch.set_active(active);
    }

    pub fn set_cv_elements_path(&self, path: Option<PathBuf>) {
        if let Some(ref p) = path {
            self.cv_elements_row.set_text(p.to_str().unwrap_or(""));
        }
        *self.cv_elements_path.borrow_mut() = path;
    }

    pub fn set_on_cv_elements_change(&self, f: impl Fn(PathBuf) + 'static) {
        *self.on_cv_elements_change.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_advanced_toggle(&self, f: impl Fn(bool) + 'static) {
        *self.on_advanced_toggle.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_create(&self, f: impl Fn(PathBuf) + 'static) {
        *self.on_create.borrow_mut() = Some(Box::new(f));
    }

    /// Register a callback that receives the generated template content and sidecar settings,
    /// without a file-save dialog. Also shows the "Apply to Current" button.
    pub fn set_on_apply(&self, f: impl Fn(String, SidecarSettings) + 'static) {
        *self.on_apply.borrow_mut() = Some(Box::new(f));
        self.apply_btn.set_visible(true);
        // Retitle the window to clarify intent
        self.window.set_title(Some("Update Template Settings"));
    }

    /// Pre-select a citation style by its internal key (e.g. "sbl", "apa").
    /// Also sets style-appropriate heading numbering defaults (overridable by sidecar).
    pub fn preselect_style(&self, style_key: &str) {
        for (i, (_, key)) in CITATION_STYLES.iter().enumerate() {
            if *key == style_key {
                self.style_row.set_selected(i as u32);
                break;
            }
        }
        match style_key {
            "ieee" => {
                self.preselect_heading_numbering(true);
                self.preselect_heading_format("I.A.1.");
            }
            "gost-r-705" | "vancouver" => {
                self.preselect_heading_numbering(true);
                self.preselect_heading_format("1.");
            }
            _ => {}
        }
    }

    pub fn preselect_dropcap_font(&self, font: &str) {
        if font.is_empty() {
            self.dropcap_font_row.set_selected(0);
            return;
        }
        if let Some(model) = self.dropcap_font_row.model()
            .and_then(|m| m.downcast::<gtk4::StringList>().ok())
        {
            for i in 0..model.n_items() {
                if model.string(i).map(|s| s.to_string()).as_deref() == Some(font) {
                    self.dropcap_font_row.set_selected(i);
                    return;
                }
            }
        }
        self.dropcap_font_row.set_selected(0);
    }

    pub fn preselect_dropcap_lines(&self, lines: u32) {
        let idx = lines.saturating_sub(2).min(4);
        self.dropcap_lines_row.set_selected(idx);
    }

    pub fn preselect_dropcap_color(&self, color: &str) {
        let idx = DROPCAP_COLORS.iter().position(|(_, v)| *v == color).unwrap_or(0);
        self.dropcap_color_row.set_selected(idx as u32);
    }

    /// Pre-select the body font by name.
    pub fn preselect_font(&self, font: &str) {
        let available = build_font_list();
        for (i, f) in available.iter().enumerate() {
            if f.eq_ignore_ascii_case(font) {
                self.font_row.set_selected(i as u32);
                return;
            }
        }
    }

    /// Pre-select paper size by its Typst key (e.g. "us-letter", "a4").
    pub fn preselect_paper(&self, paper_key: &str, custom_w: &str, custom_h: &str) {
        for (i, (_, key)) in PAPER_SIZES.iter().enumerate() {
            if *key == paper_key {
                self.paper_row.set_selected(i as u32);
                if paper_key == "custom" {
                    if let Ok(w) = custom_w.parse::<f64>() { self.custom_paper_w_row.set_value(w); }
                    if let Ok(h) = custom_h.parse::<f64>() { self.custom_paper_h_row.set_value(h); }
                }
                return;
            }
        }
    }

    /// Pre-select line spacing by its value string (e.g. "1.5em", "2.0em").
    pub fn preselect_spacing(&self, spacing_value: &str) {
        for (i, (_, val)) in SPACING_OPTIONS.iter().enumerate() {
            if *val == spacing_value {
                self.spacing_row.set_selected(i as u32);
                return;
            }
        }
    }

    /// Pre-select the margin preset by index (0=Normal, 1=Narrow, 2=Wide, 3=LaTeX, 4=Ross).
    pub fn preselect_margin(&self, idx: usize, custom_margin: &str) {
        if idx < MARGIN_PRESETS.len() {
            self.margin_row.set_selected(idx as u32);
            if idx == MARGIN_PRESETS.len() - 1 {
                if let Ok(v) = custom_margin.parse::<f64>() { self.custom_margin_row.set_value(v); }
            }
        }
    }

    /// Register a callback fired when the user clicks a pin button.
    /// Receives (author, affiliation) — save both to config.
    pub fn set_on_lock_identity(&self, f: impl Fn(String, String) + 'static) {
        *self.on_lock_identity.borrow_mut() = Some(Box::new(f));
    }

    /// Pre-fill author and affiliation from saved defaults (only if the field is currently empty).
    pub fn preselect_locked_identity(&self, author: &str, affiliation: &str) {
        if self.author_row.text().is_empty() && !author.is_empty() {
            self.author_row.set_text(author);
        }
        if self.affil_row.text().is_empty() && !affiliation.is_empty() {
            self.affil_row.set_text(affiliation);
        }
    }

    pub fn preselect_metadata(
        &self,
        title: &str,
        subtitle: &str,
        author: &str,
        affiliation: &str,
        course: &str,
        professor: &str,
        date: &str,
    ) {
        if !title.is_empty()       { self.title_row.set_text(title); }
        if !subtitle.is_empty()    { self.subtitle_row.set_text(subtitle); }
        if !author.is_empty()      { self.author_row.set_text(author); }
        if !affiliation.is_empty() { self.affil_row.set_text(affiliation); }
        if !course.is_empty()      { self.course_row.set_text(course); }
        if !professor.is_empty()   { self.professor_row.set_text(professor); }
        if !date.is_empty()        { self.date_row.set_text(date); }
    }

    pub fn preselect_toc(&self, active: bool, depth: u32) {
        self.toc_row.set_active(active);
        let idx = match depth { 1 => 0u32, 3 => 2, _ => 1 };
        self.toc_depth_row.set_selected(idx);
        self.toc_depth_row.set_sensitive(active);
    }

    pub fn preselect_abstract(&self, active: bool, text: &str) {
        self.abstract_row.set_active(active);
        if active && !text.is_empty() {
            self.abstract_text_row.set_text(text);
        }
        self.abstract_text_row.set_visible(active);
    }

    /// Pre-fill abstract text, overriding whatever the sidecar has. Used to
    /// populate the dialog from the text found directly in the .typ file.
    pub fn override_abstract_text(&self, text: &str) {
        if !text.is_empty() {
            self.abstract_text_row.set_text(text);
            self.abstract_row.set_active(true);
            self.abstract_text_row.set_visible(true);
        }
    }

    pub fn preselect_font_size(&self, size: &str) {
        let idx = match size {
            "10pt" => 0u32,
            "11pt" => 1,
            "12pt" => 2,
            "14pt" => 3,
            other => {
                let digits: String = other.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                if let Ok(v) = digits.parse::<f64>() {
                    self.custom_font_size_row.set_value(v);
                }
                4
            }
        };
        self.font_size_row.set_selected(idx);
    }

    pub fn preselect_heading_numbering(&self, active: bool) {
        self.heading_numbering_row.set_active(active);
        self.heading_format_row.set_visible(active);
    }

    pub fn preselect_heading_format(&self, format: &str) {
        for (i, (_, pat)) in NUMBERING_FORMATS.iter().enumerate() {
            if *pat == format {
                self.heading_format_row.set_selected(i as u32);
                return;
            }
        }
    }

    pub fn preselect_keywords(&self, active: bool, text: &str) {
        self.keywords_row.set_active(active);
        if active && !text.is_empty() {
            self.keywords_text_row.set_text(text);
        }
        self.keywords_text_row.set_visible(active);
    }

    pub fn preselect_page_numbers(&self, pos: u32) {
        if (pos as usize) < PAGE_NUM_OPTIONS.len() {
            self.pnum_row.set_selected(pos);
        }
    }

    pub fn preselect_header(&self, style: u32) {
        if (style as usize) < HEADER_OPTIONS.len() {
            self.header_row.set_selected(style);
        }
    }

    pub fn preselect_languages(&self, langs: &[String]) {
        for (key, sw) in &self.lang_switches {
            sw.set_active(langs.iter().any(|l| l == key));
        }
    }

    pub fn preselect_packages(&self, pkgs: &[String]) {
        for (key, sw) in &self.pkg_switches {
            sw.set_active(pkgs.iter().any(|p| p == key));
        }
        let droplet_on = pkgs.iter().any(|p| p == "pkg_droplet");
        self.dropcap_expander.set_enable_expansion(droplet_on);
    }

    /// Pre-fill all dialog fields from a sidecar. Called when opening
    /// "Update Template Settings" for a document that has a sidecar file.
    pub fn preselect_from_sidecar(&self, s: &SidecarSettings) {
        self.preselect_cv_mode(s.body_kind == "cv");
        self.preselect_style(&s.style);
        if !s.font.is_empty()      { self.preselect_font(&s.font); }
        if !s.font_size.is_empty() { self.preselect_font_size(&s.font_size); }
        if !s.paper.is_empty()     { self.preselect_paper(&s.paper, &s.custom_paper_w, &s.custom_paper_h); }
        if !s.spacing.is_empty()   { self.preselect_spacing(&s.spacing); }
        self.preselect_margin(s.margin as usize, &s.custom_margin);
        self.preselect_page_numbers(s.page_numbers);
        self.preselect_header(s.header_style);
        self.preselect_metadata(&s.title, &s.subtitle, &s.author, &s.affiliation, &s.course, &s.professor, &s.date);
        self.preselect_toc(s.toc, s.toc_depth);
        self.preselect_abstract(s.abstract_enabled, &s.abstract_text);
        self.preselect_keywords(s.keywords_enabled, &s.keywords_text);
        self.preselect_heading_numbering(s.heading_numbering);
        if !s.numbering_format.is_empty() {
            self.preselect_heading_format(&s.numbering_format);
        }
        self.preselect_languages(&s.languages);
        self.preselect_packages(&s.packages);
        self.preselect_dropcap_font(&s.dropcap_font);
        self.preselect_dropcap_lines(s.dropcap_lines);
        self.preselect_dropcap_color(&s.dropcap_color);
        if let Some(ref p) = s.bib_path {
            if !p.is_empty() {
                *self.bib_path.borrow_mut() = Some(PathBuf::from(p));
            }
        }
    }

    pub fn present(&self) {
        self.window.present();
    }
}

// ── Sidecar persistence ───────────────────────────────────────────────────────

pub fn sidecar_path(typ_path: &std::path::Path) -> PathBuf {
    let stem = typ_path.file_stem().unwrap_or_default();
    let dir  = typ_path.parent().unwrap_or(std::path::Path::new("."));
    dir.join(format!("{}.zerkalo.toml", stem.to_string_lossy()))
}

pub fn save_sidecar(typ_path: &std::path::Path, s: &SidecarSettings) {
    if let Ok(text) = toml::to_string_pretty(s) {
        let _ = std::fs::write(sidecar_path(typ_path), text);
    }
}

pub fn load_sidecar(typ_path: &std::path::Path) -> Option<SidecarSettings> {
    let path = sidecar_path(typ_path);
    let text = std::fs::read_to_string(&path).ok()?;
    match toml::from_str::<SidecarSettings>(&text) {
        Ok(s) => Some(s),
        Err(e) => {
            tracing::warn!("Sidecar {:?} is corrupt ({}); falling back to text parsing", path, e);
            None
        }
    }
}

pub fn build_sidecar(t: &TemplateSettings) -> SidecarSettings {
    SidecarSettings {
        title:             t.title.clone(),
        subtitle:          t.subtitle.clone(),
        author:            t.author.clone(),
        affiliation:       t.affiliation.clone(),
        course:            t.course.clone(),
        professor:         t.professor.clone(),
        date:              t.date.clone(),
        style:             CITATION_STYLES.get(t.style_idx).map(|(_, k)| k.to_string()).unwrap_or_default(),
        font:              t.font.clone(),
        font_size:         t.font_size.clone(),
        paper:             PAPER_SIZES.get(t.paper_idx).map(|(_, k)| k.to_string()).unwrap_or_default(),
        custom_paper_w:    t.custom_paper_w.clone(),
        custom_paper_h:    t.custom_paper_h.clone(),
        margin:            t.margin_idx as u32,
        custom_margin:     t.custom_margin.clone(),
        spacing:           t.spacing.clone(),
        page_numbers:      t.page_num_pos,
        header_style:      t.header_style,
        toc:               t.include_toc,
        toc_depth:         t.toc_depth,
        abstract_enabled:  t.include_abstract,
        abstract_text:     t.abstract_text.clone(),
        keywords_enabled:  t.include_keywords,
        keywords_text:     t.keywords.clone(),
        heading_numbering: t.heading_numbering,
        numbering_format:  t.numbering_format.clone(),
        languages:         t.languages.clone(),
        packages:          t.packages.clone(),
        dropcap_font:      t.dropcap_font.clone(),
        dropcap_lines:     t.dropcap_lines,
        dropcap_color:     t.dropcap_color.clone(),
        bib_path:          t.bib_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
        body_kind:         match t.body_kind { BodyKind::Book => "book".into(), BodyKind::Cv => "cv".into(), BodyKind::Letter => "letter".into(), BodyKind::Academic => "academic".into() },
    }
}

/// Reconstructs a [`TemplateSettings`] from a saved [`SidecarSettings`].
#[allow(dead_code)]
pub fn sidecar_to_settings(sc: &SidecarSettings) -> TemplateSettings {
    let style_idx = CITATION_STYLES
        .iter()
        .position(|(_, k)| *k == sc.style)
        .unwrap_or(0);
    let paper_idx = PAPER_SIZES
        .iter()
        .position(|(_, k)| *k == sc.paper)
        .unwrap_or(0);
    TemplateSettings {
        title: sc.title.clone(),
        subtitle: sc.subtitle.clone(),
        author: sc.author.clone(),
        affiliation: sc.affiliation.clone(),
        course: sc.course.clone(),
        professor: sc.professor.clone(),
        date: sc.date.clone(),
        style_idx,
        paper_idx,
        custom_paper_w: sc.custom_paper_w.clone(),
        custom_paper_h: sc.custom_paper_h.clone(),
        margin_idx: sc.margin as usize,
        custom_margin: sc.custom_margin.clone(),
        font: sc.font.clone(),
        font_size: sc.font_size.clone(),
        spacing: sc.spacing.clone(),
        page_num_pos: sc.page_numbers,
        header_style: sc.header_style,
        include_toc: sc.toc,
        toc_depth: sc.toc_depth,
        include_abstract: sc.abstract_enabled,
        abstract_text: sc.abstract_text.clone(),
        include_keywords: sc.keywords_enabled,
        keywords: sc.keywords_text.clone(),
        heading_numbering: sc.heading_numbering,
        numbering_format: sc.numbering_format.clone(),
        languages: sc.languages.clone(),
        packages: sc.packages.clone(),
        dropcap_font: sc.dropcap_font.clone(),
        dropcap_lines: sc.dropcap_lines,
        dropcap_color: sc.dropcap_color.clone(),
        body_kind: if sc.body_kind == "book" { BodyKind::Book } else if sc.body_kind == "cv" { BodyKind::Cv } else if sc.body_kind == "letter" { BodyKind::Letter } else { BodyKind::Academic },
        bib_path: sc.bib_path.as_ref().map(|s| std::path::PathBuf::from(s)),
    }
}

/// Parse the abstract text that the user wrote directly in a .typ file.
/// Looks for the `#block(inset: (x: 1in))[` block that follows `#align(center)[*Abstract*]`.
pub fn parse_abstract_from_doc(content: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut found_abstract_header = false;
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t == "#align(center)[*Abstract*]" {
            found_abstract_header = true;
            continue;
        }
        if found_abstract_header {
            if t.is_empty() { continue; }
            // The block form: #block(inset: (x: 1in))[ ... ]
            if t.starts_with("#block(") && t.ends_with('[') {
                let next = i + 1;
                if next < lines.len() {
                    let text = lines[next].trim().to_string();
                    if !text.is_empty() && text != "]" {
                        return Some(text);
                    }
                }
                return None;
            }
            // Inline form: text directly after the header
            if !t.starts_with('#') {
                return Some(t.to_string());
            }
            break;
        }
    }
    None
}

/// Re-inserts the `// ── Document body` marker into a file that is missing it.
/// Creates a `.typ.bak` backup first. Returns `Ok(true)` when the file was
/// modified, `Ok(false)` when the marker was already present.
pub fn repair_template_markers(path: &std::path::Path) -> Result<bool, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read file: {e}"))?;

    if has_body_marker(&content) {
        return Ok(false);
    }

    let backup = path.with_extension("typ.bak");
    std::fs::write(&backup, &content)
        .map_err(|e| format!("Cannot create backup at {}: {e}", backup.display()))?;

    // Find the end of the preamble: the last line from the top that is a
    // #directive, // comment, or blank. The first body-content line (heading,
    // paragraph text) follows it.
    let lines: Vec<&str> = content.lines().collect();
    let mut insert_before = lines.len(); // default: append at end
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if !t.starts_with('#') && !t.starts_with("//") && !t.is_empty() {
            insert_before = i;
            break;
        }
    }

    let prefix = lines[..insert_before].join("\n");
    let suffix = lines[insert_before..].join("\n");

    let mut new_content = String::with_capacity(content.len() + 128);
    new_content.push_str(&prefix);
    if !prefix.is_empty() {
        new_content.push('\n');
    }
    new_content.push_str("// ── Document body – DO NOT DELETE or Zerkalo template system will break\n");
    new_content.push_str("// ── Document body ───────────────────────────────────────────────────\n\n");
    new_content.push_str(&suffix);
    if !suffix.is_empty() && !new_content.ends_with('\n') {
        new_content.push('\n');
    }

    std::fs::write(path, &new_content)
        .map_err(|e| format!("Cannot write repaired file: {e}"))?;

    Ok(true)
}

/// Returns true when the document has a body-section marker and `apply_body_splice`
/// will safely preserve the user's writing.
pub fn has_body_marker(content: &str) -> bool {
    const BODY_MARKERS: &[&str] = &["// ── Document body", "// ── Chapters"];
    BODY_MARKERS.iter().any(|m| content.contains(m))
}

/// Regenerate the document preamble and front-matter from fresh settings while
/// preserving the user's body content. Splices at the `// ── Document body` /
/// `// ── Chapters` marker so the body is never touched, and updates the
/// bibliography style in the preserved body.
pub fn apply_body_splice(existing: &str, fresh: &str) -> String {
    const BODY_MARKERS: &[&str] = &["// ── Document body", "// ── Chapters"];

    let old_pos   = BODY_MARKERS.iter().filter_map(|m| existing.find(m)).min();
    let fresh_pos = BODY_MARKERS.iter().filter_map(|m| fresh.find(m)).min();

    match (old_pos, fresh_pos) {
        (Some(old_p), Some(fresh_p)) => {
            let old_body = &existing[old_p..];
            let style_key = parse_style_key(fresh).unwrap_or_default();
            let updated_body = if !style_key.is_empty() {
                let bib_s = bib_style(&style_key);
                let bib_t = bib_title_for_style(&style_key);
                crate::styles::update_bibliography_only(old_body, bib_s, bib_t)
            } else {
                old_body.to_string()
            };

            // CV documents created before the Skrizhal `#cv-section` rewrite
            // have a body that calls #job/#edu/#award/#presentation directly
            // — functions the regenerated preamble no longer defines on its
            // own. Re-inject them so settings changes (font, paper, margin,
            // ...) on these older documents keep compiling instead of
            // breaking on "unknown function".
            let preamble_needs_legacy_helpers =
                existing[..old_p].contains("#let job(") && !fresh[..fresh_p].contains("#let job(");
            let fresh_preamble = if preamble_needs_legacy_helpers {
                inject_legacy_cv_helpers(&fresh[..fresh_p])
            } else {
                fresh[..fresh_p].to_string()
            };

            format!("{fresh_preamble}{updated_body}")
        }
        _ => fresh.to_string(),
    }
}

/// Re-inserts the pre-Skrizhal `#job`/`#edu`/`#skill`/`#award`/`#presentation`
/// helper definitions into a freshly generated CV preamble, right before the
/// `// ZERKALO-TEMPLATE-END` marker — see `apply_body_splice`.
fn inject_legacy_cv_helpers(fresh_preamble: &str) -> String {
    let Some(end_pos) = fresh_preamble.find(TEMPLATE_END) else {
        return fresh_preamble.to_string();
    };
    format!(
        "{}{}\n{}",
        &fresh_preamble[..end_pos],
        legacy_cv_helpers_block(),
        &fresh_preamble[end_pos..]
    )
}

fn legacy_cv_helpers_block() -> String {
    let mut out = String::new();
    let _ = writeln!(out, "// #job — kept for documents created before #cv-section existed");
    let _ = writeln!(out, "#let job(title, company, years, desc) = {{");
    let _ = writeln!(out, "  if CV_STYLE == \"modern\" {{");
    let _ = writeln!(out, "    grid(columns: (1fr, auto),");
    let _ = writeln!(out, "      [*#title* #h(0.3em)#text(fill: cv-accent, size: 9.5pt)[#company]],");
    let _ = writeln!(out, "      text(size: 9pt, fill: cv-dim, style: \"italic\")[#years],");
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "  }} else if CV_STYLE == \"academic\" {{");
    let _ = writeln!(out, "    grid(columns: (1fr, auto),");
    let _ = writeln!(out, "      [*#title* #h(0.3em)#text(style: \"italic\")[#company]],");
    let _ = writeln!(out, "      text(style: \"italic\", fill: cv-muted)[#years],");
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "  }} else if CV_STYLE == \"sidebar\" {{");
    let _ = writeln!(out, "    [*#title* --- #company]");
    let _ = writeln!(out, "    linebreak()");
    let _ = writeln!(out, "    text(style: \"italic\")[#years]");
    let _ = writeln!(out, "  }} else {{");
    let _ = writeln!(out, "    grid(columns: (1fr, auto),");
    let _ = writeln!(out, "      [*#title* #h(0.25em)#text(fill: cv-muted)[—]#h(0.25em)#company],");
    let _ = writeln!(out, "      text(fill: cv-muted, style: \"italic\")[#years],");
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "  }}");
    let _ = writeln!(out, "  v(0.2em)");
    let _ = writeln!(out, "  desc");
    let _ = writeln!(out, "  v(0.5em)");
    let _ = writeln!(out, "}}");
    let _ = writeln!(out);

    let _ = writeln!(out, "#let edu(degree, institution, years, note: none) = {{");
    let _ = writeln!(out, "  if CV_STYLE == \"modern\" {{");
    let _ = writeln!(out, "    grid(columns: (1fr, auto),");
    let _ = writeln!(out, "      [*#degree* #h(0.3em)#text(fill: cv-accent, size: 9.5pt)[#institution]],");
    let _ = writeln!(out, "      text(size: 9pt, fill: cv-dim, style: \"italic\")[#years],");
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "  }} else if CV_STYLE == \"academic\" {{");
    let _ = writeln!(out, "    grid(columns: (1fr, auto),");
    let _ = writeln!(out, "      [*#degree* #h(0.3em)#text(style: \"italic\")[#institution]],");
    let _ = writeln!(out, "      text(style: \"italic\", fill: cv-muted)[#years],");
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "  }} else if CV_STYLE == \"sidebar\" {{");
    let _ = writeln!(out, "    [*#degree*]");
    let _ = writeln!(out, "    linebreak()");
    let _ = writeln!(out, "    if note != none {{ note; linebreak() }}");
    let _ = writeln!(out, "    [#institution]");
    let _ = writeln!(out, "    linebreak()");
    let _ = writeln!(out, "    [#years]");
    let _ = writeln!(out, "  }} else {{");
    let _ = writeln!(out, "    grid(columns: (1fr, auto),");
    let _ = writeln!(out, "      [*#degree* #h(0.25em)#text(fill: cv-muted)[—]#h(0.25em)#institution],");
    let _ = writeln!(out, "      text(fill: cv-muted, style: \"italic\")[#years],");
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "  }}");
    let _ = writeln!(out, "  if CV_STYLE != \"sidebar\" and note != none {{ v(0.15em); note }}");
    let _ = writeln!(out, "  v(0.45em)");
    let _ = writeln!(out, "}}");
    let _ = writeln!(out);

    let _ = writeln!(out, "#let skill(category, items) = {{");
    let _ = writeln!(out, "  if CV_STYLE == \"modern\" [");
    let _ = writeln!(out, "    #grid(columns: (6em, 1fr),");
    let _ = writeln!(out, "      text(fill: cv-accent, weight: \"bold\", size: 9pt, tracking: 0.5pt)[#upper(category)],");
    let _ = writeln!(out, "      text(fill: cv-muted)[#items.join(\"  ·  \")],");
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "    #v(0.15em)");
    let _ = writeln!(out, "  ] else if CV_STYLE == \"academic\" [");
    let _ = writeln!(out, "    *#category:* #items.join(\", \") \\");
    let _ = writeln!(out, "  ] else if CV_STYLE == \"sidebar\" [");
    let _ = writeln!(out, "    #text(weight: \"bold\")[#category]");
    let _ = writeln!(out, "    #list(..items.map(item => [#item]))");
    let _ = writeln!(out, "  ] else [");
    let _ = writeln!(out, "    #text(style: \"italic\")[#category:] #h(0.3em)#items.join(\", \") \\");
    let _ = writeln!(out, "  ]");
    let _ = writeln!(out, "}}");
    let _ = writeln!(out);

    let _ = writeln!(out, "#let award(title, org, years, desc: none) = {{");
    let _ = writeln!(out, "  if CV_STYLE == \"modern\" {{");
    let _ = writeln!(out, "    grid(columns: (1fr, auto),");
    let _ = writeln!(out, "      [*#title* #h(0.3em)#text(fill: cv-accent, size: 9.5pt)[#org]],");
    let _ = writeln!(out, "      text(size: 9pt, fill: cv-dim, style: \"italic\")[#years],");
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "  }} else if CV_STYLE == \"academic\" {{");
    let _ = writeln!(out, "    grid(columns: (1fr, auto),");
    let _ = writeln!(out, "      [*#title* #h(0.3em)#text(style: \"italic\")[#org]],");
    let _ = writeln!(out, "      text(style: \"italic\", fill: cv-muted)[#years],");
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "  }} else if CV_STYLE == \"sidebar\" {{");
    let _ = writeln!(out, "    if org != none {{ [*#title* --- #org] }} else {{ [*#title*] }}");
    let _ = writeln!(out, "    linebreak()");
    let _ = writeln!(out, "    [#years]");
    let _ = writeln!(out, "  }} else {{");
    let _ = writeln!(out, "    grid(columns: (1fr, auto),");
    let _ = writeln!(out, "      [*#title* #h(0.25em)#text(fill: cv-muted)[—]#h(0.25em)#org],");
    let _ = writeln!(out, "      text(fill: cv-muted, style: \"italic\")[#years],");
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "  }}");
    let _ = writeln!(out, "  if desc != none {{ v(0.15em); desc }}");
    let _ = writeln!(out, "  v(0.45em)");
    let _ = writeln!(out, "}}");
    let _ = writeln!(out);

    let _ = writeln!(out, "#let presentation(role, venue, title, years) = {{");
    let _ = writeln!(out, "  if CV_STYLE == \"sidebar\" {{");
    let _ = writeln!(out, "    [*#role* #h(0.25em)#venue, #text(style: \"italic\")[\"#title\"]]");
    let _ = writeln!(out, "    linebreak()");
    let _ = writeln!(out, "    text(style: \"italic\")[#years]");
    let _ = writeln!(out, "  }} else {{");
    let _ = writeln!(out, "    grid(columns: (1fr, auto),");
    let _ = writeln!(out, "      [*#role* #h(0.25em)#venue, #text(style: \"italic\")[\"#title\"]],");
    let _ = writeln!(out, "      text(fill: cv-muted, style: \"italic\")[#years],");
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "  }}");
    let _ = writeln!(out, "  v(0.35em)");
    let _ = writeln!(out, "}}");
    out
}

fn bib_title_for_style(style_key: &str) -> &'static str {
    match style_key {
        "mla"                 => "Works Cited",
        "chicago-author-date" => "References",
        "apa" | "asa" | "ieee" | "harvard" | "vancouver" => "References",
        _                     => "",
    }
}

// ── Font list ─────────────────────────────────────────────────────────────────

fn build_font_list() -> Vec<String> {
    let mut fonts = super::font_manager::FontManager::enabled_fonts();
    if fonts.is_empty() {
        return ACADEMIC_FONTS.iter().map(|s| s.to_string()).collect();
    }
    // Always put GOST type B first if present, then sort the rest
    fonts.retain(|f| f != "GOST type B");
    let mut result = vec!["GOST type B".to_string()];
    result.extend(fonts);
    result.push("Other…".to_string());
    result
}

// ── Widget helpers ────────────────────────────────────────────────────────────

fn pref_tab_box() -> GtkBox {
    let b = GtkBox::new(Orientation::Vertical, 16);
    b.set_margin_start(20);
    b.set_margin_end(20);
    b.set_margin_top(20);
    b.set_margin_bottom(20);
    b
}

fn tab_scroll(content: GtkBox) -> ScrolledWindow {
    let s = ScrolledWindow::new();
    s.set_vexpand(true);
    s.set_child(Some(&content));
    s
}

fn tab_label(text: &str) -> Label {
    Label::new(Some(text))
}

fn slug(s: &str) -> String {
    let s = s.trim().to_lowercase();
    if s.is_empty() { return "untitled".to_string(); }
    s.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

// ── Typst escaping helpers ────────────────────────────────────────────────────

/// Escape a value for use inside a Typst string literal `"..."`.
/// Only `\` and `"` need escaping in Typst string context.
fn typst_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// ── Template generator ────────────────────────────────────────────────────────

pub fn generate_typst_template(s: &TemplateSettings) -> String {
    if matches!(s.body_kind, BodyKind::Cv) {
        return generate_cv_template(s);
    }
    let style_key = CITATION_STYLES.get(s.style_idx).map(|(_, k)| *k).unwrap_or("chicago-notes");
    let style_name = CITATION_STYLES.get(s.style_idx).map(|(n, _)| *n).unwrap_or("Chicago");
    let bib = bib_style(style_key);
    let bib_line = s.bib_path.as_ref().map(|p| {
        format!("#bibliography(\"{}\", style: \"{}\")", p.display(), bib)
    });

    // GOST 7.32 mandates A4, specific margins, and 14 pt body text regardless of form selection.
    let (paper_line, mt, mb, ml, mr, font_size) = if style_key == "gost-r-705" {
        let size = if s.font_size.is_empty() { "14pt" } else { &s.font_size };
        ("paper: \"a4\",".to_string(), "20mm".to_string(), "20mm".to_string(), "30mm".to_string(), "15mm".to_string(), size)
    } else {
        let p = PAPER_SIZES.get(s.paper_idx).map(|(_, k)| *k).unwrap_or("us-letter");
        let paper_line = if p == "custom" {
            let w = if s.custom_paper_w.is_empty() { "210" } else { &s.custom_paper_w };
            let h = if s.custom_paper_h.is_empty() { "297" } else { &s.custom_paper_h };
            format!("width: {w}mm,\n  height: {h}mm,")
        } else {
            format!("paper: \"{p}\",")
        };
        let (mt, mb, ml, mr) = margin_values(s.margin_idx, &s.custom_margin);
        let size = if s.font_size.is_empty() { "12pt" } else { &s.font_size };
        (paper_line, mt, mb, ml, mr, size)
    };

    let mut out = String::new();

    let _ = writeln!(out, "{TEMPLATE_BEGIN}");
    let _ = writeln!(out, "// Created with Zerkalo · {style_name} style");
    let _ = writeln!(out, "// @zerkalo-style: {style_key}");
    let _ = writeln!(out, "// @zerkalo-version: {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(out);

    // Package imports
    for pkg in &s.packages {
        if let Some(import) = package_import(pkg) {
            let _ = writeln!(out, "{import}");
        }
    }
    if s.packages.contains(&"pkg_droplet".to_string()) {
        let has_font = !s.dropcap_font.is_empty();
        let has_height = s.dropcap_lines != 3;
        let has_color = !s.dropcap_color.is_empty();
        if has_font || has_height || has_color {
            let mut args = Vec::new();
            if has_font  { args.push(format!("font: \"{}\"", typst_str(&s.dropcap_font))); }
            if has_height { args.push(format!("height: {}", s.dropcap_lines)); }
            if has_color { args.push(format!("fill: {}", s.dropcap_color)); }
            let _ = writeln!(out, "#let dropcap = dropcap.with({})", args.join(", "));
        }
    }
    if !s.packages.is_empty() {
        let _ = writeln!(out);
    }

    // Page setup
    let page_num_code = page_num_block(s.page_num_pos);
    let _ = writeln!(out, "#set page(");
    let _ = writeln!(out, "  {paper_line}");
    let _ = writeln!(out, "  margin: (top: {mt}, bottom: {mb}, left: {ml}, right: {mr}),");
    if !page_num_code.is_empty() {
        let _ = writeln!(out, "  {page_num_code}");
    }
    let _ = writeln!(out, ")");
    let _ = writeln!(out);

    // Typography
    // "LaTeX Look" mandates Computer Modern and its own tighter paragraph rhythm
    // (leading, spacing, first-line-indent), regardless of the font/spacing fields.
    if style_key == "latex" {
        let _ = writeln!(out, "#set text(font: \"New Computer Modern\", size: {font_size}, lang: \"en\")");
        let _ = writeln!(out, "#set par(leading: 0.55em, spacing: 0.55em, first-line-indent: 1.8em, justify: true)");
        let _ = writeln!(out, "#show raw: set text(font: \"New Computer Modern Mono\")");
        let _ = writeln!(out, "#show math.equation: set text(weight: \"regular\")");
    } else {
        let _ = writeln!(out, "#set text(font: \"{}\", size: {font_size}, lang: \"en\")", typst_str(&s.font));
        let _ = writeln!(out, "#set par(leading: {}, spacing: 1.2em, first-line-indent: 1em, justify: true)", s.spacing);
    }
    let _ = writeln!(out);

    // Heading styles (with counter display injected when numbering is enabled)
    let num_fmt = if s.heading_numbering {
        if s.numbering_format.is_empty() { "1.".to_string() } else { s.numbering_format.clone() }
    } else {
        String::new()
    };
    let heading_code = inject_heading_numbering(
        heading_styles(style_key).trim_start_matches('\n'),
        s.heading_numbering,
        &num_fmt,
    );
    let _ = writeln!(out, "{heading_code}");
    let _ = writeln!(out);

    // Heading numbering — user-controlled for all styles (IEEE, GOST, Vancouver default to on)
    if s.heading_numbering {
        let fmt = if s.numbering_format.is_empty() { "1." } else { s.numbering_format.as_str() };
        let _ = writeln!(out, "#set heading(numbering: \"{fmt}\")");
        let _ = writeln!(out);
    }

    // Style-specific extras
    if style_key == "ieee" {
        let _ = writeln!(out, "#set page(columns: 2)");
        let _ = writeln!(out);
    }

    // Language support
    for lang in &s.languages {
        if let Some(block) = language_block(lang) {
            let _ = writeln!(out, "{block}");
        }
    }
    if !s.languages.is_empty() {
        let _ = writeln!(out);
    }

    let _ = writeln!(out, "{TEMPLATE_END}");
    let _ = writeln!(out);

    // Title block (style-specific) — letters get a letterhead instead of a title page
    let title_block = if matches!(s.body_kind, BodyKind::Letter) {
        generate_letter_header(s)
    } else {
        generate_title_page(style_key, s)
    };
    let _ = write!(out, "{title_block}");

    // Abstract
    if s.include_abstract {
        let _ = writeln!(out, "#align(center)[*Abstract*]");
        if !s.abstract_text.is_empty() {
            let _ = writeln!(out, "#block(inset: (x: 1in))[");
            let _ = writeln!(out, "  {}", s.abstract_text);
            let _ = writeln!(out, "]");
        }
        let _ = writeln!(out);
    }

    // Keywords
    if s.include_keywords && !s.keywords.is_empty() {
        let _ = writeln!(out, "_Keywords:_ {}", s.keywords);
        let _ = writeln!(out);
    }

    // Table of contents (always followed by a page break)
    if s.include_toc {
        let _ = writeln!(out, "#outline(depth: {})", s.toc_depth);
        let _ = writeln!(out, "#pagebreak()");
        let _ = writeln!(out);
    }

    // Body
    match s.body_kind {
        BodyKind::Book => {
            let _ = writeln!(out, "// ── Chapters – DO NOT DELETE or Zerkalo template system will break");
            let _ = writeln!(out, "// ── Chapters ────────────────────────────────────────────────────────");
            let _ = writeln!(out);
            let _ = writeln!(out, "= Chapter One: The Beginning");
            let _ = writeln!(out);
            let _ = writeln!(out, "Start writing your opening chapter here...");
            let _ = writeln!(out);
            let _ = writeln!(out, "#pagebreak()");
            let _ = writeln!(out);
            let _ = writeln!(out, "= Chapter Two");
            let _ = writeln!(out);
            let _ = writeln!(out, "Continue here...");
            let _ = writeln!(out);
            let _ = writeln!(out, "#pagebreak()");
            let _ = writeln!(out);
            let _ = writeln!(out, "// ── Back matter ─────────────────────────────────────────────────────");
            if let Some(ref line) = bib_line {
                let _ = writeln!(out, "{line}");
            } else {
                let _ = writeln!(out, "// Set your .bib file path in Settings > Extras, then regenerate.");
                let _ = writeln!(out, "// #bibliography(\"refs.bib\", style: \"{bib}\")");
            }
        }
        BodyKind::Academic => {
            let _ = writeln!(out, "// ── Document body – DO NOT DELETE or Zerkalo template system will break");
            let _ = writeln!(out, "// ── Document body ───────────────────────────────────────────────────");
            let _ = writeln!(out);
            let _ = writeln!(out, "= Introduction");
            let _ = writeln!(out);
            let _ = writeln!(out, "Start writing here...");
            let _ = writeln!(out);
            let _ = writeln!(out, "#pagebreak()");
            let _ = writeln!(out);
            let _ = writeln!(out, "// ── Bibliography ────────────────────────────────────────────────────");
            if let Some(ref line) = bib_line {
                let _ = writeln!(out, "{line}");
            } else {
                let _ = writeln!(out, "// Set your .bib file path in Settings > Extras, then regenerate.");
                let _ = writeln!(out, "// #bibliography(\"refs.bib\", style: \"{bib}\")");
            }
        }
        BodyKind::Letter => {
            let _ = writeln!(out, "// ── Document body – DO NOT DELETE or Zerkalo template system will break");
            let _ = writeln!(out, "// ── Document body ───────────────────────────────────────────────────");
            let _ = writeln!(out);
            let _ = writeln!(out, "Start writing your letter here...");
            let _ = writeln!(out);
            let _ = writeln!(out, "#v(2em)");
            let _ = writeln!(out, "Sincerely,");
            let _ = writeln!(out);
            let _ = writeln!(out, "#v(2.5em)");
            let _ = writeln!(out, "#doc-author");
            let _ = writeln!(out, "#if doc-affil != \"\" [\\ #doc-affil]");
        }
        BodyKind::Cv => { /* dispatched to generate_cv_template() above */ }
    }

    out
}

// ── CV template generator ─────────────────────────────────────────────────────

fn generate_cv_template(s: &TemplateSettings) -> String {
    let cv_style = match s.style_idx {
        1 => "academic",
        2 => "classic",
        3 => "sidebar",
        _ => "modern",
    };
    let paper = PAPER_SIZES.get(s.paper_idx).map(|(_, k)| *k).unwrap_or("a4");
    let (margin_x, margin_y) = match s.margin_idx {
        1 => ("1.2cm", "1.2cm"),
        2 => ("2.5cm", "2.5cm"),
        _ => ("1.5cm", "1.5cm"),
    };
    let font = if s.font.is_empty() || s.font == "Times New Roman" { "Linux Libertine" } else { &s.font };
    let font_size = if s.font_size.is_empty() { "10.5pt" } else { &s.font_size };
    let name = if s.author.is_empty() { "Your Name" } else { &s.author };

    let mut out = String::new();
    let _ = writeln!(out, "{TEMPLATE_BEGIN}");
    let _ = writeln!(out, "// Created with Zerkalo · CV / Résumé");
    let _ = writeln!(out, "// @zerkalo-style: cv");
    let _ = writeln!(out, "// @zerkalo-kind: cv");
    let _ = writeln!(out, "// @zerkalo-cv-style: {cv_style}");
    let _ = writeln!(out, "// @zerkalo-version: {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(out);
    let _ = writeln!(out, "#set page(paper: \"{paper}\", margin: (x: {margin_x}, y: {margin_y}))");
    let _ = writeln!(out, "#set text(font: \"{font}\", size: {font_size}, lang: \"en\")", font = typst_str(font));
    let _ = writeln!(out, "#set par(spacing: 0.55em, leading: 0.65em)");
    let _ = writeln!(out);
    let _ = writeln!(out, "// Change CV_STYLE to switch theme: \"modern\" | \"academic\" | \"classic\" | \"sidebar\"");
    let _ = writeln!(out, "#let CV_STYLE = \"{cv_style}\"");
    let _ = writeln!(out);

    // ── Colour palette (derived from CV_STYLE at compile time) ───────────────
    let _ = writeln!(out, "// ── Colour palette ──────────────────────────────────────────────────────");
    let _ = writeln!(out, "#let cv-accent = if CV_STYLE == \"modern\" {{ rgb(\"#2a5298\") }} else {{ black }}");
    let _ = writeln!(out, "#let cv-muted  = if CV_STYLE == \"modern\" {{ rgb(\"#555555\") }} else {{ luma(90) }}");
    let _ = writeln!(out, "#let cv-dim    = if CV_STYLE == \"modern\" {{ rgb(\"#888888\") }} else {{ luma(130) }}");
    let _ = writeln!(out);

    // ── Skrizhal CV data ──────────────────────────────────────────────────────
    // #cv-section pulls entries from your Skrizhal CV-element file (see
    // Settings → Extras → CV Elements) and formats them for CV_STYLE above —
    // no need to hand-write each job/degree/award as Typst source.
    let _ = writeln!(out, "// ── Skrizhal CV data ─────────────────────────────────────────────────────");
    let _ = writeln!(out, "#import \"cv-helpers.typ\": cv-section");
    let _ = writeln!(out);

    // ── Helper functions ─────────────────────────────────────────────────────
    let _ = writeln!(out, "// ── Layout helpers ───────────────────────────────────────────────────────");
    let _ = writeln!(out);

    // #section — sidebar style uses a plain native heading (no rule, no manual
    // spacing) to match a hand-written CV's `== Heading` exactly.
    let _ = writeln!(out, "#let section(title, body) = {{");
    let _ = writeln!(out, "  if CV_STYLE == \"sidebar\" {{");
    let _ = writeln!(out, "    heading(level: 2)[#title]");
    let _ = writeln!(out, "  }} else {{");
    let _ = writeln!(out, "    v(0.9em)");
    let _ = writeln!(out, "    if CV_STYLE == \"modern\" [");
    let _ = writeln!(out, "      #grid(columns: (4pt, 1fr), gutter: 0.45em,");
    let _ = writeln!(out, "        box(height: 0.9em, fill: cv-accent, radius: 1pt),");
    let _ = writeln!(out, "        text(weight: \"bold\", size: 9.5pt, fill: cv-accent, tracking: 1pt)[#upper(title)],");
    let _ = writeln!(out, "      )");
    let _ = writeln!(out, "      #v(-0.5em)");
    let _ = writeln!(out, "      #line(length: 100%, stroke: 0.4pt + cv-accent)");
    let _ = writeln!(out, "    ] else if CV_STYLE == \"academic\" [");
    let _ = writeln!(out, "      #text(weight: \"bold\", size: 10pt)[#smallcaps(upper(title))]");
    let _ = writeln!(out, "      #v(-0.45em)");
    let _ = writeln!(out, "      #line(length: 100%, stroke: 1pt)");
    let _ = writeln!(out, "    ] else [");
    let _ = writeln!(out, "      #text(weight: \"bold\", style: \"italic\")[#title]");
    let _ = writeln!(out, "      #v(-0.4em)");
    let _ = writeln!(out, "      #line(length: 100%, stroke: 0.5pt)");
    let _ = writeln!(out, "    ]");
    let _ = writeln!(out, "    v(0.4em)");
    let _ = writeln!(out, "  }}");
    let _ = writeln!(out, "  body");
    let _ = writeln!(out, "}}");
    let _ = writeln!(out);

    // #mylink — clickable link, underlined; sidebar uses a plain blue to match a
    // hand-written CV's link colour, other styles pick up the CV's own accent.
    let _ = writeln!(out, "#let mylink(url, label) = link(url)[#underline(text(fill: if CV_STYLE == \"sidebar\" {{ blue }} else {{ cv-accent }}, label))]");
    let _ = writeln!(out);

    // #taglist — plain list with no category label (Interests, etc., that
    // aren't backed by a Skrizhal category); sidebar renders real bullet
    // points, other styles a single dot-joined line.
    let _ = writeln!(out, "#let taglist(items) = {{");
    let _ = writeln!(out, "  if CV_STYLE == \"sidebar\" {{");
    let _ = writeln!(out, "    list(..items.map(item => [#item]))");
    let _ = writeln!(out, "  }} else {{");
    let _ = writeln!(out, "    text(fill: cv-muted)[#items.join(\"  ·  \")]");
    let _ = writeln!(out, "    v(0.15em)");
    let _ = writeln!(out, "  }}");
    let _ = writeln!(out, "}}");
    let _ = writeln!(out);

    let _ = writeln!(out, "{TEMPLATE_END}");
    let _ = writeln!(out);

    // ── Personal details + styled header ────────────────────────────────────
    let _ = writeln!(out, "// ── Personal details ─────────────────────────────────────────────────");
    let _ = writeln!(out, "#let cv-name     = \"{}\"", typst_str(name));
    let _ = writeln!(out, "#let cv-email    = \"your@email.com\"");
    let _ = writeln!(out, "#let cv-phone    = \"+1 555 000 0000\"");
    let _ = writeln!(out, "#let cv-location = \"City, Country\"");
    let _ = writeln!(out, "#let cv-links    = \"github.com/handle\"");
    let _ = writeln!(out);

    // Modern header: large tracked name, accent-colored contact row, thick rule
    let _ = writeln!(out, "#if CV_STYLE == \"modern\" [");
    let _ = writeln!(out, "  #align(center)[");
    let _ = writeln!(out, "    #text(size: 26pt, weight: \"bold\", tracking: 1pt)[#cv-name]");
    let _ = writeln!(out, "    #v(0.35em)");
    let _ = writeln!(out, "    #text(size: 9.5pt, fill: cv-accent)[");
    let _ = writeln!(out, "      #cv-email #h(0.5em)·#h(0.5em) #cv-phone #h(0.5em)·#h(0.5em) #cv-location #h(0.5em)·#h(0.5em) #cv-links");
    let _ = writeln!(out, "    ]");
    let _ = writeln!(out, "  ]");
    let _ = writeln!(out, "  #v(0.55em)");
    let _ = writeln!(out, "  #line(length: 100%, stroke: 1.5pt + cv-accent)");
    // Academic header: smallcaps name, two-line contact, 1pt rule
    let _ = writeln!(out, "] else if CV_STYLE == \"academic\" [");
    let _ = writeln!(out, "  #align(center)[");
    let _ = writeln!(out, "    #text(size: 22pt, weight: \"bold\")[#smallcaps(cv-name)]");
    let _ = writeln!(out, "    #v(0.3em)");
    let _ = writeln!(out, "    #text(size: 9.5pt)[#cv-email · #cv-phone]");
    let _ = writeln!(out, "    \\");
    let _ = writeln!(out, "    #text(size: 9.5pt)[#cv-location · #cv-links]");
    let _ = writeln!(out, "  ]");
    let _ = writeln!(out, "  #v(0.45em)");
    let _ = writeln!(out, "  #line(length: 100%, stroke: 1pt)");
    // Sidebar header: matches a plain hand-written CV — 20pt name, tight gap,
    // location-first contact line in the default ink colour, one thin rule after.
    let _ = writeln!(out, "] else if CV_STYLE == \"sidebar\" [");
    let _ = writeln!(out, "  #align(center)[");
    let _ = writeln!(out, "    #text(size: 20pt, weight: \"bold\")[#cv-name]");
    let _ = writeln!(out, "    #v(21pt)");
    let _ = writeln!(out, "    #cv-location · #mylink(\"mailto:\" + cv-email, cv-email) · #cv-phone · #mylink(\"https://\" + cv-links, cv-links)");
    let _ = writeln!(out, "  ]");
    let _ = writeln!(out, "  #v(10pt)");
    let _ = writeln!(out, "  #line(length: 100%)");
    let _ = writeln!(out, "  #v(5pt)");
    // Classic header: centered name, muted contact, thin rule
    let _ = writeln!(out, "] else [");
    let _ = writeln!(out, "  #align(center)[");
    let _ = writeln!(out, "    #text(size: 22pt, weight: \"bold\")[#cv-name]");
    let _ = writeln!(out, "    #v(0.25em)");
    let _ = writeln!(out, "    #text(size: 9.5pt, fill: cv-muted)[#cv-email · #cv-phone · #cv-location · #cv-links]");
    let _ = writeln!(out, "  ]");
    let _ = writeln!(out, "  #v(0.4em)");
    let _ = writeln!(out, "  #line(length: 100%, stroke: 0.5pt)");
    let _ = writeln!(out, "]");
    let _ = writeln!(out);
    if cv_style != "sidebar" {
        let _ = writeln!(out, "#v(0.6em)");
        let _ = writeln!(out);
    }

    // ── Document body ────────────────────────────────────────────────────────
    let _ = writeln!(out, "// ── Document body ─────────────────────────────────────────────────────");
    let _ = writeln!(out);

    if cv_style == "sidebar" {
        return generate_cv_sidebar_body(out);
    }

    let _ = writeln!(out, "// Toggle optional sections per application");
    let _ = writeln!(out, "#let show-presentations = true");
    let _ = writeln!(out, "#let show-extracurricular = true");
    let _ = writeln!(out);
    let _ = writeln!(out, "#section(\"Experience\")[");
    let _ = writeln!(out, "  #cv-section(category: (\"Employment\", \"Ministry Position\"), style: CV_STYLE)");
    let _ = writeln!(out, "]");
    let _ = writeln!(out);
    let _ = writeln!(out, "#section(\"Education\")[");
    let _ = writeln!(out, "  #cv-section(category: \"Education\", style: CV_STYLE)");
    let _ = writeln!(out, "]");
    let _ = writeln!(out);
    let _ = writeln!(out, "#section(\"Skills\")[");
    let _ = writeln!(out, "  #cv-section(category: \"Language Skill\", style: CV_STYLE, mode: \"tags\")");
    let _ = writeln!(out, "]");
    let _ = writeln!(out);
    let _ = writeln!(out, "#section(\"Awards & Honours\")[");
    let _ = writeln!(out, "  #cv-section(category: (\"Award\", \"Certification\"), style: CV_STYLE)");
    let _ = writeln!(out, "]");
    let _ = writeln!(out);
    let _ = writeln!(out, "#if show-presentations [");
    let _ = writeln!(out, "  #section(\"Presentations & Publications\")[");
    let _ = writeln!(out, "    #cv-section(category: (\"Publication\", \"Presentation\"), style: CV_STYLE)");
    let _ = writeln!(out, "  ]");
    let _ = writeln!(out, "]");
    let _ = writeln!(out);
    let _ = writeln!(out, "#if show-extracurricular [");
    let _ = writeln!(out, "  #section(\"Extracurricular\")[");
    let _ = writeln!(out, "    #cv-section(category: (\"Service\", \"Committee Appointment\", \"Volunteer\", \"Project\"), style: CV_STYLE)");
    let _ = writeln!(out, "  ]");
    let _ = writeln!(out, "]");

    out
}

// ── CV: two-column sidebar layout ─────────────────────────────────────────────
// A distinct body shape from the single-column styles above: sidebar (education,
// skills, interests, awards) beside a main column (experience, presentations,
// extracurricular). Reuses the same #section/#taglist helpers and #cv-section
// import already written into the TEMPLATE block, so switching CV_STYLE
// afterwards still recolors it correctly.
fn generate_cv_sidebar_body(mut out: String) -> String {
    let _ = writeln!(out, "// Toggle optional sections per application");
    let _ = writeln!(out, "#let show-presentations = true");
    let _ = writeln!(out, "#let show-extracurricular = true");
    let _ = writeln!(out);
    let _ = writeln!(out, "#grid(");
    let _ = writeln!(out, "  columns: (1fr, 2fr),");
    let _ = writeln!(out, "  gutter: 24pt,");
    let _ = writeln!(out);
    let _ = writeln!(out, "  // ── Left: sidebar ──────────────────────────────────────────────────");
    let _ = writeln!(out, "  [");
    let _ = writeln!(out, "    #section(\"Education\")[");
    let _ = writeln!(out, "      #cv-section(category: \"Education\", style: CV_STYLE)");
    let _ = writeln!(out, "    ]");
    let _ = writeln!(out);
    let _ = writeln!(out, "    #section(\"Skills\")[");
    let _ = writeln!(out, "      #cv-section(category: \"Language Skill\", style: CV_STYLE, mode: \"tags\")");
    let _ = writeln!(out, "    ]");
    let _ = writeln!(out);
    let _ = writeln!(out, "    #section(\"Interests\")[");
    let _ = writeln!(out, "      #taglist((\"Interest one\", \"Interest two\", \"Interest three\"))");
    let _ = writeln!(out, "    ]");
    let _ = writeln!(out);
    let _ = writeln!(out, "    #section(\"Awards\")[");
    let _ = writeln!(out, "      #cv-section(category: (\"Award\", \"Certification\"), style: CV_STYLE)");
    let _ = writeln!(out, "    ]");
    let _ = writeln!(out, "  ],");
    let _ = writeln!(out);
    let _ = writeln!(out, "  // ── Right: main column ─────────────────────────────────────────────");
    let _ = writeln!(out, "  [");
    let _ = writeln!(out, "    #section(\"Experience\")[");
    let _ = writeln!(out, "      #cv-section(category: (\"Employment\", \"Ministry Position\"), style: CV_STYLE)");
    let _ = writeln!(out, "    ]");
    let _ = writeln!(out);
    let _ = writeln!(out, "    #if show-presentations [");
    let _ = writeln!(out, "      #section(\"Presentations & Publications\")[");
    let _ = writeln!(out, "        #cv-section(category: (\"Publication\", \"Presentation\"), style: CV_STYLE)");
    let _ = writeln!(out, "      ]");
    let _ = writeln!(out, "    ]");
    let _ = writeln!(out);
    let _ = writeln!(out, "    #if show-extracurricular [");
    let _ = writeln!(out, "      #section(\"Extracurricular\")[");
    let _ = writeln!(out, "        #cv-section(category: (\"Service\", \"Committee Appointment\", \"Volunteer\", \"Project\"), style: CV_STYLE)");
    let _ = writeln!(out, "      ]");
    let _ = writeln!(out, "    ]");
    let _ = writeln!(out, "  ],");
    let _ = writeln!(out, ")");

    out
}

fn generate_title_page(style_key: &str, s: &TemplateSettings) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "// ── Title block ─────────────────────────────────────────────────────");

    // Live metadata variables — editing these directly updates the rendered title page.
    let _ = writeln!(out, "// Edit these variables to update the title page:");
    let _ = writeln!(out, "#let doc-title = \"{}\"", typst_str(if s.title.is_empty() { "Untitled" } else { &s.title }));
    let _ = writeln!(out, "#let doc-subtitle = \"{}\"", typst_str(&s.subtitle));
    let _ = writeln!(out, "#let doc-author = \"{}\"", typst_str(&s.author));
    let _ = writeln!(out, "#let doc-affil = \"{}\"", typst_str(&s.affiliation));
    let _ = writeln!(out, "#let doc-course = \"{}\"", typst_str(&s.course));
    let _ = writeln!(out, "#let doc-professor = \"{}\"", typst_str(&s.professor));
    let date_val = if s.date.is_empty() {
        Local::now().format("%B %-d, %Y").to_string()
    } else {
        s.date.clone()
    };
    let _ = writeln!(out, "#let doc-date = \"{}\"", typst_str(&date_val));
    if let Some(hdr) = header_block(s.header_style) {
        let _ = writeln!(out, "{hdr}");
    }
    let _ = writeln!(out);

    match style_key {
        // MLA: no separate title page — left-aligned header block then centred title
        "mla" => {
            let _ = writeln!(out, "#set par(first-line-indent: 0pt)");
            let _ = writeln!(out, "#if doc-author != \"\" [#doc-author \\ ]");
            let _ = writeln!(out, "#if doc-affil != \"\" [#doc-affil \\ ]");
            let _ = writeln!(out, "#if doc-course != \"\" [#doc-course \\ ]");
            let _ = writeln!(out, "#if doc-professor != \"\" [#doc-professor \\ ]");
            let _ = writeln!(out, "#if doc-date != \"\" [#doc-date]");
            let _ = writeln!(out);
            let _ = writeln!(out, "#align(center)[#doc-title]");
            let _ = writeln!(out, "#if doc-subtitle != \"\" [#align(center)[#text(style: \"italic\")[#doc-subtitle]]]");
            let _ = writeln!(out);
        }
        // IEEE: no title page — title + authors as header block in two-column layout
        "ieee" => {
            let _ = writeln!(out, "#align(center)[");
            let _ = writeln!(out, "  #text(size: 18pt, weight: \"bold\")[#upper[#doc-title]]");
            let _ = writeln!(out, "  #if doc-subtitle != \"\" [\\ #text(size: 13pt, style: \"italic\")[#doc-subtitle]]");
            let _ = writeln!(out, "  #if doc-author != \"\" [\\ #text(size: 11pt)[#doc-author]]");
            let _ = writeln!(out, "  #if doc-affil != \"\" [\\ #text(size: 10pt, style: \"italic\")[#doc-affil]]");
            let _ = writeln!(out, "]");
            let _ = writeln!(out);
        }
        // APA / ASA / Harvard: title page with running head, all centred
        "apa" | "asa" | "harvard" => {
            let _ = writeln!(out, "#page(header: align(left)[#text(size: 10pt)[Running head: #upper[#doc-title]]])[");
            let _ = writeln!(out, "  #set align(center)");
            let _ = writeln!(out, "  #v(2.5in)");
            let _ = writeln!(out, "  #text(size: 14pt, weight: \"bold\")[#doc-title]");
            let _ = writeln!(out, "  #if doc-subtitle != \"\" [\\ #text(size: 12pt, style: \"italic\")[#doc-subtitle]]");
            let _ = writeln!(out, "  #v(1em)");
            let _ = writeln!(out, "  #if doc-author != \"\" [#doc-author]");
            let _ = writeln!(out, "  #if doc-affil != \"\" [\\ #doc-affil]");
            let _ = writeln!(out, "  #if doc-course != \"\" [\\ #doc-course]");
            let _ = writeln!(out, "  #if doc-professor != \"\" [\\ #doc-professor]");
            let _ = writeln!(out, "  #if doc-date != \"\" [\\ #doc-date]");
            let _ = writeln!(out, "]");
            let _ = writeln!(out);
            let _ = writeln!(out, "#pagebreak()");
            let _ = writeln!(out);
        }
        // GOST R 7.0-5: structured cover page
        "gost-r-705" => {
            let _ = writeln!(out, "#page(header: none, footer: none, numbering: none)[");
            let _ = writeln!(out, "  #set align(center)");
            let _ = writeln!(out, "  #if doc-affil != \"\" [#text(size: 14pt)[#upper[#doc-affil]] #v(1em)]");
            let _ = writeln!(out, "  #v(3cm)");
            let _ = writeln!(out, "  #text(size: 16pt, weight: \"bold\")[#doc-title]");
            let _ = writeln!(out, "  #if doc-subtitle != \"\" [#v(0.5cm) #text(size: 12pt)[#doc-subtitle]]");
            let _ = writeln!(out, "  #v(3cm)");
            let _ = writeln!(out, "  #set align(right)");
            let _ = writeln!(out, "  #if doc-author != \"\" [#doc-author]");
            let _ = writeln!(out, "  #v(1fr)");
            let _ = writeln!(out, "  #set align(center)");
            let _ = writeln!(out, "  #if doc-date != \"\" [#doc-date]");
            let _ = writeln!(out, "]");
            let _ = writeln!(out);
            let _ = writeln!(out, "#counter(page).update(1)");
            let _ = writeln!(out, "#pagebreak()");
            let _ = writeln!(out);
        }
        // SBL, Chicago, Turabian, and everything else: full separate title page
        _ => {
            let _ = writeln!(out, "#page(header: none, footer: none, numbering: none)[");
            let _ = writeln!(out, "  #set align(center)");
            let _ = writeln!(out, "  #v(1fr)");
            let _ = writeln!(out, "  #text(size: 16pt, weight: \"bold\")[#doc-title]");
            let _ = writeln!(out, "  #if doc-subtitle != \"\" [\\ #text(size: 13pt, style: \"italic\")[#doc-subtitle]]");
            let _ = writeln!(out, "  #v(2fr)");
            let _ = writeln!(out, "  #if doc-author != \"\" [#doc-author]");
            let _ = writeln!(out, "  #if doc-affil != \"\" [\\ #text(style: \"italic\")[#doc-affil]]");
            let _ = writeln!(out, "  #if doc-course != \"\" [\\ #doc-course]");
            let _ = writeln!(out, "  #if doc-professor != \"\" [\\ #doc-professor]");
            let _ = writeln!(out, "  #if doc-date != \"\" [\\ #doc-date]");
            let _ = writeln!(out, "  #v(1fr)");
            let _ = writeln!(out, "]");
            let _ = writeln!(out);
            let _ = writeln!(out, "#counter(page).update(1)");
            let _ = writeln!(out, "#pagebreak()");
            let _ = writeln!(out);
        }
    }

    out
}

/// Letterhead for `BodyKind::Letter` — no separate title page. Straight into
/// a date, a recipient block, and a salutation, the way an actual letter opens.
fn generate_letter_header(s: &TemplateSettings) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "// ── Title block ─────────────────────────────────────────────────────");
    let _ = writeln!(out, "// Edit these variables to update the letterhead:");
    let _ = writeln!(out, "#let doc-title = \"{}\"", typst_str(if s.title.is_empty() { "Untitled" } else { &s.title }));
    let _ = writeln!(out, "#let doc-subtitle = \"{}\"", typst_str(&s.subtitle));
    let _ = writeln!(out, "#let doc-author = \"{}\"", typst_str(&s.author));
    let _ = writeln!(out, "#let doc-affil = \"{}\"", typst_str(&s.affiliation));
    let _ = writeln!(out, "#let doc-course = \"{}\"", typst_str(&s.course));
    let _ = writeln!(out, "#let doc-professor = \"{}\"", typst_str(&s.professor));
    let date_val = if s.date.is_empty() {
        Local::now().format("%B %-d, %Y").to_string()
    } else {
        s.date.clone()
    };
    let _ = writeln!(out, "#let doc-date = \"{}\"", typst_str(&date_val));
    if let Some(hdr) = header_block(s.header_style) {
        let _ = writeln!(out, "{hdr}");
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "#if doc-date != \"\" [#doc-date]");
    let _ = writeln!(out, "#v(1.5em)");
    let _ = writeln!(out);
    let _ = writeln!(out, "Recipient Name \\");
    let _ = writeln!(out, "Recipient Title \\");
    let _ = writeln!(out, "Recipient Institution \\");
    let _ = writeln!(out, "Street Address \\");
    let _ = writeln!(out, "City, State ZIP");
    let _ = writeln!(out);
    let _ = writeln!(out, "#v(1em)");
    let _ = writeln!(out, "Dear Recipient Name,");
    let _ = writeln!(out);

    out
}

/// Rebuild the title page in `content` for a new style, preserving existing metadata.
/// Called by apply_style so the title page layout updates when the style dropdown changes.
pub fn rebuild_title_page_for_style(content: &str, new_style_key: &str) -> String {
    let s = TemplateSettings {
        title: parse_meta(content, "title"),
        subtitle: parse_meta(content, "subtitle"),
        author: parse_meta(content, "author"),
        affiliation: parse_meta(content, "affiliation"),
        course: parse_meta(content, "course"),
        professor: parse_meta(content, "professor"),
        date: parse_meta(content, "date"),
        // Remaining fields are not used by generate_title_page
        style_idx: 0, paper_idx: 0, margin_idx: 0,
        custom_paper_w: String::new(), custom_paper_h: String::new(), custom_margin: String::new(),
        font: String::new(), font_size: String::new(), spacing: String::new(), page_num_pos: 0, header_style: 0,
        include_toc: false, toc_depth: 2,
        include_abstract: false, abstract_text: String::new(),
        include_keywords: false, keywords: String::new(),
        heading_numbering: false, numbering_format: String::new(),
        languages: vec![], packages: vec![], dropcap_font: String::new(), dropcap_lines: 3,
        dropcap_color: String::new(),
        body_kind: BodyKind::Academic,
        bib_path: None,
    };
    let new_page = generate_title_page(new_style_key, &s);
    // Wrap with a fake TEMPLATE_END so replace_title_page can locate the zone start
    let fake = format!("{TEMPLATE_END}\n\n{new_page}");
    replace_title_page(content, &fake)
}

fn margin_values(idx: usize, custom_in: &str) -> (String, String, String, String) {
    match idx {
        1 => ("0.5in".into(), "0.5in".into(), "0.5in".into(), "0.5in".into()),
        2 => ("1in".into(), "1in".into(), "2in".into(), "2in".into()),
        3 => ("1.75in".into(), "1.75in".into(), "1.75in".into(), "1.75in".into()),
        // Right margin is a relative length — Typst resolves 33% against the
        // page width directly, so this stays correct across paper sizes.
        4 => ("1.25in".into(), "1.25in".into(), "1.25in".into(), "33%".into()),
        5 => {
            let v = if custom_in.is_empty() { "1" } else { custom_in };
            let m = format!("{v}in");
            (m.clone(), m.clone(), m.clone(), m)
        }
        _ => ("1in".into(), "1in".into(), "1.25in".into(), "1.25in".into()),
    }
}

/// Resolves the Font Size ComboRow selection to a Typst size string, reading
/// the custom SpinRow's value when "Custom…" (index 4) is selected.
fn resolve_font_size(selected: u32, custom_pt: f64) -> String {
    match selected {
        0 => "10pt".to_string(),
        1 => "11pt".to_string(),
        3 => "14pt".to_string(),
        4 => format!("{}pt", custom_pt as i64),
        _ => "12pt".to_string(),
    }
}

fn page_num_block(pos: u32) -> &'static str {
    // Returns parameters to embed inside #set page(...).
    // The \n  keeps correct indentation when inserted as a single writeln line.
    match pos {
        0 => "numbering: \"1\",\n  number-align: bottom + center,",
        1 => "numbering: \"1\",\n  number-align: bottom + right,",
        2 => "numbering: \"1\",\n  number-align: top + center,",
        3 => "numbering: \"1\",\n  number-align: top + right,",
        _ => "",
    }
}

fn header_block(style: u32) -> Option<String> {
    match style {
        0 => None,
        1 => Some(String::from("#set page(header: align(center)[#doc-title])")),
        2 => Some(String::from("#set page(header: align(center)[#doc-author])")),
        3 => Some(String::from(
            "#set page(header: context {\n  let hs = query(heading.where(level: 1).before(here()))\n  if hs.len() > 0 { align(center, hs.last().body) }\n})"
        )),
        4 => Some(String::from("#set page(header: align(center)[#doc-title \u{b7} #doc-author])")),
        5 => Some(String::from(
            "#set page(header: context {\n  let hs = query(heading.where(level: 1).before(here()))\n  let sec = if hs.len() > 0 { [ \u{b7} ] + hs.last().body } else { [] }\n  align(center)[#doc-title#sec]\n})"
        )),
        6 => Some(String::from(
            "#set page(header: context {\n  let hs = query(heading.where(level: 1).before(here()))\n  let sec = if hs.len() > 0 { [ \u{b7} ] + hs.last().body } else { [] }\n  align(center)[#doc-author#sec]\n})"
        )),
        7 => Some(String::from("#set page(header: align(center)[#doc-author \u{b7} #doc-title])")),
        _ => None,
    }
}

fn default_dropcap_lines() -> u32 { 3 }

pub fn bib_style(style_key: &str) -> &'static str {
    match style_key {
        "sbl" | "turabian" => "chicago-notes",
        "chicago-notes" => "chicago-notes",
        "chicago-author-date" | "harvard" => "chicago-author-date",
        "mla" => "mla",
        "apa" | "asa" => "apa",
        "ieee" => "ieee",
        "gost-r-705" => "gost-r-705-2008-numeric",
        "vancouver" => "vancouver",
        _ => "apa",
    }
}

fn package_import(key: &str) -> Option<&'static str> {
    match key {
        "pkg_droplet" => Some("#import \"@preview/droplet:0.3.1\": dropcap"),
        "pkg_codly" => {
            Some("#import \"@preview/codly:1.3.0\": *\n#show: codly-init.with()")
        }
        "pkg_showybox" => Some("#import \"@preview/showybox:2.0.4\": showybox"),
        "pkg_gentle" => Some("#import \"@preview/gentle-clues:1.2.0\": *"),
        "pkg_tablex" => Some("#import \"@preview/tablex:0.0.9\": tablex, cellx"),
        "pkg_marginalia" => Some(
            "#import \"@preview/marginalia:0.3.1\" as marginalia: note, notefigure, wideblock\n\
             #show: marginalia.setup.with()",
        ),
        "pkg_drafting" => Some("#import \"@preview/drafting:0.2.2\": *"),
        _ => None,
    }
}

fn language_block(lang_key: &str) -> Option<&'static str> {
    // Each block sets up inline helpers for inserting foreign-script words inside
    // English text. Usage: #ru[Привет], #he[שלום], etc.
    // The document language/direction is NOT changed globally.
    match lang_key {
        "lang_ru" => Some(
            "// Russian inline helper — wraps Cyrillic words with the right font.\n\
             // Needs a font with Cyrillic coverage (Linux Libertine O, FreeSerif, XITS, etc.).\n\
             // Usage: #ru[Привет мир]\n\
             #let ru(content) = text(\n\
               lang: \"ru\",\n\
               font: (\"Linux Libertine O\", \"FreeSerif\", \"XITS\"),\n\
               content\n\
             )",
        ),
        "lang_he" => Some(
            "// Hebrew inline helper — wraps RTL words, preserving LTR document flow.\n\
             // Usage: #he[שָׁלוֹם]\n\
             #let he(content) = text(\n\
               lang: \"he\",\n\
               dir: rtl,\n\
               content\n\
             )",
        ),
        "lang_el" => Some(
            "// Ancient / Modern Greek inline helper.\n\
             // Needs a Unicode polytonic font (Linux Libertine O, GFS Artemisia, Gentium Plus).\n\
             // Usage: #el[λόγος]\n\
             #let el(content) = text(\n\
               lang: \"el\",\n\
               font: (\"Linux Libertine O\", \"GFS Artemisia\", \"Gentium Plus\"),\n\
               content\n\
             )",
        ),
        "lang_ja" => Some(
            "// Japanese inline helper — install Noto Serif CJK JP (or Source Han Serif JP).\n\
             // Linux/openSUSE: zypper install google-noto-serif-cjk-fonts\n\
             // Usage: #ja[日本語]\n\
             #let ja(content) = text(\n\
               lang: \"ja\",\n\
               font: (\"Noto Serif CJK JP\", \"Source Han Serif JP\"),\n\
               content\n\
             )",
        ),
        "lang_sa" => Some(
            "// Sanskrit / Devanagari inline helper — install Noto Serif Devanagari.\n\
             // Linux/openSUSE: zypper install google-noto-serif-devanagari-fonts\n\
             // Usage: #sa[संस्कृत]\n\
             #let sa(content) = text(\n\
               lang: \"sa\",\n\
               font: (\"Noto Serif Devanagari\", \"Sanskrit 2003\"),\n\
               content\n\
             )",
        ),
        "lang_bo" => Some(
            "// Tibetan inline helper — install Noto Serif Tibetan.\n\
             // Linux/openSUSE: zypper install google-noto-serif-tibetan-fonts\n\
             // Usage: #bo[བོད་སྐད]\n\
             #let bo(content) = text(\n\
               lang: \"bo\",\n\
               font: \"Noto Serif Tibetan\",\n\
               content\n\
             )",
        ),
        "lang_zh" => Some(
            "// Chinese (Simplified) inline helper — install Noto Serif CJK SC.\n\
             // Linux/openSUSE: zypper install google-noto-serif-cjk-fonts\n\
             // Usage: #zh[中文]\n\
             #let zh(content) = text(\n\
               lang: \"zh\",\n\
               font: (\"Noto Serif CJK SC\", \"Source Han Serif SC\"),\n\
               content\n\
             )",
        ),
        _ => None,
    }
}

pub fn heading_styles(style_key: &str) -> &'static str {
    match style_key {
        "sbl" => {
            // SBL HS §2.4/§2.6: H1 centred ALL CAPS; H2 centred bold; H3 centred plain;
            // H4 flush-left bold italic; H5 flush-left plain
            r#"
// SBL heading styles
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#upper(it.body)]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(weight: "bold")[#it.body]]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.6em, below: 0.3em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#it.body]
]
#show heading.where(level: 4): it => block(width: 100%, above: 0.5em, below: 0.2em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold", style: "italic")[#it.body]
]
#show heading.where(level: 5): it => block(width: 100%, above: 0.4em, below: 0.1em)[
  #set par(first-line-indent: 0pt)
  #it.body
]"#
        }
        "chicago-notes" => {
            r#"
// Chicago (Notes-Bibliography) heading styles
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(weight: "bold")[#it.body]]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(style: "italic")[#it.body]]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.6em, below: 0.2em)[
  #set par(first-line-indent: 0pt)
  #text(style: "italic")[#it.body]
]"#
        }
        "turabian" => {
            // Turabian §A.2.2.4: H1 centred bold; H2 centred plain; H3 flush-left italic
            r#"
// Turabian heading styles
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(weight: "bold")[#it.body]]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#it.body]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.6em, below: 0.2em)[
  #set par(first-line-indent: 0pt)
  #text(style: "italic")[#it.body]
]"#
        }
        "mla" => {
            r#"
// MLA heading styles (no decorative formatting)
#show heading: it => block(width: 100%, above: 0.6em, below: 0.3em)[
  #set par(first-line-indent: 0pt)
  #text(it.body)
]"#
        }
        "latex" => {
            r#"
// LaTeX-look heading spacing
#show heading: set block(above: 1.4em, below: 1em)"#
        }
        "apa" | "harvard" => {
            // APA 7 §2.27; Harvard follows same hierarchy
            r#"
// APA heading styles
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(weight: "bold")[#it.body]]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold")[#it.body]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.6em, below: 0.2em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold", style: "italic")[#it.body]
]"#
        }
        "chicago-author-date" => {
            r#"
// Chicago (Author-Date) heading styles
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(weight: "bold")[#it.body]]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(style: "italic")[#it.body]]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.6em, below: 0.2em)[
  #set par(first-line-indent: 0pt)
  #text(style: "italic")[#it.body]
]"#
        }
        "ieee" => {
            // IEEE Std: H1 centred bold ALL CAPS with Roman-numeral numbering;
            // H2 flush-left bold italic with capital-letter numbering; H3 run-in italic.
            r#"
// IEEE heading styles
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(weight: "bold")[#upper(it.body)]]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold", style: "italic")[#it.body]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.4em, below: 0em)[
  #set par(first-line-indent: 0pt)
  #text(style: "italic")[#it.body]
]"#
        }
        "gost-r-705" => {
            // GOST R 7.0-5: numbered decimal headings; H1 centred bold upper;
            // H2 flush-left bold; H3 flush-left bold italic.
            r#"
// GOST R 7.0-5 heading styles
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(weight: "bold")[#upper(it.body)]]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold")[#it.body]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.6em, below: 0.2em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold", style: "italic")[#it.body]
]"#
        }
        "vancouver" => {
            // Vancouver (ICMJE): numbered headings; H1 bold; H2 bold italic; H3 italic run-in
            r#"
// Vancouver heading styles
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold")[#it.body]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold", style: "italic")[#it.body]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.4em, below: 0em)[
  #set par(first-line-indent: 0pt)
  #text(style: "italic")[#it.body]
]"#
        }
        "asa" => {
            // ASA §4.2: H1 flush-left ALL CAPS no bold; H2 flush-left italic;
            // H3 indented italic run-in with trailing period
            r#"
// ASA heading styles
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #upper(it.body)
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #text(style: "italic")[#it.body]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.4em, below: 0em)[
  #set par(first-line-indent: 0pt)
  #h(0.5in)#text(style: "italic")[#(it.body + [.])]
]"#
        }
        _ => {
            r#"
// Default heading styles
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(weight: "bold")[#it.body]]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold")[#it.body]
]"#
        }
    }
}

// Returns (numbering_on, format_string) by scanning the template block for
// `#set heading(numbering: "...")`.
fn extract_heading_numbering(block: &str) -> (bool, String) {
    for line in block.lines() {
        if let Some(rest) = line.trim().strip_prefix("#set heading(numbering: \"") {
            if let Some(end) = rest.find('"') {
                return (true, rest[..end].to_string());
            }
        }
    }
    (false, String::new())
}

// Injects a counter display before each heading body reference when numbering
// is enabled. Uses the format string directly so no `it.numbering` field access
// is needed — `it.numbering` is not available in Typst's non-PDF export modes.
fn inject_heading_numbering(rules: &str, numbering_on: bool, format: &str) -> String {
    if !numbering_on {
        return rules.to_string();
    }
    let fmt = if format.is_empty() { "1." } else { format };
    let prefix = format!("#context counter(heading).display(\"{fmt}\")#h(0.3em)");
    rules
        .replace("#upper(it.body)", &format!("{prefix}#upper(it.body)"))
        .replace("#text(it.body)", &format!("{prefix}#text(it.body)"))
        .replace("#it.body", &format!("{prefix}#it.body"))
}

// ── Preview helper ────────────────────────────────────────────────────────────

const PREVIEW_BIB: &str = r#"@book{smith2020,
  author = {Smith, John A.},
  title = {Academic Writing: A Comprehensive Guide},
  year = {2020},
  publisher = {Oxford University Press},
  address = {Oxford},
}
@article{jones2019,
  author = {Jones, Jane B.},
  title = {Modern Approaches to Scholarly Communication},
  journal = {Journal of Academic Research},
  year = {2019},
  volume = {45},
  number = {3},
  pages = {123--145},
}
"#;

fn generate_preset_preview(idx: usize) -> Result<Vec<u8>, String> {
    // Check on-disk cache first (24 h TTL, version-tagged)
    let version = env!("CARGO_PKG_VERSION");
    let cache_path = std::env::temp_dir()
        .join(format!("zerkalo_tmpl_preview_{idx}_v{version}.png"));
    let cache_valid = std::fs::metadata(&cache_path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|mtime| mtime.elapsed().ok())
        .map(|age| age < std::time::Duration::from_secs(86400))
        .unwrap_or(false);
    if cache_valid {
        if let Ok(bytes) = std::fs::read(&cache_path) {
            return Ok(bytes);
        }
    }

    let p = &TEMPLATE_PRESETS[idx];
    let bib_key = CITATION_STYLES
        .get(p.style_idx as usize)
        .map(|(_, k)| *k)
        .unwrap_or("chicago-notes");
    let bib_style_name = bib_style(bib_key);
    let spacing = SPACING_OPTIONS
        .get(p.spacing_idx as usize)
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| "1.5em".to_string());

    let settings = TemplateSettings {
        title: "Sample Document".to_string(),
        subtitle: String::new(),
        author: "Author Name".to_string(),
        affiliation: "Sample University".to_string(),
        course: String::new(),
        professor: String::new(),
        date: "2026".to_string(),
        style_idx: p.style_idx as usize,
        paper_idx: p.paper_idx as usize,
        custom_paper_w: String::new(),
        custom_paper_h: String::new(),
        margin_idx: p.margin_idx as usize,
        custom_margin: String::new(),
        font: "Times New Roman".to_string(),
        font_size: "12pt".to_string(),
        spacing,
        page_num_pos: p.page_num_pos,
        header_style: p.header_idx,
        include_toc: false,
        toc_depth: 2,
        include_abstract: p.include_abstract,
        abstract_text: "This sample abstract demonstrates the layout for this template style. \
            It summarises the main argument and methodology of the paper."
            .to_string(),
        include_keywords: false,
        keywords: String::new(),
        heading_numbering: false,
        numbering_format: String::new(),
        languages: Vec::new(),
        packages: Vec::new(),
        dropcap_font: String::new(),
        dropcap_lines: 3,
        dropcap_color: String::new(),
        body_kind: p.body_kind,
        bib_path: None,
    };

    let mut preamble = generate_typst_template(&settings);

    // For CVs and letters the template already contains full content — no body to append.
    if matches!(p.body_kind, BodyKind::Cv | BodyKind::Letter) {
        let tmp_dir = std::env::temp_dir();
        let typ_path = tmp_dir.join(format!("zerkalo_tmpl_preview_{idx}.typ"));
        std::fs::write(&typ_path, &preamble).map_err(|e| e.to_string())?;
        // CV templates `#import "cv-helpers.typ"` — inject it as a virtual
        // override next to the preview file rather than relying on a real
        // file existing at that path (nothing else in the app writes one
        // there, so every CV preset preview failed to compile before this).
        let mut overrides = std::collections::HashMap::new();
        if matches!(p.body_kind, BodyKind::Cv) {
            overrides.insert(tmp_dir.join("cv-helpers.typ"), crate::cv_mode::CV_HELPERS_TYPST.to_string());
        }
        return crate::compiler::compile_to_png_bytes(&typ_path, 1.5, &overrides, &std::collections::HashMap::new())
            .map(|pages| {
                let png = pages.into_iter().next().unwrap_or_default();
                let _ = std::fs::write(&cache_path, &png);
                png
            });
    }

    // Replace the starter body with richer sample content
    let body = match p.body_kind {
        BodyKind::Book => PREVIEW_BOOK_BODY,
        BodyKind::Academic | BodyKind::Cv | BodyKind::Letter => PREVIEW_ACADEMIC_BODY,
    };
    // Strip everything from the first chapter/section marker onward and append rich body
    let marker = match p.body_kind {
        BodyKind::Book => "// ── Chapters",
        BodyKind::Academic | BodyKind::Cv | BodyKind::Letter => "// ── Document body",
    };
    if let Some(pos) = preamble.find(marker) {
        preamble.truncate(pos);
    }
    let bib_line = format!("#bibliography(\"zerkalo_preview_refs.bib\", style: \"{bib_style_name}\")");
    preamble.push_str(body);
    preamble.push('\n');
    preamble.push_str(&bib_line);
    preamble.push('\n');

    let tmp_dir = std::env::temp_dir();
    let bib_path = tmp_dir.join("zerkalo_preview_refs.bib");
    let typ_path = tmp_dir.join(format!("zerkalo_tmpl_preview_{idx}.typ"));
    std::fs::write(&bib_path, PREVIEW_BIB).map_err(|e| e.to_string())?;
    std::fs::write(&typ_path, &preamble).map_err(|e| e.to_string())?;

    crate::compiler::compile_to_png_bytes(&typ_path, 1.5, &std::collections::HashMap::new(), &std::collections::HashMap::new())
        .map(|pages| {
            // Page 2 shows the content style; fall back to page 1 if only one page
            let page_idx = if pages.len() > 1 { 1 } else { 0 };
            let png = pages.into_iter().nth(page_idx).unwrap_or_default();
            let _ = std::fs::write(&cache_path, &png);
            png
        })
}

const PREVIEW_ACADEMIC_BODY: &str = r#"
= Introduction

This study examines the nature of academic discourse @jones2019[p.~125].
As recent scholarship has shown, effective writing requires careful
attention to structure and argumentation @smith2020.
The present analysis builds upon established frameworks to offer
a new perspective on scholarly communication.

Prior research has established the fundamental importance of clear
argumentation in academic writing @smith2020[chap.~3].
The relationship between form and content has been the subject of
considerable scholarly debate, and the field continues to evolve.

== Background

Several theoretical traditions have informed this inquiry.
First, rhetorical theory emphasises the importance of audience
awareness and situational context @jones2019.
Second, genre theory draws attention to the conventions that
govern scholarly communication across disciplines @smith2020.

= Methods

The methodology employed in this study follows standard practices
in the field @jones2019. Data were gathered from a range of primary
and secondary sources, then analysed using established interpretive
techniques. Each source was evaluated for reliability and relevance.

== Data Analysis

The analysis proceeded in three stages. First, all sources were
catalogued and cross-referenced. Second, thematic patterns were
identified across the corpus. Third, these patterns were interpreted
in light of current theoretical frameworks @smith2020[pp.~45--67].

= Conclusion

This paper has demonstrated the central role of clear structure
in academic writing @smith2020.
Further research should examine the ways in which digital tools
continue to reshape scholarly communication practices @jones2019.

#pagebreak()

"#;

const PREVIEW_BOOK_BODY: &str = r#"
= Chapter One: The Beginning

The first chapter opens the narrative, establishing the world
and the central questions that will guide the reader through the text.
Here the author sets the stage, introducing the key themes that
will develop over the course of the work.

Second paragraph continues the opening, deepening the scene and
beginning to draw the reader into the argument.
Details accumulate, each one chosen to serve the larger purpose
of the work.

== A First Section

Further elaboration follows, each paragraph contributing to the whole.
The author's voice comes through clearly, guiding the reader
with care and precision through the material.

#pagebreak()

= Chapter Two: Development

The second chapter advances the central argument, building on
the foundations laid in the opening chapter.
New material is introduced, connecting to earlier themes
while moving the narrative forward in unexpected directions.

Further paragraphs deepen the analysis, drawing threads together
and pointing toward the resolution that the final chapters
will bring. The reader is carried forward by the momentum
of the argument and the clarity of the prose.

#pagebreak()

"#;

const TEMPLATE_BEGIN: &str = "// ZERKALO-TEMPLATE-BEGIN";
const TEMPLATE_END: &str = "// ZERKALO-TEMPLATE-END";

/// Extract the `@zerkalo-style` key from a document's header comments.
/// Map a `@zerkalo-style` key back to the human-readable citation style name.
pub fn style_name_for_key(key: &str) -> Option<&'static str> {
    CITATION_STYLES.iter().find(|(_, k)| *k == key).map(|(name, _)| *name)
}

pub fn parse_style_key(content: &str) -> Option<String> {
    for line in content.lines() {
        if let Some(rest) = line.trim().strip_prefix("// @zerkalo-style:") {
            let key = rest.trim().to_string();
            if !key.is_empty() {
                return Some(key);
            }
        }
    }
    None
}

pub fn parse_doc_kind(content: &str) -> Option<String> {
    for line in content.lines().take(20) {
        if let Some(rest) = line.trim().strip_prefix("// @zerkalo-kind:") {
            let kind = rest.trim().to_string();
            if !kind.is_empty() { return Some(kind); }
        }
    }
    None
}

pub fn parse_cv_style(content: &str) -> Option<String> {
    for line in content.lines().take(20) {
        if let Some(rest) = line.trim().strip_prefix("// @zerkalo-cv-style:") {
            let style = rest.trim().to_string();
            if !style.is_empty() { return Some(style); }
        }
    }
    None
}

/// Parse `#set text(font: "…")` from document content.
pub fn parse_font(content: &str) -> Option<String> {
    // Handle both inline  ("#set text(font: "X", ...)")
    // and multi-line  ("#set text(\n  font: "X",\n)") forms.
    // Returns the LAST occurrence so the effective (overriding) value is reported.
    let mut last_found: Option<String> = None;
    let mut in_set_text = false;
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("//") { continue; }
        if t.starts_with("#set text(") {
            in_set_text = true;
        }
        if in_set_text {
            if let Some(start) = t.find("font:") {
                let after = t[start + 5..].trim_start();
                if let Some(after) = after.strip_prefix('"') {
                    if let Some(end) = after.find('"') {
                        let f = after[..end].to_string();
                        if !f.is_empty() { last_found = Some(f); }
                    }
                }
            }
            // Close the block: inline form ends with ")" on same line as "#set text(",
            // multi-line form has ")" alone on its own line.
            let opened_inline = t.starts_with("#set text(") && t.contains(')');
            let closed_alone  = !t.starts_with("#set text(") && t.starts_with(')');
            if opened_inline || closed_alone {
                in_set_text = false;
            }
        }
    }
    last_found
}

pub fn parse_dropcap_font(content: &str) -> Option<String> {
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("#let dropcap = dropcap.with(") {
            if let Some(start) = t.find("font:") {
                let after = t[start + 5..].trim_start();
                if let Some(after) = after.strip_prefix('"') {
                    if let Some(end) = after.find('"') {
                        let f = after[..end].to_string();
                        if !f.is_empty() { return Some(f); }
                    }
                }
            }
        }
    }
    None
}

pub fn parse_dropcap_color(content: &str) -> Option<String> {
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("#let dropcap = dropcap.with(") {
            if let Some(start) = t.find("fill:") {
                let after = t[start + 5..].trim_start();
                let value: String = after
                    .chars()
                    .take_while(|c| *c != ',' && *c != ')')
                    .collect();
                let value = value.trim().to_string();
                if !value.is_empty() { return Some(value); }
            }
        }
    }
    None
}

/// Parse `size: Xpt` from `#set text(…)` in document content.
pub fn parse_font_size(content: &str) -> Option<String> {
    let mut last_found: Option<String> = None;
    let mut in_set_text = false;
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("//") { continue; }
        if t.starts_with("#set text(") { in_set_text = true; }
        if in_set_text {
            if let Some(start) = t.find("size:") {
                let after = t[start + 5..].trim_start();
                let token: String = after.chars().take_while(|c| !c.is_whitespace() && *c != ',').collect();
                if !token.is_empty() { last_found = Some(token); }
            }
            let opened_inline = t.starts_with("#set text(") && t.contains(')');
            let closed_alone  = !t.starts_with("#set text(") && t.starts_with(')');
            if opened_inline || closed_alone { in_set_text = false; }
        }
    }
    last_found
}

/// Parse `paper: "…"` from `#set page(…)` in document content.
pub fn parse_paper(content: &str) -> Option<String> {
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("paper:") || t.contains("paper:") {
            if let Some(start) = t.find("paper:") {
                let after = t[start + 6..].trim_start();
                if let Some(after) = after.strip_prefix('"') {
                    if let Some(end) = after.find('"') {
                        let p = after[..end].to_string();
                        if !p.is_empty() { return Some(p); }
                    }
                }
            }
        }
    }
    None
}

/// Parse `leading: …` from `#set par(…)` in document content.
/// Returns the LAST effective value so the overriding occurrence is reported.
pub fn parse_spacing(content: &str) -> Option<String> {
    let mut last_found: Option<String> = None;
    let mut in_set_par = false;
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("//") { continue; }
        if t.starts_with("#set par(") { in_set_par = true; }
        if in_set_par {
            if let Some(start) = t.find("leading:") {
                let after = t[start + 8..].trim_start();
                let val: String = after.chars().take_while(|c| !matches!(c, ',' | ')')).collect();
                let val = val.trim().to_string();
                if !val.is_empty() { last_found = Some(val); }
            }
            let opened_inline = t.starts_with("#set par(") && t.contains(')');
            let closed_alone  = !t.starts_with("#set par(") && t.starts_with(')');
            if opened_inline || closed_alone { in_set_par = false; }
        }
    }
    last_found
}

/// Detect the margin preset index (0=Normal, 1=Narrow, 2=Wide, 3=LaTeX, 4=Ross)
/// from the content. Reads the left/right-margin values from
/// `#set page(margin: (...))` (generated on one line) and maps them to a preset.
pub fn parse_margin(content: &str) -> usize {
    let mut in_page = false;
    let mut in_margin = false;
    let mut paren_depth = 0i32;
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("//") { continue; }
        if t.starts_with("#set page(") { in_page = true; }
        if in_page {
            if t.contains("margin:") { in_margin = true; }
            if in_margin {
                if let Some(pos) = t.find("left:") {
                    let after = t[pos + 5..].trim_start();
                    let val: String = after.chars()
                        .take_while(|c| !matches!(c, ',' | ')'))
                        .collect();
                    let val = val.trim();
                    // Ross's distinctive percentage right margin is checked first
                    // since its left value (1.25in) is otherwise identical to Normal's.
                    if let Some(rpos) = t.find("right:") {
                        let rafter = t[rpos + 6..].trim_start();
                        let rval: String = rafter.chars()
                            .take_while(|c| !matches!(c, ',' | ')'))
                            .collect();
                        if rval.trim().contains('%') { return 4; }
                    }
                    if val.starts_with("0.5") { return 1; }
                    if val.starts_with("2in") || val == "2in" { return 2; }
                    if val.starts_with("1.75in") { return 3; }
                    return 0; // Normal (1.25in) or unrecognised
                }
                paren_depth += t.chars().filter(|&c| c == '(').count() as i32;
                paren_depth -= t.chars().filter(|&c| c == ')').count() as i32;
                if paren_depth <= 0 { in_margin = false; paren_depth = 0; }
            }
            // End of #set page block
            let inline = t.starts_with("#set page(") && t.ends_with(')');
            let alone  = !t.starts_with("#set page(") && t.trim() == ")";
            if inline || alone { in_page = false; in_margin = false; }
        }
    }
    0
}

/// Remove the legacy ZERKALO-STYLE-BEGIN/END block if present. The template section
/// owns font, spacing, and page settings; a stale style block after it would override them.
/// Generate a minimal Zerkalo template preamble for wrapping imported content.
/// Returns the TEMPLATE_BEGIN…TEMPLATE_END block with sensible academic defaults.
/// The user can immediately update font, spacing, and citation style via
/// "Update Template Settings" after import.
pub fn default_import_preamble() -> String {
    let settings = TemplateSettings {
        title: String::new(),
        subtitle: String::new(),
        author: String::new(),
        affiliation: String::new(),
        course: String::new(),
        professor: String::new(),
        date: String::new(),
        style_idx: 1,    // Chicago (Notes-Bib) — common humanities default
        paper_idx: 0,    // US Letter
        custom_paper_w: String::new(),
        custom_paper_h: String::new(),
        margin_idx: 0,   // Normal (1" / 1.25")
        custom_margin: String::new(),
        font: "Times New Roman".to_string(),
        font_size: "12pt".to_string(),
        spacing: "0.9em".to_string(),
        page_num_pos: 0, // Bottom center
        header_style: 0,
        include_toc: false,
        toc_depth: 2,
        include_abstract: false,
        abstract_text: String::new(),
        include_keywords: false,
        keywords: String::new(),
        heading_numbering: false,
        numbering_format: String::new(),
        languages: vec![],
        packages: vec![],
        dropcap_font: String::new(),
        dropcap_lines: 3,
        dropcap_color: String::new(),
        body_kind: BodyKind::default(),
        bib_path: None,
    };
    let full = generate_typst_template(&settings);
    if let Some(end_pos) = full.find(TEMPLATE_END) {
        format!("{}\n", &full[..end_pos + TEMPLATE_END.len()])
    } else {
        String::new()
    }
}

/// Remove any `#show heading` and `#set heading(numbering:...)` rules that appear
/// OUTSIDE the TEMPLATE markers. Those rules always override the template block's
/// heading styles (Typst applies the last-defined show rule), so they must be gone
/// for the style guide to take full effect.
pub fn strip_conflicting_heading_rules(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result: Vec<&str> = Vec::with_capacity(lines.len());
    let mut in_template = false;
    let mut skipping_show = false;
    let mut bracket_depth = 0i32;

    for &line in &lines {
        let t = line.trim();

        // Track entry/exit of the template block — keep everything inside it unchanged.
        if t == TEMPLATE_BEGIN { in_template = true; }
        if in_template {
            result.push(line);
            if t == TEMPLATE_END { in_template = false; }
            continue;
        }

        // Skip continuation lines of a multi-line #show heading block.
        if skipping_show {
            bracket_depth += t.chars().filter(|&c| c == '[').count() as i32;
            bracket_depth -= t.chars().filter(|&c| c == ']').count() as i32;
            if bracket_depth <= 0 {
                skipping_show = false;
                bracket_depth = 0;
            }
            continue;
        }

        // Drop any #show heading rule (single- or multi-line).
        if t.starts_with("#show heading") {
            bracket_depth = t.chars().filter(|&c| c == '[').count() as i32
                          - t.chars().filter(|&c| c == ']').count() as i32;
            if bracket_depth > 0 { skipping_show = true; }
            continue;
        }

        // Drop #set heading(numbering: ...) — always single-line.
        if t.starts_with("#set heading(") {
            continue;
        }

        result.push(line);
    }

    let joined = result.join("\n");
    if content.ends_with('\n') && !joined.ends_with('\n') {
        joined + "\n"
    } else {
        joined
    }
}

pub fn strip_style_block(content: &str) -> String {
    const STYLE_BEGIN: &str = "// ZERKALO-STYLE-BEGIN";
    const STYLE_END: &str = "// ZERKALO-STYLE-END";
    let (Some(begin_pos), Some(end_pos)) = (content.find(STYLE_BEGIN), content.find(STYLE_END))
    else {
        return content.to_string();
    };
    let end_full = end_pos + STYLE_END.len();
    let after = if content[end_full..].starts_with('\n') { end_full + 1 } else { end_full };
    format!("{}{}", &content[..begin_pos], &content[after..])
}

// ── Template-aware style application ─────────────────────────────────────────

/// Replace the heading styles section within the TEMPLATE block when the user
/// selects a new style from the dropdown. Also updates the @zerkalo-style annotation.
/// For template documents only; non-template documents use the legacy STYLE block path.
pub fn replace_heading_styles_in_template(content: &str, style_key: &str) -> String {
    let (Some(begin_pos), Some(end_marker_pos)) = (
        content.find(TEMPLATE_BEGIN),
        content.find(TEMPLATE_END),
    ) else {
        return content.to_string();
    };

    let block_end = end_marker_pos + TEMPLATE_END.len();
    let before_block = &content[..begin_pos];
    let after_block = &content[block_end..];
    let template_block = &content[begin_pos..block_end];

    let updated_block = update_template_block_headings(template_block, style_key);

    // Safety check: if the heading replacement lost the template markers, return
    // the original unchanged rather than writing a broken document.
    if !updated_block.contains(TEMPLATE_BEGIN) || !updated_block.contains(TEMPLATE_END) {
        tracing::error!(
            "replace_heading_styles_in_template: heading replacement produced a \
             block without TEMPLATE markers for key '{style_key}' — returning original"
        );
        return content.to_string();
    }

    let with_headings = format!("{before_block}{updated_block}{after_block}");

    // Strip any legacy ZERKALO-STYLE-BEGIN block — it conflicts with the template block.
    let no_style_block = strip_style_block(&with_headings);

    // Strip any #show heading / #set heading(numbering:) rules that sit outside the
    // template markers — they override the template's heading styles in Typst.
    strip_conflicting_heading_rules(&no_style_block)
}

/// Update the `paper:` and `margin:` values inside `#set page(...)` for the new style.
/// GOST 7.32 mandates A4 + specific margins; switching away from GOST resets to normal.
/// Other style transitions keep the current margin.
fn update_page_settings_for_style(block: &str, new_style_key: &str) -> String {
    let is_currently_gost = block.contains("left: 30mm");
    if new_style_key == "gost-r-705" {
        // Force GOST mandatory page settings.
        let b = replace_in_line(block, "paper:", "paper: \"a4\",");
        replace_margin_line(&b, "top: 20mm, bottom: 20mm, left: 30mm, right: 15mm")
    } else if is_currently_gost {
        // Leaving GOST — restore Normal letter-size settings.
        let b = replace_in_line(block, "paper:", "paper: \"us-letter\",");
        replace_margin_line(&b, "top: 1in, bottom: 1in, left: 1.25in, right: 1.25in")
    } else {
        block.to_string()
    }
}

/// Replace a `paper: "..."` line inside the block.
fn replace_in_line(block: &str, key: &str, new_full_line_content: &str) -> String {
    block.lines().map(|line| {
        let t = line.trim();
        if !t.starts_with("//") && t.starts_with(key) {
            // Preserve indentation
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            format!("{indent}{new_full_line_content}")
        } else {
            line.to_string()
        }
    }).collect::<Vec<_>>().join("\n") + if block.ends_with('\n') { "\n" } else { "" }
}

/// Replace `margin: (...)` — possibly multi-line — with a single-line version.
fn replace_margin_line(block: &str, new_margin: &str) -> String {
    let mut result = String::new();
    let mut skip_margin = false;
    for line in block.lines() {
        let t = line.trim();
        if !t.starts_with("//") && (t.starts_with("margin:") || (skip_margin)) {
            if !skip_margin {
                // First line of margin block — emit replacement
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                result.push_str(&format!("{indent}margin: ({new_margin}),\n"));
                // If the original margin is multi-line (no closing paren on this line), skip until we find it
                if !t.contains(')') {
                    skip_margin = true;
                }
            } else {
                // Continuation line — skip it
                if t.contains(')') {
                    skip_margin = false;
                }
            }
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }
    if !block.ends_with('\n') && result.ends_with('\n') {
        result.truncate(result.len() - 1);
    }
    result
}

/// Heading numbering that a style mandates when the document doesn't already
/// have numbering explicitly configured. Mirrors `preselect_style`'s defaults
/// so switching an existing document's style behaves the same as creating one.
fn mandated_heading_numbering(style_key: &str) -> Option<&'static str> {
    match style_key {
        "ieee" => Some("I.A.1."),
        "gost-r-705" | "vancouver" => Some("1."),
        _ => None,
    }
}

fn update_template_block_headings(block: &str, new_style_key: &str) -> String {
    let (mut num_on, mut num_fmt) = extract_heading_numbering(block);
    if !num_on {
        if let Some(mandated_fmt) = mandated_heading_numbering(new_style_key) {
            num_on = true;
            num_fmt = mandated_fmt.to_string();
        }
    }
    let raw = inject_heading_numbering(
        heading_styles(new_style_key).trim_start_matches('\n'),
        num_on,
        &num_fmt,
    );
    let new_heading_code = raw.trim().to_string();
    let new_heading_code = new_heading_code.as_str();
    let style_name = CITATION_STYLES.iter()
        .find(|(_, k)| *k == new_style_key)
        .map(|(n, _)| *n)
        .unwrap_or("Unknown");

    // Step 1: update @zerkalo-style and creation comment
    let mut annotated = String::new();
    for line in block.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("// @zerkalo-style:") {
            let _ = rest; // suppress unused
            annotated.push_str(&format!("// @zerkalo-style: {new_style_key}\n"));
        } else if t.starts_with("// Created with Zerkalo") {
            annotated.push_str(&format!("// Created with Zerkalo · {style_name} style\n"));
        } else {
            annotated.push_str(line);
            annotated.push('\n');
        }
    }

    // Step 2: replace heading section within the annotated block
    let lines: Vec<&str> = annotated.lines().collect();
    let mut heading_start: Option<usize> = None;
    let mut heading_end: Option<usize> = None;
    let mut in_heading = false;
    let mut bracket_depth = 0i32;

    for (i, &line) in lines.iter().enumerate() {
        let t = line.trim();

        if !in_heading {
            let is_heading_comment = t.starts_with("//") && {
                let lower = t.to_lowercase();
                lower.contains("heading style") || lower.contains("heading styles")
                    || lower.contains("default heading")
            };
            let is_show_heading = t.starts_with("#show heading");
            let is_set_heading_num = t.starts_with("#set heading(");
            if is_heading_comment || is_show_heading || is_set_heading_num {
                heading_start = Some(i);
                in_heading = true;
            }
        }

        if in_heading {
            bracket_depth += t.chars().filter(|&c| c == '[').count() as i32;
            bracket_depth -= t.chars().filter(|&c| c == ']').count() as i32;
            // Clamp to zero — a one-liner rule with balanced brackets reads as 0,
            // and we must not let unmatched `]` in a comment send depth negative
            // (which would wrongly fire the terminator check on every following line).
            if bracket_depth < 0 { bracket_depth = 0; }
            if bracket_depth == 0 {
                let is_lang_block = t.starts_with("//") && t.contains("inline helper");
                let is_template_end = t.starts_with(TEMPLATE_END);
                let is_columns_extra = t == "#set page(columns: 2)";
                if is_lang_block || is_template_end || is_columns_extra {
                    let mut end = i;
                    while end > 0 && lines[end - 1].trim().is_empty() {
                        end -= 1;
                    }
                    heading_end = Some(end);
                    in_heading = false;
                }
            }
        }
    }

    // If no explicit terminator found, heading goes to the last line before TEMPLATE_END
    if heading_start.is_some() && heading_end.is_none() {
        let mut end = lines.len();
        while end > 0 && (lines[end - 1].trim().is_empty()
            || lines[end - 1].trim() == TEMPLATE_END)
        {
            end -= 1;
        }
        heading_end = Some(end);
    }

    if let (Some(start), Some(end)) = (heading_start, heading_end) {
        // Strip any existing #set page(columns: 2) from the after portion
        let after_lines: Vec<&str> = lines[end..]
            .iter()
            .filter(|&&l| l.trim() != "#set page(columns: 2)")
            .cloned()
            .collect();

        let mut result = lines[..start].join("\n");
        result.push('\n');
        result.push_str(new_heading_code);
        result.push('\n');
        if num_on {
            result.push_str(&format!("\n#set heading(numbering: \"{num_fmt}\")\n"));
        }
        if new_style_key == "ieee" {
            result.push_str("\n#set page(columns: 2)\n");
        }
        result.push('\n');
        result.push_str(&after_lines.join("\n"));
        // Ensure trailing newline is preserved correctly
        if !result.ends_with('\n') {
            result.push('\n');
        }
        update_page_settings_for_style(&result, new_style_key)
    } else {
        // No heading section found — insert before TEMPLATE_END
        let numbering_line = if num_on {
            format!("\n#set heading(numbering: \"{num_fmt}\")\n")
        } else {
            String::new()
        };
        let with_headings = annotated.replace(
            TEMPLATE_END,
            &format!("{new_heading_code}{numbering_line}\n\n{TEMPLATE_END}"),
        );
        update_page_settings_for_style(&with_headings, new_style_key)
    }
}

// ── Metadata parsers ─────────────────────────────────────────────────────────

/// Parse metadata for a field. Checks (in order):
/// 1. `#let doc-FIELD = "..."` variable (new format — editing this live-updates the title page)
/// 2. `// @meta:FIELD: ...` comment (old format — backward compatibility)
/// 3. Style-specific content extraction (very old documents)
pub fn parse_meta(content: &str, field: &str) -> String {
    // New format: #let doc-* variable
    let var_name = match field {
        "title"       => "doc-title",
        "subtitle"    => "doc-subtitle",
        "author"      => "doc-author",
        "affiliation" => "doc-affil",
        "course"      => "doc-course",
        "professor"   => "doc-professor",
        "date"        => "doc-date",
        _ => "",
    };
    if !var_name.is_empty() {
        let prefix = format!("#let {var_name} = \"");
        for line in content.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix(&prefix) {
                return parse_typst_string_value(rest);
            }
        }
    }

    // Old format: @meta: comment
    let prefix = format!("// @meta:{field}: ");
    for line in content.lines() {
        if let Some(rest) = line.trim().strip_prefix(&prefix) {
            return rest.trim().to_string();
        }
    }
    // Style-specific fallbacks after TEMPLATE_END (best-effort only)
    let body = content.find(TEMPLATE_END).map(|p| &content[p..]).unwrap_or(content);
    match field {
        "title" => {
            for line in body.lines() {
                let t = line.trim();
                if t.contains("size: 16pt") && t.contains("weight: \"bold\"") {
                    if let Some(s) = extract_first_bracket_content(t) {
                        if !s.is_empty() { return s; }
                    }
                }
            }
        }
        "author" => {
            let mut after_v2fr = false;
            for line in body.lines() {
                let t = line.trim();
                if t == "#v(2fr)" { after_v2fr = true; continue; }
                if !after_v2fr { continue; }
                if t == "]" { break; }
                if t.is_empty() || t.starts_with('#') || t.starts_with('\\') { continue; }
                let cleaned = t.trim_matches(|c| c == '[' || c == ']').trim().to_string();
                if !cleaned.is_empty() { return cleaned; }
            }
        }
        _ => {}
    }
    String::new()
}

/// Parse the content of a Typst string literal up to the first unescaped `"`.
/// Input `s` is everything AFTER the opening `"` of the literal.
fn parse_typst_string_value(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => break,
            '\\' => match chars.next() {
                Some('"')  => result.push('"'),
                Some('\\') => result.push('\\'),
                Some('n')  => result.push('\n'),
                Some(other) => { result.push('\\'); result.push(other); }
                None => {}
            },
            other => result.push(other),
        }
    }
    result
}

fn extract_first_bracket_content(s: &str) -> Option<String> {
    let start = s.find('[')?;
    let rest = &s[start + 1..];
    let mut depth = 1i32;
    for (i, c) in rest.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(rest[..i].trim().to_string());
                }
            }
            _ => {}
        }
    }
    None
}

// ── Body front-matter parsers ─────────────────────────────────────────────────

/// True if the document has a live (uncommented) `#outline(` call.
pub fn parse_has_toc(content: &str) -> bool {
    content.lines().any(|l| {
        let t = l.trim();
        !t.starts_with("//") && t.starts_with("#outline(")
    })
}

pub fn parse_toc_depth(content: &str) -> u32 {
    for line in content.lines() {
        let t = line.trim();
        if !t.starts_with("//") && t.starts_with("#outline(depth:") {
            let after = t["#outline(depth:".len()..].trim_start();
            let val: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = val.parse::<u32>() { return n; }
        }
    }
    2
}

/// True if the document body contains an `*Abstract*` heading.
pub fn parse_has_abstract(content: &str) -> bool {
    content.lines().any(|l| {
        let t = l.trim();
        !t.starts_with("//") && t.contains("*Abstract*")
    })
}

pub fn parse_abstract_text(content: &str) -> String {
    let mut lines = content.lines().peekable();
    while let Some(line) = lines.next() {
        let t = line.trim();
        if !t.starts_with("//") && t.contains("*Abstract*") {
            // Next line may be #block(inset:...) [
            if let Some(next) = lines.next() {
                if next.trim().starts_with("#block(inset:") {
                    if let Some(text_line) = lines.next() {
                        return text_line.trim().to_string();
                    }
                }
            }
            return String::new();
        }
    }
    String::new()
}

/// True if the document body contains a `_Keywords:_` line.
pub fn parse_has_keywords(content: &str) -> bool {
    content.lines().any(|l| {
        let t = l.trim();
        !t.starts_with("//") && t.starts_with("_Keywords:_")
    })
}

pub fn parse_keywords_text(content: &str) -> String {
    for line in content.lines() {
        let t = line.trim();
        if !t.starts_with("//") {
            if let Some(rest) = t.strip_prefix("_Keywords:_") {
                return rest.trim().to_string();
            }
        }
    }
    String::new()
}

// ── Title-page updater ───────────────────────────────────────────────────────

/// Replace the title-page section in `existing` with the one from `new_template`.
/// The title block is identified by the `// ── Title block` comment and ends at
/// the first `#pagebreak()` that follows TEMPLATE_END (or at the body marker for
/// styles without a separate title page).
pub fn replace_title_page(existing: &str, new_template: &str) -> String {
    const TITLE_MARKER: &str = "// ── Title block";

    let Some(new_start) = new_template.find(TITLE_MARKER) else {
        return existing.to_string();
    };
    let Some(old_start) = existing.find(TITLE_MARKER) else {
        return existing.to_string();
    };

    // Find the end of the title block zone: first #pagebreak() that belongs to
    // the title page (i.e. before any front-matter or body marker), or the first
    // such marker when the style has no dedicated title-page break (MLA/IEEE).
    // Searching the whole document was wrong: MLA docs have no title-page break,
    // so the search would find a body #pagebreak() and wipe out the front-matter.
    let title_page_end = |s: &str, zone_start: usize| -> usize {
        let template_end_pos = s.find(TEMPLATE_END)
            .map(|p| p + TEMPLATE_END.len())
            .unwrap_or(0);
        let search_from = zone_start.max(template_end_pos);

        const STOP_MARKERS: &[&str] = &[
            "#align(center)[*Abstract*]",
            "_Keywords:_",
            "#outline(",
            "// ── Document body",
            "// ── Chapters",
        ];
        let stop_pos = STOP_MARKERS.iter()
            .filter_map(|m| s[search_from..].find(m).map(|p| search_from + p))
            .min()
            .unwrap_or(s.len());

        // Only look for a title-page #pagebreak() before the first body/front-matter marker.
        if let Some(pb_off) = s[search_from..stop_pos].find("#pagebreak()") {
            let pb_pos = search_from + pb_off;
            let after = &s[pb_pos + "#pagebreak()".len()..];
            pb_pos + "#pagebreak()".len() + after.find('\n').map(|i| i + 1).unwrap_or(0)
        } else {
            stop_pos
        }
    };

    let new_end = title_page_end(new_template, new_start);
    let old_end = title_page_end(existing, old_start);

    let new_title_block = &new_template[new_start..new_end];
    format!("{}{}{}", &existing[..old_start], new_title_block, &existing[old_end..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gallery_preset_preview_compiles() {
        // Regression coverage for the New from Template gallery: every preset
        // (including the CV ones, whose preview previously always failed —
        // they `#import "cv-helpers.typ"` but generate_preset_preview passed
        // an empty override map, so nothing ever provided that file) must
        // actually render, or the gallery silently shows a blank preview.
        for (idx, p) in TEMPLATE_PRESETS.iter().enumerate() {
            let result = generate_preset_preview(idx);
            assert!(result.is_ok(), "preset {idx} ({}) should preview: {:?}", p.name, result.err());
            assert!(!result.unwrap().is_empty(), "preset {idx} ({}) produced an empty preview", p.name);
        }
    }

    #[test]
    fn sidebar_cv_uses_two_column_layout_and_new_helpers() {
        let settings = TemplateSettings {
            title: String::new(), subtitle: String::new(),
            author: "Jane Doe".to_string(), affiliation: String::new(),
            course: String::new(), professor: String::new(), date: String::new(),
            style_idx: 3, paper_idx: 1,
            custom_paper_w: String::new(), custom_paper_h: String::new(),
            margin_idx: 1, custom_margin: String::new(),
            font: "Linux Libertine".to_string(), font_size: "10.5pt".to_string(),
            spacing: "0.65em".to_string(), page_num_pos: 4, header_style: 0,
            include_toc: false, toc_depth: 2,
            include_abstract: false, abstract_text: String::new(),
            include_keywords: false, keywords: String::new(),
            heading_numbering: false, numbering_format: String::new(),
            languages: Vec::new(), packages: Vec::new(),
            dropcap_font: String::new(), dropcap_lines: 3, dropcap_color: String::new(),
            body_kind: BodyKind::Cv, bib_path: None,
        };
        let src = generate_typst_template(&settings);
        assert!(src.contains("#let CV_STYLE = \"sidebar\""));
        assert!(src.contains("#let mylink(url, label)"));
        assert!(src.contains("#let taglist(items)"));
        assert!(src.contains("#import \"cv-helpers.typ\": cv-section"));
        assert!(src.contains("columns: (1fr, 2fr)"));
        assert!(src.contains("#taglist((\"Interest one\""));
        assert!(src.contains("#cv-section(category: (\"Publication\", \"Presentation\"), style: CV_STYLE)"));
    }

    #[test]
    fn all_cv_styles_compile_with_skrizhal_data() {
        use std::collections::HashMap;

        // One entry per shape (job/edu/award/presentation/tag) so every
        // #cv-section call in the generated template hits real data instead
        // of just the "No entries yet." empty state.
        let cv_data = r#"
pastor-role:
  category: Ministry Position
  title: Youth Pastor
  organization: Hope United Church
  location: Springfield
  date: 2023-01/2025-06
  description:
    - Led weekly youth group
mdiv:
  category: Education
  title: Master of Divinity
  organization: Atlantic School of Theology
  date: 2020/2023
deans-list:
  category: Award
  title: Dean's List
  organization: Springfield Seminary
  date: 2020
  description:
    - Top of cohort
conference-talk:
  category: Presentation
  title: Faith and Community
  organization: Annual Ministry Conference
  date: 2024
  role: Panelist
volunteer-role:
  category: Volunteer
  title: Food Bank Coordinator
  organization: Springfield Food Bank
  date: 2019/2022
french:
  category: Language Skill
  title: French (conversational)
"#;

        fn settings_with_style(style_idx: usize) -> TemplateSettings {
            TemplateSettings {
                title: String::new(), subtitle: String::new(),
                author: "Jane Doe".to_string(), affiliation: String::new(),
                course: String::new(), professor: String::new(), date: String::new(),
                style_idx, paper_idx: 1,
                custom_paper_w: String::new(), custom_paper_h: String::new(),
                margin_idx: 1, custom_margin: String::new(),
                font: "Linux Libertine".to_string(), font_size: "10.5pt".to_string(),
                spacing: "0.65em".to_string(), page_num_pos: 4, header_style: 0,
                include_toc: false, toc_depth: 2,
                include_abstract: false, abstract_text: String::new(),
                include_keywords: false, keywords: String::new(),
                heading_numbering: false, numbering_format: String::new(),
                languages: Vec::new(), packages: Vec::new(),
                dropcap_font: String::new(), dropcap_lines: 3, dropcap_color: String::new(),
                body_kind: BodyKind::Cv, bib_path: None,
            }
        }

        for style_idx in 0..=3 {
            let settings = settings_with_style(style_idx);
            let src = generate_typst_template(&settings);

            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::path::PathBuf::from(format!(
                "/tmp/zerkalo_test_cv_style_{}_{}.typ",
                std::process::id(),
                n
            ));
            std::fs::write(&path, &src).unwrap();

            let cv_helpers_src = include_str!("../../templates/cv-helpers.typ");
            let mut overrides = HashMap::new();
            overrides.insert(
                std::path::PathBuf::from("/tmp/cv-helpers.typ"),
                cv_helpers_src.to_string(),
            );
            let mut inputs = HashMap::new();
            inputs.insert("skrizhal-cv-data".to_string(), cv_data.to_string());

            let result = crate::compiler::compile_to_pdf_bytes(&path, &overrides, &inputs);
            assert!(
                result.is_ok(),
                "CV style_idx={style_idx} should compile: {:?}",
                result.err()
            );
            assert!(result.unwrap().starts_with(b"%PDF-"));

            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn regenerating_legacy_cv_document_keeps_it_compiling() {
        use std::collections::HashMap;

        // Simulates a CV created before the #cv-section rewrite: its body
        // hand-calls #job/#edu/#award directly, and its (now-stale) preamble
        // still defines them. Changing font/paper/margin on a document like
        // this regenerates the preamble from scratch via generate_typst_template
        // — which, post-rewrite, no longer defines those functions — then
        // splices it onto the OLD body via apply_body_splice. Without the
        // legacy-helper reinjection this produces "unknown function: job".
        let settings = TemplateSettings {
            title: String::new(), subtitle: String::new(),
            author: "Jane Doe".to_string(), affiliation: String::new(),
            course: String::new(), professor: String::new(), date: String::new(),
            style_idx: 0, paper_idx: 1,
            custom_paper_w: String::new(), custom_paper_h: String::new(),
            margin_idx: 1, custom_margin: String::new(),
            font: "Linux Libertine".to_string(), font_size: "10.5pt".to_string(),
            spacing: "0.65em".to_string(), page_num_pos: 4, header_style: 0,
            include_toc: false, toc_depth: 2,
            include_abstract: false, abstract_text: String::new(),
            include_keywords: false, keywords: String::new(),
            heading_numbering: false, numbering_format: String::new(),
            languages: Vec::new(), packages: Vec::new(),
            dropcap_font: String::new(), dropcap_lines: 3, dropcap_color: String::new(),
            body_kind: BodyKind::Cv, bib_path: None,
        };

        // generate_typst_template only ever produces the current (post-rewrite)
        // shape now, so build the "old document" fixture the same way
        // apply_body_splice would recognize a genuinely legacy one: current
        // preamble + the legacy helper block manually re-added, matching what
        // a document saved before the rewrite actually looks like on disk.
        let current_preamble = generate_typst_template(&settings);
        assert!(!current_preamble.contains("#let job("), "fresh templates should no longer define #job");
        let idx = current_preamble.find("// ── Document body").unwrap();
        let legacy_preamble = inject_legacy_cv_helpers(&current_preamble[..idx]);
        assert!(legacy_preamble.contains("#let job("), "test fixture must look legacy");
        let legacy_body = "// ── Document body ─────────────────────────────────────────────────────\n\n\
            #job(\"Youth Pastor\", \"Hope United Church\", \"2023 – present\", [Led weekly youth group])\n";
        let legacy_doc = format!("{legacy_preamble}{legacy_body}");

        // Regenerate (as a font change would) and splice onto the legacy body.
        let fresh = generate_typst_template(&settings);
        let spliced = apply_body_splice(&legacy_doc, &fresh);
        assert!(
            spliced.contains("#let job("),
            "splice must reinject legacy helpers so the old body still resolves"
        );

        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::path::PathBuf::from(format!(
            "/tmp/zerkalo_test_legacy_cv_{}_{}.typ",
            std::process::id(),
            n
        ));
        std::fs::write(&path, &spliced).unwrap();

        let cv_helpers_src = include_str!("../../templates/cv-helpers.typ");
        let mut overrides = HashMap::new();
        overrides.insert(
            std::path::PathBuf::from("/tmp/cv-helpers.typ"),
            cv_helpers_src.to_string(),
        );

        let result = crate::compiler::compile_to_pdf_bytes(&path, &overrides, &HashMap::new());
        assert!(
            result.is_ok(),
            "regenerated legacy CV document should still compile: {:?}",
            result.err()
        );
        assert!(result.unwrap().starts_with(b"%PDF-"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn custom_paper_and_margin_generate_expected_typst() {
        let settings = TemplateSettings {
            title: "Custom Test".to_string(),
            subtitle: String::new(),
            author: "Author".to_string(),
            affiliation: String::new(),
            course: String::new(),
            professor: String::new(),
            date: String::new(),
            style_idx: 1,
            paper_idx: 5,
            custom_paper_w: "150".to_string(),
            custom_paper_h: "200".to_string(),
            margin_idx: 5,
            custom_margin: "1.4".to_string(),
            font: "Times New Roman".to_string(),
            font_size: "13pt".to_string(),
            spacing: "0.9em".to_string(),
            page_num_pos: 0,
            header_style: 0,
            include_toc: false,
            toc_depth: 2,
            include_abstract: false,
            abstract_text: String::new(),
            include_keywords: false,
            keywords: String::new(),
            heading_numbering: false,
            numbering_format: String::new(),
            languages: Vec::new(),
            packages: Vec::new(),
            dropcap_font: String::new(),
            dropcap_lines: 3,
            dropcap_color: String::new(),
            body_kind: BodyKind::Academic,
            bib_path: None,
        };
        let src = generate_typst_template(&settings);
        assert!(src.contains("width: 150mm"));
        assert!(src.contains("height: 200mm"));
        assert!(src.contains("margin: (top: 1.4in, bottom: 1.4in, left: 1.4in, right: 1.4in)"));
        assert!(src.contains("size: 13pt"));
    }

    #[test]
    fn parse_font_inline() {
        let doc = "#set text(font: \"Times New Roman\", size: 12pt)\n";
        assert_eq!(parse_font(doc), Some("Times New Roman".to_string()));
    }

    #[test]
    fn parse_font_multiline() {
        let doc = "#set text(\n  font: \"Junicode\",\n  size: 12pt,\n)\n";
        assert_eq!(parse_font(doc), Some("Junicode".to_string()));
    }

    #[test]
    fn parse_font_last_wins() {
        let doc = "#set text(font: \"Arial\", size: 12pt)\n\
                   #set text(\n  font: \"Times New Roman\",\n)\n";
        assert_eq!(parse_font(doc), Some("Times New Roman".to_string()));
    }

    #[test]
    fn parse_font_ignores_comments() {
        let doc = "// font: \"Arial\"\n#set text(font: \"Garamond\", size: 12pt)\n";
        assert_eq!(parse_font(doc), Some("Garamond".to_string()));
    }

    #[test]
    fn parse_paper_basic() {
        let doc = "#set page(paper: \"a4\", margin: (top: 1in))\n";
        assert_eq!(parse_paper(doc), Some("a4".to_string()));
    }

    #[test]
    fn parse_spacing_leading_inline() {
        let doc = "#set par(leading: 0.9em, spacing: 1.2em, justify: true)\n";
        assert_eq!(parse_spacing(doc), Some("0.9em".to_string()));
    }

    #[test]
    fn parse_spacing_leading_multiline() {
        let doc = "#set par(\n  leading: 1.2em,\n  justify: false,\n)\n";
        assert_eq!(parse_spacing(doc), Some("1.2em".to_string()));
    }

    #[test]
    fn parse_spacing_last_wins() {
        let doc = "#set par(leading: 0.65em)\n#set par(\n  leading: 1.2em,\n)\n";
        assert_eq!(parse_spacing(doc), Some("1.2em".to_string()));
    }

    #[test]
    fn parse_spacing_ignores_comments() {
        let doc = "// leading: 1.5em\n#set par(leading: 0.9em)\n";
        assert_eq!(parse_spacing(doc), Some("0.9em".to_string()));
    }

    #[test]
    fn strip_style_block_removes_section() {
        let doc = "before\n// ZERKALO-STYLE-BEGIN\n#set text(font: \"X\")\n// ZERKALO-STYLE-END\nafter\n";
        let result = strip_style_block(doc);
        assert_eq!(result, "before\nafter\n");
    }

    #[test]
    fn strip_style_block_noop_when_absent() {
        let doc = "no style block here\n";
        assert_eq!(strip_style_block(doc), doc);
    }

    #[test]
    fn replace_heading_styles_updates_style_key() {
        let settings = TemplateSettings {
            title: "Test".into(), subtitle: String::new(), author: String::new(),
            affiliation: String::new(), course: String::new(), professor: String::new(), date: String::new(),
            style_idx: 1, // Chicago
            paper_idx: 0, margin_idx: 0,
            font: "Times New Roman".into(), spacing: "0.9em".into(),
            page_num_pos: 0, header_style: 0, include_toc: false, toc_depth: 2,
            include_abstract: false, abstract_text: String::new(),
            include_keywords: false, keywords: String::new(),
            languages: vec![], packages: vec![], dropcap_font: String::new(), dropcap_lines: 3, dropcap_color: String::new(), body_kind: BodyKind::Academic,
            font_size: "12pt".into(), heading_numbering: false, numbering_format: String::new(),
            custom_paper_w: String::new(), custom_paper_h: String::new(), custom_margin: String::new(),
            bib_path: None,
        };
        let doc = generate_typst_template(&settings);
        assert!(doc.contains("@zerkalo-style: chicago-notes"));
        assert!(doc.contains("Chicago (Notes-Bibliography) heading styles"));

        let updated = replace_heading_styles_in_template(&doc, "apa");
        assert!(updated.contains("@zerkalo-style: apa"), "style key updated");
        assert!(updated.contains("APA heading styles"), "new heading comment present");
        assert!(!updated.contains("Chicago (Notes-Bibliography) heading styles"), "old heading removed");
        assert!(!updated.contains("@zerkalo-style: chicago-notes"), "old style key removed");
    }

    #[test]
    fn replace_heading_styles_ieee_adds_columns() {
        let settings = TemplateSettings {
            title: String::new(), subtitle: String::new(), author: String::new(),
            affiliation: String::new(), course: String::new(), professor: String::new(), date: String::new(),
            style_idx: 0, paper_idx: 0, margin_idx: 0,
            font: "Times New Roman".into(), spacing: "0.9em".into(),
            page_num_pos: 0, header_style: 0, include_toc: false, toc_depth: 2,
            include_abstract: false, abstract_text: String::new(),
            include_keywords: false, keywords: String::new(),
            languages: vec![], packages: vec![], dropcap_font: String::new(), dropcap_lines: 3, dropcap_color: String::new(), body_kind: BodyKind::Academic,
            font_size: "12pt".into(), heading_numbering: false, numbering_format: String::new(),
            custom_paper_w: String::new(), custom_paper_h: String::new(), custom_margin: String::new(),
            bib_path: None,
        };
        let doc = generate_typst_template(&settings);
        let to_ieee = replace_heading_styles_in_template(&doc, "ieee");
        assert!(to_ieee.contains("#set page(columns: 2)"), "columns added for ieee");
        assert!(to_ieee.contains("#set heading(numbering: \"I.A.1.\")"), "ieee numbering present");

        // Switch back to non-ieee removes the columns line
        let to_apa = replace_heading_styles_in_template(&to_ieee, "apa");
        assert!(!to_apa.contains("#set page(columns: 2)"), "columns removed when leaving ieee");
    }

    #[test]
    fn strip_conflicting_heading_rules_outside_template() {
        let doc = "\
// ZERKALO-STYLE-BEGIN\n\
#show heading.where(level: 1): it => block[][#it.body]\n\
// ZERKALO-STYLE-END\n\
\n\
// ZERKALO-TEMPLATE-BEGIN\n\
#show heading.where(level: 1): it => block[][#align(center)[#it.body]]\n\
// ZERKALO-TEMPLATE-END\n\
\n\
#set heading(numbering: \"1.1.\")\n\
#show heading: it => [#it.body]\n\
\n\
= Section\n\
Body.\n";
        let result = strip_conflicting_heading_rules(doc);
        // Heading rules inside template block are kept
        assert!(result.contains("#align(center)[#it.body]"), "template heading kept");
        // Heading rules outside template block are removed
        assert!(!result.contains("#set heading(numbering:"), "numbering stripped");
        assert!(!result.contains("#show heading: it => [#it.body]"), "outside show rule stripped");
        // Body content is kept
        assert!(result.contains("= Section"), "content kept");
    }

    #[test]
    fn replace_heading_strips_style_block_and_conflicts() {
        // Simulate the real-world document structure from the imported file
        let doc = "\
// ZERKALO-STYLE-BEGIN\n\
#show heading.where(level: 1): it => block[][old style]\n\
// ZERKALO-STYLE-END\n\
\n\
// ZERKALO-TEMPLATE-BEGIN\n\
// Created with Zerkalo · MLA style\n\
// @zerkalo-style: mla\n\
// @zerkalo-version: 0.7.1\n\
\n\
#set text(font: \"GOST type B\", size: 12pt, lang: \"en\")\n\
#set par(leading: 1.2em, spacing: 1.2em, first-line-indent: 1em, justify: true)\n\
\n\
// MLA heading styles (no decorative formatting)\n\
#show heading: it => block(width: 100%, above: 0.6em, below: 0.3em)[\n\
  #set par(first-line-indent: 0pt)\n\
  #text(it.body)\n\
]\n\
// ZERKALO-TEMPLATE-END\n\
\n\
#set heading(numbering: \"1.1.\")\n\
#show heading: it => [\n\
  custom show rule\n\
]\n\
\n\
= Introduction\n\
Body text.\n";

        let result = replace_heading_styles_in_template(doc, "apa");

        assert!(!result.contains("// ZERKALO-STYLE-BEGIN"), "style block removed");
        assert!(!result.contains("old style"), "old style content gone");
        assert!(!result.contains("numbering: \"1.1.\""), "numbering stripped");
        assert!(!result.contains("custom show rule"), "conflicting show rule stripped");
        assert!(result.contains("@zerkalo-style: apa"), "new style key set");
        assert!(result.contains("APA heading styles"), "new heading comment present");
        assert!(result.contains("= Introduction"), "body content preserved");
    }

    #[test]
    fn parse_body_elements() {
        let doc = "before\n#pagebreak()\n\n#align(center)[*Abstract*]\n#block(inset: (x: 1in))[\n  My abstract text\n]\n\n_Keywords:_ one, two\n\n#outline(depth: 3)\n\n// ── Document body\n= Intro\n";
        assert!(parse_has_toc(doc));
        assert_eq!(parse_toc_depth(doc), 3);
        assert!(parse_has_abstract(doc));
        assert_eq!(parse_abstract_text(doc), "My abstract text");
        assert!(parse_has_keywords(doc));
        assert_eq!(parse_keywords_text(doc), "one, two");
    }

    #[test]
    fn parse_meta_reads_tags() {
        let doc = "// @meta:title: My Essay\n// @meta:author: Jane Smith\n// @meta:date: June 2026\n";
        assert_eq!(parse_meta(doc, "title"), "My Essay");
        assert_eq!(parse_meta(doc, "author"), "Jane Smith");
        assert_eq!(parse_meta(doc, "date"), "June 2026");
        assert_eq!(parse_meta(doc, "subtitle"), ""); // absent = empty
    }

    #[test]
    fn replace_title_page_swaps_block() {
        let settings = TemplateSettings {
            title: "Old Title".to_string(),
            subtitle: String::new(),
            author: "Author".to_string(),
            affiliation: String::new(),
            course: String::new(),
            professor: String::new(),
            date: "2025".to_string(),
            style_idx: 1,
            paper_idx: 0,
            margin_idx: 0,
            font: "Times New Roman".to_string(),
            spacing: "1.2em".to_string(),
            page_num_pos: 0,
            header_style: 0,
            include_toc: false,
            toc_depth: 2,
            include_abstract: false,
            abstract_text: String::new(),
            include_keywords: false,
            keywords: String::new(),
            languages: vec![],
            packages: vec![],
            dropcap_font: String::new(),
            dropcap_lines: 3,
            dropcap_color: String::new(),
            body_kind: BodyKind::Academic,
            font_size: "12pt".into(), heading_numbering: false, numbering_format: String::new(),
            custom_paper_w: String::new(), custom_paper_h: String::new(), custom_margin: String::new(),
            bib_path: None,
        };
        let old_doc = generate_typst_template(&settings);
        let new_settings = TemplateSettings {
            title: "New Title".to_string(),
            author: "New Author".to_string(),
            date: "2026".to_string(),
            ..settings
        };
        let new_doc = generate_typst_template(&new_settings);
        let result = replace_title_page(&old_doc, &new_doc);
        assert!(result.contains("doc-title = \"New Title\""), "new title variable");
        assert!(result.contains("doc-author = \"New Author\""), "new author variable");
        assert!(!result.contains("doc-title = \"Old Title\""), "old title removed");
    }

    // ── Sidecar tests ─────────────────────────────────────────────────────────

    #[test]
    fn sidecar_path_stems_correctly() {
        let p = std::path::Path::new("/home/user/docs/thesis.typ");
        assert_eq!(
            sidecar_path(p),
            std::path::PathBuf::from("/home/user/docs/thesis.zerkalo.toml")
        );
    }

    #[test]
    fn build_and_round_trip_sidecar() {
        let settings = TemplateSettings {
            title: "My Thesis".to_string(),
            subtitle: "A Study".to_string(),
            author: "Cal".to_string(),
            affiliation: "University".to_string(),
            course: "Grad Seminar".to_string(),
            professor: String::new(),
            date: "2026".to_string(),
            style_idx: 4,  // APA 7th
            paper_idx: 1,  // A4
            margin_idx: 0, // Normal
            font: "EB Garamond".to_string(),
            spacing: "1.2em".to_string(),
            page_num_pos: 3,
            header_style: 0,
            include_toc: true,
            toc_depth: 3,
            include_abstract: true,
            abstract_text: "This is the abstract.".to_string(),
            include_keywords: true,
            keywords: "one, two, three".to_string(),
            languages: vec!["lang_ru".to_string(), "lang_he".to_string()],
            packages: vec!["pkg_codly".to_string()],
            dropcap_font: String::new(),
            dropcap_lines: 3,
            dropcap_color: String::new(),
            body_kind: BodyKind::Academic,
            font_size: "12pt".into(), heading_numbering: false, numbering_format: String::new(),
            custom_paper_w: String::new(), custom_paper_h: String::new(), custom_margin: String::new(),
            bib_path: Some(std::path::PathBuf::from("/home/user/refs.bib")),
        };

        let sc = build_sidecar(&settings);
        assert_eq!(sc.style, "apa");
        assert_eq!(sc.paper, "a4");
        assert_eq!(sc.font, "EB Garamond");
        assert_eq!(sc.spacing, "1.2em");
        assert_eq!(sc.page_numbers, 3);
        assert!(sc.toc);
        assert_eq!(sc.toc_depth, 3);
        assert!(sc.abstract_enabled);
        assert_eq!(sc.abstract_text, "This is the abstract.");
        assert!(sc.keywords_enabled);
        assert_eq!(sc.keywords_text, "one, two, three");
        assert_eq!(sc.languages, vec!["lang_ru", "lang_he"]);
        assert_eq!(sc.packages, vec!["pkg_codly"]);
        assert_eq!(sc.body_kind, "academic");
        assert_eq!(sc.bib_path, Some("/home/user/refs.bib".to_string()));

        // Round-trip back to TemplateSettings
        let rt = sidecar_to_settings(&sc);
        assert_eq!(rt.title, "My Thesis");
        assert_eq!(rt.style_idx, 4);  // APA 7th
        assert_eq!(rt.paper_idx, 1);  // A4
        assert_eq!(rt.font, "EB Garamond");
        assert_eq!(rt.spacing, "1.2em");
        assert_eq!(rt.page_num_pos, 3);
        assert!(rt.include_toc);
        assert_eq!(rt.toc_depth, 3);
        assert!(rt.include_abstract);
        assert_eq!(rt.abstract_text, "This is the abstract.");
        assert!(rt.include_keywords);
        assert_eq!(rt.keywords, "one, two, three");
        assert_eq!(rt.languages, vec!["lang_ru", "lang_he"]);
        assert_eq!(rt.packages, vec!["pkg_codly"]);
        assert_eq!(rt.bib_path, Some(std::path::PathBuf::from("/home/user/refs.bib")));
    }

    #[test]
    fn sidecar_toml_serialises_readably() {
        let sc = SidecarSettings {
            title: "Test Doc".to_string(),
            style: "chicago-notes".to_string(),
            font: "Times New Roman".to_string(),
            paper: "us-letter".to_string(),
            margin: 0,
            spacing: "0.9em".to_string(),
            ..Default::default()
        };
        let toml_str = toml::to_string_pretty(&sc).expect("serialise");
        assert!(toml_str.contains("title = \"Test Doc\""));
        assert!(toml_str.contains("style = \"chicago-notes\""));
        assert!(toml_str.contains("font = \"Times New Roman\""));

        let back: SidecarSettings = toml::from_str(&toml_str).expect("deserialise");
        assert_eq!(back.title, "Test Doc");
        assert_eq!(back.style, "chicago-notes");
    }

    #[test]
    fn save_and_load_sidecar_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        let typ_path = dir.path().join("paper.typ");
        std::fs::write(&typ_path, "// placeholder").unwrap();

        let sc = SidecarSettings {
            title: "Saved Paper".to_string(),
            author: "Test Author".to_string(),
            style: "mla".to_string(),
            font: "Palatino".to_string(),
            paper: "a4".to_string(),
            toc: true,
            toc_depth: 2,
            keywords_enabled: true,
            keywords_text: "alpha, beta".to_string(),
            languages: vec!["lang_el".to_string()],
            ..Default::default()
        };

        save_sidecar(&typ_path, &sc);

        let sc_path = sidecar_path(&typ_path);
        assert!(sc_path.exists(), "sidecar file must exist after save");

        let loaded = load_sidecar(&typ_path).expect("load must succeed");
        assert_eq!(loaded.title, "Saved Paper");
        assert_eq!(loaded.author, "Test Author");
        assert_eq!(loaded.style, "mla");
        assert_eq!(loaded.font, "Palatino");
        assert!(loaded.toc);
        assert_eq!(loaded.toc_depth, 2);
        assert!(loaded.keywords_enabled);
        assert_eq!(loaded.keywords_text, "alpha, beta");
        assert_eq!(loaded.languages, vec!["lang_el"]);
    }

    #[test]
    fn load_sidecar_returns_none_for_missing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let typ_path = dir.path().join("no_sidecar.typ");
        assert!(load_sidecar(&typ_path).is_none());
    }

    #[test]
    fn apply_body_splice_preserves_body() {
        let settings = TemplateSettings {
            title: "Draft".to_string(),
            subtitle: String::new(),
            author: "Author".to_string(),
            affiliation: String::new(),
            course: String::new(),
            professor: String::new(),
            date: "2026".to_string(),
            style_idx: 1,  // Chicago
            paper_idx: 0,  // US Letter
            margin_idx: 0,
            font: "Times New Roman".to_string(),
            spacing: "0.9em".to_string(),
            page_num_pos: 3,
            header_style: 0,
            include_toc: false,
            toc_depth: 2,
            include_abstract: false,
            abstract_text: String::new(),
            include_keywords: false,
            keywords: String::new(),
            languages: vec![],
            packages: vec![],
            dropcap_font: String::new(),
            dropcap_lines: 3,
            dropcap_color: String::new(),
            body_kind: BodyKind::Academic,
            font_size: "12pt".into(), heading_numbering: false, numbering_format: String::new(),
            custom_paper_w: String::new(), custom_paper_h: String::new(), custom_margin: String::new(),
            bib_path: None,
        };
        let original = generate_typst_template(&settings);

        // Simulate user writing content in the body
        let user_body = original.replace(
            "Start writing here...",
            "This is the user's actual thesis text. It must survive apply.",
        );

        // Now apply new settings (switch to APA)
        let new_settings = TemplateSettings {
            style_idx: 4,  // APA
            author: "New Author".to_string(),
            include_abstract: true,
            abstract_text: "My abstract.".to_string(),
            ..settings
        };
        let fresh = generate_typst_template(&new_settings);
        let result = apply_body_splice(&user_body, &fresh);

        // Body content preserved
        assert!(result.contains("This is the user's actual thesis text. It must survive apply."),
            "user body content must be preserved");

        // New preamble applied
        assert!(result.contains("@zerkalo-style: apa"), "APA style key in new preamble");
        assert!(result.contains("APA heading styles"), "APA headings applied");

        // Old preamble gone
        assert!(!result.contains("@zerkalo-style: chicago-notes"), "old style key removed");

        // New author in title block
        assert!(result.contains("doc-author = \"New Author\""), "new author in title block");

        // Abstract in front-matter (between title block and body)
        assert!(result.contains("*Abstract*"), "abstract present in front-matter");
    }

    #[test]
    fn apply_body_splice_updates_bib_style() {
        let settings = TemplateSettings {
            title: String::new(), subtitle: String::new(), author: String::new(),
            affiliation: String::new(), course: String::new(), professor: String::new(), date: String::new(),
            style_idx: 2,  // Chicago Author-Date
            paper_idx: 0, margin_idx: 0,
            font: "Times New Roman".to_string(), spacing: "0.9em".to_string(),
            page_num_pos: 0, header_style: 0, include_toc: false, toc_depth: 2,
            include_abstract: false, abstract_text: String::new(),
            include_keywords: false, keywords: String::new(),
            languages: vec![], packages: vec![], dropcap_font: String::new(), dropcap_lines: 3, dropcap_color: String::new(), body_kind: BodyKind::Academic,
            font_size: "12pt".into(), heading_numbering: false, numbering_format: String::new(),
            custom_paper_w: String::new(), custom_paper_h: String::new(), custom_margin: String::new(),
            bib_path: Some(std::path::PathBuf::from("refs.bib")),
        };
        let existing = generate_typst_template(&settings);
        assert!(existing.contains("style: \"chicago-author-date\""), "original bib style");

        let new_settings = TemplateSettings { style_idx: 3, ..settings }; // MLA
        let fresh = generate_typst_template(&new_settings);
        let result = apply_body_splice(&existing, &fresh);

        assert!(result.contains("style: \"mla\""), "bib style updated to MLA");
        assert!(!result.contains("style: \"chicago-author-date\""),
            "old bib style must be gone");
    }

    #[test]
    fn apply_body_splice_fallback_when_no_body_marker() {
        // A document with no body marker should get the full fresh template
        let existing = "// some old stuff\n= Heading\nContent.\n";
        let fresh_settings = TemplateSettings {
            title: "Fresh".to_string(), subtitle: String::new(), author: String::new(),
            affiliation: String::new(), course: String::new(), professor: String::new(), date: String::new(),
            style_idx: 0, paper_idx: 0, margin_idx: 0,
            font: "Times New Roman".to_string(), spacing: "0.9em".to_string(),
            page_num_pos: 0, header_style: 0, include_toc: false, toc_depth: 2,
            include_abstract: false, abstract_text: String::new(),
            include_keywords: false, keywords: String::new(),
            languages: vec![], packages: vec![], dropcap_font: String::new(), dropcap_lines: 3, dropcap_color: String::new(), body_kind: BodyKind::Academic,
            font_size: "12pt".into(), heading_numbering: false, numbering_format: String::new(),
            custom_paper_w: String::new(), custom_paper_h: String::new(), custom_margin: String::new(),
            bib_path: None,
        };
        let fresh = generate_typst_template(&fresh_settings);
        let result = apply_body_splice(existing, &fresh);
        // When neither document has body markers, get the full fresh doc
        assert!(result.contains("ZERKALO-TEMPLATE-BEGIN"), "has template markers");
        assert!(result.contains("doc-title = \"Fresh\""), "fresh title present");
    }

    #[test]
    fn letter_body_kind_produces_letterhead_not_title_page() {
        let settings = TemplateSettings {
            title: "Re: Recommendation".to_string(), subtitle: String::new(),
            author: "Jane Doe".to_string(), affiliation: "Springfield Seminary".to_string(),
            course: String::new(), professor: String::new(), date: String::new(),
            style_idx: 0, paper_idx: 0, margin_idx: 0,
            custom_paper_w: String::new(), custom_paper_h: String::new(), custom_margin: String::new(),
            font: "Times New Roman".to_string(), font_size: "12pt".to_string(),
            spacing: "0.65em".to_string(), page_num_pos: 4, header_style: 0,
            include_toc: false, toc_depth: 2,
            include_abstract: false, abstract_text: String::new(),
            include_keywords: false, keywords: String::new(),
            heading_numbering: false, numbering_format: String::new(),
            languages: Vec::new(), packages: Vec::new(),
            dropcap_font: String::new(), dropcap_lines: 3, dropcap_color: String::new(),
            body_kind: BodyKind::Letter, bib_path: None,
        };
        let src = generate_typst_template(&settings);

        assert!(src.contains("Dear Recipient Name,"), "salutation present");
        assert!(src.contains("Sincerely,"), "closing present");
        assert!(src.contains("#doc-author"), "signature references the author");
        assert!(src.contains("// ── Document body"), "Simple Mode marker present");
        assert!(!src.contains("#counter(page).update(1)"),
            "letters skip the separate-title-page cover, unlike Academic/Book");

        let path = std::path::PathBuf::from(format!(
            "/tmp/zerkalo_test_letter_{}.typ",
            std::process::id()
        ));
        std::fs::write(&path, &src).unwrap();
        let result = crate::compiler::compile_to_pdf_bytes(
            &path,
            &std::collections::HashMap::new(),
            &std::collections::HashMap::new(),
        );
        assert!(result.is_ok(), "letter template should compile: {:?}", result.err());
        assert!(result.unwrap().starts_with(b"%PDF-"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn sidecar_round_trips_letter_body_kind() {
        let settings = TemplateSettings {
            title: String::new(), subtitle: String::new(), author: String::new(),
            affiliation: String::new(), course: String::new(), professor: String::new(), date: String::new(),
            style_idx: 0, paper_idx: 0, margin_idx: 0,
            custom_paper_w: String::new(), custom_paper_h: String::new(), custom_margin: String::new(),
            font: String::new(), font_size: String::new(), spacing: String::new(),
            page_num_pos: 4, header_style: 0, include_toc: false, toc_depth: 2,
            include_abstract: false, abstract_text: String::new(),
            include_keywords: false, keywords: String::new(),
            heading_numbering: false, numbering_format: String::new(),
            languages: Vec::new(), packages: Vec::new(),
            dropcap_font: String::new(), dropcap_lines: 3, dropcap_color: String::new(),
            body_kind: BodyKind::Letter, bib_path: None,
        };
        let sc = build_sidecar(&settings);
        assert_eq!(sc.body_kind, "letter");
        let round_tripped = sidecar_to_settings(&sc);
        assert_eq!(round_tripped.body_kind, BodyKind::Letter);
    }
}
