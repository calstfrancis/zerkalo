//! Markdown → the shared import model, via pulldown-cmark.
//!
//! Tables, footnotes and strikethrough are enabled because real documents use
//! them; anything the parser reports that we can't represent is recorded in
//! `notes` rather than dropped silently.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use super::{Block, Imported, Inline};

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Where the parser currently is. Markdown nests, so blocks and inlines are
/// built on a stack rather than assuming a flat document.
enum Frame {
    Paragraph(Vec<Inline>),
    Heading {
        level: u8,
        body: Vec<Inline>,
    },
    Bold(Vec<Inline>),
    Italic(Vec<Inline>),
    Link {
        href: String,
        body: Vec<Inline>,
    },
    List {
        ordered: bool,
        items: Vec<Vec<Block>>,
    },
    Item(Vec<Block>),
    Quote(Vec<Block>),
    Table {
        rows: Vec<Vec<Vec<Inline>>>,
    },
    Row(Vec<Vec<Inline>>),
    Cell(Vec<Inline>),
}

#[derive(Default)]
struct Builder {
    stack: Vec<Frame>,
    blocks: Vec<Block>,
    notes: Vec<String>,
}

impl Builder {
    /// Adds an inline to whatever is currently open, or opens a paragraph.
    fn push_inline(&mut self, inline: Inline) {
        match self.stack.last_mut() {
            Some(Frame::Paragraph(v))
            | Some(Frame::Heading { body: v, .. })
            | Some(Frame::Bold(v))
            | Some(Frame::Italic(v))
            | Some(Frame::Link { body: v, .. })
            | Some(Frame::Cell(v)) => v.push(inline),
            _ => self.stack.push(Frame::Paragraph(vec![inline])),
        }
    }

    /// Closes a paragraph opened implicitly by loose text.
    ///
    /// A "tight" markdown list emits its text straight inside the item with no
    /// paragraph events around it, so an implicit paragraph frame gets opened
    /// and never closed — and the End(Item) below would then pop *that* instead
    /// of the item, losing the entry.
    fn flush_open_paragraph(&mut self) {
        if !matches!(self.stack.last(), Some(Frame::Paragraph(_))) {
            return;
        }
        if let Some(Frame::Paragraph(v)) = self.stack.pop() {
            // Appended directly rather than through push_block, which calls
            // this — that would recurse.
            let target = match self.stack.last_mut() {
                Some(Frame::Item(x)) | Some(Frame::Quote(x)) => x,
                _ => &mut self.blocks,
            };
            target.push(Block::Paragraph(v));
        }
    }

    fn push_block(&mut self, block: Block) {
        // A block starting while an implicit paragraph is open — a nested list
        // inside a tight list item is the common case — must close that
        // paragraph first. Without this the nested block is added past the
        // paragraph frame and lands at document level, so a sub-list appeared
        // before its parent instead of indented under it.
        self.flush_open_paragraph();
        match self.stack.last_mut() {
            Some(Frame::Item(v)) | Some(Frame::Quote(v)) => v.push(block),
            _ => self.blocks.push(block),
        }
    }

    /// The block list the next block would be appended to.
    fn current_blocks_mut(&mut self) -> &mut Vec<Block> {
        match self.stack.last_mut() {
            Some(Frame::Item(v)) | Some(Frame::Quote(v)) => v,
            _ => &mut self.blocks,
        }
    }
}

