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
#set par(leading: 1em, spacing: 2em, first-line-indent: 0.5in, justify: false)
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
#set par(leading: 1em, spacing: 2em, first-line-indent: 0.5in, justify: true)
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
#set par(leading: 1em, spacing: 2em, first-line-indent: 0.5in, justify: true)
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
#set par(leading: 1em, spacing: 2em, first-line-indent: 0.5in, justify: false)
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
#set par(leading: 1em, spacing: 2em, first-line-indent: 0.5in, justify: false)
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
#set par(leading: 1em, spacing: 2em, first-line-indent: 0.5in, justify: false)
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
#set par(leading: 1em, spacing: 2em, first-line-indent: 0.5in, justify: true)
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
#set par(leading: 1em, spacing: 2em, first-line-indent: 0.5in, justify: true)
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

/// The top-level arguments of a call whose text begins at (or before) its
/// opening `(`, as raw slices. Strings and nested brackets are skipped over,
/// so a comma inside either doesn't split an argument; the list ends at the
/// call's matching `)`, which may be on a later line.
fn call_arg_spans(call: &str) -> Vec<std::ops::Range<usize>> {
    let Some(open) = call.find('(') else {
        return Vec::new();
    };
    let b = call.as_bytes();
    let mut args = Vec::new();
    let (mut depth, mut start, mut i) = (0usize, open + 1, open + 1);
    while i < b.len() {
        match b[i] {
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                if depth == 0 {
                    args.push(start..i.min(call.len()));
                    return args;
                }
                depth -= 1;
            }
            b',' if depth == 0 => {
                args.push(start..i);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if start <= call.len() {
        args.push(start..call.len());
    }
    args
}

fn call_args(call: &str) -> Vec<&str> {
    call_arg_spans(call).into_iter().map(|r| &call[r]).collect()
}

/// Where the string literals inside `s` are, without their quotes.
fn string_literal_spans(s: &str) -> Vec<std::ops::Range<usize>> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'"' {
            let start = i + 1;
            i = start;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            if i < b.len() {
                out.push(start..i);
            }
        }
        i += 1;
    }
    out
}

fn string_literals(s: &str) -> Vec<&str> {
    string_literal_spans(s).into_iter().map(|r| &s[r]).collect()
}

/// Where the path string sits inside a `#bibliography(...)` call's text:
/// the first positional string. An array of files has no single path to
/// point at, so it gives `None`.
fn path_literal_span(call: &str) -> Option<std::ops::Range<usize>> {
    for arg in call_arg_spans(call) {
        let text = call[arg.clone()].trim_start();
        let lead = call[arg.clone()].len() - text.len();
        if text.starts_with('(') {
            return None;
        }
        if text.starts_with('"') {
            let first = string_literal_spans(&call[arg.start + lead..arg.end])
                .into_iter()
                .next()?;
            return Some(arg.start + lead + first.start..arg.start + lead + first.end);
        }
    }
    None
}

/// The bibliography path in a single `#bibliography(...)` call, if its path
/// is one plain string — `#bibliography(style: "apa", "refs.bib")` gives
/// `refs.bib`, not `apa`. An array of files gives `None`, so code that
/// rewrites the call never collapses several files into one.
pub(crate) fn extract_bib_filename(s: &str) -> Option<&str> {
    for arg in call_args(s) {
        let arg = arg.trim();
        if arg.starts_with('"') {
            return string_literals(arg).into_iter().next();
        }
        if arg.starts_with('(') {
            return None;
        }
    }
    None
}

/// Finds a document's active (non-commented) `#bibliography(...)` call — it
/// may span several lines — and returns its path argument, the first one if
/// an array of files is given. Used by the compiler to detect a bibliography
/// path that needs the sandbox root widened to reach — whether or not that
/// path is also reflected in `Config::bib_path`, which a hand-edited or
/// hand-typed `#bibliography(...)` line never is.
pub(crate) fn find_bibliography_path(content: &str) -> Option<&str> {
    let mut offset = 0;
    for line in content.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#bibliography(") {
            let call = &content[offset + (line.len() - trimmed.len())..];
            return call_args(call).into_iter().find_map(|arg| {
                let arg = arg.trim();
                (arg.starts_with('"') || arg.starts_with('('))
                    .then(|| string_literals(arg).into_iter().next())
                    .flatten()
            });
        }
        offset += line.len();
    }
    None
}

