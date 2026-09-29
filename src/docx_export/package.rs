//! Writing the converted document as a `.docx` (Office Open XML) package.
//!
//! Every paragraph and character gets a named style and nothing else, so InDesign's
//! Word import (and Word itself) can map or restyle the whole document in one step.
//! The style definitions carry the document's own look.

use std::fmt::Write as _;
use std::io::Write as _;

use super::convert::{Block, Cell, Doc, Fmt, ListKind, PStyle, Para, Run};
use super::source::{HeadingLook, HeadingLooks, Size};
use super::Target;

const TWIPS: f64 = 20.0;
const EMU_PER_PT: f64 = 12700.0;

pub fn write(doc: &Doc, looks: &HeadingLooks, target: Target) -> Result<Vec<u8>, String> {
    let mut parts: Vec<(String, Vec<u8>)> = Vec::new();
    let mut rels = Rels::default();
    let body = document_xml(doc, target, &mut rels);
    let footnotes = footnotes_xml(doc, target, &mut rels);

    parts.push((
        "[Content_Types].xml".into(),
        content_types(doc).into_bytes(),
    ));
    parts.push(("_rels/.rels".into(), ROOT_RELS.as_bytes().to_vec()));
    parts.push(("docProps/core.xml".into(), core_xml(doc).into_bytes()));
    parts.push(("docProps/app.xml".into(), APP_XML.as_bytes().to_vec()));
    parts.push(("word/document.xml".into(), body.into_bytes()));
    parts.push(("word/footnotes.xml".into(), footnotes.into_bytes()));
    parts.push((
        "word/styles.xml".into(),
        styles_xml(doc, looks).into_bytes(),
    ));
    parts.push(("word/numbering.xml".into(), numbering_xml(doc).into_bytes()));
    parts.push(("word/settings.xml".into(), SETTINGS_XML.as_bytes().to_vec()));
    parts.push((
        "word/_rels/document.xml.rels".into(),
        rels.document_rels(doc).into_bytes(),
    ));
    parts.push((
        "word/_rels/footnotes.xml.rels".into(),
        rels.footnote_rels(doc).into_bytes(),
    ));
    for (i, img) in doc.images.iter().enumerate() {
        parts.push((
            format!("word/media/image{}.{}", i + 1, img.ext),
            img.data.clone(),
        ));
    }

    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in parts {
            zip.start_file(name, opts).map_err(|e| e.to_string())?;
            zip.write_all(&data).map_err(|e| e.to_string())?;
        }
        zip.finish().map_err(|e| e.to_string())?;
    }
    Ok(buf.into_inner())
}

/// Relationship ids used while writing; hyperlinks need one per part they appear in.
#[derive(Default)]
struct Rels {
    doc_links: Vec<usize>,
    note_links: Vec<usize>,
    in_notes: bool,
}

impl Rels {
    fn link(&mut self, idx: usize) -> String {
        let list = if self.in_notes {
            &mut self.note_links
        } else {
            &mut self.doc_links
        };
        if !list.contains(&idx) {
            list.push(idx);
        }
        format!("rIdLink{idx}")
    }

