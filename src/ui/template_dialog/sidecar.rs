//! Sidecar-file persistence (`.zerkalo` settings JSON alongside a
//! document), body-splice/marker helpers for regenerating a preamble
//! without touching the body, and the legacy CV-helpers compatibility block.
//! Split out of `template_dialog.rs` — see HEALTH-PLAN.md Phase 9c.

use super::*;

// ── Sidecar persistence ───────────────────────────────────────────────────────

pub fn sidecar_path(typ_path: &std::path::Path) -> PathBuf {
    let stem = typ_path.file_stem().unwrap_or_default();
    let dir = typ_path.parent().unwrap_or(std::path::Path::new("."));
    dir.join(format!("{}.zerkalo.toml", stem.to_string_lossy()))
}

pub fn save_sidecar(typ_path: &std::path::Path, s: &SidecarSettings) {
    if let Ok(text) = toml::to_string_pretty(s) {
        let _ = write_atomically(&sidecar_path(typ_path), &text);
    }
}

/// Write `contents` to `path` without ever leaving a half-written file behind:
/// a temp file in the same directory, flushed, then renamed over the target.
///
/// Every write in this module lands on a document the user has been writing
/// for hours. A plain `fs::write` truncates first and fills after, so a crash,
/// a full disk, or a killed flatpak between those two steps leaves a truncated
/// or empty `.typ` — the document is gone with no backup and no undo. The
/// rename is atomic on every filesystem Zerkalo runs on, so the file is either
/// entirely the old content or entirely the new one.
pub fn write_atomically(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write;

    let dir = path.parent().unwrap_or(std::path::Path::new("."));
    let stem = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{stem}.zerkalo-tmp"));

    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(contents.as_bytes())?;
        f.sync_all()?;
    }
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Copy `path` to a fresh `.typ.bak` before something destructive happens to
/// it. Returns the backup's path so the caller can name it to the user.
pub fn backup_document(path: &std::path::Path) -> std::io::Result<PathBuf> {
    let contents = std::fs::read_to_string(path)?;
    let backup = unique_backup_path(path);
    write_atomically(&backup, &contents)?;
    Ok(backup)
}

pub fn load_sidecar(typ_path: &std::path::Path) -> Option<SidecarSettings> {
    let path = sidecar_path(typ_path);
    let text = std::fs::read_to_string(&path).ok()?;
    match toml::from_str::<SidecarSettings>(&text) {
        Ok(s) => Some(s),
        Err(e) => {
            tracing::warn!(
                "Sidecar {:?} is corrupt ({}); falling back to text parsing",
                path,
                e
            );
            None
        }
    }
}

pub fn build_sidecar(t: &TemplateSettings) -> SidecarSettings {
    SidecarSettings {
        title: t.title.clone(),
        subtitle: t.subtitle.clone(),
        author: t.author.clone(),
        affiliation: t.affiliation.clone(),
        course: t.course.clone(),
        professor: t.professor.clone(),
        date: t.date.clone(),
        style: CITATION_STYLES
            .get(t.style_idx)
            .map(|(_, k)| k.to_string())
            .unwrap_or_default(),
        font: t.font.clone(),
        font_size: t.font_size.clone(),
        paper: PAPER_SIZES
            .get(t.paper_idx)
            .map(|(_, k)| k.to_string())
            .unwrap_or_default(),
        custom_paper_w: t.custom_paper_w.clone(),
        custom_paper_h: t.custom_paper_h.clone(),
        margin: t.margin_idx as u32,
        custom_margin: t.custom_margin.clone(),
        spacing: t.spacing.clone(),
        page_numbers: t.page_num_pos,
        header_style: t.header_style,
        toc: t.include_toc,
        toc_depth: t.toc_depth,
        abstract_enabled: t.include_abstract,
        abstract_text: t.abstract_text.clone(),
        keywords_enabled: t.include_keywords,
        keywords_text: t.keywords.clone(),
        heading_numbering: t.heading_numbering,
        numbering_format: t.numbering_format.clone(),
        languages: t.languages.clone(),
        packages: t.packages.clone(),
        dropcap_font: t.dropcap_font.clone(),
        dropcap_lines: t.dropcap_lines,
        dropcap_color: t.dropcap_color.clone(),
        bib_path: t
            .bib_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned()),
        title_page_enabled: t.include_title_page,
        bibliography_enabled: t.include_bibliography,
        body_kind: match t.body_kind {
            BodyKind::Book => "book".into(),
            BodyKind::Cv => "cv".into(),
            BodyKind::Letter => "letter".into(),
            BodyKind::Academic => "academic".into(),
        },
        // Written only for CV documents, and read back via `cv_style_index`
        // instead of the `style`/CITATION_STYLES aliasing above — see
        // CV_STYLE_OPTIONS' doc comment for why `style` alone isn't reliable
        // for CVs if CITATION_STYLES is ever reordered.
        cv_style: if t.body_kind == BodyKind::Cv {
            CV_STYLE_OPTIONS
                .get(t.style_idx)
                .map(|(_, k, _)| k.to_string())
                .unwrap_or_default()
        } else {
            String::new()
        },
    }
}

