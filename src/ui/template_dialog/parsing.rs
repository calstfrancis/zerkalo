//! Pure reader/writer functions for a document's preamble text — parsing
//! settings back out of Typst source, and small surgical edits to it (page
//! args, heading styles, title page swap) that don't need the full generator.
//! Split out of `template_dialog.rs` — see HEALTH-PLAN.md Phase 9a.

use super::*;

// ── Preamble parsers for documents with no sidecar ───────────────────────────
// True when the document carries the generated block this module owns. Callers
// use it to tell "the document says this setting is off" apart from "this isn't
// a Zerkalo document, so nothing can be read from it" — the two look identical
// to a parser that returns a plain value.
pub fn has_template_block(content: &str) -> bool {
    template_block_line_span(&content.lines().collect::<Vec<_>>()).is_some()
}

/// True when the preamble actually sets page margins. `parse_margin` reports
/// preset 0 ("Normal") for a document that sets none, so a caller that can't
/// tell the two apart would overwrite a remembered custom margin with Normal.
pub fn has_page_margins(content: &str) -> bool {
    page_margins(content).is_some()
}

// "Update Template Settings" pre-fills from the sidecar, and falls back to
// reading the document when there isn't one. Every setting missing from that
// fallback comes back as a form default, and Apply then writes that default
// into the document — so a setting with no parser here is a setting the dialog
// silently resets on a sidecar-less file. These close that gap for the
// remaining generated settings.

/// The page-number position index (see `PAGE_NUM_OPTIONS`) from the
/// `number-align:` the generator emitted, or 4 ("None") when numbering is off.
pub fn parse_page_numbers(content: &str) -> u32 {
    let region = preamble_region(content);
    let mut found = None;
    for args in set_page_args(region) {
        if page_arg(&args, "numbering").is_none() {
            continue;
        }
        found = Some(match page_arg(&args, "number-align").as_deref().map(str::trim) {
            Some("bottom + right") => 1,
            Some("top + center")   => 2,
            Some("top + right")    => 3,
            _                      => 0,
        });
    }
    found.unwrap_or(4)
}

/// The running-header index (see `HEADER_OPTIONS`) — matched against the exact
/// blocks `header_block` emits, so only a header Zerkalo wrote is recognised.
pub fn parse_header_style(content: &str) -> u32 {
    let region = preamble_region_with_frontmatter(content);
    for style in 1..=7u32 {
        if let Some(block) = header_block(style) {
            if let Some(first) = block.lines().next() {
                if region.lines().any(|l| l.trim() == first.trim()) {
                    return style;
                }
            }
        }
    }
    0
}

/// The `EXTRA_PACKAGES` keys the document already imports.
pub fn parse_packages(content: &str) -> Vec<String> {
    let region = preamble_region(content);
    EXTRA_PACKAGES
        .iter()
        .map(|(key, _, _, _)| *key)
        .filter(|key| {
            package_import(key)
                .and_then(|imp| imp.lines().next())
                .and_then(|line| line.split_once("@preview/"))
                .and_then(|(_, rest)| rest.split(':').next())
                .is_some_and(|pkg| region.contains(&format!("@preview/{pkg}:")))
        })
        .map(str::to_string)
        .collect()
}

/// The `LANGUAGES` keys whose inline helper the document already defines.
pub fn parse_languages(content: &str) -> Vec<String> {
    let region = preamble_region(content);
    LANGUAGES
        .iter()
        .map(|(key, _, _)| *key)
        .filter(|key| {
            key.strip_prefix("lang_")
                .is_some_and(|short| region.contains(&format!("#let {short}(content)")))
        })
        .map(str::to_string)
        .collect()
}

/// Whether heading numbering is on, and the pattern it uses.
pub fn parse_heading_numbering(content: &str) -> (bool, String) {
    extract_heading_numbering(preamble_region(content))
}

/// `preamble_region` stops at `ZERKALO-TEMPLATE-END`, but the header block and
/// title metadata are emitted *after* it — this covers both, stopping at the
/// body marker so the user's own writing is still out of scope.
pub(crate) fn preamble_region_with_frontmatter(content: &str) -> &str {
    const BODY_MARKERS: &[&str] = &["// ── Document body", "// ── Chapters"];
    match BODY_MARKERS.iter().filter_map(|m| content.find(m)).min() {
        Some(p) => &content[..p],
        None => content,
    }
}

