//! Apostrophes Typst would set as quotation marks.
//!
//! Typst picks a smart quote's shape from the character just before it in the
//! laid-out text: after a letter a `'` is an apostrophe, but after `)`, `]`, a
//! full stop, a closing quote or a citation like "(Smith 2020)" it opens a new
//! quotation (‘), after a digit it becomes a prime (′), after a space it always
//! opens (‘tis, the ‘90s), and inside an open single quotation a `'` between two
//! letters closes it — invisible in English, but `‚Kant's Werk‘` came out as
//! `‚Kant‘s Werk’` in German. Text that reaches the page as a Typst string —
//! the title page fields — never gets smart quotes at all.
//!
//! The `'`s that are apostrophes are handed to the compiler as a literal ’, and
//! in the title-page strings the quotes are curled. One character always
//! replaces one, so lines and character offsets — everything the preview sync
//! and error locations use — stay exactly where they were.

use typst::syntax::{LinkedNode, SyntaxKind};

const APOSTROPHE: char = '\u{2019}';
const OPEN_SINGLE: char = '\u{2018}';

/// Words a leading apostrophe shortens: `'tis`, `rock 'n' roll`, `'em`.
const ELISIONS: &[&str] = &[
    "tis", "twas", "twere", "twould", "twill", "em", "til", "bout", "cept", "nuff", "cause", "n",
];

/// The title-page fields the template dialog writes as strings.
const DISPLAY_STRINGS: &[&str] = &[
    "doc-title",
    "doc-subtitle",
    "doc-author",
    "doc-affil",
    "doc-course",
    "doc-professor",
    "doc-date",
];

/// `text` with those apostrophes made literal, or `None` when nothing changes.
pub fn fix(text: &str) -> Option<String> {
    if !text.contains('\'') || text.contains("smartquote") {
        return None;
    }
    let root = typst::syntax::parse(text);
    let mut at = Vec::new();
    walk(&LinkedNode::new(&root), &mut at);
    if at.is_empty() {
        return None;
    }
    at.sort_unstable_by_key(|&(offset, _)| offset);
    let mut out = String::with_capacity(text.len() + at.len() * 2);
    let mut last = 0;
    for (offset, c) in at {
        out.push_str(&text[last..offset]);
        out.push(c);
        last = offset + 1;
    }
    out.push_str(&text[last..]);
    Some(out)
}

/// Curly quotes for text typed into a field that becomes a Typst string, where
/// Typst won't do it: `Kant's "Critique"` → `Kant’s “Critique”`.
pub fn smarten_plain(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 8);
    let mut open_single = false;
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '\'' => out.push(plain_single(&chars, i, &mut open_single)),
            '"' => out.push(if opens_after(i.checked_sub(1).map(|p| chars[p])) {
                '\u{201C}'
            } else {
                '\u{201D}'
            }),
            _ => out.push(c),
        }
    }
    out
}

fn plain_single(chars: &[char], i: usize, open: &mut bool) -> char {
    let prev = i.checked_sub(1).map(|p| chars[p]);
    let next = chars.get(i + 1).copied();
    if opens_after(prev) {
        let rest: String = chars[i + 1..].iter().collect();
        let word = chars[i + 1..]
            .iter()
            .take_while(|c| c.is_alphabetic())
            .count();
        if is_elision(&rest, chars.get(i + 1 + word) == Some(&'\'')) {
            return APOSTROPHE;
        }
        *open = true;
        return OPEN_SINGLE;
    }
    if next.is_some_and(char::is_alphabetic) || !*open {
        return APOSTROPHE;
    }
    *open = false;
    APOSTROPHE
}

fn opens_after(prev: Option<char>) -> bool {
    prev.is_none_or(|c| c.is_whitespace() || opens(c))
}

fn walk(node: &LinkedNode, at: &mut Vec<(usize, char)>) {
    match node.kind() {
        SyntaxKind::Markup => markup(node, at),
        SyntaxKind::LetBinding => display_string(node, at),
        _ => {}
    }
    for child in node.children() {
        walk(&child, at);
    }
}