    fn document_rels(&self, doc: &Doc) -> String {
        let mut out = String::from(XML_DECL);
        out.push_str(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#);
        for (id, ty, target) in [
            ("rIdStyles", "styles", "styles.xml"),
            ("rIdNumbering", "numbering", "numbering.xml"),
            ("rIdSettings", "settings", "settings.xml"),
            ("rIdFootnotes", "footnotes", "footnotes.xml"),
        ] {
            let _ = write!(
                out,
                r#"<Relationship Id="{id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/{ty}" Target="{target}"/>"#
            );
        }
        image_rels(doc, &mut out);
        link_rels(doc, &self.doc_links, &mut out);
        out.push_str("</Relationships>");
        out
    }

    fn footnote_rels(&self, doc: &Doc) -> String {
        let mut out = String::from(XML_DECL);
        out.push_str(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#);
        image_rels(doc, &mut out);
        link_rels(doc, &self.note_links, &mut out);
        out.push_str("</Relationships>");
        out
    }
}

fn image_rels(doc: &Doc, out: &mut String) {
    for (i, img) in doc.images.iter().enumerate() {
        let _ = write!(
            out,
            r#"<Relationship Id="rIdImg{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image{n}.{ext}"/>"#,
            n = i + 1,
            ext = img.ext
        );
    }
}

fn link_rels(doc: &Doc, used: &[usize], out: &mut String) {
    for &idx in used {
        if let Some(url) = doc.links.get(idx) {
            let _ = write!(
                out,
                r#"<Relationship Id="rIdLink{idx}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="{}" TargetMode="External"/>"#,
                esc(url)
            );
        }
    }
}

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

const NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture""#;

fn document_xml(doc: &Doc, target: Target, rels: &mut Rels) -> String {
    let mut out = String::from(XML_DECL);
    let _ = write!(out, "<w:document {NS}><w:body>");
    let mut ids = DrawingIds::default();
    for block in &doc.blocks {
        write_block(&mut out, block, doc, target, rels, &mut ids);
    }
    if target == Target::Canva && !doc.notes.is_empty() {
        let heading = Para {
            style: PStyle::NotesHeading,
            runs: vec![Run::Text(
                notes_title(&doc.style.lang).into(),
                Fmt::default(),
            )],
            list: None,
        };
        write_para(&mut out, &heading, doc, target, rels, &mut ids, None);
        for note in &doc.notes {
            for (i, p) in note.paras.iter().enumerate() {
                let prefix = (i == 0).then(|| format!("{}. ", note.number));
                write_para(&mut out, p, doc, target, rels, &mut ids, prefix.as_deref());
            }
        }
    }
    let s = &doc.style;
    let _ = write!(
        out,
        r#"<w:sectPr><w:footnotePr><w:numFmt w:val="decimal"/></w:footnotePr><w:pgSz w:w="{}" w:h="{}"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr>"#,
        (s.page_width * TWIPS).round(),
        (s.page_height * TWIPS).round()
    );
    out.push_str("</w:body></w:document>");
    out
}

fn notes_title(lang: &str) -> &'static str {
    match lang {
        "ru" => "Примечания",
        "fr" => "Notes",
        "de" => "Anmerkungen",
        "es" => "Notas",
        _ => "Notes",
    }
}

fn footnotes_xml(doc: &Doc, target: Target, rels: &mut Rels) -> String {
    let mut out = String::from(XML_DECL);
    let _ = write!(out, "<w:footnotes {NS}>");
    out.push_str(r#"<w:footnote w:type="separator" w:id="-1"><w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:separator/></w:r></w:p></w:footnote>"#);
    out.push_str(r#"<w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>"#);
    if target == Target::InDesign {
        rels.in_notes = true;
        let mut ids = DrawingIds { next: 1000 };
        for (i, note) in doc.notes.iter().enumerate() {
            let _ = write!(out, r#"<w:footnote w:id="{}">"#, i + 1);
            for (j, p) in note.paras.iter().enumerate() {
                if j == 0 {
                    write_para_with_ref(&mut out, p, doc, target, rels, &mut ids);
                } else {
                    write_para(&mut out, p, doc, target, rels, &mut ids, None);
                }
            }
            out.push_str("</w:footnote>");
        }
        rels.in_notes = false;
    }
    out.push_str("</w:footnotes>");
    out
}

#[derive(Default)]
struct DrawingIds {
    next: u32,
}

fn write_block(
    out: &mut String,
    block: &Block,
    doc: &Doc,
    target: Target,
    rels: &mut Rels,
    ids: &mut DrawingIds,
) {
    match block {
        Block::Para(p) => write_para(out, p, doc, target, rels, ids, None),
        Block::Table(rows) => write_table(out, rows, doc, target, rels, ids),
    }
}

fn style_id(style: PStyle) -> String {
    match style {
        PStyle::Title => "Title".into(),
        PStyle::Heading(n) => format!("Heading{}", n.clamp(1, 6)),
        PStyle::Body => "BodyText".into(),
        PStyle::FirstBody => "FirstParagraph".into(),
        PStyle::Quote => "BlockQuote".into(),
        PStyle::Caption => "Caption".into(),
        PStyle::Bibliography => "Bibliography".into(),
        PStyle::ListItem | PStyle::ListContinue => "ListParagraph".into(),
        PStyle::Figure => "Figure".into(),
        PStyle::NoteText => "FootnoteText".into(),
        PStyle::NotesHeading => "Heading1".into(),
        PStyle::Centered => "Centered".into(),
        PStyle::Right => "RightAligned".into(),
    }
}

fn para_open(out: &mut String, p: &Para, target: Target) {
    out.push_str("<w:p><w:pPr>");
    let id = match (p.style, target) {
        (PStyle::NoteText, Target::Canva) => "EndnoteText".to_string(),
        (style, _) => style_id(style),
    };
    let _ = write!(out, r#"<w:pStyle w:val="{id}"/>"#);
    if let Some((num, level)) = p.list {
        let _ = write!(
            out,
            r#"<w:numPr><w:ilvl w:val="{level}"/><w:numId w:val="{num}"/></w:numPr>"#
        );
    }
    out.push_str("</w:pPr>");
}

fn write_para(
    out: &mut String,
    p: &Para,
    doc: &Doc,
    target: Target,
    rels: &mut Rels,
    ids: &mut DrawingIds,
    prefix: Option<&str>,
) {
    para_open(out, p, target);
    if let Some(prefix) = prefix {
        write_text_run(out, prefix, &Fmt::default());
    }
    write_runs(out, &p.runs, doc, target, rels, ids);
    out.push_str("</w:p>");
}

/// A footnote's first paragraph, which must carry the `footnoteRef` mark itself.
fn write_para_with_ref(
    out: &mut String,
    p: &Para,
    doc: &Doc,
    target: Target,
    rels: &mut Rels,
    ids: &mut DrawingIds,
) {
    para_open(out, p, target);
    out.push_str(
        r#"<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteRef/></w:r>"#,
    );
    write_text_run(out, " ", &Fmt::default());
    write_runs(out, &p.runs, doc, target, rels, ids);
    out.push_str("</w:p>");
}

fn write_runs(
    out: &mut String,
    runs: &[Run],
    doc: &Doc,
    target: Target,
    rels: &mut Rels,
    ids: &mut DrawingIds,
) {
    for run in runs {
        match run {
            Run::Text(text, fmt) => {
                if let Some(link) = fmt.link {
                    let id = rels.link(link);
                    let _ = write!(out, r#"<w:hyperlink r:id="{id}">"#);
                    write_text_run(out, text, fmt);
                    out.push_str("</w:hyperlink>");
                } else {
                    write_text_run(out, text, fmt);
                }
            }
            Run::NoteRef(idx) => match target {
                Target::InDesign => {
                    let _ = write!(
                        out,
                        r#"<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteReference w:id="{}"/></w:r>"#,
                        idx + 1
                    );
                }
                Target::Canva => {
                    let number = doc
                        .notes
                        .get(*idx)
                        .map(|n| n.number.clone())
                        .unwrap_or_default();
                    let _ = write!(
                        out,
                        r#"<w:r><w:rPr><w:rStyle w:val="NoteReference"/></w:rPr><w:t>{}</w:t></w:r>"#,
                        esc(&number)
                    );
                }
            },
            Run::Break => out.push_str("<w:r><w:br/></w:r>"),
            Run::Image(idx) => {
                if let Some(img) = doc.images.get(*idx) {
                    ids.next += 1;
                    let (cx, cy) = (
                        (img.width_pt * EMU_PER_PT).round() as i64,
                        (img.height_pt * EMU_PER_PT).round() as i64,
                    );
                    let n = idx + 1;
                    let id = ids.next;
                    let _ = write!(
                        out,
                        r#"<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="{id}" name="Picture {n}"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="{id}" name="image{n}.{ext}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rIdImg{n}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#,
                        ext = img.ext
                    );
                }
            }
        }
    }
}

fn write_text_run(out: &mut String, text: &str, fmt: &Fmt) {
    if text.is_empty() {
        return;
    }
    out.push_str("<w:r>");
    let mut props = String::new();
    let char_style = match (fmt.italic, fmt.bold, fmt.code, fmt.link.is_some()) {
        (_, _, true, _) => Some("Code"),
        (_, _, _, true) => Some("Hyperlink"),
        (true, true, _, _) => Some("StrongEmphasis"),
        (true, false, _, _) => Some("Emphasis"),
        (false, true, _, _) => Some("Strong"),
        _ if fmt.smallcaps => Some("SmallCaps"),
        _ => None,
    };
    if let Some(s) = char_style {
        let _ = write!(props, r#"<w:rStyle w:val="{s}"/>"#);
    }
    // Property order is fixed by the OOXML schema, and Word rejects a file that
    // strays from it: b, i, smallCaps, strike, u, vertAlign.
    if fmt.code || fmt.link.is_some() {
        if fmt.bold {
            props.push_str("<w:b/>");
        }
        if fmt.italic {
            props.push_str("<w:i/>");
        }
    }
    if fmt.smallcaps && char_style != Some("SmallCaps") {
        props.push_str("<w:smallCaps/>");
    }
    if fmt.strike {
        props.push_str("<w:strike/>");
    }
    if fmt.underline {
        props.push_str(r#"<w:u w:val="single"/>"#);
    }
    if fmt.sup {
        props.push_str(r#"<w:vertAlign w:val="superscript"/>"#);
    } else if fmt.sub {
        props.push_str(r#"<w:vertAlign w:val="subscript"/>"#);
    }
    if !props.is_empty() {
        let _ = write!(out, "<w:rPr>{props}</w:rPr>");
    }
    let _ = write!(
        out,
        r#"<w:t xml:space="preserve">{}</w:t></w:r>"#,
        esc(text)
    );
}

fn write_table(
    out: &mut String,
    rows: &[Vec<Cell>],
    doc: &Doc,
    target: Target,
    rels: &mut Rels,
    ids: &mut DrawingIds,
) {
    let cols = rows
        .iter()
        .map(|r| r.iter().map(|c| c.colspan.max(1)).sum::<u32>())
        .max()
        .unwrap_or(1)
        .max(1);
    out.push_str(r#"<w:tbl><w:tblPr><w:tblStyle w:val="Table"/><w:tblW w:w="5000" w:type="pct"/></w:tblPr><w:tblGrid>"#);
    let text_width = (doc.style.page_width - 144.0).max(144.0) * TWIPS;
    for _ in 0..cols {
        let _ = write!(
            out,
            r#"<w:gridCol w:w="{}"/>"#,
            (text_width / cols as f64).round()
        );
    }
    out.push_str("</w:tblGrid>");
    for row in rows {
        out.push_str("<w:tr>");
        if row.iter().all(|c| c.header) {
            out.push_str("<w:trPr><w:tblHeader/></w:trPr>");
        }
        for cell in row {
            out.push_str("<w:tc><w:tcPr>");
            if cell.colspan > 1 {
                let _ = write!(out, r#"<w:gridSpan w:val="{}"/>"#, cell.colspan);
            }
            out.push_str("</w:tcPr>");
            if cell.paras.is_empty() {
                out.push_str(r#"<w:p><w:pPr><w:pStyle w:val="TableText"/></w:pPr></w:p>"#);
            }
            for p in &cell.paras {
                let mut p = p.clone();
                if matches!(p.style, PStyle::Body | PStyle::FirstBody) {
                    p.style = PStyle::Body;
                }
                let mut para = String::new();
                write_para(&mut para, &p, doc, target, rels, ids, None);
                out.push_str(&para.replacen(
                    r#"<w:pStyle w:val="BodyText"/>"#,
                    r#"<w:pStyle w:val="TableText"/>"#,
                    1,
                ));
            }
            out.push_str("</w:tc>");
        }
        out.push_str("</w:tr>");
    }
    out.push_str("</w:tbl>");
}

// ── Styles ───────────────────────────────────────────────────────────────────

fn heading_look(looks: &HeadingLooks, level: u8) -> Option<&HeadingLook> {
    looks
        .iter()
        .find(|(l, _)| *l == Some(level))
        .or_else(|| looks.iter().find(|(l, _)| l.is_none()))
        .map(|(_, look)| look)
}

fn styles_xml(doc: &Doc, looks: &HeadingLooks) -> String {
    let s = &doc.style;
    let font = esc(&s.font);
    let half = |pt: f64| (pt * 2.0).round() as i64;
    let tw = |pt: f64| (pt * TWIPS).round() as i64;
    // Typst lines are a cap height plus `leading` apart; Word's "at least" line
    // spacing is baseline-to-baseline.
    let pitch = |size: f64, leading: f64| tw(size * 0.7 + leading);
    let after = tw((s.spacing - s.leading).max(0.0));
    let jc = if s.justify {
        r#"<w:jc w:val="both"/>"#
    } else {
        ""
    };
    let note_size = (s.size * 0.85 * 2.0).round() / 2.0;

    let mut out = String::from(XML_DECL);
    let _ = write!(
        out,
        r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#
    );
    let _ = write!(
        out,
        r#"<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="{font}" w:hAnsi="{font}" w:eastAsia="{font}" w:cs="{font}"/><w:sz w:val="{sz}"/><w:szCs w:val="{sz}"/><w:lang w:val="{lang}"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="0" w:line="{line}" w:lineRule="atLeast"/></w:pPr></w:pPrDefault></w:docDefaults>"#,
        sz = half(s.size),
        lang = esc(&s.lang),
        line = pitch(s.size, s.leading)
    );

    let para = |out: &mut String,
                id: &str,
                name: &str,
                based: Option<&str>,
                ppr: &str,
                rpr: &str,
                extra: &str| {
        let _ = write!(
            out,
            r#"<w:style w:type="paragraph" w:styleId="{id}"><w:name w:val="{name}"/>"#
        );
        if let Some(b) = based {
            let _ = write!(out, r#"<w:basedOn w:val="{b}"/>"#);
        }
        out.push_str(extra);
        out.push_str(r#"<w:qFormat/>"#);
        if !ppr.is_empty() {
            let _ = write!(out, "<w:pPr>{ppr}</w:pPr>");
        }
        if !rpr.is_empty() {
            let _ = write!(out, "<w:rPr>{rpr}</w:rPr>");
        }
        out.push_str("</w:style>");
    };
    let chr = |out: &mut String, id: &str, name: &str, rpr: &str| {
        let _ = write!(
            out,
            r#"<w:style w:type="character" w:styleId="{id}"><w:name w:val="{name}"/><w:qFormat/><w:rPr>{rpr}</w:rPr></w:style>"#
        );
    };

    para(
        &mut out,
        "Normal",
        "Normal",
        None,
        "",
        "",
        r#"<w:uiPriority w:val="0"/>"#,
    );
    let body_ppr = format!(
        r#"<w:spacing w:after="{after}"/><w:ind w:firstLine="{}"/>{jc}"#,
        tw(s.indent)
    );
    para(
        &mut out,
        "BodyText",
        "Body Text",
        Some("Normal"),
        &body_ppr,
        "",
        "",
    );
    para(
        &mut out,
        "FirstParagraph",
        "First Paragraph",
        Some("BodyText"),
        r#"<w:ind w:firstLine="0"/>"#,
        "",
        r#"<w:next w:val="BodyText"/>"#,
    );
    para(
        &mut out,
        "Title",
        "Title",
        Some("Normal"),
        &format!(
            r#"<w:spacing w:after="{}"/><w:jc w:val="center"/>"#,
            tw(s.size)
        ),
        &format!(
            r#"<w:b/><w:sz w:val="{0}"/><w:szCs w:val="{0}"/>"#,
            half(s.size * 1.7)
        ),
        r#"<w:next w:val="FirstParagraph"/>"#,
    );

    for level in 1..=6u8 {
        let look = heading_look(looks, level);
        let default_scale = match level {
            1 => 1.4,
            2 => 1.2,
            _ => 1.0,
        };
        let size = match look {
            Some(l) => match l.size {
                Some(Size::Pt(pt)) => pt,
                Some(Size::Em(em)) => em * s.size,
                None => s.size,
            },
            None => s.size * default_scale,
        };
        let bold = look.is_none_or(|l| l.bold);
        let mut rpr = String::new();
        if bold {
            rpr.push_str("<w:b/><w:bCs/>");
        }
        if look.is_some_and(|l| l.italic) {
            rpr.push_str("<w:i/><w:iCs/>");
        }
        if look.is_some_and(|l| l.upper) {
            rpr.push_str("<w:caps/>");
        }
        if look.is_some_and(|l| l.smallcaps) {
            rpr.push_str("<w:smallCaps/>");
        }
        let _ = write!(
            rpr,
            r#"<w:sz w:val="{0}"/><w:szCs w:val="{0}"/>"#,
            half(size)
        );
        let align = match look {
            Some(l) if l.center => r#"<w:jc w:val="center"/>"#,
            Some(l) if l.right => r#"<w:jc w:val="right"/>"#,
            _ => "",
        };
        let ppr = format!(
            r#"<w:keepNext/><w:keepLines/><w:spacing w:before="{}" w:after="{}"/>{align}<w:outlineLvl w:val="{}"/>"#,
            tw(size * 1.2),
            tw(size * 0.5),
            level - 1
        );
        para(
            &mut out,
            &format!("Heading{level}"),
            &format!("heading {level}"),
            Some("Normal"),
            &ppr,
            &rpr,
            r#"<w:next w:val="FirstParagraph"/>"#,
        );
    }

    para(
        &mut out,
        "BlockQuote",
        "Block Quote",
        Some("Normal"),
        &format!(
            r#"<w:spacing w:before="{0}" w:after="{0}"/><w:ind w:left="720" w:right="720"/>{jc}"#,
            after.max(tw(s.size * 0.5))
        ),
        "",
        r#"<w:next w:val="FirstParagraph"/>"#,
    );
    para(
        &mut out,
        "ListParagraph",
        "List Paragraph",
        Some("Normal"),
        &format!(
            r#"<w:spacing w:after="{}"/><w:ind w:left="720"/>"#,
            tw(s.size * 0.25)
        ),
        "",
        "",
    );
    para(
        &mut out,
        "Figure",
        "Figure",
        Some("Normal"),
        &format!(
            r#"<w:spacing w:before="{0}" w:after="{0}"/><w:jc w:val="center"/>"#,
            tw(s.size * 0.5)
        ),
        "",
        "",
    );
    para(
        &mut out,
        "Caption",
        "caption",
        Some("Normal"),
        &format!(
            r#"<w:spacing w:after="{}"/><w:jc w:val="center"/>"#,
            tw(s.size)
        ),
        &format!(
            r#"<w:i/><w:sz w:val="{0}"/><w:szCs w:val="{0}"/>"#,
            half(note_size)
        ),
        "",
    );
    para(
        &mut out,
        "Bibliography",
        "Bibliography",
        Some("Normal"),
        &format!(
            r#"<w:spacing w:after="{}"/><w:ind w:left="720" w:hanging="720"/>"#,
            tw(s.size * 0.5)
        ),
        "",
        "",
    );
    para(
        &mut out,
        "TableText",
        "Table Text",
        Some("Normal"),
        "",
        "",
        "",
    );
    para(
        &mut out,
        "Centered",
        "Centered",
        Some("Normal"),
        &format!(r#"<w:spacing w:after="{after}"/><w:jc w:val="center"/>"#),
        "",
        "",
    );
    para(
        &mut out,
        "RightAligned",
        "Right Aligned",
        Some("Normal"),
        &format!(r#"<w:spacing w:after="{after}"/><w:jc w:val="right"/>"#),
        "",
        "",
    );
    para(
        &mut out,
        "FootnoteText",
        "footnote text",
        Some("Normal"),
        &format!(
            r#"<w:spacing w:after="{}" w:line="{}" w:lineRule="atLeast"/>"#,
            tw(note_size * 0.3),
            pitch(note_size, s.leading * 0.85)
        ),
        &format!(
            r#"<w:sz w:val="{0}"/><w:szCs w:val="{0}"/>"#,
            half(note_size)
        ),
        "",
    );

    para(
        &mut out,
        "EndnoteText",
        "Endnote Text",
        Some("Normal"),
        &format!(
            r#"<w:spacing w:after="{}" w:line="{}" w:lineRule="atLeast"/><w:ind w:left="360" w:hanging="360"/>"#,
            tw(note_size * 0.5),
            pitch(note_size, s.leading * 0.85)
        ),
        &format!(
            r#"<w:sz w:val="{0}"/><w:szCs w:val="{0}"/>"#,
            half(note_size)
        ),
        "",
    );

    chr(
        &mut out,
        "FootnoteReference",
        "footnote reference",
        r#"<w:vertAlign w:val="superscript"/>"#,
    );
    chr(
        &mut out,
        "NoteReference",
        "Note Reference",
        r#"<w:vertAlign w:val="superscript"/>"#,
    );
    chr(&mut out, "Emphasis", "Emphasis", "<w:i/><w:iCs/>");
    chr(&mut out, "Strong", "Strong", "<w:b/><w:bCs/>");
    chr(
        &mut out,
        "StrongEmphasis",
        "Strong Emphasis",
        "<w:b/><w:bCs/><w:i/><w:iCs/>",
    );
    chr(&mut out, "SmallCaps", "Small Caps", "<w:smallCaps/>");
    chr(
        &mut out,
        "Hyperlink",
        "Hyperlink",
        r#"<w:u w:val="single"/>"#,
    );
    chr(
        &mut out,
        "Code",
        "Code",
        r#"<w:rFonts w:ascii="Courier New" w:hAnsi="Courier New" w:cs="Courier New"/>"#,
    );

    out.push_str(r#"<w:style w:type="table" w:styleId="Table"><w:name w:val="Table"/><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:bottom w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:insideH w:val="single" w:sz="2" w:space="0" w:color="auto"/></w:tblBorders><w:tblCellMar><w:left w:w="108" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style>"#);
    out.push_str("</w:styles>");
    out
}

fn numbering_xml(doc: &Doc) -> String {
    let mut out = String::from(XML_DECL);
    out.push_str(
        r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
    );
    for (abs, kind) in [(0, ListKind::Bullet), (1, ListKind::Decimal)] {
        let _ = write!(
            out,
            r#"<w:abstractNum w:abstractNumId="{abs}"><w:multiLevelType w:val="hybridMultilevel"/>"#
        );
        for lvl in 0..9u32 {
            let (fmt, text) = match kind {
                ListKind::Bullet => ("bullet", ["•", "◦", "▪"][lvl as usize % 3].to_string()),
                ListKind::Decimal => ("decimal", format!("%{}.", lvl + 1)),
            };
            let _ = write!(
                out,
                r#"<w:lvl w:ilvl="{lvl}"><w:start w:val="1"/><w:numFmt w:val="{fmt}"/><w:lvlText w:val="{text}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{}" w:hanging="360"/></w:pPr></w:lvl>"#,
                720 * (lvl + 1)
            );
        }
        out.push_str("</w:abstractNum>");
    }
    for (i, kind) in doc.lists.iter().enumerate() {
        let abs = if *kind == ListKind::Bullet { 0 } else { 1 };
        let _ = write!(
            out,
            r#"<w:num w:numId="{}"><w:abstractNumId w:val="{abs}"/>"#,
            i + 1
        );
        if *kind == ListKind::Decimal {
            for lvl in 0..9 {
                let _ = write!(
                    out,
                    r#"<w:lvlOverride w:ilvl="{lvl}"><w:startOverride w:val="1"/></w:lvlOverride>"#
                );
            }
        }
        out.push_str("</w:num>");
    }
    out.push_str("</w:numbering>");
    out
}

fn content_types(doc: &Doc) -> String {
    let mut out = String::from(XML_DECL);
    out.push_str(r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>"#);
    let mut exts: Vec<&str> = doc.images.iter().map(|i| i.ext).collect();
    exts.sort();
    exts.dedup();
    for ext in exts {
        let _ = write!(
            out,
            r#"<Default Extension="{ext}" ContentType="image/{ext}"/>"#
        );
    }
    for (part, ty) in [
        (
            "/word/document.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
        ),
        (
            "/word/styles.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml",
        ),
        (
            "/word/numbering.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml",
        ),
        (
            "/word/settings.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml",
        ),
        (
            "/word/footnotes.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml",
        ),
        (
            "/docProps/core.xml",
            "application/vnd.openxmlformats-package.core-properties+xml",
        ),
        (
            "/docProps/app.xml",
            "application/vnd.openxmlformats-officedocument.extended-properties+xml",
        ),
    ] {
        let _ = write!(out, r#"<Override PartName="{part}" ContentType="{ty}"/>"#);
    }
    out.push_str("</Types>");
    out
}

fn core_xml(doc: &Doc) -> String {
    let title = doc.title.as_deref().map(esc).unwrap_or_default();
    format!(
        r#"{XML_DECL}<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title>{title}</dc:title><dc:language>{}</dc:language></cp:coreProperties>"#,
        esc(&doc.style.lang)
    )
}

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#;

const APP_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>Zerkalo</Application></Properties>"#;

const SETTINGS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:defaultTabStop w:val="720"/><w:footnotePr><w:footnote w:id="-1"/><w:footnote w:id="0"/></w:footnotePr><w:compat><w:doNotExpandShiftReturn/><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat></w:settings>"#;

/// XML-escapes text, dropping characters XML 1.0 can't hold at all.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 || c == '\u{fffe}' || c == '\u{ffff}' => {}
            c => out.push(c),
        }
    }
    out
}