// ── Surgical preamble edits ──────────────────────────────────────────────────
// The format bar's font and size pickers used to change one value by
// regenerating the entire preamble from the sidecar. That made a two-field
// tweak as destructive as a full "Apply": with no sidecar on disk (a document
// copied without its `.zerkalo.toml`, one written before sidecars existed, or
// a corrupt one) the regeneration ran from `SidecarSettings::default()` and
// silently reset paper, margins, citation style, title page and metadata — and
// on a document with no body marker at all it replaced the user's whole file
// with a starter template. These edit the one line that actually holds the
// value, so nothing else in the document can be lost by picking a font.

/// Rewrite `font:` in the template block's `#set text(…)`. `None` when the
/// document has no template block for Zerkalo to edit.
pub fn set_template_font(content: &str, font: &str) -> Option<String> {
    let font = font.trim();
    if font.is_empty() {
        return None;
    }
    replace_set_text_arg(content, "font", &format!("\"{}\"", typst_str(font)))
}

/// Rewrite `size:` in the template block's `#set text(…)`. `None` when the
/// document has no template block, or when `size` isn't a valid length.
pub fn set_template_font_size(content: &str, size: &str) -> Option<String> {
    let value = user_length(size, "pt")?;
    replace_set_text_arg(content, "size", &value)
}

/// The line span of the `ZERKALO-TEMPLATE-BEGIN`…`-END` block, so edits stay
/// inside the region Zerkalo generated and can't touch a `#set text` the user
/// wrote in their own body.
pub(crate) fn template_block_line_span(lines: &[&str]) -> Option<(usize, usize)> {
    let begin = lines.iter().position(|l| l.trim_start().starts_with(TEMPLATE_BEGIN))?;
    let end   = lines.iter().position(|l| l.trim_start().starts_with(TEMPLATE_END))?;
    (begin < end).then_some((begin, end))
}

pub(crate) fn replace_set_text_arg(content: &str, key: &str, new_value: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let (begin, end) = template_block_line_span(&lines)?;

    // The last `#set text` wins in Typst, so that's the one worth editing.
    let needle = format!("{key}:");
    let mut target = None;
    let mut in_set_text = false;
    for (i, line) in lines.iter().enumerate().take(end).skip(begin) {
        let t = line.trim();
        if t.starts_with("//") {
            continue;
        }
        if t.starts_with("#set text(") {
            in_set_text = true;
        }
        if in_set_text {
            if t.contains(&needle) {
                target = Some(i);
            }
            let opened_inline = t.starts_with("#set text(") && t.contains(')');
            let closed_alone  = !t.starts_with("#set text(") && t.starts_with(')');
            if opened_inline || closed_alone {
                in_set_text = false;
            }
        }
    }

    let idx = target?;
    let replaced = replace_arg_value(lines[idx], key, new_value)?;
    let mut out: Vec<&str> = lines.clone();
    out[idx] = &replaced;
    let mut joined = out.join("\n");
    if content.ends_with('\n') {
        joined.push('\n');
    }
    Some(joined)
}

/// Replace the value of `key:` in one line of Typst arguments, leaving the
/// surrounding arguments, spacing and trailing comment exactly as they were.
pub(crate) fn replace_arg_value(line: &str, key: &str, new_value: &str) -> Option<String> {
    let at = line.find(&format!("{key}:"))?;
    let value_at = at + key.len() + 1;
    let rest = &line[value_at..];
    let indent: usize = rest.len() - rest.trim_start().len();
    let value = &rest[indent..];

    let len = if value.starts_with('"') {
        let bytes = value.as_bytes();
        let mut i = 1;
        loop {
            match bytes.get(i) {
                Some(b'\\') => i += 2,
                Some(b'"') => { i += 1; break }
                Some(_) => i += 1,
                None => return None, // unterminated string — leave the line alone
            }
        }
        i
    } else {
        let n = value.find([',', ')'])?;
        if n == 0 { return None }
        n
    };

    Some(format!(
        "{}{}{}{}",
        &line[..value_at],
        &rest[..indent],
        new_value,
        &value[len..],
    ))
}

