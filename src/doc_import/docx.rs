//! `.docx` → the shared import model.
//!
//! A .docx is a ZIP holding `word/document.xml`. Paragraphs are `<w:p>`, runs
//! of consistently-formatted text are `<w:r>`, and a paragraph's role (heading,
//! list item, quote) comes from the style it names in `<w:pStyle>` plus its
//! `<w:numPr>` numbering reference.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use crate::comments::SuggestionKind;

use super::{Block, Imported, Inline, Media};

/// Reads one entry out of a ZIP as bytes.
fn read_entry(
    zip: &mut zip::ZipArchive<std::fs::File>,
    name: &str,
) -> Option<Vec<u8>> {
    let mut file = zip.by_name(name).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// Maps a Word style id to a heading level, e.g. `Heading2` / `heading 2` → 2.
///
/// Word writes the style id without a space and the style *name* with one, and
/// localised builds vary further, so both spellings are accepted.
fn heading_level_from_style(style: &str) -> Option<u8> {
    let lower = style.to_lowercase().replace([' ', '-', '_'], "");
    let rest = lower.strip_prefix("heading")?;
    rest.parse::<u8>().ok().filter(|n| (1..=9).contains(n))
}

fn is_quote_style(style: &str) -> bool {
    let lower = style.to_lowercase().replace([' ', '-', '_'], "");
    lower == "quote" || lower == "intensequote" || lower == "blockquote"
}

/// Collects the inline runs of one `<w:p>`.
fn paragraph_inlines(p: roxmltree::Node, rels: &HashMap<String, String>) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::new();

    for node in p.descendants() {
        match node.tag_name().name() {
            "r" if !matches!(
                node.parent().map(|n| n.tag_name().name()),
                Some("hyperlink") | Some("ins") | Some("del")
            ) => {
                push_run(node, &mut out);
            }
            // <w:ins>/<w:del> wrap the runs of an inserted or deleted span.
            // Only their direct <w:r> children are gathered here — the same
            // runs are also visited by the "r" arm above as the outer
            // descendants() walk reaches them, but the guard there excludes
            // an ins/del parent, so this is the only place they're read.
            "ins" | "del" => {
                let kind = if node.tag_name().name() == "ins" {
                    SuggestionKind::Insertion
                } else {
                    SuggestionKind::Deletion
                };
                let mut body = Vec::new();
                for r in node.children().filter(|c| c.tag_name().name() == "r") {
                    push_run(r, &mut body);
                }
                if !body.is_empty() {
                    out.push(Inline::Tracked { kind, body });
                }
            }
            "hyperlink" => {
                let href = node
                    .attribute(("http://schemas.openxmlformats.org/officeDocument/2006/relationships", "id"))
                    .and_then(|id| rels.get(id))
                    .cloned();
                let mut body = Vec::new();
                for r in node.descendants().filter(|n| n.tag_name().name() == "r") {
                    push_run(r, &mut body);
                }
                if body.is_empty() {
                    continue;
                }
                match href {
                    Some(href) => out.push(Inline::Link { href, body }),
                    None => out.extend(body),
                }
            }
            "br" => out.push(Inline::Break),
            _ => {}
        }
    }
    out
}

/// Turns one `<w:r>` into inlines, honouring bold/italic on its run properties.
fn push_run(r: roxmltree::Node, out: &mut Vec<Inline>) {
    let props = r.children().find(|c| c.tag_name().name() == "rPr");
    let has = |name: &str| {
        props
            .map(|p| {
                p.children().any(|c| {
                    c.tag_name().name() == name
                        // <w:b w:val="0"/> switches it back off.
                        && c.attribute((
                            "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
                            "val",
                        ))
                        .map(|v| v != "0" && v != "false")
                        .unwrap_or(true)
                })
            })
            .unwrap_or(false)
    };

    let mut text = String::new();
    for child in r.children() {
        match child.tag_name().name() {
            // Word writes a deleted run's text as <w:delText>, not <w:t>, so
            // that tools reading only <w:t> don't silently double-count it.
            "t" | "delText" => text.push_str(child.text().unwrap_or("")),
            "tab" => text.push('\t'),
            "br" => text.push('\n'),
            _ => {}
        }
    }
    if text.is_empty() {
        return;
    }

    let mut inline = Inline::Text(text);
    if has("i") {
        inline = Inline::Italic(vec![inline]);
    }
    if has("b") {
        inline = Inline::Bold(vec![inline]);
    }
    out.push(inline);
}

