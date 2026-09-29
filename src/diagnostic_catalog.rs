// The one place that knows what a problem reported by the engine means to a
// person: how to name it, what to do about it, and whether a one-click fix
// exists. Both the wording and the fix hang off the same `Kind`, decided once
// from the engine's own message, so the two can't drift apart.
//
// Wording lives in `locales/en/diagnostics.ftl`. Fixes are keyed on `Kind`,
// never on the plain-language headline, so rewording never retires a Fix button.

use std::path::Path;

use crate::i18n::{tr, tr_args};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    UnknownVariable,
    UnknownFont,
    FileNotFound,
    BibliographyUnreadable,
    PackageUnavailable,
    MissingBrace,
    MissingBracket,
    MissingParen,
    UnclosedDelimiter,
    UnclosedOther,
    UnclosedDollar,
    UnclosedStar,
    UnclosedUnderscore,
    UnclosedLabel,
    UnexpectedEnd,
    MissingArgument,
    UnexpectedArgument,
    MissingLabel,
    AtSign,
    StrayHash,
    ExpectedExpression,
    WrongType,
    DivideByZero,
    IncompleteRule,
    FileAccess,
    Other,
}

pub struct Diagnosis {
    pub kind: Kind,
    pub headline: String,
    pub advice: String,
}

/// Where a problem starts in its file, as a fix needs it: the whole source and
/// the spot, with 0-based lines and 0-based character columns.
pub struct Site<'a> {
    pub source: &'a str,
    pub line: usize,
    pub col: usize,
}

impl<'a> Site<'a> {
    /// From a problem's 1-based line and character column.
    pub fn new(source: &'a str, line: u32, col: u32) -> Self {
        Site {
            source,
            line: (line as usize).saturating_sub(1),
            col: (col as usize).saturating_sub(1),
        }
    }

    fn start(&self) -> Option<usize> {
        char_offset(self.source, self.line, self.col)
    }
}

/// One replacement in the source, in character offsets (what the editor uses).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

impl Edit {
    fn insert(at: usize, text: impl Into<String>) -> Self {
        Edit {
            start: at,
            end: at,
            text: text.into(),
        }
    }

    pub fn apply_to(&self, source: &str) -> String {
        let mut chars: Vec<char> = source.chars().collect();
        let end = self.end.min(chars.len());
        let start = self.start.min(end);
        chars.splice(start..end, self.text.chars());
        chars.into_iter().collect()
    }
}

pub struct Fix {
    pub description: String,
    /// Returns the single edit that fixes the problem at `site`, if it can.
    pub apply: fn(&Site) -> Option<Edit>,
}

/// Names a problem in plain language. `raw` is the engine's own message; `at`
/// is what the source holds at the reported spot (the text of the range, or,
/// when the range is empty, the character just before it) — several distinct
/// mistakes share one engine message and only differ there.
pub fn diagnose(raw: &str, at: Option<&str>) -> Diagnosis {
    let kind = classify(raw, at);
    let (headline, advice) = wording(kind, raw, at);
    Diagnosis {
        kind,
        headline,
        advice,
    }
}

/// The one-click fix for a kind of problem, if there is one.
pub fn fix_for(kind: Kind) -> Option<Fix> {
    let (id, apply): (&str, fn(&Site) -> Option<Edit>) = match kind {
        Kind::UnclosedDollar => ("fix-escape-dollar", |s| escape_here(s, '$')),
        Kind::UnclosedStar => ("fix-escape-star", |s| escape_here(s, '*')),
        Kind::UnclosedUnderscore => ("fix-escape-underscore", |s| escape_here(s, '_')),
        Kind::UnclosedLabel => ("fix-escape-label", |s| escape_here(s, '<')),
        Kind::AtSign => ("fix-escape-at", |s| escape_here(s, '@')),
        Kind::StrayHash => ("fix-escape-hash", |s| escape_here(s, '#')),
        Kind::UnknownVariable => ("fix-plain-text", |s| escape_here(s, '#')),
        Kind::UnclosedDelimiter => ("fix-close-here", close_opener_at_site),
        Kind::MissingBrace => ("fix-close-brace", |s| close_error_line(s, "}")),
        Kind::MissingBracket => ("fix-close-bracket", |s| close_error_line(s, "]")),
        Kind::MissingParen => ("fix-close-paren", |s| close_error_line(s, ")")),
        Kind::UnexpectedEnd => ("fix-close-all", close_everything),
        _ => return None,
    };
    Some(Fix {
        description: tr(id),
        apply,
    })
}

