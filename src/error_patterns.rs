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
