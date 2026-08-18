/// (name, typst_code, bib_style_key, bib_title, zerkalo_style_key)
///
/// `bib_style_key`      — Typst built-in style name for `#bibliography(style: ...)`.
/// `bib_title`          — Override title for the bibliography section; empty string
///                        uses Typst's default ("Bibliography").
/// `zerkalo_style_key`  — The key stored in `// @zerkalo-style:` in template documents.
pub const STYLES: &[(&str, &str, &str, &str, &str)] = &[
    ("Default", "", "", "", ""),
    (
        "SBL",
        // SBL HS 2nd ed. §4.1
        // H1: centred, bold, ALL CAPS
        // H2: centred, bold
        // H3: centred, italic
        // H4: flush left, bold
        // H5: flush left, bold italic
        // block(width: 100%) + set par(first-line-indent: 0pt) prevents the
        // paragraph indent from shifting centred headings off-axis.
        r#"#set text(size: 12pt, font: "Times New Roman", lang: "en")
#set par(leading: 1em, spacing: 1em, first-line-indent: 0.5in, justify: false)
#set page(margin: 1in, numbering: "1", number-align: top + right)
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#upper(it.body)]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(weight: "bold")[#it.body]]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.6em, below: 0.3em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#it.body]
]
#show heading.where(level: 4): it => block(width: 100%, above: 0.5em, below: 0.2em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold", style: "italic")[#it.body]
]
#show heading.where(level: 5): it => block(width: 100%, above: 0.4em, below: 0.1em)[
  #set par(first-line-indent: 0pt)
  #it.body
]"#,
        "chicago-notes",
        "",
        "sbl",
    ),
    (
        "Chicago (Notes-Bib)",
        r#"#set text(size: 12pt, font: "Times New Roman", lang: "en")
#set par(leading: 1em, spacing: 1em, first-line-indent: 0.5in, justify: true)
#set page(margin: 1in, numbering: "1", number-align: top + right)
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(weight: "bold")[#it.body]]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(style: "italic")[#it.body]]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.6em, below: 0.2em)[
  #set par(first-line-indent: 0pt)
  #text(style: "italic")[#it.body]
]"#,
        "chicago-notes",
        "",
        "chicago-notes",
    ),
    (
        "Chicago (Author-Date)",
        r#"#set text(size: 12pt, font: "Times New Roman", lang: "en")
#set par(leading: 1em, spacing: 1em, first-line-indent: 0.5in, justify: true)
#set page(margin: 1in, numbering: "1", number-align: top + right)
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(weight: "bold")[#it.body]]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(style: "italic")[#it.body]]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.6em, below: 0.2em)[
  #set par(first-line-indent: 0pt)
  #text(style: "italic")[#it.body]
]"#,
        "chicago-author-date",
        "References",
        "chicago-author-date",
    ),
    (
        "MLA",
        r#"#set text(size: 12pt, font: "Times New Roman", lang: "en")
#set par(leading: 1em, spacing: 1em, first-line-indent: 0.5in, justify: false)
#set page(margin: 1in, numbering: "1", number-align: top + right)
#show heading: it => block(width: 100%, above: 0.6em, below: 0.3em)[
  #set par(first-line-indent: 0pt)
  #text(it.body)
]"#,
        "mla",
        "Works Cited",
        "mla",
    ),
    (
        "APA 7th",
        // APA 7 §2.27: H1 centred bold; H2 flush-left bold; H3 flush-left bold italic;
        // H4 indented bold (run-in); H5 indented bold italic (run-in)
        r#"#set text(size: 12pt, font: "Times New Roman", lang: "en")
#set par(leading: 1em, spacing: 1em, first-line-indent: 0.5in, justify: false)
#set page(margin: 1in, numbering: "1", number-align: top + right)
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(weight: "bold")[#it.body]]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold")[#it.body]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.6em, below: 0.2em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold", style: "italic")[#it.body]
]"#,
        "apa",
        "References",
        "apa",
    ),
    (
        "ASA",
        r#"#set text(size: 12pt, font: "Times New Roman", lang: "en")
#set par(leading: 1em, spacing: 1em, first-line-indent: 0.5in, justify: false)
#set page(margin: 1in, numbering: "1", number-align: top + right)
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #upper(it.body)
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #text(style: "italic")[#it.body]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.4em, below: 0em)[
  #set par(first-line-indent: 0pt)
  #h(0.5in)#text(style: "italic")[#(it.body + [.])]
]"#,
        "apa",
        "References",
        "asa",
    ),
    (
        "Turabian",
        r#"#set text(size: 12pt, font: "Times New Roman", lang: "en")
#set par(leading: 1em, spacing: 1em, first-line-indent: 0.5in, justify: true)
#set page(margin: 1in, numbering: "1", number-align: top + right)
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(weight: "bold")[#it.body]]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#it.body]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.6em, below: 0.2em)[
  #set par(first-line-indent: 0pt)
  #text(style: "italic")[#it.body]
]"#,
        "chicago-notes",
        "",
        "turabian",
    ),
    (
        "IEEE",
        // IEEE conference paper: 10 pt, two-column, numbered headings I./A./1.
        r#"#set text(size: 10pt, font: "Times New Roman", lang: "en")
#set par(leading: 0.65em, first-line-indent: 0.15in, justify: true)
#set page(margin: 0.75in, columns: 2, numbering: "1", number-align: bottom + center)
#set heading(numbering: "I.A.1.")
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(weight: "bold")[#upper(it.body)]]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold", style: "italic")[#it.body]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.4em, below: 0em)[
  #set par(first-line-indent: 0pt)
  #text(style: "italic")[#it.body]
]"#,
        "ieee",
        "References",
        "ieee",
    ),
    (
        "GOST R 7.0-5 (numeric)",
        // GOST R 7.0-5: A4, 30 mm left / 15 mm right / 20 mm top-bottom,
        // 14 pt Times New Roman, 1.5 leading, numbered headings.
        r#"#set text(size: 14pt, font: "Times New Roman", lang: "en")
#set par(leading: 0.85em, first-line-indent: 12.5mm, justify: true)
#set page(
  paper: "a4",
  margin: (left: 30mm, right: 15mm, top: 20mm, bottom: 20mm),
  numbering: "1",
  number-align: bottom + center,
)
#set heading(numbering: "1.")
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(weight: "bold")[#upper(it.body)]]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold")[#it.body]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.6em, below: 0.2em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold", style: "italic")[#it.body]
]"#,
        "gost-r-705-2008-numeric",
        "",
        "gost-r-705",
    ),
    (
        "Vancouver",
        // Vancouver (ICMJE): 12 pt, US Letter, numbered headings, references numbered.
        r#"#set text(size: 12pt, font: "Times New Roman", lang: "en")
#set par(leading: 0.65em, first-line-indent: 0pt, justify: false)
#set page(margin: 1in, numbering: "1", number-align: bottom + center)
#set heading(numbering: "1.")
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold")[#it.body]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold", style: "italic")[#it.body]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.4em, below: 0em)[
  #set par(first-line-indent: 0pt)
  #text(style: "italic")[#it.body]
]"#,
        "vancouver",
        "References",
        "vancouver",
    ),
    (
        "Harvard",
        r#"#set text(size: 12pt, font: "Times New Roman", lang: "en")
#set par(leading: 1em, spacing: 1em, first-line-indent: 0.5in, justify: true)
#set page(margin: 1in, numbering: "1", number-align: top + right)
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(weight: "bold")[#it.body]]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold")[#it.body]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.6em, below: 0.2em)[
  #set par(first-line-indent: 0pt)
  #text(weight: "bold", style: "italic")[#it.body]
]"#,
        "chicago-author-date",
        "References",
        "harvard",
    ),
    ("Custom (CSL file)", "", "custom", "", "custom"),
];

