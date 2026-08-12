//! Small self-contained helpers: the system-font list, a few widget-
//! builder shims shared by the tab constructors, and Typst-escaping/
//! sanitizing functions for values that reach the generated document as
//! raw source. Split out of `template_dialog.rs` — see HEALTH-PLAN.md
//! Phase 9d.

use super::*;

// ── Font list ─────────────────────────────────────────────────────────────────

pub(crate) fn build_font_list() -> Vec<String> {
    let mut fonts = crate::ui::font_manager::FontManager::enabled_fonts();
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

pub(crate) fn pref_tab_box() -> GtkBox {
    let b = GtkBox::new(Orientation::Vertical, 16);
    b.set_margin_start(20);
    b.set_margin_end(20);
    b.set_margin_top(20);
    b.set_margin_bottom(20);
    b
}

pub(crate) fn tab_scroll(content: GtkBox) -> ScrolledWindow {
    let s = ScrolledWindow::new();
    s.set_vexpand(true);
    s.set_child(Some(&content));
    s
}

pub(crate) fn tab_label(text: &str) -> Label {
    Label::new(Some(text))
}

pub(crate) fn slug(s: &str) -> String {
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
pub(crate) fn typst_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Every value below reaches the generated document as raw Typst source, so an
/// unvalidated one doesn't produce an ugly document — it produces a document
/// that doesn't compile at all, with an error pointing at generated code the
/// user never wrote. A custom margin of "wide" became the literal length
/// `widein`; an empty spacing became `leading: ,`. These sanitisers are the
/// single choke point: nothing user-entered goes into a generated template
/// without passing through one of them.
///
/// Parse a user-entered length ("1.4", "1.4in", "20 mm", "33%") into a valid
/// Typst length literal, appending `default_unit` when the user typed a bare
/// number. Returns `None` for anything that isn't a non-negative length, so
/// callers fall back to a preset instead of writing nonsense into the document.
pub(crate) fn user_length(raw: &str, default_unit: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let split = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(s.len());
    let value: f64 = s[..split].parse().ok()?;
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    let unit = s[split..].trim();
    let unit = if unit.is_empty() { default_unit } else { unit };
    if !matches!(unit, "in" | "mm" | "cm" | "pt" | "em" | "%") {
        return None;
    }
    Some(format!("{value}{unit}"))
}

/// A length that must always resolve to something compilable.
pub(crate) fn user_length_or(raw: &str, default_unit: &str, fallback: &str) -> String {
    user_length(raw, default_unit).unwrap_or_else(|| fallback.to_string())
}

/// Validate a dropcap `fill:` value. Accepts the presets from
/// [`DROPCAP_COLORS`] and a bare `#rrggbb` hex the user may have typed, and
/// rejects everything else — an arbitrary string is emitted as a Typst
/// *expression*, so "maroon" would compile but "notacolor" is an unknown
/// variable that fails the whole document.
pub(crate) fn user_color(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    if DROPCAP_COLORS.iter().any(|(_, v)| !v.is_empty() && *v == s) {
        return Some(s.to_string());
    }
    let hex = s.strip_prefix('#').unwrap_or(s);
    if matches!(hex.len(), 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some(format!("rgb(\"#{hex}\")"));
    }
    None
}

/// Escape user free text that lands in Typst *markup* context (the abstract
/// body, the keywords line). Deliberate markup is left alone when its brackets
/// balance; when they don't, every bracket is escaped — an unclosed `[`
/// otherwise swallows the remainder of the document.
pub(crate) fn typst_markup(s: &str) -> String {
    let mut depth = 0i32;
    for c in s.chars() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth < 0 {
                    break;
                }
            }
            _ => {}
        }
    }
    if depth == 0 {
        return s.to_string();
    }
    s.replace('[', "\\[").replace(']', "\\]")
}

/// The heading numbering pattern is emitted inside a Typst string literal, so
/// a stray quote from the "Custom…" field would terminate it early.
pub(crate) fn numbering_pattern(raw: &str) -> String {
    let s = raw.trim();
    if s.is_empty() {
        "1.".to_string()
    } else {
        typst_str(s)
    }
}

