//! Apostrophes Typst would set as opening quotes.
//!
//! Typst picks a smart quote's shape from the character just before it in the
//! laid-out text: after a letter a `'` is an apostrophe, but after `)`, `]`, a
//! full stop, a closing quote or a citation like "(Smith 2020)" it opens a new
//! quotation (‘), and after a digit it becomes a prime (′). So `(USA)'s`,
//! `[sic]'s`, `U.S.'s` and `1990's` came out with a backwards or straight mark.
//!
//! A `'` with a letter right after it and something other than a letter, space
//! or opening mark right before it is always an apostrophe in practice, so those
//! are handed to the compiler as a literal ’. One character replaces one, so
//! lines and character offsets — everything the preview sync and error
//! locations use — stay exactly where they were.

use typst::syntax::{LinkedNode, SyntaxKind};

const APOSTROPHE: &str = "\u{2019}";

/// `text` with those apostrophes made literal, or `None` when nothing changes.
pub fn fix(text: &str) -> Option<String> {
    if !has_candidate(text) || text.contains("smartquote") {
        return None;
    }
    let root = typst::syntax::parse(text);
    let mut at = Vec::new();
    collect(&LinkedNode::new(&root), &mut at);
    if at.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(text.len() + at.len() * 2);
    let mut last = 0;
    for offset in at {
        out.push_str(&text[last..offset]);
        out.push_str(APOSTROPHE);
        last = offset + 1;
    }
    out.push_str(&text[last..]);
    Some(out)
}

/// A cheap pass over the raw text so ordinary documents skip the parse: some
/// `'` with a letter after it and, before it, neither a space nor a letter —
/// unless that letter ends an `@citation`.
fn has_candidate(text: &str) -> bool {
    text.match_indices('\'').any(|(i, _)| {
        let after = text[i + 1..].chars().next();
        if !after.is_some_and(char::is_alphabetic) {
            return false;
        }
        match text[..i].chars().next_back() {
            None => false,
            Some(c) if c.is_whitespace() => false,
            Some(c) if c.is_alphabetic() => {
                let word = text[..i].trim_end_matches(|c: char| {
                    c.is_alphanumeric() || matches!(c, '_' | '-' | ':' | '.')
                });
                word.ends_with('@')
            }
            Some(_) => true,
        }
    })
}

fn collect(node: &LinkedNode, at: &mut Vec<usize>) {
    if node.kind() == SyntaxKind::SmartQuote && node.leaf_text() == "'" && is_apostrophe(node) {
        at.push(node.offset());
    }
    for child in node.children() {
        collect(&child, at);
    }
}

fn is_apostrophe(quote: &LinkedNode) -> bool {
    let next_is_letter = neighbour(quote, 1)
        .filter(|n| n.kind() == SyntaxKind::Text)
        .and_then(|n| n.leaf_text().chars().next())
        .is_some_and(char::is_alphabetic);
    next_is_letter && neighbour(quote, -1).is_some_and(|p| ends_a_word(&p))
}

/// The sibling right beside `node`, spaces and comments included — unlike
/// `LinkedNode::prev_sibling`, which skips them and would make `(a) 'b'`
/// look like `(a)'b'`.
fn neighbour<'a>(node: &LinkedNode<'a>, step: isize) -> Option<LinkedNode<'a>> {
    let index = node.index().checked_add_signed(step)?;
    node.parent()?.children().nth(index)
}

/// Whether what comes before a `'` ends a word, so the `'` can't open a quote.
fn ends_a_word(prev: &LinkedNode) -> bool {
    match prev.kind() {
        SyntaxKind::Text => prev.leaf_text().chars().last().is_some_and(|c| {
            // Letters Typst already handles itself.
            !c.is_alphabetic() && !c.is_whitespace() && !opens(c)
        }),
        // `"Hamlet"'s`: only when that `"` closed a quotation.
        SyntaxKind::SmartQuote => {
            prev.leaf_text() == "\""
                && neighbour(prev, -1).is_some_and(|p| match p.kind() {
                    SyntaxKind::Text => p
                        .leaf_text()
                        .chars()
                        .last()
                        .is_some_and(|c| !c.is_whitespace() && !opens(c)),
                    SyntaxKind::Space | SyntaxKind::Parbreak | SyntaxKind::Linebreak => false,
                    _ => true,
                })
        }
        // `@smith's`, `#footnote[…]'s`, `*bold*'s`, `` `code`'s ``: the text
        // these produce can end in a bracket or a digit.
        SyntaxKind::Ref
        | SyntaxKind::Strong
        | SyntaxKind::Emph
        | SyntaxKind::Raw
        | SyntaxKind::Link
        | SyntaxKind::Equation => true,
        _ => is_embedded_code(prev),
    }
}

