// Known Typst error patterns paired with actionable fix descriptions and
// automated text transformations. Each entry:
//   - `pattern`: substring to look for in the error message (case-insensitive)
//   - `description`: short human-readable explanation shown in the Fix popup
//   - `fix_fn`: optional function that takes (source_text, error_line 0-based)
//              and returns a patched version of the source

pub struct ErrorFix {
    pub pattern: &'static str,
    pub description: &'static str,
    pub fix_fn: Option<fn(&str, usize) -> Option<String>>,
}

pub static PATTERNS: &[ErrorFix] = &[
    ErrorFix {
        pattern: "expected closing brace",
        description: "Insert a closing `}` at the end of the error line",
        fix_fn: Some(fix_add_closing_brace),
    },
    ErrorFix {
        pattern: "expected closing bracket",
        description: "Insert a closing `]` at the end of the error line",
        fix_fn: Some(fix_add_closing_bracket),
    },
    ErrorFix {
        pattern: "expected closing paren",
        description: "Insert a closing `)` at the end of the error line",
        fix_fn: Some(fix_add_closing_paren),
    },
    ErrorFix {
        pattern: "unknown variable",
        description: "Add a `#let <name> = ...` definition above this line",
        fix_fn: Some(fix_add_let_binding),
    },
    ErrorFix {
        pattern: "file not found",
        description: "Check that the referenced file path is correct relative to the project root",
        fix_fn: None,
    },
    ErrorFix {
        pattern: "package not found",
        description: "Run ☰ → Browse Packages… to install the missing package",
        fix_fn: None,
    },
    ErrorFix {
        pattern: "cannot divide by zero",
        description: "A division by zero occurred — check the denominator expression",
        fix_fn: None,
    },
    ErrorFix {
        pattern: "unexpected end of file",
        description: "A block or expression is not closed — check for missing `}`, `]`, or `)`",
        fix_fn: Some(fix_unclosed_delimiters),
    },
    ErrorFix {
        pattern: "missing argument",
        description: "A required value wasn't passed to a function — check its parentheses for a missing value",
        fix_fn: None,
    },
    ErrorFix {
        pattern: "unexpected argument",
        description: "An extra or misspelled argument was passed to a function — check argument names and commas",
        fix_fn: None,
    },
    ErrorFix {
        pattern: ", found ",
        description: "A value has the wrong type here — try wrapping it in [brackets] for content or \"quotes\" for text",
        fix_fn: None,
    },
];

/// Return the first matching fix for `error_msg`.
pub fn match_fix(error_msg: &str) -> Option<&'static ErrorFix> {
    let lower = error_msg.to_lowercase();
    PATTERNS.iter().find(|p| lower.contains(p.pattern))
}

// ── Fix implementations ───────────────────────────────────────────────────────

fn fix_add_closing_brace(source: &str, line_idx: usize) -> Option<String> {
    append_to_line(source, line_idx, "}")
}

fn fix_add_closing_bracket(source: &str, line_idx: usize) -> Option<String> {
    append_to_line(source, line_idx, "]")
}

fn fix_add_closing_paren(source: &str, line_idx: usize) -> Option<String> {
    append_to_line(source, line_idx, ")")
}

fn fix_add_let_binding(source: &str, line_idx: usize) -> Option<String> {
    let nl = line_ending(source);
    // Extract the unknown variable name from the line if possible
    let lines: Vec<&str> = source.lines().collect();
    let error_line = lines.get(line_idx)?;
    // Try to find an identifier after # on the line
    let var_name = error_line
        .split_whitespace()
        .find(|t| t.starts_with('#'))
        .map(|t| {
            t.trim_start_matches('#')
                .trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_')
        })
        .filter(|n| !n.is_empty())
        .unwrap_or("variable");

    // No trailing newline here — new_lines.join(nl) below supplies it, so
    // adding one here would insert a doubled blank line above the binding.
    let insertion = format!("#let {var_name} = \"\"  // TODO: define {var_name}");
    let mut new_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    new_lines.insert(line_idx, insertion);
    Some(new_lines.join(nl))
}

/// Whole-document delimiter balance is the only reliable signal for "unexpected
/// end of file" — the missing closer can be anywhere above the reported line,
/// so (unlike the other fixes here) this ignores `_line_idx` and appends any
/// outstanding closers to the end of the document.
fn fix_unclosed_delimiters(source: &str, _line_idx: usize) -> Option<String> {
    let mut depth_brace = 0i32;
    let mut depth_paren = 0i32;
    let mut depth_bracket = 0i32;
    for ch in source.chars() {
        match ch {
            '{' => depth_brace += 1,
            '}' => depth_brace -= 1,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '[' => depth_bracket += 1,
            ']' => depth_bracket -= 1,
            _ => {}
        }
    }
    let mut suffix = String::new();
    for _ in 0..depth_brace.max(0) {
        suffix.push('}');
    }
    for _ in 0..depth_paren.max(0) {
        suffix.push(')');
    }
    for _ in 0..depth_bracket.max(0) {
        suffix.push(']');
    }
    if suffix.is_empty() {
        None
    } else {
        Some(format!("{source}{}{suffix}", line_ending(source)))
    }
}

fn append_to_line(source: &str, line_idx: usize, suffix: &str) -> Option<String> {
    let nl = line_ending(source);
    let mut lines: Vec<String> = source.lines().map(|l| l.to_string()).collect();
    let line = lines.get_mut(line_idx)?;
    line.push_str(suffix);
    Some(lines.join(nl))
}

