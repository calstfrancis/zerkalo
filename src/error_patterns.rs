/// Known Typst error patterns paired with actionable fix descriptions and
/// automated text transformations.  Each entry:
///   - `pattern`: substring to look for in the error message (case-insensitive)
///   - `description`: short human-readable explanation shown in the Fix popup
///   - `fix_fn`: optional function that takes (source_text, error_line 0-based)
///               and returns a patched version of the source

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
    // Extract the unknown variable name from the line if possible
    let lines: Vec<&str> = source.lines().collect();
    let error_line = lines.get(line_idx)?;
    // Try to find an identifier after # on the line
    let var_name = error_line
        .split_whitespace()
        .find(|t| t.starts_with('#'))
        .map(|t| t.trim_start_matches('#').trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_'))
        .filter(|n| !n.is_empty())
        .unwrap_or("variable");

    let insertion = format!("#let {var_name} = \"\"  // TODO: define {var_name}\n");
    let mut new_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    new_lines.insert(line_idx, insertion);
    Some(new_lines.join("\n"))
}

fn append_to_line(source: &str, line_idx: usize, suffix: &str) -> Option<String> {
    let mut lines: Vec<String> = source.lines().map(|l| l.to_string()).collect();
    let line = lines.get_mut(line_idx)?;
    line.push_str(suffix);
    Some(lines.join("\n"))
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
        for p in PATTERNS {
            if let Some(f) = p.fix_fn {
                let result = f("some line\nanother line", 0);
                assert!(result.is_some(), "fix for '{}' returned None on valid input", p.pattern);
            }
        }
    }
}