/// Whether the fix for `kind` can act on this particular problem. Most fixes
/// always can; "show as plain text" needs a `#` at the spot, so it is decided
/// from the problem's own line (`col` is the 1-based character column).
pub fn fix_applies(kind: Kind, line_text: Option<&str>, col: u32) -> bool {
    if fix_for(kind).is_none() {
        return false;
    }
    if kind != Kind::UnknownVariable {
        return true;
    }
    let chars: Vec<char> = line_text.unwrap_or("").chars().collect();
    let at = (col as usize).saturating_sub(1);
    chars.get(at) == Some(&'#') || (at > 0 && chars.get(at - 1) == Some(&'#'))
}

/// Syntax problems Typst's own parser finds in `text`: message and byte range.
fn syntax_errors(text: &str) -> Vec<(String, std::ops::Range<usize>)> {
    use typst::syntax::DiagSpanKind;
    let source = typst::syntax::Source::detached(text.to_string());
    let (errors, _) = source.root().errors_and_warnings();
    errors
        .into_iter()
        .filter_map(|e| match e.span.get() {
            DiagSpanKind::Number { num, sub_range, .. } => {
                Some((e.message.to_string(), source.range(num, sub_range)?))
            }
            DiagSpanKind::Range { range, .. } => Some((e.message.to_string(), range)),
            DiagSpanKind::Detached => None,
        })
        .collect()
}

pub fn syntax_error_count(text: &str) -> usize {
    syntax_errors(text).len()
}

/// Whether applying a fix moved things the right way. Only syntax problems can
/// be judged instantly and exactly, by re-parsing; for the rest the fix is
/// taken at its word.
pub fn fix_helped(kind: Kind, before: &str, after: &str) -> bool {
    let syntax = matches!(
        kind,
        Kind::UnclosedDelimiter
            | Kind::UnclosedDollar
            | Kind::UnclosedStar
            | Kind::UnclosedUnderscore
            | Kind::UnclosedLabel
            | Kind::UnexpectedEnd
            | Kind::MissingBrace
            | Kind::MissingBracket
            | Kind::MissingParen
            | Kind::StrayHash
    );
    !syntax || syntax_error_count(after) < syntax_error_count(before)
}

fn classify(raw: &str, at: Option<&str>) -> Kind {
    let lower = raw.to_lowercase();
    let at_first = at.and_then(|a| a.trim_start().chars().next());

    if lower.starts_with("unknown variable") {
        Kind::UnknownVariable
    } else if lower.starts_with("unknown font family")
        || (lower.contains("font") && lower.contains("not found"))
    {
        Kind::UnknownFont
    } else if lower.contains("file not found") {
        Kind::FileNotFound
    } else if lower.contains("failed to parse biblatex")
        || lower.contains("failed to parse hayagriva")
    {
        Kind::BibliographyUnreadable
    } else if lower.contains("package not found") || lower.contains("failed to download package") {
        Kind::PackageUnavailable
    } else if lower.contains("expected closing brace") {
        Kind::MissingBrace
    } else if lower.contains("expected closing bracket") {
        Kind::MissingBracket
    } else if lower.contains("expected closing paren") {
        Kind::MissingParen
    } else if lower.contains("unclosed delimiter") {
        match at_first {
            Some('$') => Kind::UnclosedDollar,
            Some('*') => Kind::UnclosedStar,
            Some('_') => Kind::UnclosedUnderscore,
            Some('(' | '[' | '{') => Kind::UnclosedDelimiter,
            _ => Kind::UnclosedOther,
        }
    } else if lower.contains("unclosed label") {
        Kind::UnclosedLabel
    } else if lower.contains("unexpected end of file") {
        Kind::UnexpectedEnd
    } else if lower.starts_with("missing argument") {
        Kind::MissingArgument
    } else if lower.starts_with("unexpected argument") {
        Kind::UnexpectedArgument
    } else if lower.contains("does not exist in the document") {
        if at_first == Some('@') && label_key(raw).is_some_and(|k| looks_like_web_address(&k)) {
            Kind::AtSign
        } else {
            Kind::MissingLabel
        }
    } else if lower.starts_with("expected") && lower.contains(", found ") {
        Kind::WrongType
    } else if lower.contains("cannot divide by zero") {
        Kind::DivideByZero
    } else if lower.contains("expected string or function") {
        Kind::IncompleteRule
    } else if lower.contains("cannot access file system") {
        Kind::FileAccess
    } else if lower == "expected expression" {
        if at.map(str::trim) == Some("#") {
            Kind::StrayHash
        } else {
            Kind::ExpectedExpression
        }
    } else {
        Kind::Other
    }
}