/// Reads `word/_rels/document.xml.rels` into id → target, for hyperlinks and
/// images.
fn read_relationships(zip: &mut zip::ZipArchive<std::fs::File>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Some(bytes) = read_entry(zip, "word/_rels/document.xml.rels") else { return map };
    let Ok(text) = String::from_utf8(bytes) else { return map };
    let Ok(doc) = roxmltree::Document::parse(&text) else { return map };
    for node in doc.descendants().filter(|n| n.tag_name().name() == "Relationship") {
        if let (Some(id), Some(target)) = (node.attribute("Id"), node.attribute("Target")) {
            map.insert(id.to_string(), target.to_string());
        }
    }
    map
}

pub fn read(path: &Path) -> Result<Imported, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Couldn't open the file: {e}"))?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|_| "This doesn't look like a Word file — it isn't a readable .docx archive.".to_string())?;

    let rels = read_relationships(&mut zip);

    let bytes = read_entry(&mut zip, "word/document.xml")
        .ok_or_else(|| "This .docx has no document body (word/document.xml is missing).".to_string())?;
    let text = String::from_utf8(bytes)
        .map_err(|_| "The document's text isn't valid UTF-8.".to_string())?;
    let doc = roxmltree::Document::parse(&text)
        .map_err(|e| format!("The document's XML couldn't be read: {e}"))?;

    let mut blocks: Vec<Block> = Vec::new();
    let mut media: Vec<Media> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    // A run of consecutive list paragraphs is gathered so they become one list
    // rather than one list per line.
    let mut pending_list: Option<(bool, Vec<Vec<Block>>)> = None;

    let body = doc
        .descendants()
        .find(|n| n.tag_name().name() == "body")
        .ok_or_else(|| "This .docx has no document body.".to_string())?;

    for node in body.children() {
        match node.tag_name().name() {
            "p" => {
                let props = node.children().find(|c| c.tag_name().name() == "pPr");
                let style = props
                    .and_then(|p| p.children().find(|c| c.tag_name().name() == "pStyle"))
                    .and_then(|s| {
                        s.attribute((
                            "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
                            "val",
                        ))
                    })
                    .unwrap_or("");
                let is_list = props
                    .map(|p| p.children().any(|c| c.tag_name().name() == "numPr"))
                    .unwrap_or(false);

                let inlines = paragraph_inlines(node, &rels);

                // An image anchored in this paragraph.
                let image_rel = node
                    .descendants()
                    .find(|n| n.tag_name().name() == "blip")
                    .and_then(|b| {
                        b.attribute((
                            "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
                            "embed",
                        ))
                    })
                    .and_then(|id| rels.get(id));

                if inlines.is_empty() && image_rel.is_none() {
                    continue;
                }

                let ordered = style.to_lowercase().contains("number");
                if is_list {
                    let entry = pending_list.get_or_insert((ordered, Vec::new()));
                    entry.1.push(vec![Block::Paragraph(inlines)]);
                    continue;
                }
                if let Some((ordered, items)) = pending_list.take() {
                    blocks.push(Block::List { ordered, items });
                }

                if let Some(target) = image_rel {
                    let name = target.rsplit('/').next().unwrap_or("image").to_string();
                    let entry_name = format!("word/{}", target.trim_start_matches("../"));
                    if let Some(data) = read_entry(&mut zip, &entry_name) {
                        media.push(Media { name: name.clone(), bytes: data });
                        blocks.push(Block::Image { src: name, alt: String::new() });
                    }
                    if inlines.is_empty() {
                        continue;
                    }
                }

                if let Some(level) = heading_level_from_style(style) {
                    blocks.push(Block::Heading { level, body: inlines });
                } else if is_quote_style(style) {
                    blocks.push(Block::Quote(vec![Block::Paragraph(inlines)]));
                } else {
                    blocks.push(Block::Paragraph(inlines));
                }
            }
            "tbl" => {
                if let Some((ordered, items)) = pending_list.take() {
                    blocks.push(Block::List { ordered, items });
                }
                let mut rows: Vec<Vec<Vec<Inline>>> = Vec::new();
                for tr in node.children().filter(|c| c.tag_name().name() == "tr") {
                    let mut cells: Vec<Vec<Inline>> = Vec::new();
                    for tc in tr.children().filter(|c| c.tag_name().name() == "tc") {
                        let mut cell: Vec<Inline> = Vec::new();
                        for p in tc.children().filter(|c| c.tag_name().name() == "p") {
                            cell.extend(paragraph_inlines(p, &rels));
                        }
                        cells.push(cell);
                    }
                    if !cells.is_empty() {
                        rows.push(cells);
                    }
                }
                if !rows.is_empty() {
                    blocks.push(Block::Table { rows });
                }
            }
            _ => {}
        }
    }
    if let Some((ordered, items)) = pending_list.take() {
        blocks.push(Block::List { ordered, items });
    }

    // Citation-manager fields are stored as proprietary XML that isn't text, so
    // they convert to nothing at all — worth saying rather than leaving the
    // reader to notice their citations are missing.
    let lower = text.to_lowercase();
    if lower.contains("zotero") || lower.contains("mendeley") || lower.contains("endnote") {
        notes.push(
            "This document's citations come from a reference manager and can't be read \
             directly. In Word, use the citation manager's \"Unlink Citations\" first, \
             then import again."
                .into(),
        );
    }
    if blocks.is_empty() {
        return Err("This .docx appears to have no text in it.".to_string());
    }

    let tracked_changes = super::collect_tracked_changes(&blocks);
    Ok(Imported { blocks, media, notes, tracked_changes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_styles_are_recognised_in_the_spellings_word_uses() {
        assert_eq!(heading_level_from_style("Heading1"), Some(1));
        assert_eq!(heading_level_from_style("heading 2"), Some(2));
        assert_eq!(heading_level_from_style("Heading-3"), Some(3));
        assert_eq!(heading_level_from_style("Title"), None);
        assert_eq!(heading_level_from_style("Heading0"), None);
        assert_eq!(heading_level_from_style("BodyText"), None);
    }

    #[test]
    fn quote_styles_are_recognised() {
        assert!(is_quote_style("Quote"));
        assert!(is_quote_style("Intense Quote"));
        assert!(!is_quote_style("Heading1"));
    }

    /// Builds a minimal but structurally real .docx in memory.
    fn make_docx(document_xml: &str) -> std::path::PathBuf {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!(
            "zerkalo_docx_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.docx");
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        zip.start_file("word/document.xml", opts).unwrap();
        zip.write_all(document_xml.as_bytes()).unwrap();
        zip.finish().unwrap();
        path
    }

    fn wrap(inner: &str) -> String {
        format!(
            r#"<?xml version="1.0"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>{inner}</w:body></w:document>"#
        )
    }

    #[test]
    fn a_heading_and_a_paragraph_are_read() {
        let path = make_docx(&wrap(
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>The Title</w:t></w:r></w:p>
               <w:p><w:r><w:t>Body text here.</w:t></w:r></w:p>"#,
        ));
        let doc = read(&path).expect("should read");
        assert_eq!(
            doc.blocks[0],
            Block::Heading { level: 1, body: vec![Inline::Text("The Title".into())] }
        );
        assert_eq!(doc.blocks[1], Block::Paragraph(vec![Inline::Text("Body text here.".into())]));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn bold_and_italic_runs_carry_their_formatting() {
        let path = make_docx(&wrap(
            r#"<w:p>
                 <w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r>
                 <w:r><w:rPr><w:i/></w:rPr><w:t>italic</w:t></w:r>
                 <w:r><w:rPr><w:b w:val="0"/></w:rPr><w:t>plain</w:t></w:r>
               </w:p>"#,
        ));
        let doc = read(&path).expect("should read");
        let Block::Paragraph(inlines) = &doc.blocks[0] else { panic!("expected paragraph") };
        assert_eq!(inlines[0], Inline::Bold(vec![Inline::Text("bold".into())]));
        assert_eq!(inlines[1], Inline::Italic(vec![Inline::Text("italic".into())]));
        // w:val="0" turns the property off — treating it as on is a classic
        // OOXML mistake that makes whole documents come out bold.
        assert_eq!(inlines[2], Inline::Text("plain".into()));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn consecutive_list_paragraphs_become_one_list() {
        let path = make_docx(&wrap(
            r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>one</w:t></w:r></w:p>
               <w:p><w:pPr><w:numPr><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>two</w:t></w:r></w:p>
               <w:p><w:r><w:t>after</w:t></w:r></w:p>"#,
        ));
        let doc = read(&path).expect("should read");
        match &doc.blocks[0] {
            Block::List { items, .. } => assert_eq!(items.len(), 2, "both items in one list"),
            other => panic!("expected a list, got {other:?}"),
        }
        assert_eq!(doc.blocks[1], Block::Paragraph(vec![Inline::Text("after".into())]));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_table_is_read_row_by_row() {
        let path = make_docx(&wrap(
            r#"<w:tbl>
                 <w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc>
                       <w:tc><w:p><w:r><w:t>b</w:t></w:r></w:p></w:tc></w:tr>
               </w:tbl>"#,
        ));
        let doc = read(&path).expect("should read");
        match &doc.blocks[0] {
            Block::Table { rows } => {
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].len(), 2);
            }
            other => panic!("expected a table, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_converted_word_document_compiles() {
        let path = make_docx(&wrap(
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Costs &amp; Values</w:t></w:r></w:p>
               <w:p><w:r><w:t>Prices are $5 and $10, mail a@b.com, use #tags.</w:t></w:r></w:p>
               <w:p><w:pPr><w:numPr><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>one</w:t></w:r></w:p>
               <w:tbl><w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc>
                     <w:tc><w:p><w:r><w:t>b</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
        ));
        let doc = read(&path).expect("should read");
        crate::doc_import::tests::assert_compiles(&crate::doc_import::to_typst(&doc));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_file_that_is_not_a_zip_is_reported_in_plain_language() {
        let dir = std::env::temp_dir().join(format!("zerkalo_docx_bad_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("not-really.docx");
        std::fs::write(&path, b"this is just text").unwrap();
        let err = read(&path).expect_err("should fail");
        assert!(err.contains("Word file"), "got: {err}");
        assert!(!err.contains("ZipError"), "internal error leaked: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_empty_document_is_reported_rather_than_producing_an_empty_file() {
        let path = make_docx(&wrap(""));
        let err = read(&path).expect_err("should fail");
        assert!(err.contains("no text"), "got: {err}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn an_insertion_run_is_read_as_a_tracked_change_and_kept_in_the_text() {
        let path = make_docx(&wrap(
            r#"<w:p><w:r><w:t>Before </w:t></w:r>
               <w:ins><w:r><w:t>added text</w:t></w:r></w:ins>
               <w:r><w:t> after</w:t></w:r></w:p>"#,
        ));
        let doc = read(&path).expect("should read");
        assert_eq!(doc.tracked_changes.len(), 1);
        assert_eq!(doc.tracked_changes[0].kind, SuggestionKind::Insertion);
        assert_eq!(doc.tracked_changes[0].text, "added text");
        let Block::Paragraph(inlines) = &doc.blocks[0] else { panic!("expected paragraph") };
        assert!(matches!(&inlines[1], Inline::Tracked { kind: SuggestionKind::Insertion, .. }));
        let out = super::super::to_typst(&doc);
        assert!(out.contains("added text"), "insertion should still appear in the rendered text: {out}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_deletion_run_uses_deltext_and_is_also_kept_in_the_rendered_text() {
        let path = make_docx(&wrap(
            r#"<w:p><w:del><w:r><w:delText>old phrase</w:delText></w:r></w:del></w:p>"#,
        ));
        let doc = read(&path).expect("should read");
        assert_eq!(doc.tracked_changes.len(), 1);
        assert_eq!(doc.tracked_changes[0].kind, SuggestionKind::Deletion);
        assert_eq!(doc.tracked_changes[0].text, "old phrase");
        let out = super::super::to_typst(&doc);
        assert!(out.contains("old phrase"), "deletion stays visible until reviewed: {out}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_document_with_no_track_changes_has_an_empty_list() {
        let path = make_docx(&wrap(r#"<w:p><w:r><w:t>plain text</w:t></w:r></w:p>"#));
        let doc = read(&path).expect("should read");
        assert!(doc.tracked_changes.is_empty());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_document_with_track_changes_still_compiles() {
        let path = make_docx(&wrap(
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Title</w:t></w:r></w:p>
               <w:p><w:r><w:t>Kept. </w:t></w:r>
               <w:ins><w:r><w:t>Added sentence.</w:t></w:r></w:ins>
               <w:del><w:r><w:delText>Removed sentence.</w:delText></w:r></w:del></w:p>"#,
        ));
        let doc = read(&path).expect("should read");
        crate::doc_import::tests::assert_compiles(&crate::doc_import::to_typst(&doc));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn reference_manager_citations_are_flagged() {
        let path = make_docx(&wrap(
            r#"<w:p><w:r><w:instrText>ADDIN ZOTERO_ITEM CSL_CITATION</w:instrText></w:r>
               <w:r><w:t>Some text</w:t></w:r></w:p>"#,
        ));
        let doc = read(&path).expect("should read");
        assert!(
            doc.notes.iter().any(|n| n.contains("reference manager")),
            "should warn: {:?}",
            doc.notes
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
