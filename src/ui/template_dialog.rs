use std::cell::RefCell;
use std::fmt::Write as FmtWrite;
use std::path::PathBuf;
use std::rc::Rc;

use chrono::Local;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Label, Notebook, Orientation, Overlay, Picture, PolicyType,
    PositionType, ScrolledWindow, Separator, Spinner,
};
use gtk4::glib;
use libadwaita as adw;
use adw::prelude::*;

type OnCreateCb = Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>;
type OnApplyCb  = Rc<RefCell<Option<Box<dyn Fn(String, SidecarSettings)>>>>;

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
];

const PAPER_SIZES: &[(&str, &str)] = &[
    ("US Letter", "us-letter"),
    ("A4", "a4"),
    ("A5", "a5"),
    ("Legal", "us-legal"),
    ("Executive", "executive"),
];

const MARGIN_PRESETS: &[&str] = &[
    "Normal (1\" / 1.25\")",
    "Narrow (0.5\" all)",
    "Wide (1\" / 2\")",
];

const PAGE_NUM_OPTIONS: &[&str] = &[
    "Bottom center",
    "Bottom right",
    "Top center",
    "Top right",
    "None",
];

const SPACING_OPTIONS: &[(&str, &str)] = &[
    ("Single", "0.65em"),
    ("1.5 Lines", "0.9em"),
    ("Double", "1.2em"),
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
    ("pkg_codly", "Codly", "Beautiful code listings with syntax highlighting"),
    ("pkg_showybox", "Showybox", "Coloured callout and theorem boxes"),
    ("pkg_gentle", "Gentle Clues", "Admonition blocks: note, tip, warning, important"),
    ("pkg_tablex", "Tablex", "Advanced tables with merged cells and styling"),
    ("pkg_drafting", "Drafting", "Margin notes and annotation tools"),
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
    include_toc: bool,
    include_abstract: bool,
    include_keywords: bool,
    body_kind: BodyKind,
}

// Indices reference CITATION_STYLES, PAPER_SIZES, MARGIN_PRESETS, SPACING_OPTIONS, PAGE_NUM_OPTIONS.
const TEMPLATE_PRESETS: &[TemplatePreset] = &[
    TemplatePreset {
        name: "Generic Academic",
        description: "Chicago Notes-Bib · US Letter · normal margins · 1.5-line spacing · page numbers bottom center",
        style_idx: 1,   // Chicago (Notes-Bib)
        paper_idx: 0,   // US Letter
        margin_idx: 0,  // Normal
        spacing_idx: 1, // 1.5em
        page_num_pos: 0, // bottom center
        include_toc: false,
        include_abstract: false,
        include_keywords: false,
        body_kind: BodyKind::Academic,
    },
    TemplatePreset {
        name: "Research Article (APA)",
        description: "APA 7th · US Letter · double-spaced · abstract & keywords",
        style_idx: 4,   // APA 7th
        paper_idx: 0,
        margin_idx: 0,
        spacing_idx: 2, // Double (2.0em)
        page_num_pos: 3, // top right
        include_toc: false,
        include_abstract: true,
        include_keywords: true,
        body_kind: BodyKind::Academic,
    },
    TemplatePreset {
        name: "GOST R 7.0-5 Technical Report",
        description: "A4 · GOST margins · 1.5-line · ToC included",
        style_idx: 9,   // GOST R 7.0-5
        paper_idx: 1,   // A4
        margin_idx: 0,
        spacing_idx: 1, // 1.5em
        page_num_pos: 0, // bottom center
        include_toc: true,
        include_abstract: false,
        include_keywords: false,
        body_kind: BodyKind::Academic,
    },
    TemplatePreset {
        name: "IEEE Conference Paper",
        description: "IEEE · US Letter · narrow margins · single-spaced · abstract",
        style_idx: 8,   // IEEE
        paper_idx: 0,
        margin_idx: 1,  // Narrow
        spacing_idx: 0, // Single
        page_num_pos: 0, // bottom center
        include_toc: false,
        include_abstract: true,
        include_keywords: true,
        body_kind: BodyKind::Academic,
    },
    TemplatePreset {
        name: "Academic Letter",
        description: "Formal letter layout · US Letter · single-spaced · no page numbers",
        style_idx: 0,   // SBL (minimal heading impact)
        paper_idx: 0,
        margin_idx: 0,
        spacing_idx: 0, // Single
        page_num_pos: 4, // None
        include_toc: false,
        include_abstract: false,
        include_keywords: false,
        body_kind: BodyKind::Academic,
    },
    TemplatePreset {
        name: "Book / Long-form",
        description: "Chapter structure · TOC · wide margins · Chicago footnotes",
        style_idx: 1,   // Chicago (Notes-Bib) — footnotes suit prose
        paper_idx: 0,   // US Letter
        margin_idx: 2,  // Wide (1" / 2")
        spacing_idx: 1, // 1.5em
        page_num_pos: 0, // bottom center
        include_toc: true,
        include_abstract: false,
        include_keywords: false,
        body_kind: BodyKind::Book,
    },
];

