//! Document import that doesn't need pandoc.
//!
//! `.docx` and `.odt` are ZIP archives of XML and `.md` is text, so the three
//! formats people actually import most can be converted in-process. Only
//! LaTeX, EPUB, RTF and HTML still hand off to pandoc — which, in the flatpak,
//! means a tool installed outside the sandbox that most users won't have.
//!
//! Every reader produces the same small [`Block`]/[`Inline`] model and one
//! emitter turns that into Typst, so escaping and spacing are decided once
//! rather than three times.

pub mod docx;
pub mod markdown;
pub mod odt;

use std::path::Path;

/// An image lifted out of a source document, to be written next to the result.
#[derive(Debug, Clone, PartialEq)]
pub struct Media {
    /// Filename relative to the document's media folder.
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    Bold(Vec<Inline>),
    Italic(Vec<Inline>),
    Code(String),
    Link {
        href: String,
        body: Vec<Inline>,
    },
    /// A hard line break inside a paragraph.
    Break,
    /// Wraps inlines that came from a track-changes run (Word's
    /// `<w:ins>`/`<w:del>`, ODT's `<text:change>`). Renders exactly like
    /// untracked content — Typst has no track-changes markup, so the
    /// proposed text is simply inlined for the reader to see in context —
    /// the wrapping exists only so [`collect_tracked_changes`] can later
    /// flatten it into `Imported::tracked_changes` for the comments sidebar.
    Tracked {
        kind: crate::comments::SuggestionKind,
        body: Vec<Inline>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading {
        level: u8,
        body: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    List {
        ordered: bool,
        items: Vec<Vec<Block>>,
    },
    Quote(Vec<Block>),
    Code {
        lang: Option<String>,
        text: String,
    },
    Table {
        rows: Vec<Vec<Vec<Inline>>>,
    },
    Image {
        src: String,
        alt: String,
    },
    Rule,
}

/// What a reader produces: the document's blocks plus any embedded images.
#[derive(Debug, Default)]
pub struct Imported {
    pub blocks: Vec<Block>,
    pub media: Vec<Media>,
    /// Anything the reader understood but couldn't represent, surfaced in the
    /// preview so the conversion doesn't quietly lose things.
    pub notes: Vec<String>,
    /// Track-changes runs found while parsing, flattened to plain text, in
    /// document order — the import flow locates each one's exact text in the
    /// rendered Typst output to anchor a [`crate::comments::Suggestion`].
    /// Only DOCX populates this today; ODT/Markdown default to empty.
    pub tracked_changes: Vec<TrackedChange>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackedChange {
    pub kind: crate::comments::SuggestionKind,
    pub text: String,
}

/// Flattens every [`Inline::Tracked`] span found anywhere in `blocks`, in
/// document order, to plain text (markup like bold/italic stripped, since
/// the search this feeds is a substring match against escaped Typst output
/// where those characters render differently).
pub fn collect_tracked_changes(blocks: &[Block]) -> Vec<TrackedChange> {
    let mut out = Vec::new();
    collect_tracked_in_blocks(blocks, &mut out);
    out
}

fn collect_tracked_in_blocks(blocks: &[Block], out: &mut Vec<TrackedChange>) {
    for block in blocks {
        match block {
            Block::Heading { body, .. } | Block::Paragraph(body) => {
                collect_tracked_in_inlines(body, out)
            }
            Block::List { items, .. } => {
                for item in items {
                    collect_tracked_in_blocks(item, out);
                }
            }
            Block::Quote(inner) => collect_tracked_in_blocks(inner, out),
            Block::Table { rows } => {
                for row in rows {
                    for cell in row {
                        collect_tracked_in_inlines(cell, out);
                    }
                }
            }
            Block::Code { .. } | Block::Image { .. } | Block::Rule => {}
        }
    }
}

fn collect_tracked_in_inlines(inlines: &[Inline], out: &mut Vec<TrackedChange>) {
    for item in inlines {
        match item {
            Inline::Tracked { kind, body } => {
                let mut text = String::new();
                flatten_inline_plain(body, &mut text);
                if !text.is_empty() {
                    out.push(TrackedChange {
                        kind: kind.clone(),
                        text,
                    });
                }
                // Handles a tracked span nested inside another (rare, but
                // possible if a document has overlapping revisions) as a
                // second, separate entry.
                collect_tracked_in_inlines(body, out);
            }
            Inline::Bold(inner) | Inline::Italic(inner) => collect_tracked_in_inlines(inner, out),
            Inline::Link { body, .. } => collect_tracked_in_inlines(body, out),
            Inline::Text(_) | Inline::Code(_) | Inline::Break => {}
        }
    }
}

fn flatten_inline_plain(inlines: &[Inline], out: &mut String) {
    for item in inlines {
        match item {
            Inline::Text(t) | Inline::Code(t) => out.push_str(t),
            Inline::Bold(inner) | Inline::Italic(inner) => flatten_inline_plain(inner, out),
            Inline::Link { body, .. } => flatten_inline_plain(body, out),
            Inline::Tracked { body, .. } => flatten_inline_plain(body, out),
            Inline::Break => out.push('\n'),
        }
    }
}

/// Characters that start Typst markup and so must be escaped in body text.
///
/// Missing one doesn't produce a visible mistake — it produces a document that
/// fails to compile, or silently changes meaning (`*` turning a price into bold
/// text), which is the failure mode users can least diagnose.
///
/// `pub(crate)`: the import flow also needs it to find a tracked change's
/// plain text inside the escaped Typst output it produced.
pub(crate) fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '#' | '$' | '*' | '_' | '`' | '<' | '>' | '@' | '=' | '[' | ']' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

fn emit_inlines(items: &[Inline], out: &mut String) {
    for item in items {
        match item {
            Inline::Text(t) => out.push_str(&escape_text(t)),
            Inline::Bold(inner) => {
                out.push('*');
                emit_inlines(inner, out);
                out.push('*');
            }
            Inline::Italic(inner) => {
                out.push('_');
                emit_inlines(inner, out);
                out.push('_');
            }
            Inline::Code(t) => {
                // One longer than the longest run of backticks in the content,
                // so the delimiter can't appear inside it.
                let longest_run = t
                    .split(|c| c != '`')
                    .map(|run| run.len())
                    .max()
                    .unwrap_or(0);
                let ticks = "`".repeat(longest_run + 1);
                out.push_str(&ticks);
                out.push_str(t);
                out.push_str(&ticks);
            }
            Inline::Link { href, body } => {
                out.push_str(&format!("#link(\"{}\")[", href.replace('"', "%22")));
                emit_inlines(body, out);
                out.push(']');
            }
            Inline::Break => out.push_str(" \\\n"),
            Inline::Tracked { body, .. } => emit_inlines(body, out),
        }
    }
}

fn emit_blocks(blocks: &[Block], out: &mut String, depth: usize) {
    for block in blocks {
        match block {
            Block::Heading { level, body } => {
                out.push_str(&"=".repeat((*level).clamp(1, 6) as usize));
                out.push(' ');
                emit_inlines(body, out);
                out.push_str("\n\n");
            }
            Block::Paragraph(body) => {
                let mut text = String::new();
                emit_inlines(body, &mut text);
                if !text.trim().is_empty() {
                    out.push_str(text.trim_end());
                    out.push_str("\n\n");
                }
            }
            Block::List { ordered, items } => {
                for item in items {
                    let marker = if *ordered { "+" } else { "-" };
                    let indent = "  ".repeat(depth);
                    let mut inner = String::new();
                    // Rendered at the *same* depth: the continuation prefix
                    // below supplies this level's indentation, so passing
                    // depth + 1 here indented nested content twice.
                    emit_blocks(item, &mut inner, depth);
                    let inner = inner.trim_end().to_string();
                    // Continuation lines line up under the marker so nested
                    // content stays part of the same item. Blank lines are
                    // dropped — in Typst an empty line ends the list item, so
                    // keeping them would split one item into several lists.
                    let mut lines = inner.lines().filter(|l| !l.trim().is_empty());
                    if let Some(first) = lines.next() {
                        out.push_str(&format!("{indent}{marker} {first}\n"));
                        for line in lines {
                            out.push_str(&format!("{indent}  {line}\n"));
                        }
                    }
                }
                out.push('\n');
            }
            Block::Quote(inner) => {
                let mut text = String::new();
                emit_blocks(inner, &mut text, depth);
                out.push_str("#quote(block: true)[\n");
                for line in text.trim_end().lines() {
                    out.push_str(&format!("  {line}\n"));
                }
                out.push_str("]\n\n");
            }
            Block::Code { lang, text } => {
                let fence = "`".repeat(
                    text.lines()
                        .map(|l| l.matches("```").count())
                        .max()
                        .unwrap_or(0)
                        * 3
                        + 3,
                );
                out.push_str(&fence);
                if let Some(l) = lang {
                    out.push_str(l);
                }
                out.push('\n');
                out.push_str(text.trim_end_matches('\n'));
                out.push('\n');
                out.push_str(&fence);
                out.push_str("\n\n");
            }
            Block::Table { rows } => {
                let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
                out.push_str(&format!("#table(\n  columns: {cols},\n"));
                for row in rows {
                    out.push_str("  ");
                    for cell in row {
                        let mut text = String::new();
                        emit_inlines(cell, &mut text);
                        out.push_str(&format!("[{}], ", text.trim()));
                    }
                    // Pad short rows — Typst requires every cell position.
                    for _ in row.len()..cols {
                        out.push_str("[], ");
                    }
                    out.push('\n');
                }
                out.push_str(")\n\n");
            }
            Block::Image { src, alt } => {
                out.push_str(&format!(
                    "#figure(\n  image(\"{}\"),\n  caption: [{}],\n)\n\n",
                    src.replace('"', "%22"),
                    escape_text(alt),
                ));
            }
            Block::Rule => out.push_str("#line(length: 100%)\n\n"),
        }
    }
}

/// Renders an imported document as Typst body text (no template preamble —
/// the caller splices this into whatever template applies).
pub fn to_typst(doc: &Imported) -> String {
    let mut out = String::new();
    emit_blocks(&doc.blocks, &mut out, 0);
    while out.ends_with("\n\n\n") {
        out.pop();
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// True when this file can be converted in-process, with no pandoc.
pub fn handles(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("docx") | Some("odt") | Some("md") | Some("markdown")
    )
}

/// Converts `path` in-process. Returns `None` when the format isn't one of
/// ours, so the caller can fall back to pandoc.
pub fn import(path: &Path) -> Option<Result<Imported, String>> {
    let ext = path.extension().and_then(|e| e.to_str())?.to_lowercase();
    match ext.as_str() {
        "docx" => Some(docx::read(path)),
        "odt" => Some(odt::read(path)),
        "md" | "markdown" => Some(
            std::fs::read_to_string(path)
                .map_err(|e| format!("Couldn't read the file: {e}"))
                .map(|text| markdown::read(&text)),
        ),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn body_text_is_escaped_so_it_cannot_become_markup() {
        // Unescaped, a price list or an email address silently turns into
        // emphasis, a function call, or a document that won't compile at all.
        let doc = Imported {
            blocks: vec![Block::Paragraph(vec![Inline::Text(
                "Costs $5 *each*, mail a@b.com #now [ok]".into(),
            )])],
            ..Default::default()
        };
        let out = to_typst(&doc);
        assert!(out.contains("\\$5"), "got: {out}");
        assert!(out.contains("\\*each\\*"), "got: {out}");
        assert!(out.contains("a\\@b.com"), "got: {out}");
        assert!(out.contains("\\#now"), "got: {out}");
        assert!(out.contains("\\[ok\\]"), "got: {out}");
    }

    #[test]
    fn a_backslash_is_escaped_before_anything_else() {
        let doc = Imported {
            blocks: vec![Block::Paragraph(vec![Inline::Text("a\\b".into())])],
            ..Default::default()
        };
        assert!(to_typst(&doc).contains("a\\\\b"));
    }

    #[test]
    fn headings_clamp_to_typsts_range() {
        let doc = Imported {
            blocks: vec![
                Block::Heading {
                    level: 1,
                    body: vec![Inline::Text("One".into())],
                },
                Block::Heading {
                    level: 9,
                    body: vec![Inline::Text("Deep".into())],
                },
            ],
            ..Default::default()
        };
        let out = to_typst(&doc);
        assert!(out.contains("= One"));
        assert!(out.contains("====== Deep"), "got: {out}");
    }

    #[test]
    fn a_table_pads_short_rows_so_the_cell_count_matches() {
        // Typst needs every position filled; a ragged row otherwise shifts
        // every following cell into the wrong column.
        let doc = Imported {
            blocks: vec![Block::Table {
                rows: vec![
                    vec![
                        vec![Inline::Text("a".into())],
                        vec![Inline::Text("b".into())],
                    ],
                    vec![vec![Inline::Text("c".into())]],
                ],
            }],
            ..Default::default()
        };
        let out = to_typst(&doc);
        assert!(out.contains("columns: 2"), "got: {out}");
        assert_eq!(out.matches('[').count(), 4, "row should be padded: {out}");
    }

    #[test]
    fn inline_code_picks_a_fence_the_content_cannot_break() {
        let doc = Imported {
            blocks: vec![Block::Paragraph(vec![Inline::Code("a ` b".into())])],
            ..Default::default()
        };
        let out = to_typst(&doc);
        assert!(out.contains("``a ` b``"), "got: {out}");
    }

    #[test]
    fn inline_code_fence_clears_the_longest_run_of_backticks() {
        let doc = Imported {
            blocks: vec![Block::Paragraph(vec![Inline::Code("a `` b ` c".into())])],
            ..Default::default()
        };
        let out = to_typst(&doc);
        assert!(out.contains("```a `` b ` c```"), "got: {out}");
    }

    #[test]
    fn nested_lists_indent_under_their_parent() {
        let doc = Imported {
            blocks: vec![Block::List {
                ordered: false,
                items: vec![vec![
                    Block::Paragraph(vec![Inline::Text("outer".into())]),
                    Block::List {
                        ordered: true,
                        items: vec![vec![Block::Paragraph(vec![Inline::Text("inner".into())])]],
                    },
                ]],
            }],
            ..Default::default()
        };
        let out = to_typst(&doc);
        assert!(out.contains("- outer"), "got: {out}");
        assert!(
            out.contains("  + inner"),
            "nested item should indent: {out}"
        );
    }

    #[test]
    #[ignore = "manual: prints a full conversion for eyeballing"]
    fn show_a_full_markdown_conversion() {
        let md = [
            "# Report",
            "",
            "Intro with **bold**, *italic*, and a [link](https://x.co).",
            "",
            "## Findings",
            "",
            "- first item",
            "- second item",
            "  - nested",
            "",
            "1. step one",
            "2. step two",
            "",
            "| Name | Value |",
            "|---|---|",
            "| a | $5 |",
            "",
            "> A quotation.",
            "",
            "```rust",
            "let x = 1;",
            "```",
            "",
            "Costs $10 and mentions me@example.com.",
        ]
        .join("\n");
        println!("=== TYPST OUTPUT ===\n{}", to_typst(&markdown::read(&md)));
    }

    /// The conversion is only worth anything if Typst accepts the result, so
    /// this compiles it for real rather than asserting on the text.
    pub(crate) fn assert_compiles(body: &str) {
        let dir = std::env::temp_dir().join(format!(
            "zerkalo_import_compile_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("doc.typ");
        std::fs::write(&path, body).unwrap();
        let result = crate::compiler::compile_to_pdf_bytes(
            &path,
            &std::collections::HashMap::new(),
            &std::collections::HashMap::new(),
            None,
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_ok(),
            "converted document must compile:\n{body}\n\nerror: {:?}",
            result.err()
        );
    }

    #[test]
    fn a_converted_markdown_document_compiles() {
        let md = [
            "# Report",
            "",
            "Text with **bold**, *italic*, `code`, and a [link](https://x.co).",
            "",
            "- first item",
            "- second item",
            "  - nested",
            "",
            "1. step one",
            "2. step two",
            "",
            "| Name | Value |",
            "|---|---|",
            "| a | $5 |",
            "",
            "> A quotation.",
            "",
            "```rust",
            "let x = 1;",
            "```",
            "",
            "---",
            "",
            "Costs $10, mentions me@example.com, uses #hash and *stars*.",
        ]
        .join("\n");
        assert_compiles(&to_typst(&markdown::read(&md)));
    }

    #[test]
    fn text_that_looks_like_typst_markup_still_compiles() {
        // The escaping exists so a document full of these characters converts
        // into something Typst accepts rather than a syntax error.
        let md = r#"Prices: $5, $10. Email a@b.com. Code #let x = 1. Brackets [a] <b>.
Backslash \ and underscore _mid_word_ and asterisk 2*3=6."#;
        assert_compiles(&to_typst(&markdown::read(md)));
    }

    #[test]
    fn the_formats_we_handle_are_the_ones_that_skip_pandoc() {
        for good in ["a.docx", "a.odt", "a.md", "a.MARKDOWN"] {
            assert!(
                handles(Path::new(good)),
                "{good} should be handled natively"
            );
        }
        for pandoc in ["a.tex", "a.epub", "a.rtf", "a.html"] {
            assert!(
                !handles(Path::new(pandoc)),
                "{pandoc} should fall back to pandoc"
            );
        }
    }
}