/// Reconstructs a [`TemplateSettings`] from a saved [`SidecarSettings`].
#[allow(dead_code)]
pub fn sidecar_to_settings(sc: &SidecarSettings) -> TemplateSettings {
    // For CVs, prefer the dedicated `cv_style` field over aliasing through
    // CITATION_STYLES — falls back to the legacy alias lookup only for
    // sidecars saved before `cv_style` existed.
    let style_idx = if sc.body_kind == "cv" && !sc.cv_style.is_empty() {
        cv_style_index(&sc.cv_style).unwrap_or(0)
    } else {
        CITATION_STYLES
            .iter()
            .position(|(_, k)| *k == sc.style)
            .unwrap_or(0)
    };
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
        body_kind: body_kind_from_key(&sc.body_kind),
        bib_path: sc.bib_path.as_ref().map(std::path::PathBuf::from),
        include_title_page: sc.title_page_enabled,
        include_bibliography: sc.bibliography_enabled,
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
            if t.is_empty() {
                continue;
            }
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
    let content = std::fs::read_to_string(path).map_err(|e| format!("Cannot read file: {e}"))?;

    if has_body_marker(&content) {
        return Ok(false);
    }

    let backup = unique_backup_path(path);
    write_atomically(&backup, &content)
        .map_err(|e| format!("Cannot create backup at {}: {e}", backup.display()))?;

    let insert_before = preamble_end_line(&content);
    let lines: Vec<&str> = content.lines().collect();
    let prefix = lines[..insert_before].join("\n");
    let suffix = lines[insert_before..].join("\n");

    let mut new_content = String::with_capacity(content.len() + 128);
    new_content.push_str(&prefix);
    if !prefix.is_empty() {
        new_content.push('\n');
    }
    new_content.push_str("// ── Document body — Zerkalo uses this exact line to find where your writing starts. Leave it in place; everything below it is yours to edit freely.\n");
    new_content
        .push_str("// ── Document body ───────────────────────────────────────────────────\n\n");
    new_content.push_str(&suffix);
    if !suffix.is_empty() && !new_content.ends_with('\n') {
        new_content.push('\n');
    }

    write_atomically(path, &new_content).map_err(|e| format!("Cannot write repaired file: {e}"))?;

    Ok(true)
}