fn wording(kind: Kind, raw: &str, at: Option<&str>) -> (String, String) {
    let named = |named_id: &str, plain_id: &str, arg: &str, value: Option<String>| match value {
        Some(v) => tr_args(named_id, &[(arg, &v)]),
        None => tr(plain_id),
    };
    let both = |id: &str| (tr(id), tr(&format!("{id}-advice")));

    match kind {
        Kind::UnknownVariable => (
            named(
                "diag-unknown-variable-named",
                "diag-unknown-variable",
                "name",
                subject(raw),
            ),
            tr("diag-unknown-variable-advice"),
        ),
        Kind::UnknownFont => (
            named(
                "diag-unknown-font-named",
                "diag-unknown-font",
                "name",
                subject(raw),
            ),
            tr("diag-unknown-font-advice"),
        ),
        Kind::FileNotFound => (
            named(
                "diag-file-not-found-named",
                "diag-file-not-found",
                "name",
                missing_file_name(raw),
            ),
            tr("diag-file-not-found-advice"),
        ),
        Kind::BibliographyUnreadable => both("diag-bibliography-unreadable"),
        Kind::PackageUnavailable => both("diag-package-unavailable"),
        Kind::MissingBrace => closer_wording("}"),
        Kind::MissingBracket => closer_wording("]"),
        Kind::MissingParen => closer_wording(")"),
        Kind::UnclosedDelimiter => {
            let closer = match at.and_then(|a| a.trim_start().chars().next()) {
                Some('(') => Some(")"),
                Some('[') => Some("]"),
                Some('{') => Some("}"),
                Some('"') => Some("\""),
                _ => None,
            };
            match closer {
                Some(c) => closer_wording(c),
                None => both("diag-missing-closer"),
            }
        }
        Kind::UnclosedOther => both("diag-missing-closer"),
        Kind::UnclosedDollar => both("diag-trap-dollar"),
        Kind::UnclosedStar => both("diag-trap-star"),
        Kind::UnclosedUnderscore => both("diag-trap-underscore"),
        Kind::UnclosedLabel => both("diag-trap-label"),
        Kind::UnexpectedEnd => both("diag-unexpected-end"),
        Kind::MissingArgument => (
            named(
                "diag-missing-argument-named",
                "diag-missing-argument",
                "what",
                subject(raw),
            ),
            tr("diag-missing-argument-advice"),
        ),
        Kind::UnexpectedArgument => (
            named(
                "diag-unexpected-argument-named",
                "diag-unexpected-argument",
                "name",
                subject(raw),
            ),
            tr("diag-unexpected-argument-advice"),
        ),
        Kind::MissingLabel => (
            named(
                "diag-missing-label-named",
                "diag-missing-label",
                "key",
                label_key(raw),
            ),
            tr("diag-missing-label-advice"),
        ),
        Kind::AtSign => (
            tr_args(
                "diag-trap-at",
                &[("word", at.map(str::trim).unwrap_or("@"))],
            ),
            tr("diag-trap-at-advice"),
        ),
        Kind::StrayHash => both("diag-trap-hash"),
        Kind::ExpectedExpression => both("diag-expected-expression"),
        Kind::WrongType => both("diag-wrong-kind"),
        Kind::DivideByZero => both("diag-divide-by-zero"),
        Kind::IncompleteRule => both("diag-incomplete-rule"),
        Kind::FileAccess => both("diag-file-access"),
        Kind::Other => {
            let first = raw.lines().next().unwrap_or("").trim();
            if first.is_empty() {
                (tr("diag-empty"), String::new())
            } else {
                (
                    tr("diag-fallback"),
                    tr_args("diag-fallback-advice", &[("raw", first)]),
                )
            }
        }
    }
}

