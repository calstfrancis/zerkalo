//! Word export built into Zerkalo — no pandoc. The document is compiled to HTML by the
//! same Typst compiler as the PDF (so citations are formatted identically), with a few
//! export-only adjustments (see `compiler::compile_to_html_for_export`), and that
//! structured output is written out as a `.docx` whose every paragraph uses a named
//! style.
//!
//! Two targets, because InDesign and Canva handle notes differently: InDesign places a
//! Word file's footnotes as real, reflowing footnotes, while Canva has no notes at all
//! and drops them — so the Canva file carries them as numbered endnotes in plain text.

mod convert;
mod html;
mod package;
mod source;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    InDesign,
    Canva,
}

pub struct Exported {
    pub bytes: Vec<u8>,
    /// Plain-language notes for the export log (things converted or left out).
    pub notes: Vec<String>,
}

pub fn export(
    root_file: &Path,
    overrides: &HashMap<PathBuf, String>,
    sys_inputs: &HashMap<String, String>,
    extra_root: Option<&Path>,
    target: Target,
) -> Result<Exported, String> {
    let content = match overrides.get(root_file) {
        Some(c) => c.clone(),
        None => std::fs::read_to_string(root_file)
            .map_err(|e| format!("Couldn't read {}: {e}", root_file.display()))?,
    };
    let (prepared, looks) = source::prepare(&content);
    let mut overrides = overrides.clone();
    overrides.insert(root_file.to_path_buf(), prepared);
    let html =
        crate::compiler::compile_to_html_for_export(root_file, &overrides, sys_inputs, extra_root)?;
    let doc = convert::convert(&html::parse(&html));
    let bytes = package::write(&doc, &looks, target)?;
    let mut notes = doc.warnings;
    if target == Target::Canva && !doc.notes.is_empty() {
        notes.push(format!(
            "{} note(s) placed as endnotes at the end — Canva has no footnotes",
            doc.notes.len()
        ));
    }
    Ok(Exported { bytes, notes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read as _;

    fn project(tag: &str, doc: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("zerkalo_docx_{tag}_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("refs.bib"),
            "@book{a, author={Alpha, Ann}, title={First}, year={2001}, publisher={P}}\n\
             @book{b, author={Beta, Bob}, title={Second}, year={2002}, publisher={P}}\n",
        )
        .unwrap();
        let path = dir.join("main.typ");
        std::fs::write(&path, doc).unwrap();
        path
    }

    fn part(bytes: &[u8], name: &str) -> String {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut out = String::new();
        zip.by_name(name).unwrap().read_to_string(&mut out).unwrap();
        out
    }

    const DOC: &str =
        "#show heading.where(level: 1): it => align(center, text(weight: \"bold\", it.body))\n\
                       = Chapter\n\
                       Text@a with a note#footnote[A _plain_ note.] and @b.\n\n\
                       Second paragraph.\n\n\
                       #bibliography(\"refs.bib\", style: \"chicago-notes\")\n";

    #[test]
    fn indesign_target_writes_real_footnotes_and_named_styles() {
        let path = project("indesign", DOC);
        let out = export(
            &path,
            &HashMap::new(),
            &HashMap::new(),
            None,
            Target::InDesign,
        )
        .unwrap();
        let body = part(&out.bytes, "word/document.xml");
        let notes = part(&out.bytes, "word/footnotes.xml");
        let styles = part(&out.bytes, "word/styles.xml");
        assert_eq!(body.matches("<w:footnoteReference").count(), 3, "{body}");
        assert_eq!(notes.matches("<w:footnoteRef/>").count(), 3);
        assert!(notes.contains("Ann Alpha") && notes.contains("plain"));
        assert!(body.contains(r#"<w:pStyle w:val="Heading1"/>"#));
        assert!(body.contains(r#"<w:pStyle w:val="FirstParagraph"/>"#));
        assert!(body.contains(r#"<w:pStyle w:val="BodyText"/>"#));
        assert!(body.contains(r#"<w:pStyle w:val="Bibliography"/>"#));
        let h1 = &styles[styles.find(r#"w:styleId="Heading1""#).unwrap()..];
        let h1 = &h1[..h1.find("</w:style>").unwrap()];
        assert!(h1.contains(r#"<w:jc w:val="center"/>"#), "{h1}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_commented_out_bibliography_keeps_the_notes_and_drops_the_list() {
        let path = project("nobib", &DOC.replace("#bibliography", "// #bibliography"));
        let out = export(
            &path,
            &HashMap::new(),
            &HashMap::new(),
            None,
            Target::InDesign,
        )
        .unwrap();
        let body = part(&out.bytes, "word/document.xml");
        let notes = part(&out.bytes, "word/footnotes.xml");
        assert_eq!(body.matches("<w:footnoteReference").count(), 3, "{body}");
        assert!(notes.contains("Ann Alpha"), "{notes}");
        assert!(
            !body.contains(r#"<w:pStyle w:val="Bibliography"/>"#),
            "{body}"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn canva_target_puts_numbered_notes_at_the_end() {
        let path = project("canva", DOC);
        let out = export(&path, &HashMap::new(), &HashMap::new(), None, Target::Canva).unwrap();
        let body = part(&out.bytes, "word/document.xml");
        assert!(!body.contains("<w:footnoteReference"));
        assert_eq!(
            body.matches(r#"<w:rStyle w:val="NoteReference"/>"#).count(),
            3
        );
        let notes_at = body.find(">Notes<").expect("a Notes heading");
        assert!(body[notes_at..].contains("1. "));
        assert!(body[notes_at..].contains("A plain note") || body[notes_at..].contains("plain"));
        assert!(out.notes.iter().any(|n| n.contains("endnotes")));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// Writes `ZKDOCX_DOC` (a .typ path) out as `ZKDOCX_OUT`, for opening by hand
    /// (`cargo test -- --ignored`).
    #[test]
    #[ignore]
    fn export_for_inspection() {
        let src = PathBuf::from(std::env::var("ZKDOCX_DOC").unwrap());
        let target = if std::env::var("ZKDOCX_CANVA").is_ok() {
            Target::Canva
        } else {
            Target::InDesign
        };
        let out = export(&src, &HashMap::new(), &HashMap::new(), None, target).unwrap();
        std::fs::write(std::env::var("ZKDOCX_OUT").unwrap(), out.bytes).unwrap();
        eprintln!("notes: {:?}", out.notes);
    }
}