/// A `.typ.bak` path that doesn't already exist, so repairing a file twice
/// doesn't destroy the backup taken the first time — which is the copy holding
/// the last known-good version of the document.
pub(crate) fn unique_backup_path(path: &std::path::Path) -> PathBuf {
    let first = path.with_extension("typ.bak");
    if !first.exists() {
        return first;
    }
    for n in 2..1000 {
        let candidate = path.with_extension(format!("typ.bak{n}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    first
}

/// The line index where the preamble ends and body content begins.
///
/// Tracking bracket depth matters: a continuation line of a multi-line
/// directive (`  paper: "a4",` inside `#set page(`) starts with neither `#`
/// nor `//`, so a line-shape test alone reads it as the first body line and
/// splices the marker into the middle of the call, leaving a document that
/// can't compile.
pub(crate) fn preamble_end_line(content: &str) -> usize {
    let lines: Vec<&str> = content.lines().collect();
    let mut depth = 0i32;
    for (i, line) in lines.iter().enumerate() {
        let code = match line.find("//") {
            Some(p) => &line[..p],
            None => line,
        };
        let t = code.trim();
        if depth <= 0 && !t.starts_with('#') && !t.is_empty() && !line.trim().starts_with("//") {
            return i;
        }
        let mut in_str = false;
        for c in code.chars() {
            match c {
                '"' => in_str = !in_str,
                '(' | '[' | '{' if !in_str => depth += 1,
                ')' | ']' | '}' if !in_str => depth -= 1,
                _ => {}
            }
        }
        if depth < 0 {
            depth = 0;
        }
    }
    lines.len()
}

/// Returns true when the document has a body-section marker and `apply_body_splice`
/// will safely preserve the user's writing.
pub fn has_body_marker(content: &str) -> bool {
    const BODY_MARKERS: &[&str] = &["// ── Document body", "// ── Chapters"];
    BODY_MARKERS.iter().any(|m| content.contains(m))
}

/// What `apply_body_splice` actually did, so the caller can tell the user
/// instead of leaving a click that appears to have done nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpliceOutcome {
    /// Preamble regenerated, the user's body preserved verbatim. The normal case.
    Preserved,
    /// Body regenerated because the CV layout crossed the sidebar boundary.
    BodyRegenerated,
    /// Nothing applied: the new preamble would not compile against this body.
    RefusedIncompatible,
    /// The whole document was replaced — there was no body to preserve.
    WholeDocumentReplaced,
}

/// Regenerate the document preamble and front-matter from fresh settings while
/// preserving the user's body content. Splices at the `// ── Document body` /
/// `// ── Chapters` marker so the body is never touched, and updates the
/// bibliography style in the preserved body.
#[cfg(test)]
pub fn apply_body_splice(existing: &str, fresh: &str) -> String {
    apply_body_splice_reporting(existing, fresh).0
}

/// [`apply_body_splice`], plus what it decided to do — see [`SpliceOutcome`].
pub fn apply_body_splice_reporting(existing: &str, fresh: &str) -> (String, SpliceOutcome) {
    const BODY_MARKERS: &[&str] = &["// ── Document body", "// ── Chapters"];

    let old_pos = BODY_MARKERS.iter().filter_map(|m| existing.find(m)).min();
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

            // Guard against producing a document that can't compile: a preserved
            // body that still calls #section(...)/#cv-section(...) needs a CV
            // preamble — the only kind that defines those helpers (via
            // cv-helpers.typ). If `fresh_preamble` isn't one, the caller's
            // body-kind state must have disagreed with what the document's body
            // actually is (see body_looks_like_cv's callers in app_window.rs) —
            // splicing here would silently write a document that fails to
            // compile with "unknown function: section". Keep the existing,
            // working document instead of corrupting it.
            let old_body_needs_cv_helpers =
                old_body.contains("#section(") || old_body.contains("#cv-section(");
            let fresh_defines_cv_helpers = fresh_preamble.contains("#let section(");
            if old_body_needs_cv_helpers && !fresh_defines_cv_helpers {
                return (existing.to_string(), SpliceOutcome::RefusedIncompatible);
            }

            // Still a CV, but the style changed: "sidebar" (Two-Column) is the
            // only CV style with a structurally different body — a #grid
            // columns split written once at generation time, not a runtime
            // `if CV_STYLE == ...` branch like the header/section helpers. If
            // the new style crosses that sidebar <-> flat boundary, blindly
            // preserving the old body (as the bib-style-only `updated_body`
            // above does) would keep rendering the old column layout forever,
            // no matter what style is picked in "Update Template Settings" —
            // regenerate the body to match instead. Mirrors
            // EditorPane::apply_cv_style's identical fix for the in-document
            // quick-switcher.
            let old_cv_style = parse_cv_style(&existing[..old_p]);
            let new_cv_style = parse_cv_style(&fresh_preamble);
            if let (Some(old_style), Some(new_style)) = (&old_cv_style, &new_cv_style) {
                if (old_style == "sidebar") != (new_style == "sidebar") {
                    return (
                        format!("{fresh_preamble}{}", generate_cv_body(new_style)),
                        SpliceOutcome::BodyRegenerated,
                    );
                }
            }

            (
                format!("{fresh_preamble}{updated_body}"),
                SpliceOutcome::Preserved,
            )
        }
        // The existing document has a body worth keeping but the regenerated
        // one carries no marker to splice at. Returning `fresh` here — as this
        // used to — overwrote the user's writing with a starter template, with
        // no confirmation and no undo, on something as small as changing the
        // font from the status bar. Every generator emits a marker, so reaching
        // this arm means generation went wrong; keeping the body is always the
        // safer answer.
        (Some(old_p), None) => (
            format!("{fresh}{}", &existing[old_p..]),
            SpliceOutcome::Preserved,
        ),
        // No body to preserve. Callers that can reach this confirm the
        // whole-document replacement with the user first (see
        // `has_body_marker`'s call site in app_window).
        (None, _) => (fresh.to_string(), SpliceOutcome::WholeDocumentReplaced),
    }
}

