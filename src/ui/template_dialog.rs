use std::cell::RefCell;
use std::fmt::Write as FmtWrite;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Label, Notebook, Orientation, PositionType, ScrolledWindow};
use libadwaita as adw;
use adw::prelude::*;

type OnCreateCb = Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>;

// ── Static data tables ────────────────────────────────────────────────────────

const CITATION_STYLES: &[(&str, &str)] = &[
    ("SBL", "sbl"),
    ("Chicago (Notes-Bib)", "chicago-notes"),
    ("MLA", "mla"),
    ("APA 7th", "apa"),
    ("ASA", "asa"),
    ("Turabian", "turabian"),
    ("Harvard", "harvard"),
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
    ("Single (1.0em)", "1.0em"),
    ("1.5 Lines (1.5em)", "1.5em"),
    ("Double (2.0em)", "2.0em"),
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
}

// ── Dialog ────────────────────────────────────────────────────────────────────

pub struct TemplateDialog {
    window: adw::Window,
    on_create: OnCreateCb,
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

        let header = adw::HeaderBar::new();
        let cancel_btn = Button::with_label("Cancel");
        cancel_btn.add_css_class("flat");
        header.pack_start(&cancel_btn);
        let create_btn = Button::with_label("Create Document");
        create_btn.add_css_class("suggested-action");
        create_btn.add_css_class("pill");
        header.pack_end(&create_btn);

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

        Self { window, on_create }
    }

    pub fn set_on_create(&self, f: impl Fn(PathBuf) + 'static) {
        *self.on_create.borrow_mut() = Some(Box::new(f));
    }

    pub fn present(&self) {
        self.window.present();
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
    let paper = PAPER_SIZES.get(s.paper_idx).map(|(_, k)| *k).unwrap_or("us-letter");
    let (mt, mb, ml, mr) = margin_values(s.margin_idx);
    let bib = bib_style(style_key);

    let mut out = String::new();

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
    let _ = writeln!(out, "#set text(font: \"{}\", size: 12pt, lang: \"en\")", s.font);
    let _ = writeln!(out, "#set par(spacing: {}, first-line-indent: 1em, justify: true)", s.spacing);
    let _ = writeln!(out);

    // Heading styles
    let _ = writeln!(out, "{}", heading_styles(style_key).trim_start_matches('\n'));
    let _ = writeln!(out);

    // Language support
    for lang in &s.languages {
        if let Some(block) = language_block(lang) {
            let _ = writeln!(out, "{block}");
        }
    }
    if !s.languages.is_empty() {
        let _ = writeln!(out);
    }

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
    let _ = writeln!(out, "// ── Document body ───────────────────────────────────────────────────");
    let _ = writeln!(out);
    let _ = writeln!(out, "= Introduction");
    let _ = writeln!(out);
    let _ = writeln!(out, "Start writing here...");
    let _ = writeln!(out);
    let _ = writeln!(out, "// ── Bibliography ────────────────────────────────────────────────────");
    let _ = writeln!(out, "// Uncomment when your .bib file is ready:");
    let _ = writeln!(out, "// #bibliography(\"refs.bib\", style: \"{bib}\")");

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
    match pos {
        0 => "footer: align(center)[#context counter(page).display()]",
        1 => "footer: align(right)[#context counter(page).display()]",
        2 => "header: align(center)[#context counter(page).display()]",
        3 => "header: align(right)[#context counter(page).display()]",
        _ => "",
    }
}

