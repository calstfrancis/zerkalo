//! Rearranging a compiled PDF onto larger sheets — N-up and saddle-stitch
//! booklets.
//!
//! CUPS can do plain N-up, but it can't do booklet ordering, and relying on it
//! means the result depends on the driver. Doing it here means what the print
//! sheet previews is exactly what the printer receives, on every printer.
//!
//! Each source page is turned into a Form XObject and drawn into a slot on a
//! new sheet, so text stays vector — rasterising here would undo the whole
//! point of sending the portal a PDF.
//!
//! Ordering lives in [`crate::print_layout::Imposition`]; this module only
//! performs the placement it describes.

use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

use crate::print_layout::Imposition;

/// A source page, read out of the document before it is modified.
///
/// Collected in a pass of its own because the placement pass needs to mutate
/// the document, and the resource lookups borrow it immutably.
struct SourcePage {
    /// MediaBox as [x0, y0, x1, y1].
    media: [f64; 4],
    content: Vec<u8>,
    resources: Dictionary,
    /// The page's transparency group, carried onto the form so that blended
    /// content keeps compositing the way it did on its own page.
    group: Option<Object>,
}

impl SourcePage {
    fn width(&self) -> f64 {
        self.media[2] - self.media[0]
    }

    fn height(&self) -> f64 {
        self.media[3] - self.media[1]
    }
}

/// Rearrange `pdf` according to `imposition`, printing only `pages` (physical
/// indices into the document, in the order given).
///
/// Returns the new PDF. `Imposition::Off` with every page selected is returned
/// untouched — the common case must not pay for a parse and rewrite, and must
/// not risk this code path at all.
pub fn impose(pdf: &[u8], pages: &[usize], imposition: Imposition) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(pdf).map_err(|e| format!("Couldn't read the PDF: {e}"))?;

    let page_ids: Vec<ObjectId> = doc.get_pages().values().copied().collect();
    if page_ids.is_empty() {
        return Err("The document has no pages.".into());
    }
    if imposition == Imposition::Off && pages.len() == page_ids.len() && is_identity(pages) {
        return Ok(pdf.to_vec());
    }
    if let Some(bad) = pages.iter().find(|p| **p >= page_ids.len()) {
        return Err(format!("Page {} isn't in the document.", bad + 1));
    }

    let sources = read_sources(&doc, &page_ids)?;
    let sheet = sheet_size(&sources, pages, imposition)?;
    let sides = imposition.arrange(pages);

    // One form per source page, shared by every slot that shows it — a booklet
    // never repeats a page, but building them once keeps the mapping simple and
    // costs nothing.
    let mut forms: Vec<Option<ObjectId>> = vec![None; page_ids.len()];
    for index in sides.iter().flatten().flatten() {
        if forms[*index].is_none() {
            forms[*index] = Some(make_form(&mut doc, &sources[*index]));
        }
    }

    let pages_id = doc
        .catalog()
        .and_then(|c| c.get(b"Pages"))
        .and_then(Object::as_reference)
        .map_err(|e| format!("The PDF has no page tree: {e}"))?;

    let mut new_page_ids = Vec::with_capacity(sides.len());
    for side in &sides {
        new_page_ids.push(make_sheet(&mut doc, side, &sources, &forms, sheet, imposition, pages_id));
    }

    let tree = doc
        .get_dictionary_mut(pages_id)
        .map_err(|e| format!("The PDF's page tree is unreadable: {e}"))?;
    tree.set("Kids", new_page_ids.iter().map(|id| Object::Reference(*id)).collect::<Vec<_>>());
    tree.set("Count", new_page_ids.len() as i64);
    // The old page objects and their content streams are unreachable now.
    // Without this the imposed file carries both copies.
    doc.prune_objects();

    let mut out = Vec::new();
    doc.save_to(&mut out).map_err(|e| format!("Couldn't write the imposed PDF: {e}"))?;
    Ok(out)
}

fn is_identity(pages: &[usize]) -> bool {
    pages.iter().enumerate().all(|(i, p)| i == *p)
}

fn read_sources(doc: &Document, page_ids: &[ObjectId]) -> Result<Vec<SourcePage>, String> {
    let mut sources = Vec::with_capacity(page_ids.len());
    for id in page_ids {
        let media = media_box(doc, *id)?;
        let content = doc
            .get_page_content(*id)
            .is_empty()
            .then(Vec::new)
            .unwrap_or_else(|| doc.get_page_content(*id));

        let (inline, inherited) = doc
            .get_page_resources(*id)
            .map_err(|e| format!("Couldn't read a page's resources: {e}"))?;
        // `get_page_resources` returns the nearest dictionaries first; applying
        // them farthest-first lets a page override what it inherits.
        let mut resources = Dictionary::new();
        for res_id in inherited.iter().rev() {
            if let Ok(dict) = doc.get_dictionary(*res_id) {
                merge_into(&mut resources, dict);
            }
        }
        if let Some(dict) = inline {
            merge_into(&mut resources, dict);
        }

        let group = doc.get_dictionary(*id).ok().and_then(|p| p.get(b"Group").ok()).cloned();
        sources.push(SourcePage { media, content, resources, group });
    }
    Ok(sources)
}