fn is_embedded_code(node: &LinkedNode) -> bool {
    neighbour(node, -1).is_some_and(|n| n.kind() == SyntaxKind::Hash)
}

fn opens(c: char) -> bool {
    matches!(
        c,
        '(' | '[' | '{' | '“' | '‘' | '«' | '‹' | '„' | '‚' | '-' | '–' | '—' | '/'
    )
}

#[cfg(test)]
mod tests {
    use super::fix;

    fn fixed(src: &str) -> String {
        fix(src).unwrap_or_else(|| src.to_string())
    }

    #[test]
    fn after_brackets_and_punctuation() {
        assert_eq!(fixed("The (USA)'s view."), "The (USA)’s view.");
        assert_eq!(fixed("the [sic]'s point"), "the [sic]’s point");
        assert_eq!(fixed("the U.S.'s view"), "the U.S.’s view");
        assert_eq!(fixed("the 1990's"), "the 1990’s");
    }

    #[test]
    fn after_a_closing_quote() {
        assert_eq!(fixed("\"Hamlet\"'s ending"), "\"Hamlet\"’s ending");
        assert_eq!(fixed("he said \"'tis\""), "he said \"'tis\"");
    }

    #[test]
    fn after_citations_and_calls() {
        assert_eq!(fixed("@smith2020's claim"), "@smith2020’s claim");
        assert_eq!(fixed("#footnote[x]'s"), "#footnote[x]’s");
        assert_eq!(fixed("#cite(<a>)'s"), "#cite(<a>)’s");
        assert_eq!(fixed("*NATO*'s"), "*NATO*’s");
    }

    #[test]
    fn leaves_real_quotes_and_code_alone() {
        for src in [
            "don't stop",
            "'quoted' text",
            "('quoted')",
            "a 'b' c",
            "(see above) 'quoted' text",
            "the (USA) 'quoted'",
            "5'11\" tall",
            "the (USA)' view",
            "#let s = \"(a)'s\"",
            "`(a)'s`",
            "```\n(a)'s\n```",
            "// (a)'s",
            "$f(x)'$",
            "#set smartquote(enabled: false)\n(USA)'s",
        ] {
            assert_eq!(fixed(src), src, "changed: {src}");
        }
    }

    #[test]
    fn keeps_character_offsets() {
        let src = "= T\n\nThe (USA)'s view and [sic]'s.\nNext line.";
        let out = fixed(src);
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
            "The (USA)'s view.\n\nThe [sic]'s point.\n\nThe U.S.'s aim.\n\nThe 1990's.\n\n\"Hamlet\"'s end.\n\n'Quoted' and (see 'this').",
            None,
        );
        assert!(text.contains("(USA)’s"), "{text}");
        assert!(text.contains("[sic]’s"), "{text}");
        assert!(text.contains("U.S.’s"), "{text}");
        assert!(text.contains("1990’s"), "{text}");
        assert!(text.contains("”’s"), "{text}");
        assert!(text.contains("‘Quoted’"), "{text}");
        assert!(text.contains("(see ‘this’)"), "{text}");
        assert!(!text.contains('\''), "{text}");
    }

    #[test]
    fn a_citation_then_apostrophe() {
        let bib = "@book{doe2020, author = {Doe, Jane}, title = {A Book}, year = {2020}, publisher = {P}}";
        let text = compiled_text(
            "@doe2020's claim.\n\n#bibliography(\"refs.bib\", style: \"apa\")",
            Some(bib),
        );
        assert!(text.contains(")’s claim"), "{text}");
    }
}