/// The `style: "..."` argument of a document's active `#bibliography(...)`
/// call, if it has one. The compiler uses it to tell whether a custom `.csl`
/// file is a footnote style (see `compiler::SPLIT_NOTE_CITES`).
pub(crate) fn find_bibliography_style(content: &str) -> Option<&str> {
    let line = content
        .lines()
        .map(|l| l.trim())
        .find(|l| l.starts_with("#bibliography("))?;
    let after = &line[line.find("style:")? + "style:".len()..];
    let q1 = after.find('"')? + 1;
    let q2 = after[q1..].find('"')?;
    Some(&after[q1..q1 + q2])
}

const COMBINE_MARKER: &str = "// zerkalo-citations: combined";

/// Whether the document asks for serial citations (`@a @b @c`) to follow its
/// citation style — for Chicago notes, one footnote listing every source,
/// separated by semicolons — instead of Zerkalo's one-footnote-per-source
/// default. Stored in the document itself so every compile path (preview,
/// export, print, the Library window) honours it.
pub fn combines_serial_citations(content: &str) -> bool {
    content.lines().any(|l| l.trim() == COMBINE_MARKER)
}

/// Adds or removes the marker `combines_serial_citations` looks for, placed
/// just above the `#bibliography(...)` line (or at the top without one).
pub fn set_combine_serial_citations(content: &str, combine: bool) -> String {
    let trailing_nl = content.ends_with('\n');
    let mut lines: Vec<&str> = content
        .lines()
        .filter(|l| l.trim() != COMBINE_MARKER)
        .collect();
    if combine {
        let at = lines
            .iter()
            .position(|l| {
                l.trim()
                    .trim_start_matches('/')
                    .trim()
                    .starts_with("#bibliography(")
            })
            .unwrap_or(0);
        lines.insert(at, COMBINE_MARKER);
    }
    let mut out = lines.join("\n");
    if trailing_nl {
        out.push('\n');
    }
    out
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
    let escaped = new_path.replace('\\', "\\\\").replace('"', "\\\"");
    let mut offset = 0;
    let mut commented: Option<(usize, usize)> = None;
    for line in content.split_inclusive('\n') {
        let lead = line.len() - line.trim_start().len();
        let trimmed = line[lead..].trim_end_matches(['\n', '\r']);
        if trimmed.starts_with("#bibliography(") {
            // A live call: swap just the path string. A call whose path is
            // an array, or that names none, is left exactly as written.
            let call = &content[offset + lead..];
            return match path_literal_span(call) {
                Some(span) => {
                    let at = offset + lead;
                    format!(
                        "{}{}{}",
                        &content[..at + span.start],
                        escaped,
                        &content[at + span.end..]
                    )
                }
                None => content.to_string(),
            };
        }
        if commented.is_none()
            && trimmed.starts_with("//")
            && trimmed
                .trim_start_matches('/')
                .trim_start()
                .starts_with("#bibliography(")
        {
            commented = Some((
                offset + lead,
                offset + line.trim_end_matches(['\n', '\r']).len(),
            ));
        }
        offset += line.len();
    }

    match commented {
        // A commented-out call (written when a document was templated with
        // no bibliography yet) is switched on: the person just said where
        // their bibliography is.
        Some((start, end)) => {
            let core = content[start..end].trim_start_matches('/').trim_start();
            let call = match path_literal_span(core) {
                Some(span) => format!("{}{}{}", &core[..span.start], escaped, &core[span.end..]),
                None => format!("#bibliography(\"{escaped}\")"),
            };
            format!("{}{}{}", &content[..start], call, &content[end..])
        }
        None => {
            let mut out = content.to_string();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&format!("#bibliography(\"{escaped}\")"));
            if content.ends_with('\n') {
                out.push('\n');
            }
            out
        }
    }
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