fn merge_into(target: &mut Dictionary, source: &Dictionary) {
    for (key, value) in source.iter() {
        target.set(key.to_vec(), value.clone());
    }
}

/// MediaBox for a page, walking up the page tree — it is an inheritable
/// attribute, and a page that omits it takes its parent's.
fn media_box(doc: &Document, page_id: ObjectId) -> Result<[f64; 4], String> {
    let mut node = page_id;
    for _ in 0..32 {
        let dict = doc
            .get_dictionary(node)
            .map_err(|e| format!("Couldn't read a page: {e}"))?;
        if let Ok(values) = dict.get(b"MediaBox").and_then(Object::as_array) {
            let mut out = [0.0f64; 4];
            if values.len() != 4 {
                return Err("A page has a malformed MediaBox.".into());
            }
            for (slot, value) in out.iter_mut().zip(values) {
                *slot = value.as_float().map_err(|_| "A page has a malformed MediaBox.")? as f64;
            }
            // A MediaBox may be given with its corners in either order.
            return Ok([
                out[0].min(out[2]),
                out[1].min(out[3]),
                out[0].max(out[2]),
                out[1].max(out[3]),
            ]);
        }
        match dict.get(b"Parent").and_then(Object::as_reference) {
            Ok(parent) => node = parent,
            Err(_) => break,
        }
    }
    Err("A page has no MediaBox.".into())
}

/// The physical sheet every imposed page is drawn onto.
///
/// Taken from the first page actually being printed rather than the first page
/// of the document: printing a range out of a document that changes page size
/// should use the paper the range is on.
fn sheet_size(
    sources: &[SourcePage],
    pages: &[usize],
    imposition: Imposition,
) -> Result<(f64, f64), String> {
    let first = pages.first().ok_or("There are no pages to print.")?;
    let page = sources.get(*first).ok_or("There are no pages to print.")?;
    let (w, h) = (page.width(), page.height());
    if w <= 0.0 || h <= 0.0 {
        return Err("A page has no size.".into());
    }
    // Two pages side by side turn a portrait page into a landscape sheet; a
    // 2×2 grid keeps the page's own proportions, so the sheet does too.
    Ok(if imposition.rotates_sheet() { (h, w) } else { (w, h) })
}

/// Wrap a source page as a Form XObject occupying its own MediaBox.
fn make_form(doc: &mut Document, page: &SourcePage) -> ObjectId {
    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"XObject".to_vec()));
    dict.set("Subtype", Object::Name(b"Form".to_vec()));
    dict.set("FormType", 1i64);
    dict.set(
        "BBox",
        vec![
            Object::Real(page.media[0] as f32),
            Object::Real(page.media[1] as f32),
            Object::Real(page.media[2] as f32),
            Object::Real(page.media[3] as f32),
        ],
    );
    // Shift a MediaBox that doesn't start at the origin, so every form's
    // content begins at (0, 0) and slot placement is plain arithmetic.
    dict.set(
        "Matrix",
        vec![
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(1.0),
            Object::Real(-page.media[0] as f32),
            Object::Real(-page.media[1] as f32),
        ],
    );
    dict.set("Resources", Object::Dictionary(page.resources.clone()));
    if let Some(group) = &page.group {
        dict.set("Group", group.clone());
    }
    doc.add_object(Stream::new(dict, page.content.clone()))
}