/// Placeholder `bib_style_key` used by the "Custom" style entry above. The UI
/// layer resolves this to the user's configured `custom_csl_path` before it
/// reaches `apply_to`/`update_bibliography_only`.
pub const CUSTOM_STYLE_PLACEHOLDER: &str = "custom";

const STYLE_BEGIN: &str = "// ZERKALO-STYLE-BEGIN";
const STYLE_END: &str = "// ZERKALO-STYLE-END";

/// Returns true when the document uses the Zerkalo template system (has TEMPLATE markers).
/// For those documents, heading styles are owned by the template block; the legacy STYLE
/// block must not be injected on top of them.
pub fn has_template_block(content: &str) -> bool {
    content.contains("// ZERKALO-TEMPLATE-BEGIN") && content.contains("// ZERKALO-TEMPLATE-END")
}

/// Insert or replace the style block, then update/append the bibliography call.
/// For template documents (those with ZERKALO-TEMPLATE markers), this is a no-op —
/// call `editor_pane::apply_style` instead which routes to the template-aware path.
pub fn apply_to(content: &str, style_code: &str, bib_style: &str, bib_title: &str) -> String {
    // Template documents own their formatting via the TEMPLATE block.
    // Adding a STYLE block on top would cause duplicate #show heading rules.
    if has_template_block(content) {
        return content.to_string();
    }
    let new_block = if style_code.is_empty() {
        String::new()
    } else {
        format!("{STYLE_BEGIN}\n{style_code}\n{STYLE_END}\n")
    };

    let after_style = if let (Some(begin_pos), Some(end_marker_pos)) =
        (content.find(STYLE_BEGIN), content.find(STYLE_END))
    {
        let end_pos = end_marker_pos + STYLE_END.len();
        let after = if content[end_pos..].starts_with('\n') {
            end_pos + 1
        } else {
            end_pos
        };
        let before = &content[..begin_pos];
        let rest = &content[after..];
        if new_block.is_empty() {
            format!("{before}{rest}")
        } else {
            format!("{before}{new_block}{rest}")
        }
    } else if new_block.is_empty() {
        content.to_string()
    } else {
        format!("{new_block}\n{content}")
    };

    update_bibliography(&after_style, bib_style, bib_title)
}