#[cfg(test)]
mod combine_marker_tests {
    use super::*;

    #[test]
    fn combine_marker_round_trips_beside_the_bibliography() {
        let doc = "= Title\n#bibliography(\"refs.bib\")\nText @a @b.\n";
        assert!(!combines_serial_citations(doc));
        let on = set_combine_serial_citations(doc, true);
        assert!(combines_serial_citations(&on));
        assert_eq!(
            on,
            "= Title\n// zerkalo-citations: combined\n#bibliography(\"refs.bib\")\nText @a @b.\n"
        );
        assert_eq!(set_combine_serial_citations(&on, true), on);
        assert_eq!(set_combine_serial_citations(&on, false), doc);
    }
}

#[cfg(test)]
mod bibliography_call_tests {
    use super::*;

    #[test]
    fn the_path_is_the_positional_string_not_the_first_string() {
        assert_eq!(
            find_bibliography_path("#bibliography(style: \"apa\", \"refs.bib\")"),
            Some("refs.bib")
        );
        assert_eq!(
            find_bibliography_path("#bibliography(title: \"References\", \"refs.bib\")"),
            Some("refs.bib")
        );
        assert_eq!(
            find_bibliography_path("#bibliography(\"refs.bib\", style: \"apa\")"),
            Some("refs.bib")
        );
    }

    #[test]
    fn a_call_split_over_lines_is_found() {
        let doc = "= T\n#bibliography(\n  title: \"Works\",\n  \"lib/refs.bib\",\n  style: \"chicago-notes\",\n)\n";
        assert_eq!(find_bibliography_path(doc), Some("lib/refs.bib"));
    }

    #[test]
    fn an_array_of_files_gives_the_first_and_is_never_rewritten() {
        let call = "#bibliography((\"a.bib\", \"b.bib\"), style: \"apa\")";
        assert_eq!(find_bibliography_path(call), Some("a.bib"));
        assert_eq!(extract_bib_filename(call), None);
        // update_bibliography leaves such a line exactly as the author wrote it.
        assert_eq!(update_bibliography_only(call, "mla", ""), call);
    }

    #[test]
    fn a_commented_call_is_not_active() {
        assert_eq!(
            find_bibliography_path("// #bibliography(\"x.bib\")\n"),
            None
        );
    }

    #[test]
    fn restyling_keeps_the_real_path_when_style_comes_first() {
        let out =
            update_bibliography_only("#bibliography(style: \"apa\", \"refs.bib\")\n", "mla", "");
        assert_eq!(out, "#bibliography(\"refs.bib\", style: \"mla\")\n");
    }

    #[test]
    fn choosing_a_source_swaps_only_the_path_string() {
        let doc = "#bibliography(style: \"apa\", \"old.bib\")\nText\n";
        assert_eq!(
            set_bibliography_path(doc, "new.bib"),
            "#bibliography(style: \"apa\", \"new.bib\")\nText\n"
        );
    }

    #[test]
    fn a_multi_line_call_keeps_its_shape() {
        let doc = "#bibliography(\n  \"old.bib\",\n  style: \"apa\",\n)\n";
        assert_eq!(
            set_bibliography_path(doc, "lib/new.bib"),
            "#bibliography(\n  \"lib/new.bib\",\n  style: \"apa\",\n)\n"
        );
    }

    #[test]
    fn a_call_naming_several_files_is_left_alone() {
        let doc = "#bibliography((\"a.bib\", \"b.bib\"))\n";
        assert_eq!(set_bibliography_path(doc, "c.bib"), doc);
    }

    #[test]
    fn a_second_bibliography_line_is_not_touched() {
        let doc = "#bibliography(\"a.bib\")\n= Appendix\n#bibliography(\"b.bib\")\n";
        assert_eq!(
            set_bibliography_path(doc, "c.bib"),
            "#bibliography(\"c.bib\")\n= Appendix\n#bibliography(\"b.bib\")\n"
        );
    }
}