#[allow(clippy::too_many_arguments)]
fn make_sheet(
    doc: &mut Document,
    side: &[Option<usize>],
    sources: &[SourcePage],
    forms: &[Option<ObjectId>],
    sheet: (f64, f64),
    imposition: Imposition,
    parent: ObjectId,
) -> ObjectId {
    let (sheet_w, sheet_h) = sheet;
    let (cols, rows) = imposition.grid();
    let slot_w = sheet_w / cols as f64;
    let slot_h = sheet_h / rows as f64;

    let mut xobjects = Dictionary::new();
    let mut content = String::new();

    for (slot, entry) in side.iter().enumerate() {
        let Some(index) = entry else { continue };
        let Some(form_id) = forms.get(*index).copied().flatten() else { continue };
        let page = &sources[*index];
        let (pw, ph) = (page.width(), page.height());
        if pw <= 0.0 || ph <= 0.0 {
            continue;
        }

        let col = slot % cols;
        let row = slot / cols;
        // PDF's origin is bottom-left but slots are numbered in reading order,
        // so row 0 is the top row and its y grows downward from the sheet top.
        let slot_x = col as f64 * slot_w;
        let slot_y = sheet_h - (row as f64 + 1.0) * slot_h;

        let scale = (slot_w / pw).min(slot_h / ph);
        let x = slot_x + (slot_w - pw * scale) / 2.0;
        let y = slot_y + (slot_h - ph * scale) / 2.0;

        let name = format!("ZkP{slot}");
        xobjects.set(name.as_bytes().to_vec(), Object::Reference(form_id));
        content.push_str(&format!(
            "q {scale:.6} 0 0 {scale:.6} {x:.4} {y:.4} cm /{name} Do Q\n"
        ));
    }

    let mut resources = Dictionary::new();
    resources.set("XObject", Object::Dictionary(xobjects));
    let resources_id = doc.add_object(Object::Dictionary(resources));
    let content_id = doc.add_object(Stream::new(Dictionary::new(), content.into_bytes()));

    let mut page = Dictionary::new();
    page.set("Type", Object::Name(b"Page".to_vec()));
    page.set("Parent", Object::Reference(parent));
    page.set("Resources", Object::Reference(resources_id));
    page.set("Contents", Object::Reference(content_id));
    page.set(
        "MediaBox",
        vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(sheet_w as f32),
            Object::Real(sheet_h as f32),
        ],
    );
    doc.add_object(page)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal two-page PDF, written by hand so the tests don't depend on
    /// the Typst compiler.
    fn two_page_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let mut kids = Vec::new();
        for text in ["one", "two"] {
            let content_id = doc.add_object(Stream::new(
                Dictionary::new(),
                format!("BT /F1 12 Tf 10 10 Td ({text}) Tj ET").into_bytes(),
            ));
            let mut page = Dictionary::new();
            page.set("Type", Object::Name(b"Page".to_vec()));
            page.set("Parent", Object::Reference(pages_id));
            page.set("Contents", Object::Reference(content_id));
            page.set("Resources", Object::Dictionary(Dictionary::new()));
            page.set(
                "MediaBox",
                vec![
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(595.0),
                    Object::Real(842.0),
                ],
            );
            kids.push(Object::Reference(doc.add_object(page)));
        }
        let mut tree = Dictionary::new();
        tree.set("Type", Object::Name(b"Pages".to_vec()));
        tree.set("Count", kids.len() as i64);
        tree.set("Kids", kids);
        doc.objects.insert(pages_id, Object::Dictionary(tree));

        let mut catalog = Dictionary::new();
        catalog.set("Type", Object::Name(b"Catalog".to_vec()));
        catalog.set("Pages", Object::Reference(pages_id));
        let catalog_id = doc.add_object(catalog);
        doc.trailer.set("Root", Object::Reference(catalog_id));

        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    fn sheet_sizes(pdf: &[u8]) -> Vec<(f64, f64)> {
        let doc = Document::load_mem(pdf).unwrap();
        doc.get_pages()
            .values()
            .map(|id| {
                let m = media_box(&doc, *id).unwrap();
                (m[2] - m[0], m[3] - m[1])
            })
            .collect()
    }

    #[test]
    fn printing_everything_unimposed_returns_the_original_bytes() {
        // The common case must not be re-encoded: a needless parse-and-rewrite
        // risks corrupting a PDF that was already correct.
        let pdf = two_page_pdf();
        let out = impose(&pdf, &[0, 1], Imposition::Off).unwrap();
        assert_eq!(out, pdf);
    }

    #[test]
    fn a_page_subset_produces_only_those_pages() {
        let pdf = two_page_pdf();
        let out = impose(&pdf, &[1], Imposition::Off).unwrap();
        assert_eq!(sheet_sizes(&out).len(), 1);
    }

    #[test]
    fn reordering_is_honoured_even_without_imposition() {
        // Booklets aside, a caller may hand pages back to front; Off must not
        // shortcut to the original when the order differs.
        let pdf = two_page_pdf();
        let out = impose(&pdf, &[1, 0], Imposition::Off).unwrap();
        assert_ne!(out, pdf, "a reordered document is not the original");
        assert_eq!(sheet_sizes(&out).len(), 2);
    }

    #[test]
    fn two_up_halves_the_sheet_count_and_turns_the_sheet_landscape() {
        let pdf = two_page_pdf();
        let out = impose(&pdf, &[0, 1], Imposition::TwoUp).unwrap();
        let sizes = sheet_sizes(&out);
        assert_eq!(sizes.len(), 1, "two portrait pages fit on one landscape sheet");
        assert!(sizes[0].0 > sizes[0].1, "the sheet must be landscape: {:?}", sizes[0]);
        assert!((sizes[0].0 - 842.0).abs() < 1.0);
        assert!((sizes[0].1 - 595.0).abs() < 1.0);
    }

    #[test]
    fn four_up_keeps_the_sheet_in_the_pages_own_orientation() {
        let pdf = two_page_pdf();
        let out = impose(&pdf, &[0, 1], Imposition::FourUp).unwrap();
        let sizes = sheet_sizes(&out);
        assert_eq!(sizes.len(), 1);
        assert!(sizes[0].1 > sizes[0].0, "a 2×2 grid stays portrait: {:?}", sizes[0]);
    }

    #[test]
    fn a_booklet_of_two_pages_still_makes_a_whole_folded_sheet() {
        // Two pages pad to four slots, which is one folded sheet — two sides.
        let pdf = two_page_pdf();
        let out = impose(&pdf, &[0, 1], Imposition::Booklet).unwrap();
        assert_eq!(sheet_sizes(&out).len(), 2);
    }

    #[test]
    fn imposed_output_stays_a_loadable_pdf_with_vector_content() {
        let pdf = two_page_pdf();
        let out = impose(&pdf, &[0, 1], Imposition::TwoUp).unwrap();
        assert!(out.starts_with(b"%PDF-"), "output must be a PDF");
        let doc = Document::load_mem(&out).unwrap();
        let page = *doc.get_pages().values().next().unwrap();
        let content = String::from_utf8_lossy(&doc.get_page_content(page)).to_string();
        assert!(content.contains("Do"), "each slot draws its page as a form: {content}");
        assert_eq!(content.matches("Do").count(), 2, "both pages must be drawn");
        // The text operators live on in the forms rather than being rasterised.
        let has_text = doc
            .objects
            .values()
            .filter_map(|o| o.as_stream().ok())
            .any(|s| String::from_utf8_lossy(&s.content).contains("Tj"));
        assert!(has_text, "page content must survive as vector operators");
    }

    #[test]
    fn a_real_typst_pdf_imposes_into_a_booklet() {
        // The hand-built PDF above is simpler than anything Typst emits — no
        // fonts, no compression, no resource inheritance. This runs the real
        // compiler output through the same path, which is where a wrong
        // assumption about the page tree would actually show up.
        let path = std::env::temp_dir()
            .join(format!("zerkalo-imposition-test-{}.typ", std::process::id()));
        std::fs::write(
            &path,
            "#set page(width: 148mm, height: 210mm)\n\
             #for i in range(6) [ = Section #i \n Body text. #pagebreak(weak: true) ]",
        )
        .unwrap();
        let pdf = crate::compiler::compile_to_pdf_bytes(
            &path,
            &std::collections::HashMap::new(),
            &std::collections::HashMap::new(),
            None,
        )
        .expect("the fixture should compile");
        std::fs::remove_file(&path).ok();

        let page_count = Document::load_mem(&pdf).unwrap().get_pages().len();
        assert!(page_count >= 2, "the fixture should produce several pages, got {page_count}");

        let pages: Vec<usize> = (0..page_count).collect();
        let out = impose(&pdf, &pages, Imposition::Booklet).expect("a real PDF should impose");

        let sizes = sheet_sizes(&out);
        assert_eq!(
            sizes.len(),
            page_count.div_ceil(4) * 2,
            "a booklet rounds up to whole folded sheets, two sides each"
        );
        for (w, h) in &sizes {
            assert!(w > h, "booklet sheets are landscape: {w} × {h}");
        }

        // Every slot on the first sheet must actually draw something; an empty
        // content stream would print blank pages and look like a driver fault.
        let doc = Document::load_mem(&out).unwrap();
        let first = *doc.get_pages().values().next().unwrap();
        let content = String::from_utf8_lossy(&doc.get_page_content(first)).to_string();
        assert!(content.contains("Do"), "the first sheet must draw its pages: {content}");
    }

    #[test]
    fn out_of_range_pages_are_rejected() {
        let pdf = two_page_pdf();
        assert!(impose(&pdf, &[0, 5], Imposition::Off).is_err());
    }

    #[test]
    fn garbage_input_is_an_error_not_a_panic() {
        assert!(impose(b"not a pdf at all", &[0], Imposition::Off).is_err());
    }
}