fn closer_wording(closer: &str) -> (String, String) {
    (
        tr_args("diag-missing-closer-named", &[("thing", closer)]),
        tr("diag-missing-closer-advice"),
    )
}

/// The value after the first `:` in an engine message, e.g.
/// `unknown variable: foo` -> `foo`.
fn subject(raw: &str) -> Option<String> {
    let v = raw.split_once(':')?.1.trim();
    if v.is_empty() {
        None
    } else {
        Some(v.trim_matches('`').to_string())
    }
}

/// `label `<key>` does not exist in the document` -> `key`.
fn label_key(raw: &str) -> Option<String> {
    raw.split_once("label ")
        .and_then(|(_, rest)| rest.split_whitespace().next())
        .map(|s| {
            s.trim_matches(|c| c == '`' || c == '<' || c == '>')
                .to_string()
        })
        .filter(|s| !s.is_empty())
}

/// `example.com`, `site.co.uk` — but not a citation key like `smith.2020`.
fn looks_like_web_address(key: &str) -> bool {
    let mut parts = key.rsplit('.');
    let (Some(tld), Some(_)) = (parts.next(), parts.next()) else {
        return false;
    };
    tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic())
}

/// The engine's wording is `file not found (searched at /abs/path)`, so the
/// name lives inside the parentheses rather than after a colon. Show the bare
/// filename — the absolute path it searched is noise to someone who just wants
/// to know which picture is missing.
fn missing_file_name(raw: &str) -> Option<String> {
    raw.split_once("searched at ")
        .map(|(_, rest)| rest.trim_end_matches(')').trim())
        .map(|p| {
            Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.to_string())
        })
        .or_else(|| {
            raw.split_once("file not found")
                .and_then(|(_, rest)| rest.split('(').next())
                .map(|s| {
                    s.trim_matches(|c: char| c == ':' || c.is_whitespace())
                        .to_string()
                })
        })
        .filter(|s| !s.is_empty())
}

// ── Fix implementations ───────────────────────────────────────────────────────

/// Character offset of (0-based `line`, 0-based `col`), clamped to the end of
/// that line. `None` when the line doesn't exist.
fn char_offset(source: &str, line: usize, col: usize) -> Option<usize> {
    let mut offset = 0;
    for (i, text) in source.split('\n').enumerate() {
        let len = text.chars().count();
        if i == line {
            return Some(offset + col.min(len));
        }
        offset += len + 1;
    }
    None
}

fn char_at(source: &str, offset: usize) -> Option<char> {
    source.chars().nth(offset)
}

/// Put a backslash in front of `ch`, so it is written as ordinary text. The
/// character is at the reported spot — or just before it, when the range is
/// empty and the position is the point after the character (a bare `#`).
fn escape_here(site: &Site, ch: char) -> Option<Edit> {
    let start = site.start()?;
    let idx = if char_at(site.source, start) == Some(ch) {
        start
    } else if start > 0 && char_at(site.source, start - 1) == Some(ch) {
        start - 1
    } else {
        return None;
    };
    if idx > 0 && char_at(site.source, idx - 1) == Some('\\') {
        return None;
    }
    Some(Edit::insert(idx, "\\"))
}

fn closer_for(open: char) -> Option<char> {
    match open {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        _ => None,
    }
}

/// The reported spot is the opening character of the unclosed pair; close it
/// where it was opened, at the end of that line.
fn close_opener_at_site(site: &Site) -> Option<Edit> {
    let closer = closer_for(char_at(site.source, site.start()?)?)?;
    close_line(site.source, site.line, &closer.to_string())
}

fn close_error_line(site: &Site, closer: &str) -> Option<Edit> {
    close_line(site.source, site.line, closer)
}

/// Insert `closer` after the last visible character of line `line`.
fn close_line(source: &str, line: usize, closer: &str) -> Option<Edit> {
    let text = source.split('\n').nth(line)?;
    let visible = text.trim_end().chars().count();
    let start = char_offset(source, line, 0)?;
    Some(Edit::insert(start + visible, closer))
}

