//! `.odt` → the shared import model.
//!
//! Structurally the same job as [`super::docx`] — a ZIP holding XML — but
//! OpenDocument names things differently: the body is `content.xml`, headings
//! are `<text:h>` with an explicit `text:outline-level`, and emphasis lives in
//! named automatic styles that have to be resolved from `<style:style>`
//! definitions rather than read off the run.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use super::{Block, Imported, Inline, Media};

const FO: &str = "urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0";
const TEXT: &str = "urn:oasis:names:tc:opendocument:xmlns:text:1.0";
const OFFICE: &str = "urn:oasis:names:tc:opendocument:xmlns:office:1.0";
const XLINK: &str = "http://www.w3.org/1999/xlink";

/// Whether a named style is bold and/or italic.
#[derive(Default, Clone, Copy)]
struct StyleFlags {
    bold: bool,
    italic: bool,
}

/// Resolves `style:name` → flags, following `style:parent-style-name` one level
/// so a style deriving from "Emphasis" is still italic.
fn read_styles(doc: &roxmltree::Document) -> HashMap<String, StyleFlags> {
    let mut direct: HashMap<String, StyleFlags> = HashMap::new();
    let mut parents: HashMap<String, String> = HashMap::new();

    for style in doc.descendants().filter(|n| n.tag_name().name() == "style") {
        let Some(name) = style.attribute((
            "urn:oasis:names:tc:opendocument:xmlns:style:1.0",
            "name",
        )) else {
            continue;
        };
        if let Some(parent) = style.attribute((
            "urn:oasis:names:tc:opendocument:xmlns:style:1.0",
            "parent-style-name",
        )) {
            parents.insert(name.to_string(), parent.to_string());
        }
        let mut flags = StyleFlags::default();
        for props in style.children().filter(|c| c.tag_name().name() == "text-properties") {
            if props.attribute((FO, "font-weight")).map(|v| v == "bold").unwrap_or(false) {
                flags.bold = true;
            }
            if props.attribute((FO, "font-style")).map(|v| v == "italic").unwrap_or(false) {
                flags.italic = true;
            }
        }
        direct.insert(name.to_string(), flags);
    }

    let mut resolved = direct.clone();
    for (name, parent) in &parents {
        if let Some(pf) = direct.get(parent) {
            let entry = resolved.entry(name.clone()).or_default();
            entry.bold |= pf.bold;
            entry.italic |= pf.italic;
        }
        // A style named after the built-in emphasis roles carries them even
        // when the file doesn't spell the properties out.
        let lower = parent.to_lowercase();
        if lower.contains("emphasis") {
            resolved.entry(name.clone()).or_default().italic = true;
        }
        if lower.contains("strong") {
            resolved.entry(name.clone()).or_default().bold = true;
        }
    }
    resolved
}

/// Collects the inlines of one paragraph-like element.
fn inlines_of(node: roxmltree::Node, styles: &HashMap<String, StyleFlags>) -> Vec<Inline> {
    let mut out = Vec::new();
    collect(node, styles, StyleFlags::default(), &mut out);
    out
}

fn collect(
    node: roxmltree::Node,
    styles: &HashMap<String, StyleFlags>,
    inherited: StyleFlags,
    out: &mut Vec<Inline>,
) {
    for child in node.children() {
        if child.is_text() {
            let text = child.text().unwrap_or("");
            if !text.is_empty() {
                out.push(wrap(Inline::Text(text.to_string()), inherited));
            }
            continue;
        }
        match child.tag_name().name() {
            "span" => {
                let mut flags = inherited;
                if let Some(name) = child.attribute((TEXT, "style-name")) {
                    if let Some(f) = styles.get(name) {
                        flags.bold |= f.bold;
                        flags.italic |= f.italic;
                    }
                }
                collect(child, styles, flags, out);
            }
            "a" => {
                let href = child.attribute((XLINK, "href")).unwrap_or("").to_string();
                let mut body = Vec::new();
                collect(child, styles, inherited, &mut body);
                if body.is_empty() {
                    continue;
                }
                if href.is_empty() {
                    out.extend(body);
                } else {
                    out.push(Inline::Link { href, body });
                }
            }
            "line-break" => out.push(Inline::Break),
            "s" => out.push(Inline::Text(" ".into())),
            "tab" => out.push(Inline::Text("\t".into())),
            _ => collect(child, styles, inherited, out),
        }
    }
}

