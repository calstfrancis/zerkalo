use std::cell::RefCell;
use std::fmt::Write as FmtWrite;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Label, Notebook, Orientation, Overlay, Picture, PolicyType,
    PositionType, ScrolledWindow, Separator, Spinner,
};
use gtk4::glib;
use libadwaita as adw;
use adw::prelude::*;

type OnCreateCb = Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>;
type OnApplyCb  = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;

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
    ("GOST 7.32", "gost-7-32"),
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
        description: "Chicago Notes-Bib · US Letter · normal margins · 1.5-line spacing",
        style_idx: 1,   // Chicago (Notes-Bib)
        paper_idx: 0,   // US Letter
        margin_idx: 0,  // Normal
        spacing_idx: 1, // 1.5em
        page_num_pos: 3, // top right
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
        name: "GOST 7.32 Technical Report",
        description: "A4 · GOST margins · 1.5-line · ToC included",
        style_idx: 9,   // GOST 7.32
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

struct TemplateSettings {
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
    spacing: String,
    page_num_pos: u32,
    include_toc: bool,
    toc_depth: u32,
    include_abstract: bool,
    abstract_text: String,
    include_keywords: bool,
    keywords: String,
    languages: Vec<String>,
    packages: Vec<String>,
    body_kind: BodyKind,
}

// ── Dialog ────────────────────────────────────────────────────────────────────

pub struct TemplateDialog {
    window: adw::Window,
    on_create: OnCreateCb,
    on_apply: OnApplyCb,
    apply_btn: Button,
    style_row: adw::ComboRow,
    font_row: adw::ComboRow,
    paper_row: adw::ComboRow,
    spacing_row: adw::ComboRow,
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

        let header = adw::HeaderBar::new();
        let cancel_btn = Button::with_label("Cancel");
        cancel_btn.add_css_class("flat");
        header.pack_start(&cancel_btn);
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
        meta_group.add(&author_row);

        let affil_row = adw::EntryRow::new();
        affil_row.set_title("Affiliation");
        meta_group.add(&affil_row);

        let course_row = adw::EntryRow::new();
        course_row.set_title("Course / Context");
        meta_group.add(&course_row);

        let today = chrono::Local::now().format("%B %-d, %Y").to_string();
        let date_row = adw::EntryRow::new();
        date_row.set_title("Date");
        date_row.set_text(&today);
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
        font_row.set_selected(0);
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
        let w_spacing = spacing_row.clone();
        let w_pnum = pnum_row.clone();
        let w_toc = toc_row.clone();
        let w_toc_depth = toc_depth_row.clone();
        let w_abstract = abstract_row.clone();
        let w_abstract_text = abstract_text_row.clone();
        let w_keywords = keywords_row.clone();
        let w_keywords_text = keywords_text_row.clone();
        let w_langs = lang_switches.clone();
        let w_pkgs = pkg_switches.clone();
        let w_body_kind = body_kind_state.clone();

        create_btn.connect_clicked(move |_| {
            let font_idx = w_font.selected() as usize;
            let available_fonts_inner = build_font_list();
            let font = if font_idx >= available_fonts_inner.len().saturating_sub(1) {
                let s = w_custom_font.text().to_string();
                if s.is_empty() { "Times New Roman".to_string() } else { s }
            } else {
                available_fonts_inner.get(font_idx).cloned().unwrap_or_else(|| "Times New Roman".to_string())
            };

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
            };

            let content = generate_typst_template(&settings);
            let title_slug = slug(&settings.title);

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
        let a_spacing = spacing_row.clone();
        let a_pnum = pnum_row.clone();
        let a_toc = toc_row.clone();
        let a_toc_depth = toc_depth_row.clone();
        let a_abstract = abstract_row.clone();
        let a_abstract_text = abstract_text_row.clone();
        let a_keywords = keywords_row.clone();
        let a_keywords_text = keywords_text_row.clone();
        let a_langs = lang_switches.clone();
        let a_pkgs = pkg_switches.clone();
        let a_body_kind = body_kind_state.clone();
        apply_btn.connect_clicked(move |_| {
            let font_idx = a_font.selected() as usize;
            let available_fonts_inner = build_font_list();
            let font = if font_idx >= available_fonts_inner.len().saturating_sub(1) {
                let s = a_custom_font.text().to_string();
                if s.is_empty() { "Times New Roman".to_string() } else { s }
            } else {
                available_fonts_inner.get(font_idx).cloned().unwrap_or_else(|| "Times New Roman".to_string())
            };
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
                languages: a_langs.iter()
                    .filter(|(_, sw)| sw.is_active())
                    .map(|(k, _)| k.clone())
                    .collect(),
                packages: a_pkgs.iter()
                    .filter(|(_, sw)| sw.is_active())
                    .map(|(k, _)| k.clone())
                    .collect(),
                body_kind: *a_body_kind.borrow(),
            };
            let content = generate_typst_template(&settings);
            if let Some(f) = on_apply_c.borrow().as_ref() {
                f(content);
            }
            win_for_apply.close();
        });