/// Find any `#bibliography(...)` line (commented or live) and update its
/// `style:` and `title:` arguments. If none found, append a commented hint.
pub fn update_bibliography_only(content: &str, bib_style: &str, bib_title: &str) -> String {
    update_bibliography(content, bib_style, bib_title)
}

fn update_bibliography(content: &str, bib_style: &str, bib_title: &str) -> String {
    if bib_style.is_empty() {
        return content.to_string();
    }

    let trailing_nl = content.ends_with('\n');
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut found = false;

    for line in &mut lines {
        let trimmed = line.trim();
        let is_comment = trimmed.starts_with("//");
        let core = if is_comment {
            trimmed.trim_start_matches('/').trim()
        } else {
            trimmed
        };

        if core.starts_with("#bibliography(") {
            found = true;
            if let Some(filename) = extract_bib_filename(core) {
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                let comment_prefix = if is_comment { "// " } else { "" };
                *line = format!(
                    "{indent}{comment_prefix}{}",
                    build_bib_call(filename, bib_style, bib_title)
                );
            }
        }
    }

    if !found {
        lines.push(format!(
            "// {}",
            build_bib_call("refs.bib", bib_style, bib_title)
        ));
    }

    let mut result = lines.join("\n");
    if trailing_nl {
        result.push('\n');
    }
    result
}

fn build_bib_call(filename: &str, style: &str, title: &str) -> String {
    let style = style.replace('\\', "\\\\").replace('"', "\\\"");
    let style = style.as_str();
    if title.is_empty() {
        format!("#bibliography(\"{filename}\", style: \"{style}\")")
    } else {
        format!("#bibliography(\"{filename}\", style: \"{style}\", title: \"{title}\")")
    }
}

pub(crate) fn extract_bib_filename(s: &str) -> Option<&str> {
    let open = s.find('(')?;
    let inner = &s[open + 1..];
    let q1 = inner.find('"')? + 1;
    let q2 = inner[q1..].find('"')?;
    Some(&inner[q1..q1 + q2])
}

/// Finds a document's active (non-commented) `#bibliography(...)` call and
/// returns its path argument, if any. Used by the compiler to detect a
/// bibliography path that needs the sandbox root widened to reach — whether
/// or not that path is also reflected in `Config::bib_path`, which a hand-
/// edited or hand-typed `#bibliography(...)` line never is.
pub(crate) fn find_bibliography_path(content: &str) -> Option<&str> {
    content
        .lines()
        .map(|l| l.trim())
        .find(|l| l.starts_with("#bibliography("))
        .and_then(extract_bib_filename)
}