// ── Body kind ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Default)]
enum BodyKind {
    #[default]
    Academic,
    Book,
}

// ── Settings struct ───────────────────────────────────────────────────────────

pub(crate) struct TemplateSettings {
    title: String,
    subtitle: String,
    author: String,
    affiliation: String,
    course: String,
    date: String,
    style_idx: usize,
    paper_idx: usize,
    margin_idx: usize,
    font: String,
    font_size: String,
    spacing: String,
    page_num_pos: u32,
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
    pub date:               String,
    pub style:              String,
    pub font:               String,
    pub font_size:          String,
    pub paper:              String,
    pub margin:             u32,
    pub spacing:            String,
    pub page_numbers:       u32,
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
    pub bib_path:           Option<String>,
    pub body_kind:          String,
}

// ── Dialog ────────────────────────────────────────────────────────────────────

type OnLockCb = Rc<RefCell<Option<Box<dyn Fn(String, String)>>>>;

pub struct TemplateDialog {
    window: adw::Window,
    on_create: OnCreateCb,
    on_apply: OnApplyCb,
    on_lock_identity: OnLockCb,
    apply_btn: Button,
    style_row: adw::ComboRow,
    font_row: adw::ComboRow,
    paper_row: adw::ComboRow,
    margin_row: adw::ComboRow,
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
    // metadata fields
    title_row: adw::EntryRow,
    subtitle_row: adw::EntryRow,
    author_row: adw::EntryRow,
    affil_row: adw::EntryRow,
    course_row: adw::EntryRow,
    date_row: adw::EntryRow,
    bib_path: Rc<RefCell<Option<PathBuf>>>,
    pnum_row: adw::ComboRow,
    lang_switches: Vec<(String, adw::SwitchRow)>,
    pkg_switches: Vec<(String, adw::SwitchRow)>,
}