fn wrap(inline: Inline, flags: StyleFlags) -> Inline {
    let mut out = inline;
    if flags.italic {
        out = Inline::Italic(vec![out]);
    }
    if flags.bold {
        out = Inline::Bold(vec![out]);
    }
    out
}

pub fn read(path: &Path) -> Result<Imported, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Couldn't open the file: {e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| {
        "This doesn't look like an OpenDocument file — it isn't a readable .odt archive."
            .to_string()
    })?;

    let mut buf = Vec::new();
    zip.by_name("content.xml")
        .map_err(|_| "This .odt has no document body (content.xml is missing).".to_string())?
        .read_to_end(&mut buf)
        .map_err(|e| format!("Couldn't read the document body: {e}"))?;
    let text = String::from_utf8(buf).map_err(|_| "The document's text isn't valid UTF-8.".to_string())?;
    let doc = roxmltree::Document::parse(&text)
        .map_err(|e| format!("The document's XML couldn't be read: {e}"))?;

    let styles = read_styles(&doc);

    let mut blocks: Vec<Block> = Vec::new();
    let mut media: Vec<Media> = Vec::new();
    let notes: Vec<String> = Vec::new();

    let body = doc
        .descendants()
        .find(|n| n.tag_name().name() == "text" && n.tag_name().namespace() == Some(OFFICE))
        .ok_or_else(|| "This .odt has no document body.".to_string())?;

    walk_body(body, &styles, &mut blocks, &mut media, &mut zip);

    if blocks.is_empty() {
        return Err("This .odt appears to have no text in it.".to_string());
    }
    Ok(Imported { blocks, media, notes, tracked_changes: Vec::new() })
}