        Self { window, on_create, on_apply, apply_btn, style_row, font_row, paper_row, spacing_row }
    }

    pub fn set_on_create(&self, f: impl Fn(PathBuf) + 'static) {
        *self.on_create.borrow_mut() = Some(Box::new(f));
    }

    /// Register a callback that receives the generated template content directly,
    /// without a file-save dialog. Also shows the "Apply to Current" button and
    /// hides "Create Document".
    pub fn set_on_apply(&self, f: impl Fn(String) + 'static) {
        *self.on_apply.borrow_mut() = Some(Box::new(f));
        self.apply_btn.set_visible(true);
        // Retitle the window to clarify intent
        self.window.set_title(Some("Update Template Settings"));
    }

    /// Pre-select a citation style by its internal key (e.g. "sbl", "apa").
    pub fn preselect_style(&self, style_key: &str) {
        for (i, (_, key)) in CITATION_STYLES.iter().enumerate() {
            if *key == style_key {
                self.style_row.set_selected(i as u32);
                return;
            }
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

    pub fn present(&self) {
        self.window.present();
    }
}

/// Extract the preamble content (between TEMPLATE markers) from a generated document.
/// Returns the content between the markers, without the markers themselves.
pub fn extract_preamble(content: &str) -> String {
    if let (Some(begin_pos), Some(end_pos)) =
        (content.find(TEMPLATE_BEGIN), content.find(TEMPLATE_END))
    {
        let after_begin = begin_pos + TEMPLATE_BEGIN.len();
        let after_begin = if content[after_begin..].starts_with('\n') {
            after_begin + 1
        } else {
            after_begin
        };
        content[after_begin..end_pos].trim_end().to_string()
    } else {
        // No markers — return everything before the title block separator
        let sep = "// ── Title block";
        if let Some(pos) = content.find(sep) {
            content[..pos].trim_end().to_string()
        } else {
            String::new()
        }
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

// ── Template generator ────────────────────────────────────────────────────────

fn generate_typst_template(s: &TemplateSettings) -> String {
    let style_key = CITATION_STYLES.get(s.style_idx).map(|(_, k)| *k).unwrap_or("chicago-notes");
    let style_name = CITATION_STYLES.get(s.style_idx).map(|(n, _)| *n).unwrap_or("Chicago");
    let bib = bib_style(style_key);

    // GOST 7.32 mandates A4, specific margins, and 14 pt body text regardless of form selection.
    let (paper, mt, mb, ml, mr, font_size) = if style_key == "gost-7-32" {
        ("a4", "20mm", "20mm", "30mm", "15mm", "14pt")
    } else {
        let p = PAPER_SIZES.get(s.paper_idx).map(|(_, k)| *k).unwrap_or("us-letter");
        let (mt, mb, ml, mr) = margin_values(s.margin_idx);
        (p, mt, mb, ml, mr, "12pt")
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
    let _ = writeln!(out, "#set text(font: \"{}\", size: {font_size}, lang: \"en\")", s.font);
    let _ = writeln!(out, "#set par(leading: {}, spacing: 1.2em, first-line-indent: 1em, justify: true)", s.spacing);
    let _ = writeln!(out);

    // Heading styles
    let _ = writeln!(out, "{}", heading_styles(style_key).trim_start_matches('\n'));
    let _ = writeln!(out);

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

    // Title block
    let _ = writeln!(out, "// ── Title block ─────────────────────────────────────────────────────");
    let _ = writeln!(out, "#align(center)[");
    let _ = writeln!(out, "  #v(2em)");
    if !s.title.is_empty() {
        let _ = writeln!(out, "  #text(size: 16pt, weight: \"bold\")[{}]", s.title);
    } else {
        let _ = writeln!(out, "  #text(size: 16pt, weight: \"bold\")[Untitled]");
    }
    if !s.subtitle.is_empty() {
        let _ = writeln!(out, "  \\ #text(size: 13pt, style: \"italic\")[{}]", s.subtitle);
    }
    if !s.author.is_empty() {
        let _ = writeln!(out, "  \\ {}", s.author);
    }
    if !s.affiliation.is_empty() {
        let _ = writeln!(out, "  \\ #text(style: \"italic\")[{}]", s.affiliation);
    }
    if !s.course.is_empty() {
        let _ = writeln!(out, "  \\ {}", s.course);
    }
    if !s.date.is_empty() {
        let _ = writeln!(out, "  \\ {}", s.date);
    }
    let _ = writeln!(out, "  #v(1em)");
    let _ = writeln!(out, "]");
    let _ = writeln!(out);
    let _ = writeln!(out, "#pagebreak()");
    let _ = writeln!(out);

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

    // Table of contents
    if s.include_toc {
        let _ = writeln!(out, "#outline(depth: {})", s.toc_depth);
        let _ = writeln!(out);
    }

    // Body
    match s.body_kind {
        BodyKind::Book => {
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
            let _ = writeln!(out, "// Uncomment when your .bib file is ready:");
            let _ = writeln!(out, "// #bibliography(\"refs.bib\", style: \"{bib}\")");
        }
        BodyKind::Academic => {
            let _ = writeln!(out, "// ── Document body ───────────────────────────────────────────────────");
            let _ = writeln!(out);
            let _ = writeln!(out, "= Introduction");
            let _ = writeln!(out);
            let _ = writeln!(out, "Start writing here...");
            let _ = writeln!(out);
            let _ = writeln!(out, "#pagebreak()");
            let _ = writeln!(out);
            let _ = writeln!(out, "// ── Bibliography ────────────────────────────────────────────────────");
            let _ = writeln!(out, "// Uncomment when your .bib file is ready:");
            let _ = writeln!(out, "// #bibliography(\"refs.bib\", style: \"{bib}\")");
        }
    }

    out
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

fn bib_style(style_key: &str) -> &'static str {
    match style_key {
        "sbl" | "turabian" => "chicago-notes",
        "chicago-notes" => "chicago-notes",
        "chicago-author-date" | "harvard" => "chicago-author-date",
        "mla" => "mla",
        "apa" | "asa" => "apa",
        "ieee" => "ieee",
        "gost-7-32" => "apa",  // No built-in GOST CSL; use APA as fallback
        _ => "apa",
    }
}

fn package_import(key: &str) -> Option<&'static str> {
    match key {
        "pkg_droplet" => Some("#import \"@preview/droplet:0.2.0\": dropcap"),
        "pkg_codly" => {
            Some("#import \"@preview/codly:1.0.0\": *\n#show: codly-init.with()")
        }
        "pkg_showybox" => Some("#import \"@preview/showybox:2.0.1\": showybox"),
        "pkg_gentle" => Some("#import \"@preview/gentle-clues:1.0.0\": *"),
        "pkg_tablex" => Some("#import \"@preview/tablex:0.0.9\": tablex, cellx"),
        "pkg_drafting" => Some("#import \"@preview/drafting:0.2.0\": *"),
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

fn heading_styles(style_key: &str) -> &'static str {
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
#set heading(numbering: "I.A.1.")
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
        "gost-7-32" => {
            // GOST 7.32-2017: numbered decimal headings; H1 centred bold upper;
            // H2 flush-left bold; H3 flush-left bold italic.
            r#"
// GOST 7.32 heading styles
#set heading(numbering: "1.")
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
        languages: Vec::new(),
        packages: Vec::new(),
        body_kind: p.body_kind,
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

    crate::compiler::compile_to_png_bytes(&typ_path, 1.5)
        .map(|pages| {
            // Page 2 shows the content style; fall back to page 1 if only one page
            let idx = if pages.len() > 1 { 1 } else { 0 };
            pages.into_iter().nth(idx).unwrap_or_default()
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

/// Replace `old_pat` with `new_pat` only inside `block_prefix(…)` blocks,
/// skipping comment lines. Handles both inline and multi-line block forms.
fn replace_in_set_blocks(content: &str, block_prefix: &str, old_pat: &str, new_pat: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut in_block = false;
    for line in content.lines() {
        let t = line.trim();
        if !t.starts_with("//") && t.starts_with(block_prefix) { in_block = true; }
        let line_out = if in_block && !t.starts_with("//") {
            line.replace(old_pat, new_pat)
        } else {
            line.to_string()
        };
        result.push_str(&line_out);
        result.push('\n');
        if in_block {
            let opened_inline = t.starts_with(block_prefix) && t.contains(')');
            let closed_alone  = !t.starts_with(block_prefix) && t.starts_with(')');
            if opened_inline || closed_alone { in_block = false; }
        }
    }
    if !content.ends_with('\n') && result.ends_with('\n') {
        result.truncate(result.len() - 1);
    }
    result
}

/// Replace the preamble section (between TEMPLATE markers) with `new_preamble`.
/// If no markers exist, prepends the new preamble (with markers) before the body.
/// Also removes any ZERKALO-STYLE-BEGIN/END block, which would otherwise override
/// font/spacing/page settings with stale values from a previous style application.
pub fn reapply_preamble(existing: &str, new_preamble: &str) -> String {
    let wrapped = format!("{TEMPLATE_BEGIN}\n{new_preamble}\n{TEMPLATE_END}\n");

    // Capture old font/spacing BEFORE replacing the template section.
    let old_font = parse_font(existing);
    let new_font = parse_font(new_preamble);
    let old_spacing = parse_spacing(existing);
    let new_spacing = parse_spacing(new_preamble);

    let with_template = if let (Some(begin_pos), Some(end_marker_pos)) =
        (existing.find(TEMPLATE_BEGIN), existing.find(TEMPLATE_END))
    {
        let end_pos = end_marker_pos + TEMPLATE_END.len();
        let after = if existing[end_pos..].starts_with('\n') {
            end_pos + 1
        } else {
            end_pos
        };
        let before = &existing[..begin_pos];
        let rest = &existing[after..];
        format!("{before}{wrapped}{rest}")
    } else {
        // No markers — prepend before the document body separator if present
        let body_sep = "// ── Document body";
        if let Some(pos) = existing.find(body_sep) {
            let before = &existing[..pos];
            let rest = &existing[pos..];
            format!("{before}{wrapped}\n{rest}")
        } else {
            format!("{wrapped}\n{existing}")
        }
    };

    let with_style_stripped = strip_style_block(&with_template);

    // Propagate font change to any #set text(font:...) blocks in the document.
    // Only replaces inside #set text(...) blocks to avoid touching comments/strings.
    let after_font = match (old_font, new_font) {
        (Some(old), Some(new)) if old != new => replace_in_set_blocks(
            &with_style_stripped,
            "#set text(",
            &format!("font: \"{old}\""),
            &format!("font: \"{new}\""),
        ),
        _ => with_style_stripped,
    };

    // Propagate spacing (leading) change to any #set par(leading:...) blocks.
    match (old_spacing, new_spacing) {
        (Some(old), Some(new)) if old != new => replace_in_set_blocks(
            &after_font,
            "#set par(",
            &format!("leading: {old}"),
            &format!("leading: {new}"),
        ),
        _ => after_font,
    }
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
        spacing: "0.9em".to_string(),
        page_num_pos: 0, // Bottom center
        include_toc: false,
        toc_depth: 2,
        include_abstract: false,
        abstract_text: String::new(),
        include_keywords: false,
        keywords: String::new(),
        languages: vec![],
        packages: vec![],
        body_kind: BodyKind::default(),
    };
    let full = generate_typst_template(&settings);
    if let Some(end_pos) = full.find(TEMPLATE_END) {
        format!("{}\n", &full[..end_pos + TEMPLATE_END.len()])
    } else {
        String::new()
    }
}

fn strip_style_block(content: &str) -> String {
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
    fn replace_in_set_blocks_font() {
        let doc = "#set text(font: \"Arial\", size: 12pt)\n\
                   // font: \"Arial\"\n\
                   = Heading\n\
                   #set text(\n  font: \"Arial\",\n)\n";
        let result = replace_in_set_blocks(doc, "#set text(", "font: \"Arial\"", "font: \"Garamond\"");
        assert!(result.contains("font: \"Garamond\""));
        assert!(result.contains("// font: \"Arial\""), "comment should not be changed");
        assert!(!result.contains("#set text(font: \"Arial\""));
    }

    #[test]
    fn replace_in_set_blocks_leading() {
        let doc = "#set par(leading: 0.65em, spacing: 1.2em)\n\
                   #set par(\n  leading: 0.65em,\n  justify: true,\n)\n";
        let result = replace_in_set_blocks(doc, "#set par(", "leading: 0.65em", "leading: 1.2em");
        assert_eq!(result.matches("leading: 1.2em").count(), 2);
        assert!(!result.contains("leading: 0.65em"));
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
    fn reapply_preamble_replaces_markers() {
        let existing = "// ZERKALO-TEMPLATE-BEGIN\n#set text(font: \"Arial\")\n// ZERKALO-TEMPLATE-END\n= Body\n";
        let new_preamble = "#set text(font: \"Garamond\", size: 12pt)\n#set par(leading: 0.9em, spacing: 1.2em)\n";
        let result = reapply_preamble(existing, new_preamble);
        assert!(result.contains("font: \"Garamond\""));
        assert!(result.contains("= Body"));
        assert!(!result.contains("font: \"Arial\""));
    }

    #[test]
    fn reapply_preamble_propagates_font_change() {
        let existing = "// ZERKALO-TEMPLATE-BEGIN\n#set text(font: \"Arial\")\n// ZERKALO-TEMPLATE-END\n\
                        #set text(\n  font: \"Arial\",\n  size: 14pt,\n)\n= Body\n";
        let new_preamble = "#set text(font: \"Garamond\", size: 12pt)\n#set par(leading: 0.9em, spacing: 1.2em)\n";
        let result = reapply_preamble(existing, new_preamble);
        assert_eq!(result.matches("font: \"Garamond\"").count(), 2, "font in both template and manual section");
        assert!(!result.contains("font: \"Arial\""));
    }

    #[test]
    fn reapply_preamble_propagates_spacing_change() {
        let existing = "// ZERKALO-TEMPLATE-BEGIN\n#set par(leading: 0.65em, spacing: 1.2em)\n// ZERKALO-TEMPLATE-END\n\
                        #set par(\n  leading: 0.65em,\n  justify: false,\n)\n= Body\n";
        let new_preamble = "#set text(font: \"Arial\")\n#set par(leading: 1.2em, spacing: 1.2em)\n";
        let result = reapply_preamble(existing, new_preamble);
        assert_eq!(result.matches("leading: 1.2em").count(), 2, "leading in both template and manual section");
        assert!(!result.contains("leading: 0.65em"));
    }
}
