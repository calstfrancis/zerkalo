//! The one grammar for citation keys in Typst source.
//!
//! Before this module the editor, the References window, the rename command and
//! the Library each had their own regex, and they disagreed about `.`, about
//! accented letters, and about `#cite(<k>, form: "prose")`. Everything that has
//! to find, count or rename a key in document text goes through here instead.
//!
//! A key starts with a letter (any script) and continues with letters, digits
//! and `_ - : .`; a trailing `.`, `:` or `-` is punctuation and is not part of
//! the key. An `@` only starts a citation when it is not glued to the end of a
//! word, so `me@example.org` is an e-mail address and not a citation of
//! `example.org`.

use std::collections::BTreeSet;
use std::ops::Range;

/// One place a key is used. `range` covers only the key's own characters, so
/// a rename can replace exactly those bytes and leave every other byte of the
/// document alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyUse {
    pub key: String,
    pub range: Range<usize>,
}

fn is_key_start(c: char) -> bool {
    c.is_alphabetic()
}

fn is_key_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | ':' | '.')
}

/// The key beginning at `start`, trimmed of trailing sentence punctuation, as
/// a byte range, or `None` if there isn't one.
fn key_at(text: &str, start: usize) -> Option<Range<usize>> {
    let rest = &text[start..];
    let mut chars = rest.char_indices();
    let (_, first) = chars.next()?;
    if !is_key_start(first) {
        return None;
    }
    let mut end = first.len_utf8();
    for (i, c) in chars {
        if is_key_char(c) {
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    let trimmed = rest[..end].trim_end_matches(['.', ':', '-']);
    if trimmed.is_empty() {
        None
    } else {
        Some(start..start + trimmed.len())
    }
}

/// `@key`: refused when the `@` follows a letter, digit, `_`, `.` or `-`
/// (an e-mail address, a path, an `@preview/…` import after a quote is fine).
fn at_sign_starts_citation(text: &str, at: usize) -> bool {
    match text[..at].chars().next_back() {
        None => true,
        Some(c) => !(c.is_alphanumeric() || matches!(c, '_' | '.' | '-')),
    }
}

/// Every citation in `text`, in order: `@key`, `#cite(<key>…)` and
/// `#cite("key"…)`, with or without further arguments.
pub fn find_uses(text: &str) -> Vec<KeyUse> {
    let mut uses = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'@' => {
                if at_sign_starts_citation(text, i) {
                    if let Some(r) = key_at(text, i + 1) {
                        i = r.end;
                        uses.push(KeyUse {
                            key: text[r.clone()].to_string(),
                            range: r,
                        });
                        continue;
                    }
                }
                i += 1;
            }
            b'#' if text[i..].starts_with("#cite(") => {
                let mut j = i + "#cite(".len();
                while text[j..].starts_with(char::is_whitespace) {
                    j += text[j..].chars().next().map_or(1, char::len_utf8);
                }
                let (open, close) = if text[j..].starts_with('<') {
                    ('<', '>')
                } else if text[j..].starts_with('"') {
                    ('"', '"')
                } else {
                    i += 1;
                    continue;
                };
                let start = j + open.len_utf8();
                if let Some(r) = key_at(text, start) {
                    // The key must be the whole of the label/string, so
                    // `#cite("some key")` and `#cite(<a.b>)` agree with how
                    // Typst itself reads them.
                    if text[r.end..].starts_with(close) {
                        i = r.end;
                        uses.push(KeyUse {
                            key: text[r.clone()].to_string(),
                            range: r,
                        });
                        continue;
                    }
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    uses
}

pub fn keys_in(text: &str) -> BTreeSet<String> {
    find_uses(text).into_iter().map(|u| u.key).collect()
}

/// `<label>` definitions — `= Heading <intro>`, `#figure(…) <fig-1>` — which
/// are what an `@intro` or `@fig-1` cross-reference points at, as opposed to a
/// bibliography entry. A `<x>` straight after `(` is a reference, not a
/// definition.
pub fn labels_in(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (at, _) in text.match_indices('<') {
        let before = text[..at].trim_end();
        if before.ends_with('(') || before.ends_with(',') {
            continue;
        }
        if let Some(r) = key_at(text, at + 1) {
            if text[r.end..].starts_with('>') {
                out.insert(text[r].to_string());
            }
        }
    }
    out
}

/// Renames `old` to `new` everywhere it is cited in `text`. Only the key's own
/// characters are replaced; the rest of the text is byte-identical.
pub fn rename_in_text(text: &str, old: &str, new: &str) -> (String, bool) {
    let hits: Vec<KeyUse> = find_uses(text)
        .into_iter()
        .filter(|u| u.key == old)
        .collect();
    if hits.is_empty() || old == new {
        return (text.to_string(), false);
    }
    let mut out =
        String::with_capacity(text.len() + hits.len() * new.len().saturating_sub(old.len()));
    let mut last = 0;
    for h in hits {
        out.push_str(&text[last..h.range.start]);
        out.push_str(new);
        last = h.range.end;
    }
    out.push_str(&text[last..]);
    (out, true)
}

/// Why a proposed key can't be used, in words for the person typing it.
pub fn key_problem(key: &str) -> Option<&'static str> {
    if key.is_empty() {
        return Some("Give the entry a short nickname, like smith2019.");
    }
    if key.chars().any(char::is_whitespace) {
        return Some("A cite key can't contain spaces.");
    }
    if !key.chars().next().is_some_and(is_key_start) {
        return Some("A cite key must start with a letter.");
    }
    if !key.chars().all(is_key_char) {
        return Some("Use only letters, numbers and _ - : . in a cite key.");
    }
    if key.ends_with(['.', ':', '-']) {
        return Some("A cite key can't end with . : or -.");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(text: &str) -> Vec<String> {
        find_uses(text).into_iter().map(|u| u.key).collect()
    }

    #[test]
    fn finds_shorthand_and_cite_forms() {
        assert_eq!(
            keys("See @butler1990, then #cite(<cone1970>) and #cite(\"wcc\")."),
            vec!["butler1990", "cone1970", "wcc"]
        );
    }

    #[test]
    fn cite_with_extra_arguments_is_found() {
        assert_eq!(
            keys("#cite(<k>, form: \"prose\") #cite(<j>, supplement: [p. 4])"),
            vec!["k", "j"]
        );
    }

    #[test]
    fn trailing_punctuation_is_not_part_of_the_key() {
        assert_eq!(
            keys("as @smith2020. And @jones:2019: yes"),
            vec!["smith2020", "jones:2019"]
        );
    }

    #[test]
    fn dots_inside_a_key_and_unicode_letters_work() {
        assert_eq!(
            keys("@smith.2020 and @žižek2008"),
            vec!["smith.2020", "žižek2008"]
        );
    }

    #[test]
    fn an_email_address_is_not_a_citation() {
        assert!(keys("write to me@k1.org please").is_empty());
        assert_eq!(keys("(@k1) and\n@k2"), vec!["k1", "k2"]);
    }

    #[test]
    fn supplement_brackets_leave_the_key_alone() {
        assert_eq!(keys("@barth1932[p. 12]"), vec!["barth1932"]);
    }

    #[test]
    fn rename_replaces_only_the_key_bytes() {
        let text = "A @k1 B #cite(<k1>, form: \"prose\") C #cite(\"k1\") D me@k1.org E @k10 F @k1.";
        let (out, changed) = rename_in_text(text, "k1", "new1");
        assert!(changed);
        assert_eq!(
            out,
            "A @new1 B #cite(<new1>, form: \"prose\") C #cite(\"new1\") D me@k1.org E @k10 F @new1."
        );
    }

    #[test]
    fn rename_handles_dotted_and_unicode_keys() {
        let (out, _) = rename_in_text("@smith.2020, @žižek2008;", "smith.2020", "smith2020");
        assert_eq!(out, "@smith2020, @žižek2008;");
        let (out, _) = rename_in_text("@žižek2008;", "žižek2008", "zizek2008");
        assert_eq!(out, "@zizek2008;");
    }

    #[test]
    fn rename_with_nothing_to_do_returns_the_text_untouched() {
        let (out, changed) = rename_in_text("plain @other", "k1", "k2");
        assert!(!changed);
        assert_eq!(out, "plain @other");
    }

    #[test]
    fn labels_are_definitions_not_references() {
        let text = "= Intro <intro>\n#figure(x) <fig-1>\nSee #ref(<intro>) and #cite(<butler>, form: \"p\")";
        let labels = labels_in(text);
        assert!(labels.contains("intro"));
        assert!(labels.contains("fig-1"));
        assert!(!labels.contains("butler"));
    }

    #[test]
    fn key_problems_are_explained() {
        assert!(key_problem("smith2019").is_none());
        assert!(key_problem("smith.2020").is_none());
        assert!(key_problem("").is_some());
        assert!(key_problem("two words").is_some());
        assert!(key_problem("1abc").is_some());
        assert!(key_problem("a,b").is_some());
        assert!(key_problem("a{b").is_some());
    }
}