fn bib_style(style_key: &str) -> &'static str {
    match style_key {
        "sbl" | "turabian" => "chicago-notes",
        "chicago-notes" => "chicago-notes",
        "mla" => "mla",
        "apa" | "asa" => "apa",
        "harvard" => "chicago-author-date",
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
    match lang_key {
        "lang_ru" => Some(
            "// Russian: Cyrillic hyphenation, date/number locale, Cyrillic-capable font\n\
             #set text(lang: \"ru\", region: \"RU\")\n\
             // If using a Latin-only font, switch to one with Cyrillic coverage:\n\
             // #set text(font: (\"Linux Libertine O\", \"Times New Roman\"), lang: \"ru\")",
        ),
        "lang_he" => Some(
            "// Hebrew: right-to-left document\n\
             #set text(lang: \"he\", dir: ltr)  // keep ltr for body default\n\
             // Wrap Hebrew passages in: #text(lang: \"he\", dir: rtl)[...]\n\
             // For a fully-Hebrew document use: #set text(lang: \"he\", dir: rtl)",
        ),
        "lang_el" => Some(
            "// Ancient/Modern Greek: polytonic Unicode coverage\n\
             #set text(lang: \"el\")\n\
             // Recommended fonts: Linux Libertine O, GFS Artemisia, Gentium Plus\n\
             // #set text(font: \"Linux Libertine O\", lang: \"el\")",
        ),
        "lang_ja" => Some(
            "// Japanese: install Noto Serif CJK JP (or Source Han Serif JP)\n\
             // Linux/openSUSE: zypper install google-noto-serif-cjk-fonts\n\
             // macOS: brew install --cask font-noto-serif-cjk\n\
             #set text(lang: \"ja\", font: (\"Noto Serif CJK JP\", \"Source Han Serif JP\"))",
        ),
        "lang_sa" => Some(
            "// Sanskrit / Devanagari: install Noto Serif Devanagari\n\
             // Linux/openSUSE: zypper install google-noto-serif-devanagari-fonts\n\
             #set text(lang: \"sa\", font: (\"Noto Serif Devanagari\", \"Sanskrit 2003\"))",
        ),
        "lang_bo" => Some(
            "// Tibetan: install Noto Serif Tibetan\n\
             // Linux/openSUSE: zypper install google-noto-serif-tibetan-fonts\n\
             #set text(lang: \"bo\", font: \"Noto Serif Tibetan\")",
        ),
        "lang_zh" => Some(
            "// Chinese (Simplified): install Noto Serif CJK SC (or Source Han Serif SC)\n\
             // Linux/openSUSE: zypper install google-noto-serif-cjk-fonts\n\
             // macOS: brew install --cask font-noto-serif-cjk\n\
             #set text(lang: \"zh\", font: (\"Noto Serif CJK SC\", \"Source Han Serif SC\"))",
        ),
        _ => None,
    }
}

fn heading_styles(style_key: &str) -> &'static str {
    match style_key {
        "sbl" => {
            r#"
// SBL heading styles
#show heading.where(level: 1): it => {
  v(1em)
  align(center, text(weight: "bold")[#upper(it.body)])
  v(0.5em)
}
#show heading.where(level: 2): it => {
  v(0.8em)
  align(center, text(style: "italic")[#it.body])
  v(0.4em)
}
#show heading.where(level: 3): it => {
  v(0.6em)
  text(weight: "bold", style: "italic")[#it.body]
  v(0.2em)
}"#
        }
        "chicago-notes" | "turabian" => {
            r#"
// Chicago heading styles
#show heading.where(level: 1): it => {
  v(1em)
  align(center, text(weight: "bold")[#it.body])
  v(0.5em)
}
#show heading.where(level: 2): it => {
  v(0.8em)
  align(center, text(style: "italic")[#it.body])
  v(0.4em)
}
#show heading.where(level: 3): it => {
  v(0.6em)
  text(style: "italic")[#it.body]
  v(0.2em)
}"#
        }
        "mla" => {
            r#"
// MLA heading styles (no decorative formatting)
#show heading: it => {
  v(0.6em)
  text(it.body)
  v(0.3em)
}"#
        }
        "apa" | "asa" | "harvard" => {
            r#"
// APA heading styles
#show heading.where(level: 1): it => {
  v(1em)
  align(center, text(weight: "bold")[#it.body])
  v(0.5em)
}
#show heading.where(level: 2): it => {
  v(0.8em)
  text(weight: "bold")[#it.body]
  v(0.4em)
}
#show heading.where(level: 3): it => {
  v(0.6em)
  text(weight: "bold", style: "italic")[#it.body]
  v(0.2em)
}"#
        }
        _ => {
            r#"
// Default heading styles
#show heading.where(level: 1): it => {
  v(1em)
  align(center, text(weight: "bold")[#it.body])
  v(0.5em)
}
#show heading.where(level: 2): it => {
  v(0.8em)
  text(weight: "bold")[#it.body]
  v(0.4em)
}"#
        }
    }
}