fn walk_body(
    parent: roxmltree::Node,
    styles: &HashMap<String, StyleFlags>,
    blocks: &mut Vec<Block>,
    media: &mut Vec<Media>,
    zip: &mut zip::ZipArchive<std::fs::File>,
) {
    for node in parent.children().filter(|n| n.is_element()) {
        match node.tag_name().name() {
            "h" => {
                let level = node
                    .attribute((TEXT, "outline-level"))
                    .and_then(|v| v.parse::<u8>().ok())
                    .unwrap_or(1);
                let body = inlines_of(node, styles);
                if !body.is_empty() {
                    blocks.push(Block::Heading { level, body });
                }
            }
            "p" => {
                // An image lives inside a frame in the paragraph.
                if let Some(href) = node
                    .descendants()
                    .find(|n| n.tag_name().name() == "image")
                    .and_then(|i| i.attribute((XLINK, "href")))
                {
                    let name = href.rsplit('/').next().unwrap_or("image").to_string();
                    let mut buf = Vec::new();
                    if zip
                        .by_name(href.trim_start_matches("./"))
                        .ok()
                        .and_then(|mut f| f.read_to_end(&mut buf).ok())
                        .is_some()
                    {
                        media.push(Media { name: name.clone(), bytes: buf });
                        blocks.push(Block::Image { src: name, alt: String::new() });
                    }
                }
                let body = inlines_of(node, styles);
                if !body.is_empty() {
                    blocks.push(Block::Paragraph(body));
                }
            }
            "list" => {
                let ordered = node
                    .attribute((TEXT, "style-name"))
                    .map(|s| s.to_lowercase().contains("number"))
                    .unwrap_or(false);
                let mut items: Vec<Vec<Block>> = Vec::new();
                for item in node.children().filter(|c| c.tag_name().name() == "list-item") {
                    let mut inner: Vec<Block> = Vec::new();
                    walk_body(item, styles, &mut inner, media, zip);
                    if !inner.is_empty() {
                        items.push(inner);
                    }
                }
                if !items.is_empty() {
                    blocks.push(Block::List { ordered, items });
                }
            }
            "table" => {
                let mut rows: Vec<Vec<Vec<Inline>>> = Vec::new();
                for tr in node.descendants().filter(|c| c.tag_name().name() == "table-row") {
                    let mut cells: Vec<Vec<Inline>> = Vec::new();
                    for tc in tr.children().filter(|c| c.tag_name().name() == "table-cell") {
                        cells.push(inlines_of(tc, styles));
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_odt(content_xml: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "zerkalo_odt_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.odt");
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        zip.start_file("content.xml", opts).unwrap();
        zip.write_all(content_xml.as_bytes()).unwrap();
        zip.finish().unwrap();
        path
    }

    fn wrap_doc(inner: &str, styles: &str) -> String {
        format!(
            r#"<?xml version="1.0"?>
<office:document-content
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
  xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
  xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0"
  xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0"
  xmlns:xlink="http://www.w3.org/1999/xlink">
  <office:automatic-styles>{styles}</office:automatic-styles>
  <office:body><office:text>{inner}</office:text></office:body>
</office:document-content>"#
        )
    }

    #[test]
    fn headings_use_the_declared_outline_level() {
        let path = make_odt(&wrap_doc(
            r#"<text:h text:outline-level="1">Title</text:h>
               <text:h text:outline-level="3">Deep</text:h>
               <text:p>Body.</text:p>"#,
            "",
        ));
        let doc = read(&path).expect("should read");
        assert_eq!(doc.blocks[0], Block::Heading { level: 1, body: vec![Inline::Text("Title".into())] });
        assert_eq!(doc.blocks[1], Block::Heading { level: 3, body: vec![Inline::Text("Deep".into())] });
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn emphasis_is_resolved_through_the_named_style() {
        // Unlike .docx, formatting isn't on the run — it's a style reference
        // that has to be looked up, or every document comes out unformatted.
        let path = make_odt(&wrap_doc(
            r#"<text:p>plain <text:span text:style-name="T1">bold</text:span>
               <text:span text:style-name="T2">italic</text:span></text:p>"#,
            r#"<style:style style:name="T1"><style:text-properties fo:font-weight="bold"/></style:style>
               <style:style style:name="T2"><style:text-properties fo:font-style="italic"/></style:style>"#,
        ));
        let doc = read(&path).expect("should read");
        let Block::Paragraph(inlines) = &doc.blocks[0] else { panic!("expected paragraph") };
        assert!(
            inlines.iter().any(|i| matches!(i, Inline::Bold(_))),
            "bold span should resolve: {inlines:?}"
        );
        assert!(
            inlines.iter().any(|i| matches!(i, Inline::Italic(_))),
            "italic span should resolve: {inlines:?}"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_list_and_its_items_are_read() {
        let path = make_odt(&wrap_doc(
            r#"<text:list><text:list-item><text:p>one</text:p></text:list-item>
               <text:list-item><text:p>two</text:p></text:list-item></text:list>"#,
            "",
        ));
        let doc = read(&path).expect("should read");
        match &doc.blocks[0] {
            Block::List { items, .. } => assert_eq!(items.len(), 2),
            other => panic!("expected a list, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_hyperlink_keeps_its_target() {
        let path = make_odt(&wrap_doc(
            r#"<text:p><text:a xlink:href="https://example.com">click</text:a></text:p>"#,
            "",
        ));
        let doc = read(&path).expect("should read");
        let Block::Paragraph(inlines) = &doc.blocks[0] else { panic!("expected paragraph") };
        assert!(
            matches!(&inlines[0], Inline::Link { href, .. } if href == "https://example.com"),
            "got: {inlines:?}"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_file_that_is_not_a_zip_is_reported_in_plain_language() {
        let dir = std::env::temp_dir().join(format!("zerkalo_odt_bad_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("not-really.odt");
        std::fs::write(&path, b"plain text, not an archive").unwrap();
        let err = read(&path).expect_err("should fail");
        assert!(err.contains("OpenDocument"), "got: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