/// What came before a `'`, as far as deciding its shape goes.
#[derive(Clone, Copy, PartialEq)]
enum Before {
    /// Nothing, a space, an opening bracket or a dash: the `'` opens a quote
    /// unless it starts an elision.
    Open,
    Letter,
    Digit,
    /// Closing punctuation, a citation, a function's output and the like.
    Other,
}

/// The `'`s in one run of markup, in order, with a rough idea of whether a
/// single quotation is open — reset at each paragraph break, as Typst does.
fn markup(node: &LinkedNode, at: &mut Vec<(usize, char)>) {
    let mut open = false;
    for child in node.children() {
        if child.kind() == SyntaxKind::Parbreak {
            open = false;
        }
        if child.kind() != SyntaxKind::SmartQuote || child.leaf_text() != "'" {
            continue;
        }
        let next = neighbour(&child, 1).filter(|n| n.kind() == SyntaxKind::Text);
        let next_text = next.as_ref().map_or("", |n| n.leaf_text().as_str());
        let next_letter = next_text.chars().next().is_some_and(char::is_alphabetic);
        let apostrophe = match neighbour(&child, -1).map_or(Before::Open, |p| before(&p)) {
            Before::Open => {
                let then_quote = next
                    .as_ref()
                    .and_then(|n| neighbour(n, 1))
                    .is_some_and(|n| n.kind() == SyntaxKind::SmartQuote && n.leaf_text() == "'");
                let elision = is_elision(next_text, then_quote);
                open |= !elision;
                elision
            }
            _ if next_letter => true,
            // A closing quote — Typst shapes it right.
            _ if open => {
                open = false;
                false
            }
            // `the boys' toys` is already an apostrophe and `5' tall` a prime;
            // `the U.S.' policy` would open a quotation.
            Before::Other => true,
            Before::Letter | Before::Digit => false,
        };
        if apostrophe {
            at.push((child.offset(), APOSTROPHE));
        }
    }
}

fn before(prev: &LinkedNode) -> Before {
    match prev.kind() {
        SyntaxKind::Text => match prev.leaf_text().chars().last() {
            Some(c) if c.is_alphabetic() => Before::Letter,
            Some(c) if c.is_numeric() => Before::Digit,
            Some(c) if c.is_whitespace() || opens(c) => Before::Open,
            Some(_) => Before::Other,
            None => Before::Open,
        },
        // `"Hamlet"'s`: only when that `"` closed a quotation.
        SyntaxKind::SmartQuote if prev.leaf_text() == "\"" => {
            match neighbour(prev, -1).map_or(Before::Open, |p| before(&p)) {
                Before::Open => Before::Open,
                _ => Before::Other,
            }
        }
        // `@smith's`, `#footnote[…]'s`, `*bold*'s`, `` `code`'s ``: the text
        // these produce can end in a bracket or a digit.
        SyntaxKind::Ref
        | SyntaxKind::Strong
        | SyntaxKind::Emph
        | SyntaxKind::Raw
        | SyntaxKind::Link
        | SyntaxKind::Label
        | SyntaxKind::Equation
        | SyntaxKind::Escape => Before::Other,
        SyntaxKind::Shorthand if prev.leaf_text() == "..." => Before::Other,
        _ if is_embedded_code(prev) => Before::Other,
        _ => Before::Open,
    }
}

/// `'tis`, `'em`, `rock 'n' roll`, `the '90s`, `class of '22`. `then_quote`:
/// a `'` follows the word, as in `'n'` — or in `'em'`, a quoted word.
fn is_elision(after: &str, then_quote: bool) -> bool {
    let mut digits = after.chars().take_while(char::is_ascii_digit);
    if digits.by_ref().count() == 2 {
        return true;
    }
    let word: String = after
        .chars()
        .take_while(|c| c.is_alphabetic())
        .collect::<String>()
        .to_lowercase();
    match word.as_str() {
        "n" => then_quote,
        w => ELISIONS.contains(&w) && !then_quote,
    }
}

