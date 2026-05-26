/// (name, typst_code, bib_style_key, bib_title)
///
/// `bib_style_key` — Typst built-in style name for `#bibliography(style: ...)`.
/// `bib_title`     — Override title for the bibliography section; empty string
///                   uses Typst's default ("Bibliography").
pub const STYLES: &[(&str, &str, &str, &str)] = &[
    ("Default", "", "", ""),
    (
        "SBL",
        r#"#set text(size: 12pt, font: "Times New Roman", lang: "en")
#set par(leading: 1em, first-line-indent: 0.5in, justify: false)
#set page(margin: 1in)
#show heading.where(level: 1): it => {
  v(1em)
  align(center, text(weight: "bold")[#upper(it.body)])
  v(0.5em)
}
#show heading.where(level: 2): it => {
  v(0.8em)
  align(center, text(style: "italic")[#it.body])
  v(0.4em)
}
#show heading.where(level: 3): it => {
  v(0.6em)
  text(weight: "bold", style: "italic")[#it.body]
  v(0.2em)
}"#,
        "chicago-notes",
        "",
    ),
    (
        "Chicago (Notes-Bib)",
        r#"#set text(size: 12pt, font: "Times New Roman", lang: "en")
#set par(leading: 1em, first-line-indent: 0.5in, justify: true)
#set page(margin: 1in)
#show heading.where(level: 1): it => {
  v(1em)
  align(center, text(weight: "bold")[#it.body])
  v(0.5em)
}
#show heading.where(level: 2): it => {
  v(0.8em)
  align(center, text(style: "italic")[#it.body])
  v(0.4em)
}
#show heading.where(level: 3): it => {
  v(0.6em)
  text(style: "italic")[#it.body]
  v(0.2em)
}"#,
        "chicago-notes",
        "",
    ),
    (
        "Chicago (Author-Date)",
        r#"#set text(size: 12pt, font: "Times New Roman", lang: "en")
#set par(leading: 1em, first-line-indent: 0.5in, justify: true)
#set page(margin: 1in)
#show heading.where(level: 1): it => {
  v(1em)
  align(center, text(weight: "bold")[#it.body])
  v(0.5em)
}
#show heading.where(level: 2): it => {
  v(0.8em)
  align(center, text(style: "italic")[#it.body])
  v(0.4em)
}
#show heading.where(level: 3): it => {
  v(0.6em)
  text(style: "italic")[#it.body]
  v(0.2em)
}"#,
        "chicago-author-date",
        "Reference List",
    ),
    (
        "MLA",
        r#"#set text(size: 12pt, font: "Times New Roman", lang: "en")
#set par(leading: 1em, first-line-indent: 0.5in, justify: false)
#set page(margin: 1in, numbering: "1")
#show heading: it => {
  v(0.6em)
  text(it.body)
  v(0.3em)
}"#,
        "mla",
        "Works Cited",
    ),
    (
        "APA 7th",
        r#"#set text(size: 12pt, font: "Times New Roman", lang: "en")
#set par(leading: 1em, first-line-indent: 0.5in, justify: false)
#set page(margin: 1in)
#set heading(numbering: none)
#show heading.where(level: 1): it => {
  v(1em)
  align(center, text(weight: "bold")[#it.body])
  v(0.5em)
}
#show heading.where(level: 2): it => {
  v(0.8em)
  text(weight: "bold")[#it.body]
  v(0.4em)
}
#show heading.where(level: 3): it => {
  v(0.6em)
  text(weight: "bold", style: "italic")[#it.body]
  v(0.2em)
}"#,
        "apa",
        "",
    ),
    (
        "ASA",
        r#"#set text(size: 12pt, font: "Times New Roman", lang: "en")
#set par(leading: 1em, first-line-indent: 0.5in, justify: false)
#set page(margin: 1in, numbering: "1")
#show heading.where(level: 1): it => {
  v(1em)
  align(center, text(weight: "bold")[#upper(it.body)])
  v(0.5em)
}
#show heading.where(level: 2): it => {
  v(0.8em)
  text(weight: "bold")[#it.body]
  v(0.4em)
}
#show heading.where(level: 3): it => {
  v(0.6em)
  text(weight: "bold", style: "italic")[#it.body]
  v(0.2em)
}"#,
        "apa",
        "",
    ),
    (
        "Turabian",
        r#"#set text(size: 12pt, font: "Times New Roman", lang: "en")
#set par(leading: 1em, first-line-indent: 0.5in, justify: true)
#set page(margin: 1in, numbering: "1")
#show heading.where(level: 1): it => {
  v(1em)
  align(center, text(weight: "bold")[#it.body])
  v(0.5em)
}
#show heading.where(level: 2): it => {
  v(0.8em)
  align(center, text(style: "italic")[#it.body])
  v(0.4em)
}
#show heading.where(level: 3): it => {
  v(0.6em)
  text(style: "italic")[#it.body]
  v(0.2em)
}"#,
        "chicago-notes",
        "",
    ),
    (
        "Harvard",
        r#"#set text(size: 12pt, font: "Times New Roman", lang: "en")
#set par(leading: 1em, first-line-indent: 0.5in, justify: true)
#set page(margin: 1in, numbering: "1")
#show heading.where(level: 1): it => {
  v(1em)
  align(center, text(weight: "bold")[#it.body])
  v(0.5em)
}
#show heading.where(level: 2): it => {
  v(0.8em)
  text(weight: "bold")[#it.body]
  v(0.4em)
}
#show heading.where(level: 3): it => {
  v(0.6em)
  text(weight: "bold", style: "italic")[#it.body]
  v(0.2em)
}"#,
        "chicago-author-date",
        "",
    ),
];

const STYLE_BEGIN: &str = "// ZERKALO-STYLE-BEGIN";
const STYLE_END: &str = "// ZERKALO-STYLE-END";

/// Insert or replace the style block, then update/append the bibliography call.
pub fn apply_to(content: &str, style_code: &str, bib_style: &str, bib_title: &str) -> String {
    let new_block = if style_code.is_empty() {
        String::new()
    } else {
        format!("{STYLE_BEGIN}\n{style_code}\n{STYLE_END}\n")
    };

    let after_style = if let (Some(begin_pos), Some(end_marker_pos)) = (
        content.find(STYLE_BEGIN),
        content.find(STYLE_END),
    ) {
        let end_pos = end_marker_pos + STYLE_END.len();
        let after = if content[end_pos..].starts_with('\n') { end_pos + 1 } else { end_pos };
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
        let core = if is_comment { trimmed.trim_start_matches('/').trim() } else { trimmed };

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
        lines.push(format!("// {}", build_bib_call("refs.bib", bib_style, bib_title)));
    }

    let mut result = lines.join("\n");
    if trailing_nl {
        result.push('\n');
    }
    result
}

fn build_bib_call(filename: &str, style: &str, title: &str) -> String {
    if title.is_empty() {
        format!("#bibliography(\"{filename}\", style: \"{style}\")")
    } else {
        format!("#bibliography(\"{filename}\", style: \"{style}\", title: \"{title}\")")
    }
}

fn extract_bib_filename(s: &str) -> Option<&str> {
    let open = s.find('(')?;
    let inner = &s[open + 1..];
    let q1 = inner.find('"')? + 1;
    let q2 = inner[q1..].find('"')?;
    Some(&inner[q1..q1 + q2])
}
