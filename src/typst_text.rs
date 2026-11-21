//! Putting arbitrary text inside a Typst string literal or as plain markup.
//!
//! Anything built into generated Typst source — a link from an imported file, a
//! path, a title — goes through here, so a stray backslash or quote can't end
//! the string early and turn the rest of the line into code.

/// `s` as the inside of a `"…"` Typst string literal.
pub(crate) fn string_body(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parses_as_one_string(body: &str) -> bool {
        let src = format!("#f(\"{}\")", string_body(body));
        let root = typst::syntax::parse(&src);
        root.errors_and_warnings().0.is_empty()
            && root
                .children()
                .any(|c| c.kind() == typst::syntax::SyntaxKind::FuncCall)
    }

    #[test]
    fn hostile_text_stays_inside_the_string() {
        for body in [
            "a\\",
            "a\"b",
            "a\\\")[#x",
            "line\nbreak",
            "tab\there",
            "\u{7}bell",
            "plain",
        ] {
            assert!(parses_as_one_string(body), "{body:?}");
        }
    }

    #[test]
    fn the_escape_round_trips_through_typst() {
        assert_eq!(string_body("a\\b\"c"), "a\\\\b\\\"c");
        assert_eq!(string_body("x\ny"), "x\\ny");
    }
}