impl TemplateDialog {
    pub fn new(parent: &impl IsA<gtk4::Window>, work_dir: &std::path::Path) -> Self {
        let window = adw::Window::builder()
            .title("New from Template")
            .transient_for(parent)
            .modal(true)
            .default_width(620)
            .default_height(700)
            .build();

        let on_create: OnCreateCb = Rc::new(RefCell::new(None));
        let on_apply: OnApplyCb = Rc::new(RefCell::new(None));
        let on_lock_identity: OnLockCb = Rc::new(RefCell::new(None));

        let header = adw::HeaderBar::new();
        let cancel_btn = Button::with_label("Cancel");
        cancel_btn.add_css_class("flat");
        header.pack_start(&cancel_btn);
        let preview_code_btn = Button::with_label("Preview Code…");
        preview_code_btn.add_css_class("flat");
        preview_code_btn.set_tooltip_text(Some("Preview the Typst preamble that will be generated"));
        header.pack_start(&preview_code_btn);
        let create_btn = Button::with_label("Create Document");
        create_btn.add_css_class("suggested-action");
        create_btn.add_css_class("pill");
        header.pack_end(&create_btn);
        let apply_btn = Button::with_label("Apply to Current");
        apply_btn.add_css_class("suggested-action");
        apply_btn.add_css_class("pill");
        apply_btn.set_visible(false);
        header.pack_end(&apply_btn);

        let notebook = Notebook::new();
        notebook.set_tab_pos(PositionType::Top);
        notebook.set_vexpand(true);
        notebook.add_css_class("tab-strip");

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

        let margin_model = gtk4::StringList::new(MARGIN_PRESETS);
        let margin_row = adw::ComboRow::new();
        margin_row.set_title("Margins");
        margin_row.set_model(Some(&margin_model));
        margin_row.set_selected(0);
        page_group.add(&margin_row);

        let pnum_model = gtk4::StringList::new(PAGE_NUM_OPTIONS);
        let pnum_row = adw::ComboRow::new();
        pnum_row.set_title("Page Numbers");
        pnum_row.set_model(Some(&pnum_model));
        pnum_row.set_selected(0);
        page_group.add(&pnum_row);

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

        let font_size_model = gtk4::StringList::new(&["10 pt", "11 pt", "12 pt", "14 pt"]);
        let font_size_row = adw::ComboRow::new();
        font_size_row.set_title("Font Size");
        font_size_row.set_model(Some(&font_size_model));
        font_size_row.set_selected(2); // 12pt default
        typo_group.add(&font_size_row);

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
        notebook.append_page(&tab_scroll(tab3_box), Some(&tab_label("Sections")));

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
        for (key, name, desc) in EXTRA_PACKAGES {
            let sw = adw::SwitchRow::new();
            sw.set_title(name);
            sw.set_subtitle(desc);
            sw.set_active(false);
            pkg_group.add(&sw);
            pkg_switches.push((key.to_string(), sw));
        }

        let tab5_box = pref_tab_box();
        tab5_box.append(&pkg_group);
        notebook.append_page(&tab_scroll(tab5_box), Some(&tab_label("Packages")));

        // Tracks which body kind was most recently chosen via the gallery
        let body_kind_state: Rc<RefCell<BodyKind>> =
            Rc::new(RefCell::new(BodyKind::Academic));

        // ── Tab 0: Templates gallery — prepended so it becomes the first tab ─
        {
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

            notebook.prepend_page(&gallery_outer, Some(&tab_label("Templates")));
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
        let w_date = date_row.clone();
        let w_style = style_row.clone();
        let w_paper = paper_row.clone();
        let w_margin = margin_row.clone();
        let w_font = font_row.clone();
        let w_custom_font = custom_font_row.clone();
        let w_font_size = font_size_row.clone();
        let w_spacing = spacing_row.clone();
        let w_pnum = pnum_row.clone();
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

            let font_size = match w_font_size.selected() {
                0 => "10pt", 1 => "11pt", 3 => "14pt", _ => "12pt",
            }.to_string();

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
                date: w_date.text().to_string(),
                style_idx: w_style.selected() as usize,
                paper_idx: w_paper.selected() as usize,
                margin_idx: w_margin.selected() as usize,
                font,
                font_size,
                spacing: SPACING_OPTIONS
                    .get(w_spacing.selected() as usize)
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_else(|| "1.5em".to_string()),
                page_num_pos: w_pnum.selected(),
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
        let a_date = date_row.clone();
        let a_style = style_row.clone();
        let a_paper = paper_row.clone();
        let a_margin = margin_row.clone();
        let a_font = font_row.clone();
        let a_custom_font = custom_font_row.clone();
        let a_font_size = font_size_row.clone();
        let a_spacing = spacing_row.clone();
        let a_pnum = pnum_row.clone();
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
            let font_size = match a_font_size.selected() {
                0 => "10pt", 1 => "11pt", 3 => "14pt", _ => "12pt",
            }.to_string();
            let toc_depth = match a_toc_depth.selected() { 0 => 1u32, 2 => 3, _ => 2 };
            let settings = TemplateSettings {
                title: a_title.text().to_string(),
                subtitle: a_subtitle.text().to_string(),
                author: a_author.text().to_string(),
                affiliation: a_affil.text().to_string(),
                course: a_course.text().to_string(),
                date: a_date.text().to_string(),
                style_idx: a_style.selected() as usize,
                paper_idx: a_paper.selected() as usize,
                margin_idx: a_margin.selected() as usize,
                font,
                font_size,
                spacing: SPACING_OPTIONS
                    .get(a_spacing.selected() as usize)
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_else(|| "1.5em".to_string()),
                page_num_pos: a_pnum.selected(),
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
            let p_date = date_row.clone();
            let p_style = style_row.clone();
            let p_paper = paper_row.clone();
            let p_margin = margin_row.clone();
            let p_font = font_row.clone();
            let p_custom_font = custom_font_row.clone();
            let p_font_size = font_size_row.clone();
            let p_spacing = spacing_row.clone();
            let p_pnum = pnum_row.clone();
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
                let font_size = match p_font_size.selected() {
                    0 => "10pt", 1 => "11pt", 3 => "14pt", _ => "12pt",
                }.to_string();
                let toc_depth = match p_toc_depth.selected() { 0 => 1u32, 2 => 3, _ => 2 };
                let settings = TemplateSettings {
                    title: p_title.text().to_string(),
                    subtitle: p_subtitle.text().to_string(),
                    author: p_author.text().to_string(),
                    affiliation: p_affil.text().to_string(),
                    course: p_course.text().to_string(),
                    date: p_date.text().to_string(),
                    style_idx: p_style.selected() as usize,
                    paper_idx: p_paper.selected() as usize,
                    margin_idx: p_margin.selected() as usize,
                    font,
                    font_size,
                    spacing: SPACING_OPTIONS
                        .get(p_spacing.selected() as usize)
                        .map(|(_, v)| v.to_string())
                        .unwrap_or_else(|| "1.5em".to_string()),
                    page_num_pos: p_pnum.selected(),
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
            window, on_create, on_apply, on_lock_identity, apply_btn,
            style_row, font_row, font_size_row, paper_row, margin_row, spacing_row,
            toc_row, toc_depth_row, abstract_row, abstract_text_row,
            keywords_row, keywords_text_row, heading_numbering_row, heading_format_row,
            title_row, subtitle_row, author_row, affil_row, course_row, date_row,
            bib_path, pnum_row, lang_switches, pkg_switches,
        }
    }

    pub fn set_bib_path(&self, path: Option<PathBuf>) {
        *self.bib_path.borrow_mut() = path;
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
    pub fn preselect_paper(&self, paper_key: &str) {
        for (i, (_, key)) in PAPER_SIZES.iter().enumerate() {
            if *key == paper_key {
                self.paper_row.set_selected(i as u32);
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

    /// Pre-select the margin preset by index (0=Normal, 1=Narrow, 2=Wide).
    pub fn preselect_margin(&self, idx: usize) {
        if idx < MARGIN_PRESETS.len() {
            self.margin_row.set_selected(idx as u32);
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
        date: &str,
    ) {
        if !title.is_empty()       { self.title_row.set_text(title); }
        if !subtitle.is_empty()    { self.subtitle_row.set_text(subtitle); }
        if !author.is_empty()      { self.author_row.set_text(author); }
        if !affiliation.is_empty() { self.affil_row.set_text(affiliation); }
        if !course.is_empty()      { self.course_row.set_text(course); }
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
        let idx = match size { "10pt" => 0u32, "11pt" => 1, "14pt" => 3, _ => 2 };
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

    pub fn preselect_languages(&self, langs: &[String]) {
        for (key, sw) in &self.lang_switches {
            sw.set_active(langs.iter().any(|l| l == key));
        }
    }

    pub fn preselect_packages(&self, pkgs: &[String]) {
        for (key, sw) in &self.pkg_switches {
            sw.set_active(pkgs.iter().any(|p| p == key));
        }
    }

    /// Pre-fill all dialog fields from a sidecar. Called when opening
    /// "Update Template Settings" for a document that has a sidecar file.
    pub fn preselect_from_sidecar(&self, s: &SidecarSettings) {
        self.preselect_style(&s.style);
        if !s.font.is_empty()      { self.preselect_font(&s.font); }
        if !s.font_size.is_empty() { self.preselect_font_size(&s.font_size); }
        if !s.paper.is_empty()     { self.preselect_paper(&s.paper); }
        if !s.spacing.is_empty()   { self.preselect_spacing(&s.spacing); }
        self.preselect_margin(s.margin as usize);
        self.preselect_page_numbers(s.page_numbers);
        self.preselect_metadata(&s.title, &s.subtitle, &s.author, &s.affiliation, &s.course, &s.date);
        self.preselect_toc(s.toc, s.toc_depth);
        self.preselect_abstract(s.abstract_enabled, &s.abstract_text);
        self.preselect_keywords(s.keywords_enabled, &s.keywords_text);
        self.preselect_heading_numbering(s.heading_numbering);
        if !s.numbering_format.is_empty() {
            self.preselect_heading_format(&s.numbering_format);
        }
        self.preselect_languages(&s.languages);
        self.preselect_packages(&s.packages);
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
        date:              t.date.clone(),
        style:             CITATION_STYLES.get(t.style_idx).map(|(_, k)| k.to_string()).unwrap_or_default(),
        font:              t.font.clone(),
        font_size:         t.font_size.clone(),
        paper:             PAPER_SIZES.get(t.paper_idx).map(|(_, k)| k.to_string()).unwrap_or_default(),
        margin:            t.margin_idx as u32,
        spacing:           t.spacing.clone(),
        page_numbers:      t.page_num_pos,
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
        bib_path:          t.bib_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
        body_kind:         match t.body_kind { BodyKind::Book => "book".into(), BodyKind::Academic => "academic".into() },
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
        date: sc.date.clone(),
        style_idx,
        paper_idx,
        margin_idx: sc.margin as usize,
        font: sc.font.clone(),
        font_size: sc.font_size.clone(),
        spacing: sc.spacing.clone(),
        page_num_pos: sc.page_numbers,
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
        body_kind: if sc.body_kind == "book" { BodyKind::Book } else { BodyKind::Academic },
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
            format!("{}{}", &fresh[..fresh_p], updated_body)
        }
        _ => fresh.to_string(),
    }
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
    let style_key = CITATION_STYLES.get(s.style_idx).map(|(_, k)| *k).unwrap_or("chicago-notes");
    let style_name = CITATION_STYLES.get(s.style_idx).map(|(n, _)| *n).unwrap_or("Chicago");
    let bib = bib_style(style_key);
    let bib_line = s.bib_path.as_ref().map(|p| {
        format!("#bibliography(\"{}\", style: \"{}\")", p.display(), bib)
    });

    // GOST 7.32 mandates A4, specific margins, and 14 pt body text regardless of form selection.
    let (paper, mt, mb, ml, mr, font_size) = if style_key == "gost-r-705" {
        let size = if s.font_size.is_empty() { "14pt" } else { &s.font_size };
        ("a4", "20mm", "20mm", "30mm", "15mm", size)
    } else {
        let p = PAPER_SIZES.get(s.paper_idx).map(|(_, k)| *k).unwrap_or("us-letter");
        let (mt, mb, ml, mr) = margin_values(s.margin_idx);
        let size = if s.font_size.is_empty() { "12pt" } else { &s.font_size };
        (p, mt, mb, ml, mr, size)
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
    if !s.packages.is_empty() {
        let _ = writeln!(out);
    }

    // Page setup
    let page_num_code = page_num_block(s.page_num_pos);
    let _ = writeln!(out, "#set page(");
    let _ = writeln!(out, "  paper: \"{paper}\",");
    let _ = writeln!(out, "  margin: (top: {mt}, bottom: {mb}, left: {ml}, right: {mr}),");
    if !page_num_code.is_empty() {
        let _ = writeln!(out, "  {page_num_code}");
    }
    let _ = writeln!(out, ")");
    let _ = writeln!(out);

    // Typography
    let _ = writeln!(out, "#set text(font: \"{}\", size: {font_size}, lang: \"en\")", typst_str(&s.font));
    let _ = writeln!(out, "#set par(leading: {}, spacing: 1.2em, first-line-indent: 1em, justify: true)", s.spacing);
    let _ = writeln!(out);

    // Heading styles (with counter display injected so #set heading(numbering:) shows numbers)
    let heading_code = inject_heading_numbering(heading_styles(style_key).trim_start_matches('\n'));
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

    // Title block (style-specific)
    let _ = write!(out, "{}", generate_title_page(style_key, s));

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
    }

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
    let date_val = if s.date.is_empty() {
        Local::now().format("%B %-d, %Y").to_string()
    } else {
        s.date.clone()
    };
    let _ = writeln!(out, "#let doc-date = \"{}\"", typst_str(&date_val));
    let _ = writeln!(out);

    match style_key {
        // MLA: no separate title page — left-aligned header block then centred title
        "mla" => {
            let _ = writeln!(out, "#set par(first-line-indent: 0pt)");
            let _ = writeln!(out, "#if doc-author != \"\" [#doc-author \\ ]");
            let _ = writeln!(out, "#if doc-affil != \"\" [#doc-affil \\ ]");
            let _ = writeln!(out, "#if doc-course != \"\" [#doc-course \\ ]");
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

/// Rebuild the title page in `content` for a new style, preserving existing metadata.
/// Called by apply_style so the title page layout updates when the style dropdown changes.
pub fn rebuild_title_page_for_style(content: &str, new_style_key: &str) -> String {
    let s = TemplateSettings {
        title: parse_meta(content, "title"),
        subtitle: parse_meta(content, "subtitle"),
        author: parse_meta(content, "author"),
        affiliation: parse_meta(content, "affiliation"),
        course: parse_meta(content, "course"),
        date: parse_meta(content, "date"),
        // Remaining fields are not used by generate_title_page
        style_idx: 0, paper_idx: 0, margin_idx: 0,
        font: String::new(), font_size: String::new(), spacing: String::new(), page_num_pos: 0,
        include_toc: false, toc_depth: 2,
        include_abstract: false, abstract_text: String::new(),
        include_keywords: false, keywords: String::new(),
        heading_numbering: false, numbering_format: String::new(),
        languages: vec![], packages: vec![],
        body_kind: BodyKind::Academic,
        bib_path: None,
    };
    let new_page = generate_title_page(new_style_key, &s);
    // Wrap with a fake TEMPLATE_END so replace_title_page can locate the zone start
    let fake = format!("{TEMPLATE_END}\n\n{new_page}");
    replace_title_page(content, &fake)
}

fn margin_values(idx: usize) -> (&'static str, &'static str, &'static str, &'static str) {
    match idx {
        1 => ("0.5in", "0.5in", "0.5in", "0.5in"),
        2 => ("1in", "1in", "2in", "2in"),
        _ => ("1in", "1in", "1.25in", "1.25in"),
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

// Injects conditional counter display before each heading body reference so that
// #set heading(numbering: ...) actually shows numbers in custom show rules.
fn inject_heading_numbering(rules: &str) -> String {
    const PREFIX: &str =
        "#if it.numbering != none [#context counter(heading).display(it.numbering)#h(0.3em)]";
    rules
        .replace("#upper(it.body)", &format!("{PREFIX}#upper(it.body)"))
        .replace("#text(it.body)", &format!("{PREFIX}#text(it.body)"))
        .replace("#it.body", &format!("{PREFIX}#it.body"))
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
        date: "2026".to_string(),
        style_idx: p.style_idx as usize,
        paper_idx: p.paper_idx as usize,
        margin_idx: p.margin_idx as usize,
        font: "Times New Roman".to_string(),
        font_size: "12pt".to_string(),
        spacing,
        page_num_pos: p.page_num_pos,
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
        body_kind: p.body_kind,
        bib_path: None,
    };

    let mut preamble = generate_typst_template(&settings);

    // Replace the starter body with richer sample content
    let body = match p.body_kind {
        BodyKind::Book => PREVIEW_BOOK_BODY,
        BodyKind::Academic => PREVIEW_ACADEMIC_BODY,
    };
    // Strip everything from the first chapter/section marker onward and append rich body
    let marker = match p.body_kind {
        BodyKind::Book => "// ── Chapters",
        BodyKind::Academic => "// ── Document body",
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

/// Detect the margin preset index (0=Normal, 1=Narrow, 2=Wide) from the content.
/// Reads the left-margin value from `#set page(margin: (...))` and maps it to a preset.
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
                    if val.starts_with("0.5") { return 1; }
                    if val.starts_with("2in") || val == "2in" { return 2; }
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
        date: String::new(),
        style_idx: 1,    // Chicago (Notes-Bib) — common humanities default
        paper_idx: 0,    // US Letter
        margin_idx: 0,   // Normal (1" / 1.25")
        font: "Times New Roman".to_string(),
        font_size: "12pt".to_string(),
        spacing: "0.9em".to_string(),
        page_num_pos: 0, // Bottom center
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

fn update_template_block_headings(block: &str, new_style_key: &str) -> String {
    let raw = inject_heading_numbering(heading_styles(new_style_key).trim_start_matches('\n'));
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
        let with_headings = annotated.replace(
            TEMPLATE_END,
            &format!("{new_heading_code}\n\n{TEMPLATE_END}"),
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
            affiliation: String::new(), course: String::new(), date: String::new(),
            style_idx: 1, // Chicago
            paper_idx: 0, margin_idx: 0,
            font: "Times New Roman".into(), spacing: "0.9em".into(),
            page_num_pos: 0, include_toc: false, toc_depth: 2,
            include_abstract: false, abstract_text: String::new(),
            include_keywords: false, keywords: String::new(),
            languages: vec![], packages: vec![], body_kind: BodyKind::Academic,
            font_size: "12pt".into(), heading_numbering: false, numbering_format: String::new(),
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
            affiliation: String::new(), course: String::new(), date: String::new(),
            style_idx: 0, paper_idx: 0, margin_idx: 0,
            font: "Times New Roman".into(), spacing: "0.9em".into(),
            page_num_pos: 0, include_toc: false, toc_depth: 2,
            include_abstract: false, abstract_text: String::new(),
            include_keywords: false, keywords: String::new(),
            languages: vec![], packages: vec![], body_kind: BodyKind::Academic,
            font_size: "12pt".into(), heading_numbering: false, numbering_format: String::new(),
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
            date: "2025".to_string(),
            style_idx: 1,
            paper_idx: 0,
            margin_idx: 0,
            font: "Times New Roman".to_string(),
            spacing: "1.2em".to_string(),
            page_num_pos: 0,
            include_toc: false,
            toc_depth: 2,
            include_abstract: false,
            abstract_text: String::new(),
            include_keywords: false,
            keywords: String::new(),
            languages: vec![],
            packages: vec![],
            body_kind: BodyKind::Academic,
            font_size: "12pt".into(), heading_numbering: false, numbering_format: String::new(),
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
            date: "2026".to_string(),
            style_idx: 4,  // APA 7th
            paper_idx: 1,  // A4
            margin_idx: 0, // Normal
            font: "EB Garamond".to_string(),
            spacing: "1.2em".to_string(),
            page_num_pos: 3,
            include_toc: true,
            toc_depth: 3,
            include_abstract: true,
            abstract_text: "This is the abstract.".to_string(),
            include_keywords: true,
            keywords: "one, two, three".to_string(),
            languages: vec!["lang_ru".to_string(), "lang_he".to_string()],
            packages: vec!["pkg_codly".to_string()],
            body_kind: BodyKind::Academic,
            font_size: "12pt".into(), heading_numbering: false, numbering_format: String::new(),
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
            date: "2026".to_string(),
            style_idx: 1,  // Chicago
            paper_idx: 0,  // US Letter
            margin_idx: 0,
            font: "Times New Roman".to_string(),
            spacing: "0.9em".to_string(),
            page_num_pos: 3,
            include_toc: false,
            toc_depth: 2,
            include_abstract: false,
            abstract_text: String::new(),
            include_keywords: false,
            keywords: String::new(),
            languages: vec![],
            packages: vec![],
            body_kind: BodyKind::Academic,
            font_size: "12pt".into(), heading_numbering: false, numbering_format: String::new(),
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
            affiliation: String::new(), course: String::new(), date: String::new(),
            style_idx: 2,  // Chicago Author-Date
            paper_idx: 0, margin_idx: 0,
            font: "Times New Roman".to_string(), spacing: "0.9em".to_string(),
            page_num_pos: 0, include_toc: false, toc_depth: 2,
            include_abstract: false, abstract_text: String::new(),
            include_keywords: false, keywords: String::new(),
            languages: vec![], packages: vec![], body_kind: BodyKind::Academic,
            font_size: "12pt".into(), heading_numbering: false, numbering_format: String::new(),
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
            affiliation: String::new(), course: String::new(), date: String::new(),
            style_idx: 0, paper_idx: 0, margin_idx: 0,
            font: "Times New Roman".to_string(), spacing: "0.9em".to_string(),
            page_num_pos: 0, include_toc: false, toc_depth: 2,
            include_abstract: false, abstract_text: String::new(),
            include_keywords: false, keywords: String::new(),
            languages: vec![], packages: vec![], body_kind: BodyKind::Academic,
            font_size: "12pt".into(), heading_numbering: false, numbering_format: String::new(),
            bib_path: None,
        };
        let fresh = generate_typst_template(&fresh_settings);
        let result = apply_body_splice(existing, &fresh);
        // When neither document has body markers, get the full fresh doc
        assert!(result.contains("ZERKALO-TEMPLATE-BEGIN"), "has template markers");
        assert!(result.contains("doc-title = \"Fresh\""), "fresh title present");
    }
}