/// `#let doc-title = "Kant's 'Critique'"`: the `'`s curled in place.
fn display_string(binding: &LinkedNode, at: &mut Vec<(usize, char)>) {
    let mut children = binding.children();
    let named = children.any(|c| {
        c.kind() == SyntaxKind::Ident && DISPLAY_STRINGS.contains(&c.leaf_text().as_str())
    });
    let Some(string) = named
        .then(|| children.find(|c| c.kind() == SyntaxKind::Str))
        .flatten()
    else {
        return;
    };
    let literal = string.leaf_text();
    let chars: Vec<char> = literal.chars().collect();
    let mut open = false;
    let mut byte = string.offset();
    for (i, &c) in chars.iter().enumerate() {
        if c == '\'' {
            at.push((byte, plain_single(&chars, i, &mut open)));
        }
        byte += c.len_utf8();
    }
}

/// The sibling right beside `node`, spaces and comments included — unlike
/// `LinkedNode::prev_sibling`, which skips them and would make `(a) 'b'`
/// look like `(a)'b'`.
fn neighbour<'a>(node: &LinkedNode<'a>, step: isize) -> Option<LinkedNode<'a>> {
    let index = node.index().checked_add_signed(step)?;
    node.parent()?.children().nth(index)
}

fn is_embedded_code(node: &LinkedNode) -> bool {
    neighbour(node, -1).is_some_and(|n| n.kind() == SyntaxKind::Hash)
}

fn opens(c: char) -> bool {
    matches!(
        c,
        '(' | '[' | '{' | '“' | '‘' | '«' | '‹' | '„' | '‚' | '-' | '–' | '—' | '/' | '"'
    )
}

#[cfg(test)]
mod tests {
    use super::{fix, smarten_plain};

    fn fixed(src: &str) -> String {
        fix(src).unwrap_or_else(|| src.to_string())
    }

    #[test]
    fn after_brackets_and_punctuation() {
        assert_eq!(fixed("The (USA)'s view."), "The (USA)’s view.");
        assert_eq!(fixed("the [sic]'s point"), "the [sic]’s point");
        assert_eq!(fixed("the U.S.'s view"), "the U.S.’s view");
        assert_eq!(fixed("the 1990's"), "the 1990’s");
        assert_eq!(fixed("the U.S.' policy"), "the U.S.’ policy");
    }

    #[test]
    fn after_a_closing_quote() {
        assert_eq!(fixed("\"Hamlet\"'s ending"), "\"Hamlet\"’s ending");
        assert_eq!(fixed("he said \"'tis\""), "he said \"’tis\"");
    }

    #[test]
    fn after_citations_and_calls() {
        assert_eq!(fixed("@smith2020's claim"), "@smith2020’s claim");
        assert_eq!(fixed("@smith's claim"), "@smith’s claim");
        assert_eq!(fixed("#footnote[x]'s"), "#footnote[x]’s");
        assert_eq!(fixed("#cite(<a>)'s"), "#cite(<a>)’s");
        assert_eq!(fixed("*NATO*'s"), "*NATO*’s");
    }

    #[test]
    fn between_letters() {
        assert_eq!(fixed("don't stop"), "don’t stop");
        assert_eq!(fixed("'Kant's Werk'"), "'Kant’s Werk'");
    }

    #[test]
    fn leading_apostrophes() {
        assert_eq!(fixed("'tis the season"), "’tis the season");
        assert_eq!(fixed("'Twas night"), "’Twas night");
        assert_eq!(fixed("the '90s and '22."), "the ’90s and ’22.");
        assert_eq!(fixed("rock 'n' roll"), "rock ’n' roll");
        assert_eq!(fixed("let 'em go"), "let ’em go");
        assert_eq!(fixed("('tis)"), "(’tis)");
    }

    #[test]
    fn leaves_real_quotes_and_code_alone() {
        for src in [
            "'quoted' text",
            "('quoted')",
            "a 'b' c",
            "(see above) 'quoted' text",
            "the (USA) 'quoted'",
            "'em' is a unit",
            "'He left the U.S.' Then",
            "the boys' toys",
            "5'11\" tall and 5' high",
            "the '999' code",
            "#let s = \"(a)'s\"",
            "#let path = \"Tom's.bib\"",
            "`(a)'s`",
            "```\n(a)'s\n```",
            "// (a)'s",
            "$f(x)'$",
            "#set smartquote(enabled: false)\n(USA)'s don't",
        ] {
            assert_eq!(fixed(src), src, "changed: {src}");
        }
    }