/// Re-inserts the pre-Skrizhal `#job`/`#edu`/`#skill`/`#award`/`#presentation`
/// helper definitions into a freshly generated CV preamble, right before the
/// `// ZERKALO-TEMPLATE-END` marker — see `apply_body_splice`.
pub(crate) fn inject_legacy_cv_helpers(fresh_preamble: &str) -> String {
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

pub(crate) fn legacy_cv_helpers_block() -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "// #job — kept for documents created before #cv-section existed"
    );
    let _ = writeln!(out, "#let job(title, company, years, desc) = {{");
    let _ = writeln!(out, "  if CV_STYLE == \"modern\" {{");
    let _ = writeln!(out, "    grid(columns: (1fr, auto),");
    let _ = writeln!(
        out,
        "      [*#title* #h(0.3em)#text(fill: cv-accent, size: 9.5pt)[#company]],"
    );
    let _ = writeln!(
        out,
        "      text(size: 9pt, fill: cv-dim, style: \"italic\")[#years],"
    );
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "  }} else if CV_STYLE == \"academic\" {{");
    let _ = writeln!(out, "    grid(columns: (1fr, auto),");
    let _ = writeln!(
        out,
        "      [*#title* #h(0.3em)#text(style: \"italic\")[#company]],"
    );
    let _ = writeln!(
        out,
        "      text(style: \"italic\", fill: cv-muted)[#years],"
    );
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "  }} else if CV_STYLE == \"sidebar\" {{");
    let _ = writeln!(out, "    [*#title* --- #company]");
    let _ = writeln!(out, "    linebreak()");
    let _ = writeln!(out, "    text(style: \"italic\")[#years]");
    let _ = writeln!(out, "  }} else {{");
    let _ = writeln!(out, "    grid(columns: (1fr, auto),");
    let _ = writeln!(
        out,
        "      [*#title* #h(0.25em)#text(fill: cv-muted)[—]#h(0.25em)#company],"
    );
    let _ = writeln!(
        out,
        "      text(fill: cv-muted, style: \"italic\")[#years],"
    );
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
    let _ = writeln!(
        out,
        "      [*#degree* #h(0.3em)#text(fill: cv-accent, size: 9.5pt)[#institution]],"
    );
    let _ = writeln!(
        out,
        "      text(size: 9pt, fill: cv-dim, style: \"italic\")[#years],"
    );
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "  }} else if CV_STYLE == \"academic\" {{");
    let _ = writeln!(out, "    grid(columns: (1fr, auto),");
    let _ = writeln!(
        out,
        "      [*#degree* #h(0.3em)#text(style: \"italic\")[#institution]],"
    );
    let _ = writeln!(
        out,
        "      text(style: \"italic\", fill: cv-muted)[#years],"
    );
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
    let _ = writeln!(
        out,
        "      [*#degree* #h(0.25em)#text(fill: cv-muted)[—]#h(0.25em)#institution],"
    );
    let _ = writeln!(
        out,
        "      text(fill: cv-muted, style: \"italic\")[#years],"
    );
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "  }}");
    let _ = writeln!(
        out,
        "  if CV_STYLE != \"sidebar\" and note != none {{ v(0.15em); note }}"
    );
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
    let _ = writeln!(
        out,
        "    #text(style: \"italic\")[#category:] #h(0.3em)#items.join(\", \") \\"
    );
    let _ = writeln!(out, "  ]");
    let _ = writeln!(out, "}}");
    let _ = writeln!(out);

    let _ = writeln!(out, "#let award(title, org, years, desc: none) = {{");
    let _ = writeln!(out, "  if CV_STYLE == \"modern\" {{");
    let _ = writeln!(out, "    grid(columns: (1fr, auto),");
    let _ = writeln!(
        out,
        "      [*#title* #h(0.3em)#text(fill: cv-accent, size: 9.5pt)[#org]],"
    );
    let _ = writeln!(
        out,
        "      text(size: 9pt, fill: cv-dim, style: \"italic\")[#years],"
    );
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "  }} else if CV_STYLE == \"academic\" {{");
    let _ = writeln!(out, "    grid(columns: (1fr, auto),");
    let _ = writeln!(
        out,
        "      [*#title* #h(0.3em)#text(style: \"italic\")[#org]],"
    );
    let _ = writeln!(
        out,
        "      text(style: \"italic\", fill: cv-muted)[#years],"
    );
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "  }} else if CV_STYLE == \"sidebar\" {{");
    let _ = writeln!(out, "    [*#title*]");
    let _ = writeln!(out, "    linebreak()");
    let _ = writeln!(out, "    if org != none {{ [#org]; linebreak() }}");
    let _ = writeln!(out, "    [#years]");
    let _ = writeln!(out, "  }} else {{");
    let _ = writeln!(out, "    grid(columns: (1fr, auto),");
    let _ = writeln!(
        out,
        "      [*#title* #h(0.25em)#text(fill: cv-muted)[—]#h(0.25em)#org],"
    );
    let _ = writeln!(
        out,
        "      text(fill: cv-muted, style: \"italic\")[#years],"
    );
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "  }}");
    let _ = writeln!(out, "  if desc != none {{ v(0.15em); desc }}");
    let _ = writeln!(out, "  v(0.45em)");
    let _ = writeln!(out, "}}");
    let _ = writeln!(out);

    let _ = writeln!(out, "#let presentation(role, venue, title, years) = {{");
    let _ = writeln!(out, "  if CV_STYLE == \"sidebar\" {{");
    let _ = writeln!(
        out,
        "    [*#role* #h(0.25em)#venue, #text(style: \"italic\")[\"#title\"]]"
    );
    let _ = writeln!(out, "    linebreak()");
    let _ = writeln!(out, "    text(style: \"italic\")[#years]");
    let _ = writeln!(out, "  }} else {{");
    let _ = writeln!(out, "    grid(columns: (1fr, auto),");
    let _ = writeln!(
        out,
        "      [*#role* #h(0.25em)#venue, #text(style: \"italic\")[\"#title\"]],"
    );
    let _ = writeln!(
        out,
        "      text(fill: cv-muted, style: \"italic\")[#years],"
    );
    let _ = writeln!(out, "    )");
    let _ = writeln!(out, "  }}");
    let _ = writeln!(out, "  v(0.35em)");
    let _ = writeln!(out, "}}");
    out
}

pub(crate) fn bib_title_for_style(style_key: &str) -> &'static str {
    match style_key {
        "mla" => "Works Cited",
        "chicago-author-date" => "References",
        "apa" | "asa" | "ieee" | "harvard" | "vancouver" => "References",
        _ => "",
    }
}