/// The region a generated template owns: the `ZERKALO-TEMPLATE-BEGIN`…`-END`
/// block when present, otherwise everything before the body marker, otherwise
/// the whole document. The page parsers below scope themselves to this so a
/// `paper:`/`left:` the user wrote in their own prose — or in a
/// `#block(inset: (left: 0.5in))` — can't be read back as a page setting.
pub(crate) fn preamble_region(content: &str) -> &str {
    if let (Some(b), Some(e)) = (content.find(TEMPLATE_BEGIN), content.find(TEMPLATE_END)) {
        if b < e {
            return &content[b..e];
        }
    }
    const BODY_MARKERS: &[&str] = &["// ── Document body", "// ── Chapters"];
    match BODY_MARKERS.iter().filter_map(|m| content.find(m)).min() {
        Some(p) => &content[..p],
        None => content,
    }
}

/// The argument text of every `#set page(…)` in `content`, comments stripped
/// and balanced across line breaks, so a multi-line call is one string.
pub(crate) fn set_page_args(content: &str) -> Vec<String> {
    let code: String = content
        .lines()
        .map(|l| match l.find("//") {
            Some(p) => &l[..p],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut out = Vec::new();
    let mut rest = code.as_str();
    while let Some(pos) = rest.find("#set page(") {
        let args_start = pos + "#set page(".len();
        let mut depth = 1i32;
        let mut in_str = false;
        let mut end = None;
        for (i, c) in rest[args_start..].char_indices() {
            match c {
                '"' => in_str = !in_str,
                '(' if !in_str => depth += 1,
                ')' if !in_str => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(args_start + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        match end {
            Some(e) => {
                out.push(rest[args_start..e].to_string());
                rest = &rest[e..];
            }
            None => break,
        }
    }
    out
}

/// The value of `key:` in a `#set page(…)` argument list, up to the next comma
/// or closing paren at the same nesting depth.
pub(crate) fn page_arg(args: &str, key: &str) -> Option<String> {
    let needle = format!("{key}:");
    let mut search = args;
    let mut offset = 0usize;
    let pos = loop {
        let hit = search.find(&needle)?;
        let abs = offset + hit;
        // Reject `x-margin:` matching `margin:` — the key must start a token.
        let preceded_ok = abs == 0
            || !args[..abs]
                .chars()
                .next_back()
                .map(|c| c.is_alphanumeric() || c == '-' || c == '_')
                .unwrap_or(false);
        if preceded_ok {
            break abs;
        }
        offset = abs + needle.len();
        search = &args[offset..];
    };
    let after = args[pos + needle.len()..].trim_start();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut end = after.len();
    for (i, c) in after.char_indices() {
        match c {
            '"' => in_str = !in_str,
            '(' if !in_str => depth += 1,
            ')' if !in_str => {
                if depth == 0 {
                    end = i;
                    break;
                }
                depth -= 1;
            }
            ',' if !in_str && depth == 0 => {
                end = i;
                break;
            }
            _ => {}
        }
    }
    let v = after[..end].trim();
    if v.is_empty() { None } else { Some(v.to_string()) }
}

pub(crate) fn unquote(v: &str) -> Option<String> {
    let t = v.trim();
    let inner = t.strip_prefix('"')?.strip_suffix('"')?;
    if inner.is_empty() { None } else { Some(parse_typst_string_value(&format!("{inner}\""))) }
}

/// Parse the paper selection from `#set page(…)`. Returns `"custom"` for a
/// document sized with explicit `width:`/`height:` — without that, re-opening
/// "Update Template Settings" on a custom-sized document silently reset it to
/// US Letter, because nothing here reported a size at all.
pub fn parse_paper(content: &str) -> Option<String> {
    let mut found = None;
    for args in set_page_args(preamble_region(content)) {
        if let Some(p) = page_arg(&args, "paper").and_then(|v| unquote(&v)) {
            found = Some(p);
        } else if page_arg(&args, "width").is_some() && page_arg(&args, "height").is_some() {
            found = Some("custom".to_string());
        }
    }
    found
}

/// The explicit `width:`/`height:` of a custom-sized page, normalised to the
/// bare millimetre numbers the dialog's Custom fields hold.
pub fn parse_custom_paper(content: &str) -> Option<(String, String)> {
    let mut found = None;
    for args in set_page_args(preamble_region(content)) {
        if let (Some(w), Some(h)) = (page_arg(&args, "width"), page_arg(&args, "height")) {
            found = Some((length_as(&w, "mm")?, length_as(&h, "mm")?));
        }
    }
    found
}

/// Convert a Typst length literal to a bare number in `unit`, for round-tripping
/// back into the dialog's unit-less Custom entries. Returns `None` for a unit
/// that can't be converted (`%`, `em` — both relative).
pub(crate) fn length_as(v: &str, unit: &str) -> Option<String> {
    let t = v.trim();
    let split = t.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
    let value: f64 = t[..split].parse().ok()?;
    let in_mm = match t[split..].trim() {
        "mm" => value,
        "cm" => value * 10.0,
        "in" => value * 25.4,
        "pt" => value * 25.4 / 72.0,
        _ => return None,
    };
    let out = match unit {
        "mm" => in_mm,
        "cm" => in_mm / 10.0,
        "in" => in_mm / 25.4,
        "pt" => in_mm * 72.0 / 25.4,
        _ => return None,
    };
    Some(format!("{}", (out * 1000.0).round() / 1000.0))
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

/// Detect the margin preset index (0=Normal, 1=Narrow, 2=Wide, 3=LaTeX,
/// 4=Ross, 5=Custom) from the `#set page(margin: …)` call in the preamble.
pub fn parse_margin(content: &str) -> usize {
    let Some((t, b, l, r)) = page_margins(content) else { return 0 };
    // Ross's distinctive percentage right margin is checked first since its
    // left value (1.25in) is otherwise identical to Normal's.
    if r.contains('%') {
        return 4;
    }
    for idx in [0usize, 1, 2, 3] {
        let (pt, pb, pl, pr) = margin_values(idx, "");
        if (pt.as_str(), pb.as_str(), pl.as_str(), pr.as_str()) == (t.as_str(), b.as_str(), l.as_str(), r.as_str()) {
            return idx;
        }
    }
    // All four equal but matching no preset is the shape margin_values emits
    // for a Custom margin — reporting Normal here is what silently reset a
    // user's custom margin every time the dialog was re-opened.
    if t == b && b == l && l == r {
        return 5;
    }
    0
}

/// The custom margin value, as the bare inch number the dialog's Custom field
/// holds. `None` unless the document actually uses a custom margin.
pub fn parse_custom_margin(content: &str) -> Option<String> {
    if parse_margin(content) != 5 {
        return None;
    }
    let (t, ..) = page_margins(content)?;
    length_as(&t, "in")
}

/// The four resolved margin values from the last `#set page(margin: …)` in the
/// preamble. Accepts both the `(top:, bottom:, left:, right:)` form the
/// academic generator emits and the `(x:, y:)` form the CV generator uses.
pub(crate) fn page_margins(content: &str) -> Option<(String, String, String, String)> {
    let mut found = None;
    for args in set_page_args(preamble_region(content)) {
        let Some(m) = page_arg(&args, "margin") else { continue };
        let inner = m.trim().strip_prefix('(').and_then(|v| v.strip_suffix(')')).unwrap_or(&m);
        let get = |k: &str| page_arg(inner, k);
        let quad = match (get("top"), get("bottom"), get("left"), get("right")) {
            (Some(t), Some(b), Some(l), Some(r)) => Some((t, b, l, r)),
            _ => match (get("x"), get("y")) {
                (Some(x), Some(y)) => Some((y.clone(), y, x.clone(), x)),
                _ => {
                    // `margin: 1in` — a single length applies to all sides.
                    let v = inner.trim();
                    if v.is_empty() || v.contains(':') {
                        None
                    } else {
                        Some((v.to_string(), v.to_string(), v.to_string(), v.to_string()))
                    }
                }
            },
        };
        if quad.is_some() {
            found = quad;
        }
    }
    found
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
pub(crate) fn update_page_settings_for_style(block: &str, new_style_key: &str) -> String {
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
pub(crate) fn replace_in_line(block: &str, key: &str, new_full_line_content: &str) -> String {
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
pub(crate) fn replace_margin_line(block: &str, new_margin: &str) -> String {
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
pub(crate) fn mandated_heading_numbering(style_key: &str) -> Option<&'static str> {
    match style_key {
        "ieee" => Some("I.A.1."),
        "gost-r-705" | "vancouver" => Some("1."),
        _ => None,
    }
}

pub(crate) fn update_template_block_headings(block: &str, new_style_key: &str) -> String {
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
pub(crate) fn parse_typst_string_value(s: &str) -> String {
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

pub(crate) fn extract_first_bracket_content(s: &str) -> Option<String> {
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