pub fn read(text: &str) -> Imported {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);

    let mut b = Builder::default();
    // Text events between a code block's start and end are its content, not
    // paragraph text.
    let mut in_code = false;

    for event in Parser::new_ext(text, options) {
        match event {
            Event::Start(Tag::Paragraph) => b.stack.push(Frame::Paragraph(Vec::new())),
            Event::End(TagEnd::Paragraph) => {
                if let Some(Frame::Paragraph(v)) = b.stack.pop() {
                    b.push_block(Block::Paragraph(v));
                }
            }
            Event::Start(Tag::Heading { level, .. }) => b.stack.push(Frame::Heading {
                level: heading_level(level),
                body: Vec::new(),
            }),
            Event::End(TagEnd::Heading(_)) => {
                if let Some(Frame::Heading { level, body }) = b.stack.pop() {
                    b.push_block(Block::Heading { level, body });
                }
            }
            Event::Start(Tag::Strong) => b.stack.push(Frame::Bold(Vec::new())),
            Event::End(TagEnd::Strong) => {
                if let Some(Frame::Bold(v)) = b.stack.pop() {
                    b.push_inline(Inline::Bold(v));
                }
            }
            Event::Start(Tag::Emphasis) => b.stack.push(Frame::Italic(Vec::new())),
            Event::End(TagEnd::Emphasis) => {
                if let Some(Frame::Italic(v)) = b.stack.pop() {
                    b.push_inline(Inline::Italic(v));
                }
            }
            Event::Start(Tag::Link { dest_url, .. }) => b.stack.push(Frame::Link {
                href: dest_url.to_string(),
                body: Vec::new(),
            }),
            Event::End(TagEnd::Link) => {
                if let Some(Frame::Link { href, body }) = b.stack.pop() {
                    b.push_inline(Inline::Link { href, body });
                }
            }
            Event::Start(Tag::Image {
                dest_url, title, ..
            }) => {
                // Images are emitted whole rather than as an inline, since
                // Typst wants them as figures.
                b.push_block(Block::Image {
                    src: dest_url.to_string(),
                    alt: title.to_string(),
                });
                // Consume the alt-text events until the image closes.
                b.stack.push(Frame::Cell(Vec::new()));
            }
            Event::End(TagEnd::Image) => {
                b.stack.pop();
            }
            Event::Start(Tag::List(start)) => b.stack.push(Frame::List {
                ordered: start.is_some(),
                items: Vec::new(),
            }),
            Event::End(TagEnd::List(_)) => {
                if let Some(Frame::List { ordered, items }) = b.stack.pop() {
                    b.push_block(Block::List { ordered, items });
                }
            }
            Event::Start(Tag::Item) => b.stack.push(Frame::Item(Vec::new())),
            Event::End(TagEnd::Item) => {
                b.flush_open_paragraph();
                if let Some(Frame::Item(blocks)) = b.stack.pop() {
                    if let Some(Frame::List { items, .. }) = b.stack.last_mut() {
                        items.push(blocks);
                    }
                }
            }
            Event::Start(Tag::BlockQuote(_)) => b.stack.push(Frame::Quote(Vec::new())),
            Event::End(TagEnd::BlockQuote(_)) => {
                b.flush_open_paragraph();
                if let Some(Frame::Quote(v)) = b.stack.pop() {
                    b.push_block(Block::Quote(v));
                }
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(l) if !l.is_empty() => {
                        Some(l.to_string())
                    }
                    _ => None,
                };
                b.push_block(Block::Code {
                    lang,
                    text: String::new(),
                });
                in_code = true;
            }
            Event::End(TagEnd::CodeBlock) => in_code = false,
            Event::Start(Tag::Table(_)) => b.stack.push(Frame::Table { rows: Vec::new() }),
            Event::End(TagEnd::Table) => {
                if let Some(Frame::Table { rows }) = b.stack.pop() {
                    b.push_block(Block::Table { rows });
                }
            }
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => {
                b.stack.push(Frame::Row(Vec::new()))
            }
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                if let Some(Frame::Row(cells)) = b.stack.pop() {
                    if let Some(Frame::Table { rows }) = b.stack.last_mut() {
                        rows.push(cells);
                    }
                }
            }
            Event::Start(Tag::TableCell) => b.stack.push(Frame::Cell(Vec::new())),
            Event::End(TagEnd::TableCell) => {
                if let Some(Frame::Cell(v)) = b.stack.pop() {
                    if let Some(Frame::Row(cells)) = b.stack.last_mut() {
                        cells.push(v);
                    }
                }
            }
            Event::Text(t) => {
                // Text inside a fenced block belongs to the code block that was
                // just opened, wherever it sits — which may be inside a list
                // item, not only at document level.
                if in_code {
                    if let Some(Block::Code { text, .. }) = b.current_blocks_mut().last_mut() {
                        text.push_str(&t);
                        continue;
                    }
                }
                b.push_inline(Inline::Text(t.to_string()));
            }
            Event::Code(t) => b.push_inline(Inline::Code(t.to_string())),
            Event::SoftBreak => b.push_inline(Inline::Text(" ".into())),
            Event::HardBreak => b.push_inline(Inline::Break),
            Event::Rule => b.push_block(Block::Rule),
            Event::Html(_) | Event::InlineHtml(_) => {
                if !b.notes.iter().any(|n| n.contains("HTML")) {
                    b.notes.push(
                        "Raw HTML in the source was left out — Typst has no direct equivalent."
                            .into(),
                    );
                }
            }
            Event::FootnoteReference(name) => {
                b.push_inline(Inline::Text(format!("[{name}]")));
                if !b.notes.iter().any(|n| n.contains("Footnote")) {
                    b.notes.push(
                        "Footnotes were converted to plain markers — check them against the original."
                            .into(),
                    );
                }
            }
            _ => {}
        }
    }

    // Anything still open at the end (unterminated emphasis, a truncated file)
    // is flushed rather than discarded.
    while let Some(frame) = b.stack.pop() {
        match frame {
            Frame::Paragraph(v) | Frame::Heading { body: v, .. } if !v.is_empty() => {
                b.blocks.push(Block::Paragraph(v));
            }
            _ => {}
        }
    }

    Imported {
        blocks: b.blocks,
        media: Vec::new(),
        notes: b.notes,
        tracked_changes: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc_import::to_typst;

    fn convert(md: &str) -> String {
        to_typst(&read(md))
    }

    #[test]
    fn headings_and_paragraphs_convert() {
        let out = convert("# Title\n\nSome text.\n\n## Sub\n\nMore.\n");
        assert!(out.contains("= Title"), "got: {out}");
        assert!(out.contains("== Sub"), "got: {out}");
        assert!(out.contains("Some text."), "got: {out}");
    }

    #[test]
    fn emphasis_maps_to_typst_markup() {
        let out = convert("Some **bold** and *italic* text.\n");
        assert!(out.contains("*bold*"), "got: {out}");
        assert!(out.contains("_italic_"), "got: {out}");
    }

    #[test]
    fn a_list_becomes_typst_list_markers() {
        let out = convert("- one\n- two\n\n1. first\n2. second\n");
        assert!(out.contains("- one"), "got: {out}");
        assert!(out.contains("+ first"), "got: {out}");
    }

    #[test]
    fn a_link_becomes_a_link_call() {
        let out = convert("See [the docs](https://example.com/a).\n");
        assert!(
            out.contains("#link(\"https://example.com/a\")["),
            "got: {out}"
        );
        assert!(out.contains("the docs"), "got: {out}");
    }

    #[test]
    fn a_table_converts_with_its_columns() {
        let out = convert("| a | b |\n|---|---|\n| 1 | 2 |\n");
        assert!(out.contains("#table("), "got: {out}");
        assert!(out.contains("columns: 2"), "got: {out}");
    }

    #[test]
    fn literal_text_that_looks_like_markup_is_escaped() {
        // The whole point of routing through the shared emitter: a price or an
        // email in the source must not become Typst syntax.
        let out = convert("Email a@b.com, costs $5, use #tags.\n");
        assert!(out.contains("a\\@b.com"), "got: {out}");
        assert!(out.contains("\\$5"), "got: {out}");
        assert!(out.contains("\\#tags"), "got: {out}");
    }

    #[test]
    fn an_image_becomes_a_figure() {
        let out = convert("![](picture.png)\n");
        assert!(out.contains("image(\"picture.png\")"), "got: {out}");
    }

    #[test]
    fn raw_html_is_reported_rather_than_silently_dropped() {
        let doc = read("<div>hello</div>\n\nText.\n");
        assert!(
            doc.notes.iter().any(|n| n.contains("HTML")),
            "should note the omission: {:?}",
            doc.notes
        );
    }

    #[test]
    fn a_nested_list_stays_inside_its_parent_item() {
        // A tight list item holds its text without paragraph events, so the
        // nested list used to be appended past the implicit paragraph frame and
        // land at document level — rendering *before* the items it belongs to.
        let out = convert("- first\n- second\n  - nested\n");
        let first = out.find("- first").expect("first item");
        let nested = out.find("- nested").expect("nested item");
        assert!(nested > first, "nested list must follow its parent:\n{out}");
        assert!(
            out.contains("\n  - nested"),
            "should be indented one level:\n{out}"
        );
    }

    #[test]
    fn a_list_item_is_not_split_by_blank_lines() {
        // A blank line ends a list item in Typst, so continuation content has
        // to be emitted without one.
        let out = convert("- outer\n\n  - inner\n");
        let list_part: String = out
            .lines()
            .take_while(|l| !l.trim().is_empty() || l.contains('-'))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !list_part.contains("\n\n  -"),
            "blank line would end the item:\n{out}"
        );
    }

    #[test]
    fn a_code_block_inside_a_list_item_keeps_its_text() {
        let out = convert("- item\n\n  ```\n  code here\n  ```\n");
        assert!(out.contains("code here"), "code text lost:\n{out}");
    }

    #[test]
    fn an_empty_document_produces_no_output_rather_than_panicking() {
        assert_eq!(convert("").trim(), "");
    }

    #[test]
    fn unterminated_emphasis_does_not_lose_the_text() {
        let out = convert("This **never closes\n");
        assert!(out.contains("never closes"), "got: {out}");
    }
}
