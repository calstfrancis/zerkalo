//! The Typst document generators — turns a `TemplateSettings` into
//! generated document text, for both the standard body kinds and CV mode,
//! plus the per-citation-style heading rules they share.
//! Split out of `template_dialog.rs` — see HEALTH-PLAN.md Phase 9b.

use super::*;

// ── Template generator ────────────────────────────────────────────────────────

pub fn generate_typst_template(s: &TemplateSettings) -> String {
    if matches!(s.body_kind, BodyKind::Cv) {
        return generate_cv_template(s);
    }
    let style_key = CITATION_STYLES.get(s.style_idx).map(|(_, k)| *k).unwrap_or("chicago-notes");
    let style_name = CITATION_STYLES.get(s.style_idx).map(|(n, _)| *n).unwrap_or("Chicago");
    let bib = bib_style(style_key);
    let bib_line = s.bib_path.as_ref().map(|p| {
        let target = crate::bibliography::bib_target_path(p);
        format!("#bibliography(\"{}\", style: \"{}\")", typst_str(&target.to_string_lossy()), bib)
    });

    // GOST 7.32 mandates A4, specific margins, and 14 pt body text regardless of form selection.
    let (paper_line, mt, mb, ml, mr, font_size) = if style_key == "gost-r-705" {
        let size = user_length_or(&s.font_size, "pt", "14pt");
        ("paper: \"a4\",".to_string(), "20mm".to_string(), "20mm".to_string(), "30mm".to_string(), "15mm".to_string(), size)
    } else {
        let p = PAPER_SIZES.get(s.paper_idx).map(|(_, k)| *k).unwrap_or("us-letter");
        let paper_line = if p == "custom" {
            let w = user_length_or(&s.custom_paper_w, "mm", "210mm");
            let h = user_length_or(&s.custom_paper_h, "mm", "297mm");
            format!("width: {w},\n  height: {h},")
        } else {
            format!("paper: \"{p}\",")
        };
        let (mt, mb, ml, mr) = margin_values(s.margin_idx, &s.custom_margin);
        let size = user_length_or(&s.font_size, "pt", "12pt");
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
        let color = user_color(&s.dropcap_color);
        if has_font || has_height || color.is_some() {
            let mut args = Vec::new();
            if has_font  { args.push(format!("font: \"{}\"", typst_str(&s.dropcap_font))); }
            if has_height { args.push(format!("height: {}", s.dropcap_lines.clamp(2, 8))); }
            if let Some(c) = color { args.push(format!("fill: {c}")); }
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
        let font = if s.font.trim().is_empty() { "Libertinus Serif" } else { s.font.trim() };
        let leading = user_length_or(&s.spacing, "em", "0.65em");
        let _ = writeln!(out, "#set text(font: \"{}\", size: {font_size}, lang: \"en\")", typst_str(font));
        // `spacing` matches `leading` so paragraphs are marked by the indent
        // alone. A fixed 1.2em gap on top of the indent marked every paragraph
        // twice — and on a double-spaced document it also broke the even line
        // grid that APA, MLA, Chicago and Turabian all specify. The LaTeX Look
        // branch above has always tied the two together; this is the same rule.
        let _ = writeln!(out, "#set par(leading: {leading}, spacing: {leading}, first-line-indent: 1em, justify: true)");
    }
    let _ = writeln!(out);

    // Heading styles (with counter display injected when numbering is enabled)
    let num_fmt = if s.heading_numbering {
        numbering_pattern(&s.numbering_format)
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
        let _ = writeln!(out, "#set heading(numbering: \"{num_fmt}\")");
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
            // Inset as a share of the text width, not a fixed inch. On A5 or
            // Legal-with-wide-margins, an inch either side left the abstract in
            // a column a few characters wide.
            let _ = writeln!(out, "#block(inset: (x: 8%), width: 100%)[");
            let _ = writeln!(out, "  {}", typst_markup(&s.abstract_text));
            let _ = writeln!(out, "]");
        }
        let _ = writeln!(out);
    }

    // Keywords
    if s.include_keywords && !s.keywords.is_empty() {
        let _ = writeln!(out, "_Keywords:_ {}", typst_markup(&s.keywords));
        let _ = writeln!(out);
    }

    // Table of contents (always followed by a page break)
    if s.include_toc {
        let _ = writeln!(out, "#outline(depth: {})", s.toc_depth.clamp(1, 6));
        let _ = writeln!(out, "#pagebreak()");
        let _ = writeln!(out);
    }

    // Body
    match s.body_kind {
        BodyKind::Book => {
            let _ = writeln!(out, "// ── Chapters — Zerkalo uses this exact line to find where your chapters start. Leave it in place; everything below it is yours to edit freely.");
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
            let _ = writeln!(out, "// ── Document body — Zerkalo uses this exact line to find where your writing starts. Leave it in place; everything below it is yours to edit freely.");
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
            let _ = writeln!(out, "// ── Document body — Zerkalo uses this exact line to find where your writing starts. Leave it in place; everything below it is yours to edit freely.");
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

pub(crate) fn generate_cv_template(s: &TemplateSettings) -> String {
    let cv_style = match s.style_idx {
        1 => "academic",
        2 => "classic",
        3 => "sidebar",
        _ => "modern",
    };
    // "custom" is a Zerkalo selector, not a Typst paper name — emitting it as
    // one gives "expected paper name", so it becomes explicit width/height
    // exactly as the academic generator does.
    let paper = PAPER_SIZES.get(s.paper_idx).map(|(_, k)| *k).unwrap_or("a4");
    let page_size = if paper == "custom" {
        format!(
            "width: {}, height: {}",
            user_length_or(&s.custom_paper_w, "mm", "210mm"),
            user_length_or(&s.custom_paper_h, "mm", "297mm"),
        )
    } else {
        format!("paper: \"{paper}\"")
    };
    let (margin_x, margin_y) = match s.margin_idx {
        1 => ("1.2cm".to_string(), "1.2cm".to_string()),
        2 => ("2.5cm".to_string(), "2.5cm".to_string()),
        5 => {
            let m = user_length_or(&s.custom_margin, "in", "1.5cm");
            (m.clone(), m)
        }
        _ => ("1.5cm".to_string(), "1.5cm".to_string()),
    };
    // "Linux Libertine" isn't an exact font-family match on any system (the
    // installed/embedded equivalent is named "Libertinus Serif"), so Typst
    // couldn't find it and silently fell back to whatever font its FontBook
    // picked for unknown families — often a mono font, never what was
    // intended. "Libertinus Serif" is embedded directly in the Typst
    // compiler (see typst-kit's `embed-fonts` feature), so it renders
    // correctly regardless of what fonts the host system has installed.
    let font = if s.font.trim().is_empty() || s.font == "Times New Roman" { "Libertinus Serif" } else { s.font.trim() };
    let font_size = user_length_or(&s.font_size, "pt", "10.5pt");
    let name = if s.author.is_empty() { "Your Name" } else { &s.author };
    // In CV mode the Metadata group's academic-paper rows are relabeled to
    // CV-relevant fields (see the cv_switch handler in TemplateDialog::new):
    // Subtitle -> Email, Course -> Phone, Affiliation -> Location, Professor -> Links.
    let email = if s.subtitle.is_empty() { "your@email.com" } else { &s.subtitle };
    let phone = if s.course.is_empty() { "+1 555 000 0000" } else { &s.course };
    let location = if s.affiliation.is_empty() { "City, Country" } else { &s.affiliation };
    let links = if s.professor.is_empty() { "github.com/handle" } else { &s.professor };

    let mut out = String::new();
    let _ = writeln!(out, "{TEMPLATE_BEGIN}");
    let _ = writeln!(out, "// Created with Zerkalo · CV / Résumé");
    let _ = writeln!(out, "// @zerkalo-style: cv");
    let _ = writeln!(out, "// @zerkalo-kind: cv");
    let _ = writeln!(out, "// @zerkalo-cv-style: {cv_style}");
    let _ = writeln!(out, "// @zerkalo-version: {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(out);
    let _ = writeln!(out, "#set page({page_size}, margin: (x: {margin_x}, y: {margin_y}))");
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
    let _ = writeln!(out, "#let cv-email    = \"{}\"", typst_str(email));
    let _ = writeln!(out, "#let cv-phone    = \"{}\"", typst_str(phone));
    let _ = writeln!(out, "#let cv-location = \"{}\"", typst_str(location));
    let _ = writeln!(out, "#let cv-links    = \"{}\"", typst_str(links));
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
    out.push_str(&generate_cv_body(cv_style));
    out
}

/// The document body for a CV, dispatched purely on `cv_style` — no personal
/// details or page setup, so it can be regenerated on its own when the user
/// switches CV style on an existing document (see `EditorPane::apply_cv_style`)
/// without touching the preamble. "sidebar" (Two-Column) is the only style
/// with a structurally different, columnar body; the others share one flat,
/// single-column body and only differ in the (already CV_STYLE-conditional,
/// no regeneration needed) header colors/fonts above this point.
pub fn generate_cv_body(cv_style: &str) -> String {
    let mut out = String::new();
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
pub(crate) fn generate_cv_sidebar_body(mut out: String) -> String {
    let _ = writeln!(out, "// Toggle optional sections per application");
    let _ = writeln!(out, "#let show-presentations = true");
    let _ = writeln!(out, "#let show-extracurricular = true");
    let _ = writeln!(out);
    let _ = writeln!(out, "// A brief professional summary, full-width above the two columns");
    let _ = writeln!(out, "#let cv-summary = \"A brief 2\u{2013}3 sentence professional summary goes here \u{2014} your background, key strengths, and what you're looking for next.\"");
    let _ = writeln!(out);
    let _ = writeln!(out, "#section(\"Profile\")[");
    let _ = writeln!(out, "  #cv-summary");
    let _ = writeln!(out, "]");
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

pub(crate) fn generate_title_page(style_key: &str, s: &TemplateSettings) -> String {
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
            // The un-indented heading block is scoped inside its own content
            // block. As a bare top-level `#set` it turned off first-line
            // indentation for the entire document — MLA's own requirement is a
            // half-inch indent on every body paragraph, so the one style that
            // most insists on indentation was the only one generated without it.
            let _ = writeln!(out, "#block[");
            let _ = writeln!(out, "  #set par(first-line-indent: 0pt)");
            let _ = writeln!(out, "  #if doc-author != \"\" [#doc-author \\ ]");
            let _ = writeln!(out, "  #if doc-affil != \"\" [#doc-affil \\ ]");
            let _ = writeln!(out, "  #if doc-course != \"\" [#doc-course \\ ]");
            let _ = writeln!(out, "  #if doc-professor != \"\" [#doc-professor \\ ]");
            let _ = writeln!(out, "  #if doc-date != \"\" [#doc-date]");
            let _ = writeln!(out, "]");
            let _ = writeln!(out);
            let _ = writeln!(out, "#block[");
            let _ = writeln!(out, "  #set par(first-line-indent: 0pt)");
            let _ = writeln!(out, "  #align(center)[#doc-title]");
            let _ = writeln!(out, "  #if doc-subtitle != \"\" [#align(center)[#text(style: \"italic\")[#doc-subtitle]]]");
            let _ = writeln!(out, "]");
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
            // No "Running head:" label — the 7th edition dropped it, and this
            // style is offered as APA 7th. The shortened title in caps stays.
            let _ = writeln!(out, "#page(header: align(left)[#text(size: 10pt)[#upper[#doc-title]]])[");
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
pub(crate) fn generate_letter_header(s: &TemplateSettings) -> String {
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

pub(crate) fn margin_values(idx: usize, custom_in: &str) -> (String, String, String, String) {
    match idx {
        1 => ("0.5in".into(), "0.5in".into(), "0.5in".into(), "0.5in".into()),
        2 => ("1in".into(), "1in".into(), "2in".into(), "2in".into()),
        3 => ("1.75in".into(), "1.75in".into(), "1.75in".into(), "1.75in".into()),
        // Right margin is a relative length — Typst resolves 33% against the
        // page width directly, so this stays correct across paper sizes.
        4 => ("1.25in".into(), "1.25in".into(), "1.25in".into(), "33%".into()),
        5 => {
            let m = user_length_or(custom_in, "in", "1in");
            (m.clone(), m.clone(), m.clone(), m)
        }
        _ => ("1in".into(), "1in".into(), "1.25in".into(), "1.25in".into()),
    }
}

/// Resolves the Font Size ComboRow selection to a Typst size string, reading
/// the custom SpinRow's value when "Custom…" (index 4) is selected.
pub(crate) fn resolve_font_size(selected: u32, custom_pt: f64) -> String {
    match selected {
        0 => "10pt".to_string(),
        1 => "11pt".to_string(),
        3 => "14pt".to_string(),
        4 => format!("{}pt", custom_pt as i64),
        _ => "12pt".to_string(),
    }
}

pub(crate) fn page_num_block(pos: u32) -> &'static str {
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

pub(crate) fn header_block(style: u32) -> Option<String> {
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

pub(crate) fn default_dropcap_lines() -> u32 { 3 }

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

pub(crate) fn package_import(key: &str) -> Option<&'static str> {
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

pub(crate) fn language_block(lang_key: &str) -> Option<&'static str> {
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
pub(crate) fn extract_heading_numbering(block: &str) -> (bool, String) {
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
pub(crate) fn inject_heading_numbering(rules: &str, numbering_on: bool, format: &str) -> String {
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