/// "Unexpected end of file" has no useful location: the missing closer is
/// somewhere above. Ask Typst's own parser which openers were left unclosed —
/// it knows about strings, comments and prose — and close them, innermost
/// first, at the end of the document.
fn close_everything(site: &Site) -> Option<Edit> {
    let mut spots: Vec<usize> = syntax_errors(site.source)
        .into_iter()
        .filter(|(message, _)| message.contains("unclosed delimiter"))
        .map(|(_, range)| range.start)
        .collect();
    spots.sort_unstable();
    let openers: Vec<char> = spots
        .into_iter()
        .filter_map(|at| site.source.get(at..)?.chars().next())
        .collect();
    let closers: String = openers
        .iter()
        .rev()
        .filter_map(|c| closer_for(*c))
        .collect();
    if closers.is_empty() {
        return None;
    }
    let end = site.source.chars().count();
    let nl = if site.source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    Some(Edit::insert(end, format!("{nl}{closers}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headline(raw: &str) -> String {
        diagnose(raw, None).headline
    }

    // ── Wording ──────────────────────────────────────────────────────────────

    #[test]
    fn an_undefined_name_is_explained_without_jargon() {
        let d = diagnose("unknown variable: foo", None);
        assert!(d.headline.contains("foo"), "should name it: {}", d.headline);
        assert!(
            !d.headline.to_lowercase().contains("variable"),
            "headline should not use compiler jargon: {}",
            d.headline
        );
        assert!(
            d.advice.contains('#'),
            "advice should say what to do: {}",
            d.advice
        );
    }

    #[test]
    fn a_missing_file_names_the_file() {
        // The engine's phrasing puts the path in parentheses after "searched
        // at", and gives it absolute. The reader wants the filename.
        let h = headline("file not found (searched at /home/me/docs/missing-picture.png)");
        assert!(h.contains("missing-picture.png"), "got: {h}");
        assert!(!h.contains("/home/me"), "path noise leaked in: {h}");

        let h = headline("file not found: figures/plot.png");
        assert!(h.contains("plot.png"), "got: {h}");
    }

    #[test]
    fn a_wrong_option_name_is_quoted_in_the_headline() {
        let d = diagnose("unexpected argument: colour", None);
        assert!(
            d.headline.contains("colour"),
            "should name it: {}",
            d.headline
        );
        assert!(
            d.advice.contains("fill:"),
            "should suggest the real one: {}",
            d.advice
        );
    }

    #[test]
    fn a_missing_argument_is_named() {
        assert!(headline("missing argument: body").contains("body"));
    }

    #[test]
    fn every_unclosed_delimiter_phrasing_gets_an_explanation() {
        for raw in [
            "expected closing brace",
            "expected closing bracket",
            "expected closing paren",
            "unclosed delimiter",
        ] {
            let d = diagnose(raw, None);
            assert!(!d.advice.is_empty(), "{raw} should carry advice");
            assert!(
                !d.headline.to_lowercase().contains("expected"),
                "{raw} still reads like a parser message: {}",
                d.headline
            );
        }
    }

    #[test]
    fn an_unclosed_bracket_names_the_closer_it_is_waiting_for() {
        assert!(diagnose("unclosed delimiter", Some("("))
            .headline
            .contains(')'));
        assert!(diagnose("unclosed delimiter", Some("["))
            .headline
            .contains(']'));
        assert!(diagnose("unclosed delimiter", Some("{"))
            .headline
            .contains('}'));
    }

    #[test]
    fn a_malformed_bibliography_entry_gets_a_plain_language_explanation() {
        let d = diagnose("failed to parse BibLaTeX (wrong number of digits)", None);
        assert!(
            d.headline.to_lowercase().contains("bibliography"),
            "got: {}",
            d.headline
        );
        assert!(
            d.advice.contains("year"),
            "should mention the common Zotero cause: {}",
            d.advice
        );
    }

    #[test]
    fn an_unrecognised_message_is_quoted_in_the_advice_not_the_headline() {
        let d = diagnose("some completely novel error text", None);
        assert_eq!(d.kind, Kind::Other);
        assert!(!d.headline.contains("novel"), "got: {}", d.headline);
        assert!(
            d.advice.contains("some completely novel error text"),
            "the exact words must stay reachable: {}",
            d.advice
        );
    }

    #[test]
    fn an_empty_message_still_says_something() {
        assert!(!diagnose("", None).headline.is_empty());
    }

    #[test]
    fn no_headline_says_compiler_or_typst() {
        let cases: &[(&str, Option<&str>)] = &[
            ("unknown variable: foo", None),
            ("unknown font family: Foo", None),
            ("file not found", None),
            ("failed to parse BibLaTeX", None),
            ("package not found", None),
            ("unclosed delimiter", None),
            ("unclosed delimiter", Some("$")),
            ("unexpected end of file", None),
            ("missing argument", None),
            ("unexpected argument", None),
            ("label `<a>` does not exist in the document", None),
            ("expected content, found string", None),
            ("cannot divide by zero", None),
            ("expected string or function", None),
            ("cannot access file system", None),
            ("expected expression", Some("#")),
            ("expected expression", None),
            ("unclosed label", None),
            ("something else entirely", None),
        ];
        for (raw, at) in cases {
            let d = diagnose(raw, *at);
            let h = d.headline.to_lowercase();
            assert!(
                !h.starts_with("diag-") && !d.advice.starts_with("diag-"),
                "{raw}: a wording id is missing from the .ftl file"
            );
            assert!(
                !h.contains("compiler") && !h.contains("typst"),
                "{raw}: {}",
                d.headline
            );
            assert!(
                !d.advice.to_lowercase().contains("the compiler"),
                "{raw}: {}",
                d.advice
            );
        }
    }

    // ── Beginner traps (engine messages verified against the real compiler) ──

    #[test]
    fn a_lone_dollar_sign_is_explained_as_a_formula() {
        let d = diagnose("unclosed delimiter", Some("$"));
        assert_eq!(d.kind, Kind::UnclosedDollar);
        assert!(
            d.advice.contains("\\$"),
            "should show the escape: {}",
            d.advice
        );
    }

    #[test]
    fn a_lone_star_or_underscore_is_explained_as_bold_or_italic() {
        let star = diagnose("unclosed delimiter", Some("*"));
        assert_eq!(star.kind, Kind::UnclosedStar);
        assert!(star.advice.contains("bold"));
        let under = diagnose("unclosed delimiter", Some("_"));
        assert_eq!(under.kind, Kind::UnclosedUnderscore);
        assert!(under.advice.contains("italic"));
    }

    #[test]
    fn an_email_address_is_not_mistaken_for_a_missing_citation() {
        let raw = "label `<example.com>` does not exist in the document";
        let d = diagnose(raw, Some("@example.com"));
        assert_eq!(d.kind, Kind::AtSign);
        assert!(d.headline.contains("@example.com"), "got: {}", d.headline);
        assert!(d.advice.contains("\\@"), "got: {}", d.advice);
    }

    #[test]
    fn a_dotted_citation_key_is_still_a_missing_citation() {
        let raw = "label `<smith.2020>` does not exist in the document";
        assert_eq!(diagnose(raw, Some("@smith.2020")).kind, Kind::MissingLabel);
        let raw = "label `<smith>` does not exist in the document";
        assert_eq!(diagnose(raw, Some("@smith")).kind, Kind::MissingLabel);
    }

    #[test]
    fn a_hash_with_nothing_after_it_is_a_stray_hash() {
        assert_eq!(
            diagnose("expected expression", Some("#")).kind,
            Kind::StrayHash
        );
        assert_eq!(
            diagnose("expected expression", Some("+")).kind,
            Kind::ExpectedExpression
        );
        assert_eq!(
            diagnose("expected expression", None).kind,
            Kind::ExpectedExpression
        );
    }

    #[test]
    fn an_unclosed_label_is_explained() {
        assert_eq!(diagnose("unclosed label", None).kind, Kind::UnclosedLabel);
    }

    // ── Fixes ────────────────────────────────────────────────────────────────

    #[test]
    fn fixes_are_keyed_on_the_kind_not_the_wording() {
        assert!(fix_for(diagnose("expected closing brace", None).kind).is_some());
        assert!(fix_for(diagnose("unknown variable: x", None).kind).is_some());
        assert!(fix_for(diagnose("file not found", None).kind).is_none());
        assert!(fix_for(diagnose("some completely unrelated error", None).kind).is_none());
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert_eq!(
            diagnose("Error: Expected Closing Brace", None).kind,
            Kind::MissingBrace
        );
    }

    /// A site from 1-based line/column, the way the panel reports them.
    fn at(src: &str, line: u32, col: u32) -> Site<'_> {
        Site::new(src, line, col)
    }

    fn fixed(kind: Kind, src: &str, line: u32, col: u32) -> Option<String> {
        let fix = fix_for(kind)?;
        let edit = (fix.apply)(&at(src, line, col))?;
        Some(edit.apply_to(src))
    }

    #[test]
    fn every_fix_has_a_real_description() {
        for kind in [
            Kind::UnclosedDelimiter,
            Kind::UnclosedDollar,
            Kind::UnclosedStar,
            Kind::UnclosedUnderscore,
            Kind::UnclosedLabel,
            Kind::AtSign,
            Kind::StrayHash,
            Kind::UnknownVariable,
            Kind::MissingBrace,
            Kind::MissingBracket,
            Kind::MissingParen,
            Kind::UnexpectedEnd,
        ] {
            let d = fix_for(kind)
                .unwrap_or_else(|| panic!("{kind:?} lost its fix"))
                .description;
            assert!(!d.is_empty());
            assert!(!d.starts_with("fix-"), "{kind:?} shows its id: {d}");
        }
    }

    #[test]
    fn a_dollar_is_escaped_where_the_error_points() {
        assert_eq!(
            fixed(Kind::UnclosedDollar, "It costs $5 today", 1, 10).unwrap(),
            "It costs \\$5 today"
        );
    }

    #[test]
    fn the_right_dollar_is_escaped_on_a_line_with_two() {
        assert_eq!(
            fixed(Kind::UnclosedDollar, "a $b then $c", 1, 11).unwrap(),
            "a $b then \\$c"
        );
    }

    #[test]
    fn star_underscore_label_and_at_are_escaped() {
        assert_eq!(fixed(Kind::UnclosedStar, "5 * 3", 1, 3).unwrap(), "5 \\* 3");
        assert_eq!(
            fixed(Kind::UnclosedUnderscore, "snake_case", 1, 6).unwrap(),
            "snake\\_case"
        );
        assert_eq!(fixed(Kind::UnclosedLabel, "a <b", 1, 3).unwrap(), "a \\<b");
        assert_eq!(
            fixed(Kind::AtSign, "mail me@example.com", 1, 8).unwrap(),
            "mail me\\@example.com"
        );
    }

    #[test]
    fn a_stray_hash_is_found_just_before_an_empty_range() {
        assert_eq!(
            fixed(Kind::StrayHash, "issue # 4", 1, 8).unwrap(),
            "issue \\# 4"
        );
    }

    #[test]
    fn an_already_escaped_character_is_not_escaped_twice() {
        assert!(fixed(Kind::UnclosedDollar, "costs \\$5", 1, 8).is_none());
    }

    #[test]
    fn nothing_is_escaped_when_the_character_is_not_at_the_spot() {
        assert!(fixed(Kind::UnclosedDollar, "no dollar here", 1, 4).is_none());
        assert!(fixed(Kind::UnclosedDollar, "one line", 9, 1).is_none());
    }

    #[test]
    fn columns_count_characters_not_bytes() {
        assert_eq!(
            fixed(Kind::UnclosedDollar, "café $5", 1, 6).unwrap(),
            "café \\$5"
        );
    }

    #[test]
    fn an_unknown_name_is_shown_as_plain_text() {
        assert_eq!(
            fixed(Kind::UnknownVariable, "Call #foo now", 1, 7).unwrap(),
            "Call \\#foo now"
        );
        assert_eq!(
            fixed(Kind::UnknownVariable, "Call #foo now", 1, 6).unwrap(),
            "Call \\#foo now"
        );
    }

    #[test]
    fn an_unknown_name_without_a_hash_gets_no_fix_that_applies() {
        assert!(fixed(Kind::UnknownVariable, "#let a = b + 1", 1, 13).is_none());
        assert!(!fix_applies(
            Kind::UnknownVariable,
            Some("#let a = b + 1"),
            13
        ));
        assert!(fix_applies(Kind::UnknownVariable, Some("Call #foo now"), 7));
        assert!(fix_applies(Kind::MissingBrace, None, 1));
        assert!(!fix_applies(Kind::FileNotFound, None, 1));
    }

    #[test]
    fn an_unclosed_opener_is_closed_at_the_end_of_its_own_line() {
        assert_eq!(
            fixed(Kind::UnclosedDelimiter, "#foo(1, 2   \nmore text", 1, 5).unwrap(),
            "#foo(1, 2)   \nmore text"
        );
    }

    #[test]
    fn the_closer_matches_the_opener_at_the_spot() {
        assert_eq!(
            fixed(Kind::UnclosedDelimiter, "#box[hi\nnext", 1, 5).unwrap(),
            "#box[hi]\nnext"
        );
        assert!(fixed(Kind::UnclosedDelimiter, "no opener\n", 1, 3).is_none());
    }

    #[test]
    fn closing_the_error_line_keeps_crlf_endings() {
        assert_eq!(
            fixed(Kind::MissingBrace, "#let x = {\r\nfoo\r\nbar", 3, 1).unwrap(),
            "#let x = {\r\nfoo\r\nbar}"
        );
        assert_eq!(
            fixed(Kind::MissingBrace, "#let x = {\r\nfoo\r\nbar\r\nbaz", 3, 1).unwrap(),
            "#let x = {\r\nfoo\r\nbar}\r\nbaz"
        );
    }

    #[test]
    fn closing_a_line_out_of_range_does_nothing() {
        assert!(fixed(Kind::MissingBracket, "one line only", 6, 1).is_none());
    }

    #[test]
    fn closing_everything_closes_the_innermost_first() {
        assert_eq!(
            fixed(Kind::UnexpectedEnd, "#let x = {\nfoo(bar[baz", 1, 1).unwrap(),
            "#let x = {\nfoo(bar[baz\n])}"
        );
    }

    #[test]
    fn closing_everything_ignores_brackets_in_strings_and_comments() {
        let src = "#let x = (\"a ) b\" // )\n";
        let out = fixed(Kind::UnexpectedEnd, src, 1, 1).unwrap();
        assert!(out.ends_with(")"), "got {out:?}");
        assert_eq!(syntax_error_count(&out), 0, "got {out:?}");
    }

    #[test]
    fn closing_everything_keeps_crlf_endings() {
        assert_eq!(
            fixed(Kind::UnexpectedEnd, "#box[a\r\nb", 1, 1).unwrap(),
            "#box[a\r\nb\r\n]"
        );
    }

    #[test]
    fn closing_everything_needs_something_unclosed() {
        assert!(fixed(Kind::UnexpectedEnd, "#let x = (1 + 2)", 1, 1).is_none());
    }

    #[test]
    fn a_fix_that_clears_a_syntax_error_is_judged_helpful() {
        assert!(fix_helped(Kind::UnclosedDollar, "costs $5", "costs \\$5"));
        assert!(!fix_helped(Kind::UnclosedDollar, "costs $5", "costs $5 "));
        assert!(fix_helped(Kind::UnknownVariable, "#foo", "\\#foo"));
    }

    #[test]
    fn every_syntax_fix_removes_its_error_end_to_end() {
        // Real compiler-reported spots (verified against Typst 0.15.1): the
        // mistake, its kind, and the 1-based spot Zerkalo would report.
        let cases = [
            ("costs $5", Kind::UnclosedDollar),
            ("a * b", Kind::UnclosedStar),
            ("snake_case and _x", Kind::UnclosedUnderscore),
            ("see <label", Kind::UnclosedLabel),
        ];
        for (src, kind) in cases {
            let (msg, range) = syntax_errors(src).into_iter().next().unwrap_or_else(|| {
                panic!("expected a syntax error in {src:?}");
            });
            let d = diagnose(&msg, src.get(range.start..).and_then(|t| t.get(..1)));
            assert_eq!(d.kind, kind, "{src:?}: {msg}");
            let col = src[..range.start].chars().count() as u32 + 1;
            let out = fixed(kind, src, 1, col).unwrap_or_else(|| panic!("no fix for {src:?}"));
            assert!(fix_helped(kind, src, &out), "{src:?} -> {out:?}");
        }
    }
}