    #[test]
    fn title_page_strings() {
        assert_eq!(
            fixed("#let doc-title = \"Kant's 'Critique'\"\n#let doc-author = \"Pat O'Brien\""),
            "#let doc-title = \"Kant’s ‘Critique’\"\n#let doc-author = \"Pat O’Brien\""
        );
        assert_eq!(fixed("#let doc-x = \"Tom's\""), "#let doc-x = \"Tom's\"");
    }

    #[test]
    fn plain_text() {
        assert_eq!(
            smarten_plain("Kant's \"Critique\" of 'Pure' Reason in the '90s"),
            "Kant’s “Critique” of ‘Pure’ Reason in the ’90s"
        );
        assert_eq!(smarten_plain("(\"a\")"), "(“a”)");
    }

    #[test]
    fn keeps_character_offsets() {
        let src = "= T\n\n#let doc-title = \"A's\"\nThe (USA)'s view and [sic]'s.\n'tis don't.";
        let out = fixed(src);
        assert_ne!(src, out);
        assert_eq!(src.chars().count(), out.chars().count());
        assert_eq!(src.lines().count(), out.lines().count());
    }

    fn compiled_text(src: &str, bib: Option<&str>) -> String {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.typ");
        std::fs::write(&path, src).unwrap();
        if let Some(bib) = bib {
            std::fs::write(dir.path().join("refs.bib"), bib).unwrap();
        }
        let (_, _, sync) = crate::compiler::compile_to_rgba_pages(
            &path,
            0.1,
            &Default::default(),
            &Default::default(),
            None,
        )
        .unwrap();
        let layer = sync.text_layer();
        let (a, b) = layer.everything().unwrap();
        layer.text_between(a, b)
    }

    #[test]
    fn the_preview_shows_apostrophes() {
        let text = compiled_text(
            "The (USA)'s view.\n\nThe [sic]'s point.\n\nThe U.S.'s aim.\n\nThe 1990's.\n\n\"Hamlet\"'s end.\n\n'Quoted' and (see 'this').\n\n'tis the '90s, rock 'n' roll.\n\n#text(lang: \"de\")['Kant's Werk' hier]\n\n#let doc-title = \"Kant's Book\"\n#doc-title",
            None,
        );
        for want in [
            "(USA)’s",
            "[sic]’s",
            "U.S.’s",
            "1990’s",
            "”’s",
            "‘Quoted’",
            "(see ‘this’)",
            "’tis the ’90s, rock ’n’ roll",
            "‚Kant’s Werk‘",
            "Kant’s Book",
        ] {
            assert!(text.contains(want), "missing {want} in:\n{text}");
        }
        assert!(!text.contains('\''), "{text}");
    }

    const BIB: &str = "@book{doe2020, author = {O'Brien, Jane}, title = {Tom's Book: The \"Real\" Story}, year = {2020}, publisher = {P}}";

    #[test]
    fn a_citation_then_apostrophe() {
        let text = compiled_text(
            "@doe2020's claim.\n\n#bibliography(\"refs.bib\", style: \"apa\")",
            Some(BIB),
        );
        assert!(text.contains(")’s claim"), "{text}");
    }

    #[test]
    fn code_in_footnotes_keeps_straight_quotes() {
        let text = compiled_text("Text#footnote[Run `echo 'hi'` first.]", None);
        assert!(text.contains("echo 'hi'"), "{text}");
    }

    #[test]
    fn bibliography_entries_get_curly_quotes() {
        for style in ["apa", "chicago-notes"] {
            let text = compiled_text(
                &format!("Text @doe2020.\n\n#bibliography(\"refs.bib\", style: \"{style}\")"),
                Some(BIB),
            );
            assert!(text.contains("O’Brien"), "{style}:\n{text}");
            assert!(text.contains("Tom’s Book"), "{style}:\n{text}");
            assert!(text.contains("“Real”"), "{style}:\n{text}");
            assert!(!text.contains('\''), "{style}:\n{text}");
            assert!(!text.contains('"'), "{style}:\n{text}");
        }
    }
}
