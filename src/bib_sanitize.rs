//! Makes a `.bib` file's date field as forgiving to read as Zotero/BetterBibTeX
//! itself is to write — Typst's own bibliography loader is strict about date
//! syntax and fails the parse of the *entire file* over one bad entry (see
//! `KILLER-APP-PLAN.md`-adjacent bug report, 2026-08-18: a single
//! `year = {Winter/Spring 2001}` from a Zotero export took down every
//! citation in the document, not just that one), while Zerkalo's own citation
//! panel (via the same `biblatex` crate, used more leniently here) reads the
//! same file without complaint.

use biblatex::{Bibliography, Chunks, PermissiveType};

/// Rewrites `content` so any entry whose date/year field `biblatex` can't
/// interpret as a proper date has *that one field* replaced with a plain
/// 4-digit year extracted from the same text (or emptied out, if no
/// plausible year is found in it at all). Every other byte of the file —
/// including every other field of the same entry, and every other entry
/// entirely — is left untouched.
///
/// Returns `None` when nothing needed fixing (the caller should just use the
/// original file as-is), or when the file doesn't parse as valid BibTeX
/// syntax at all — a different, syntax-level problem this can't repair.
pub fn sanitize_bib(content: &str) -> Option<String> {
    let bib = Bibliography::parse(content).ok()?;

    let mut fixes: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    for entry in bib.iter() {
        let needs_fix = matches!(entry.date(), Err(_) | Ok(PermissiveType::Chunks(_)));
        if !needs_fix {
            continue;
        }
        let Some((_, chunks)) = entry
            .fields
            .get_key_value("date")
            .or_else(|| entry.fields.get_key_value("year"))
        else {
            continue;
        };
        let Some(span) = chunks_span(chunks) else { continue };
        let replacement = extract_year(&content[span.clone()]).unwrap_or_default();
        fixes.push((span, replacement));
    }

    if fixes.is_empty() {
        return None;
    }

    fixes.sort_by_key(|(span, _)| span.start);
    let mut out = String::with_capacity(content.len());
    let mut pos = 0;
    for (span, replacement) in fixes {
        out.push_str(&content[pos..span.start]);
        out.push_str(&replacement);
        pos = span.end;
    }
    out.push_str(&content[pos..]);
    Some(out)
}

/// The union byte span, in the original source, covered by a field's chunks —
/// where to cut the replacement text in.
fn chunks_span(chunks: &Chunks) -> Option<std::ops::Range<usize>> {
    let start = chunks.iter().map(|c| c.span.start).min()?;
    let end = chunks.iter().map(|c| c.span.end).max()?;
    Some(start..end)
}

/// The first run of 4 digits that reads as a plausible publication year
/// (1000-2999, a generous bound rather than hard-coding "this century") found
/// anywhere in `raw` — picks up "2001" out of "Winter/Spring 2001", the first
/// year out of a "2019-2020" range, and so on.
fn extract_year(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    for i in 0..bytes.len() {
        if i + 4 <= bytes.len() && bytes[i..i + 4].iter().all(u8::is_ascii_digit) {
            let candidate = &raw[i..i + 4];
            if let Ok(n) = candidate.parse::<u32>() {
                if (1000..=2999).contains(&n) {
                    return Some(candidate.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_season_style_date_is_reduced_to_a_plain_year() {
        let content = "@article{key,\n  title = {T},\n  author = {A},\n  year = {Winter/Spring 2001},\n}\n";
        let fixed = sanitize_bib(content).expect("should have fixed something");
        assert!(fixed.contains("year = {2001}"), "got: {fixed}");
        assert!(fixed.contains("title = {T}"), "unrelated fields must survive: {fixed}");
        // Re-parsing the fixed output must produce a clean, typed date now.
        let bib = Bibliography::parse(&fixed).unwrap();
        let entry = bib.get("key").unwrap();
        assert!(matches!(entry.date(), Ok(PermissiveType::Typed(_))), "should now parse as a real date");
    }

    #[test]
    fn a_clean_entry_is_reported_as_needing_no_fix() {
        let content = "@article{key,\n  title = {T},\n  author = {A},\n  year = {2020},\n}\n";
        assert!(sanitize_bib(content).is_none());
    }

    #[test]
    fn only_the_broken_entry_is_touched_others_are_byte_identical() {
        let content = "@article{good,\n  title = {Fine},\n  year = {2019},\n}\n\n\
                        @article{bad,\n  title = {Broken},\n  year = {Autumn 2020},\n}\n";
        let fixed = sanitize_bib(content).unwrap();
        assert!(fixed.contains("@article{good,\n  title = {Fine},\n  year = {2019},\n}"));
        assert!(fixed.contains("year = {2020}"));
        assert!(!fixed.contains("Autumn"));
    }

    #[test]
    fn a_date_with_no_extractable_year_is_emptied_rather_than_left_broken() {
        let content = "@article{key,\n  title = {T},\n  author = {A},\n  year = {n.d.},\n}\n";
        let fixed = sanitize_bib(content).unwrap();
        assert!(fixed.contains("year = {}"), "got: {fixed}");
    }

    #[test]
    fn genuinely_broken_bibtex_syntax_is_left_for_the_caller_to_report() {
        // An unbraced value containing a comma desyncs field parsing at the
        // syntax level — not something a date-field fix can repair.
        let content = "@article{key,\n  title = Some Title, With Commas,\n  year = {2020},\n}\n";
        assert!(sanitize_bib(content).is_none());
    }

    #[test]
    fn a_plain_year_range_is_already_valid_biblatex_and_needs_no_fix() {
        // "2019-2020" is documented BibLaTeX date-range syntax and parses as
        // a proper typed date on its own — nothing for this to do.
        let content = "@article{key,\n  title = {T},\n  year = {2019-2020},\n}\n";
        assert!(sanitize_bib(content).is_none());
    }

    #[test]
    fn a_season_prefixed_range_keeps_the_first_year() {
        let content = "@article{key,\n  title = {T},\n  year = {Winter 2019-Spring 2020},\n}\n";
        let fixed = sanitize_bib(content).unwrap();
        assert!(fixed.contains("year = {2019}"), "got: {fixed}");
    }
}