/// Rewrites a document's `#bibliography(...)` call to point `new_path`,
/// preserving `style:`/`title:` and everything else about the call — used
/// when the citation panel's "choose a bibliography file/vault" dialogs set
/// `Config::bib_path`, so the document the user is looking at actually picks
/// up the change instead of silently keeping the old (or no) source until
/// Update Template Settings → Apply is used by hand.
///
/// A commented-out line (`// #bibliography(...)`, written when a document
/// was templated with no bibliography configured yet) is uncommented too —
/// the user just told Zerkalo where their bibliography is, so leaving the
/// call inert would defeat the point. If no `#bibliography(...)` call
/// exists anywhere, a plain new one is appended.
pub fn set_bibliography_path(content: &str, new_path: &str) -> String {
    let trailing_nl = content.ends_with('\n');
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut found = false;

    for line in &mut lines {
        let trimmed = line.trim();
        let is_comment = trimmed.starts_with("//");
        let core = if is_comment {
            trimmed.trim_start_matches('/').trim()
        } else {
            trimmed
        };
        if core.starts_with("#bibliography(") {
            found = true;
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            *line = format!("{indent}{}", replace_bib_path_in_call(core, new_path));
        }
    }

    if !found {
        lines.push(replace_bib_path_in_call("#bibliography(\"\")", new_path));
    }

    let mut result = lines.join("\n");
    if trailing_nl {
        result.push('\n');
    }
    result
}

fn replace_bib_path_in_call(call: &str, new_path: &str) -> String {
    let escaped = new_path.replace('\\', "\\\\").replace('"', "\\\"");
    let Some(open) = call.find('(') else {
        return format!("#bibliography(\"{escaped}\")");
    };
    let Some(q1_rel) = call[open + 1..].find('"') else {
        return format!("#bibliography(\"{escaped}\")");
    };
    let q1 = open + 1 + q1_rel;
    let Some(q2_rel) = call[q1 + 1..].find('"') else {
        return format!("#bibliography(\"{escaped}\")");
    };
    let q2 = q1 + 1 + q2_rel;
    format!("{}\"{escaped}\"{}", &call[..q1], &call[q2 + 1..])
}

#[cfg(test)]
mod bib_path_tests {
    use super::*;

    #[test]
    fn set_bibliography_path_replaces_the_path_and_keeps_style() {
        let doc = "#bibliography(\"old.bib\", style: \"apa\")\n\n= Title\n";
        let out = set_bibliography_path(doc, "/home/user/new.bib");
        assert!(
            out.contains("#bibliography(\"/home/user/new.bib\", style: \"apa\")"),
            "got: {out}"
        );
        assert!(
            out.contains("= Title"),
            "rest of the document must survive: {out}"
        );
    }

    #[test]
    fn set_bibliography_path_uncomments_a_commented_out_call() {
        let doc = "// #bibliography(\"refs.bib\", style: \"chicago-author-date\")\n\n= Title\n";
        let out = set_bibliography_path(doc, "/home/user/refs.bib");
        assert!(
            out.contains("#bibliography(\"/home/user/refs.bib\", style: \"chicago-author-date\")"),
            "got: {out}"
        );
        assert!(
            !out.contains("// #bibliography"),
            "should be uncommented: {out}"
        );
    }

    #[test]
    fn set_bibliography_path_appends_a_call_when_none_exists() {
        let doc = "= Title\n\nSome prose.\n";
        let out = set_bibliography_path(doc, "/home/user/refs.bib");
        assert!(
            out.contains("#bibliography(\"/home/user/refs.bib\")"),
            "got: {out}"
        );
        assert!(
            out.contains("= Title"),
            "existing content must survive: {out}"
        );
    }

    #[test]
    fn set_bibliography_path_escapes_a_backslash_or_quote_in_the_path() {
        let doc = "#bibliography(\"old.bib\")\n";
        let out = set_bibliography_path(doc, "C:\\refs\\a\"b.bib");
        assert!(out.contains("C:\\\\refs\\\\a\\\"b.bib"), "got: {out}");
    }
}