/// Detects whether `source` uses CRLF or LF line endings, so fixes that
/// rebuild the document from `.lines()` (which strips either style) can
/// rejoin with the same style instead of silently normalizing the whole
/// file to LF the moment any quick-fix is applied.
fn line_ending(source: &str) -> &'static str {
    if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_fix_is_case_insensitive() {
        let fix = match_fix("Error: Expected Closing Brace").unwrap();
        assert_eq!(fix.pattern, "expected closing brace");
    }

    #[test]
    fn match_fix_returns_none_for_unknown_error() {
        assert!(match_fix("some completely unrelated error").is_none());
    }

    #[test]
    fn match_fix_finds_first_matching_pattern() {
        let fix = match_fix("error: unknown variable: foo").unwrap();
        assert_eq!(fix.pattern, "unknown variable");
    }

    #[test]
    fn fix_add_closing_brace_appends_to_correct_line() {
        let src = "#let x = {\nfoo\nbar";
        let fixed = fix_add_closing_brace(src, 2).unwrap();
        assert_eq!(fixed, "#let x = {\nfoo\nbar}");
    }

    #[test]
    fn fix_add_closing_bracket_out_of_range_returns_none() {
        let src = "one line only";
        assert!(fix_add_closing_bracket(src, 5).is_none());
    }

    #[test]
    fn fix_add_closing_paren_appends_paren() {
        let src = "#foo(1, 2";
        assert_eq!(fix_add_closing_paren(src, 0).unwrap(), "#foo(1, 2)");
    }

    #[test]
    fn fix_add_let_binding_extracts_var_name_from_hash_token() {
        let src = "line one\n#unknown-var + 1\nline three";
        let fixed = fix_add_let_binding(src, 1).unwrap();
        assert!(fixed.contains("#let unknown-var = \"\""), "got: {fixed}");
        // Inserted before the offending line, so the original line still follows it.
        assert!(fixed.contains("#unknown-var + 1"));
    }

    #[test]
    fn fix_add_let_binding_falls_back_to_variable_when_no_hash_token() {
        let src = "plain text with no hash token";
        let fixed = fix_add_let_binding(src, 0).unwrap();
        assert!(fixed.contains("#let variable = \"\""), "got: {fixed}");
    }

    #[test]
    fn all_patterns_with_fix_fn_actually_fix_something() {
        // Has an unclosed brace so `fix_unclosed_delimiters` also succeeds here;
        // the other fixes don't care about delimiter balance.
        for p in PATTERNS {
            if let Some(f) = p.fix_fn {
                let result = f("#let x = {\nsome line\nanother line", 0);
                assert!(
                    result.is_some(),
                    "fix for '{}' returned None on valid input",
                    p.pattern
                );
            }
        }
    }

    #[test]
    fn fix_unclosed_delimiters_appends_missing_closers() {
        let src = "#let x = {\nfoo(bar[baz";
        let fixed = fix_unclosed_delimiters(src, 0).unwrap();
        assert_eq!(fixed, "#let x = {\nfoo(bar[baz\n})]");
    }

    #[test]
    fn fix_unclosed_delimiters_returns_none_when_balanced() {
        assert!(fix_unclosed_delimiters("#let x = (1 + 2)", 0).is_none());
    }

    #[test]
    fn match_fix_finds_missing_argument() {
        let fix = match_fix("error: missing argument: caption").unwrap();
        assert_eq!(fix.pattern, "missing argument");
        assert!(fix.fix_fn.is_none());
    }

    #[test]
    fn match_fix_finds_unexpected_argument() {
        let fix = match_fix("error: unexpected argument").unwrap();
        assert_eq!(fix.pattern, "unexpected argument");
    }

    #[test]
    fn match_fix_finds_type_mismatch() {
        let fix = match_fix("error: expected content, found string").unwrap();
        assert_eq!(fix.pattern, ", found ");
    }

    #[test]
    fn fix_add_let_binding_does_not_insert_a_doubled_blank_line() {
        let src = "line one\n#unknown-var + 1\nline three";
        let fixed = fix_add_let_binding(src, 1).unwrap();
        assert!(
            !fixed.contains("\n\n"),
            "should not introduce a blank line: {fixed:?}"
        );
    }

    #[test]
    fn append_to_line_preserves_crlf_line_endings() {
        let src = "#let x = {\r\nfoo\r\nbar";
        let fixed = fix_add_closing_brace(src, 2).unwrap();
        assert_eq!(fixed, "#let x = {\r\nfoo\r\nbar}");
    }

    #[test]
    fn fix_add_let_binding_preserves_crlf_line_endings() {
        let src = "line one\r\n#unknown-var + 1\r\nline three";
        let fixed = fix_add_let_binding(src, 1).unwrap();
        assert!(fixed.contains("\r\n#let unknown-var"), "got: {fixed:?}");
        assert!(!fixed.contains("\n\n"), "no doubled blank line: {fixed:?}");
    }

    #[test]
    fn fix_unclosed_delimiters_preserves_crlf_line_endings() {
        let src = "#let x = {\r\nfoo(bar[baz";
        let fixed = fix_unclosed_delimiters(src, 0).unwrap();
        assert_eq!(fixed, "#let x = {\r\nfoo(bar[baz\r\n})]");
    }
}
