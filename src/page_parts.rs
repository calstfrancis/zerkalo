//! Which pages of a compiled document are body text, and which are the title
//! page, contents, bibliography or appendices — so the preview can show the
//! page count a page limit actually means.
//!
//! A page shared between the body and something else counts as body: page
//! limits are read that way, and it keeps the count from undershooting.

use std::collections::HashSet;

use typst::foundations::{Content, Label, NativeElement, Selector};
use typst::introspection::{Introspector, Location, Tag};
use typst::layout::{Frame, FrameItem, Point};
use typst::math::EquationElem;
use typst::model::{
    BibliographyElem, EnumElem, FigureElem, HeadingElem, ListElem, OutlineElem, ParElem, QuoteElem,
    TableElem, TermsElem,
};
use typst::text::RawElem;
use typst::utils::PicoStr;
use typst::visualize::ImageElem;
use typst_layout::PagedDocument;

/// Put on anything to say the body starts there; the pages before it are the
/// title page. Generated documents are found without it.
pub const BODY_START_LABEL: &str = "body-start";
/// Put on anything to say the appendices start there, for appendices whose
/// headings don't begin with "Appendix".
pub const APPENDICES_LABEL: &str = "appendices";
/// A line near the top of a document that sets its own page limit.
pub const LIMIT_COMMENT: &str = "// @zerkalo-page-limit:";

const REFERENCE_HEADINGS: [&str; 6] = [
    "references",
    "bibliography",
    "works cited",
    "works consulted",
    "sources cited",
    "literature cited",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PageParts {
    pub total: usize,
    pub title: usize,
    pub contents: usize,
    pub bibliography: usize,
    pub appendices: usize,
    /// The document's own `// @zerkalo-page-limit:` line, if it has one.
    pub own_limit: Option<u32>,
}

impl PageParts {
    pub fn body(&self) -> usize {
        self.total
            .saturating_sub(self.title + self.contents + self.bibliography + self.appendices)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Body,
    Title,
    Contents,
    Bibliography,
    Appendices,
}

/// Sort the pages of `doc`. `title_end_hint` is the first page (0-based) after
/// a generated title page, when the source shows one.
pub fn analyze(doc: &PagedDocument, title_end_hint: Option<usize>) -> PageParts {
    let pages = doc.pages();
    let total = pages.len();
    let mut parts = vec![Part::Body; total];
    if total == 0 {
        return PageParts::default();
    }
    let intro = doc.introspector();
    let page_of = |loc: Location| intro.page(loc).map(|p| p.get() - 1);

    let tags: Vec<Vec<PageTag>> = pages.iter().map(|p| page_tags(&p.frame)).collect();
    let mut body_elems = HashSet::new();
    let mut headings = HashSet::new();
    for page in &tags {
        for t in page {
            if let Tag::Start(elem, _) = &t.tag {
                if let Some(loc) = elem.location() {
                    if is_body_kind(elem) {
                        body_elems.insert(loc);
                    }
                    if elem.is::<HeadingElem>() {
                        headings.insert(loc);
                    }
                }
            }
        }
    }
    let ctx = Ctx {
        tags: &tags,
        body_elems: &body_elems,
        headings: &headings,
    };

    // Back matter: the earliest of a bibliography, a references heading, or
    // the appendices, runs to the end.
    let mut back: Vec<(usize, Part)> = Vec::new();
    let mut push_back = |loc: Location, part: Part| {
        if let Some(page) = page_of(loc) {
            let start = if ctx.shared_before(page, loc) {
                page + 1
            } else {
                page
            };
            back.push((start, part));
        }
    };
    for bib in intro.query(&BibliographyElem::ELEM.select()) {
        if let Some(loc) = bib.location() {
            push_back(loc, Part::Bibliography);
        }
    }
    for h in intro.query(&HeadingElem::ELEM.select()) {
        let (Some(loc), Some(packed)) = (h.location(), h.to_packed::<HeadingElem>()) else {
            continue;
        };
        let text = packed.body.plain_text().trim().to_lowercase();
        if REFERENCE_HEADINGS.contains(&text.as_str()) {
            push_back(loc, Part::Bibliography);
        } else if text.starts_with("appendix") || text.starts_with("appendices") {
            push_back(loc, Part::Appendices);
        }
    }
    for elem in query_label(intro.as_ref(), APPENDICES_LABEL) {
        if let Some(loc) = elem.location() {
            push_back(loc, Part::Appendices);
        }
    }
    back.sort_by_key(|(start, _)| *start);
    let back_start = back.first().map_or(total, |(s, _)| *s);

    let title_end = query_label(intro.as_ref(), BODY_START_LABEL)
        .first()
        .and_then(|e| e.location())
        .and_then(page_of)
        .or(title_end_hint)
        .unwrap_or(0)
        .min(back_start);
    for p in parts.iter_mut().take(title_end) {
        *p = Part::Title;
    }

    for outline in intro.query(&OutlineElem::ELEM.select()) {
        let Some(loc) = outline.location() else {
            continue;
        };
        let Some(first) = page_of(loc) else { continue };
        if first >= back_start {
            continue;
        }
        let last = ctx.end_page(loc).unwrap_or(first).max(first);
        let from = if ctx.shared_before(first, loc) {
            first + 1
        } else {
            first
        };
        let to = if ctx.shared_after(last, loc) {
            last
        } else {
            last + 1
        };
        for p in parts.iter_mut().take(to.min(back_start)).skip(from) {
            *p = Part::Contents;
        }
    }

    for (i, (start, part)) in back.iter().enumerate() {
        let end = back.get(i + 1).map_or(total, |(s, _)| *s);
        for p in parts.iter_mut().take(end.min(total)).skip(*start) {
            *p = *part;
        }
    }

    let count = |want: Part| parts.iter().filter(|p| **p == want).count();
    PageParts {
        total,
        title: count(Part::Title),
        contents: count(Part::Contents),
        bibliography: count(Part::Bibliography),
        appendices: count(Part::Appendices),
        own_limit: None,
    }
}

fn query_label(intro: &dyn Introspector, name: &str) -> Vec<Content> {
    Label::new(PicoStr::intern(name))
        .map(|l| intro.query(&Selector::Label(l)).into_iter().collect())
        .unwrap_or_default()
}

fn is_body_kind(elem: &Content) -> bool {
    elem.is::<ParElem>()
        || elem.is::<HeadingElem>()
        || elem.is::<FigureElem>()
        || elem.is::<ListElem>()
        || elem.is::<EnumElem>()
        || elem.is::<TermsElem>()
        || elem.is::<TableElem>()
        || elem.is::<EquationElem>()
        || elem.is::<QuoteElem>()
        || elem.is::<RawElem>()
        || elem.is::<ImageElem>()
}

struct PageTag {
    tag: Tag,
    /// Pushed at the page's origin by Typst for elements between page runs,
    /// not laid out on this page — a paragraph's end can land here.
    carried: bool,
}

fn page_tags(frame: &Frame) -> Vec<PageTag> {
    let mut out = Vec::new();
    for (pos, item) in frame.items() {
        match item {
            FrameItem::Tag(tag) => out.push(PageTag {
                tag: tag.clone(),
                carried: *pos == Point::zero(),
            }),
            FrameItem::Group(g) => collect_tags(&g.frame, &mut out),
            _ => {}
        }
    }
    out
}

fn collect_tags(frame: &Frame, out: &mut Vec<PageTag>) {
    for (_, item) in frame.items() {
        match item {
            FrameItem::Tag(tag) => out.push(PageTag {
                tag: tag.clone(),
                carried: false,
            }),
            FrameItem::Group(g) => collect_tags(&g.frame, out),
            _ => {}
        }
    }
}

struct Ctx<'a> {
    tags: &'a [Vec<PageTag>],
    body_elems: &'a HashSet<Location>,
    headings: &'a HashSet<Location>,
}

impl Ctx<'_> {
    /// Whether body content comes before `loc` on `page`. A heading right
    /// before it is taken as its own title (`= References` above a
    /// `#bibliography(title: none)`), not as body.
    fn shared_before(&self, page: usize, loc: Location) -> bool {
        let Some(tags) = self.tags.get(page) else {
            return false;
        };
        let mut before: Vec<Location> = Vec::new();
        for t in tags {
            if matches!(&t.tag, Tag::Start(..)) && t.tag.location() == loc {
                break;
            }
            let l = t.tag.location();
            if !t.carried && self.body_elems.contains(&l) {
                before.push(l);
            }
        }
        if let Some(&last) = before.last() {
            if self.headings.contains(&last) {
                before.retain(|l| *l != last);
            }
        }
        !before.is_empty()
    }

    /// Whether body content starts after `loc` ends on `page`.
    fn shared_after(&self, page: usize, loc: Location) -> bool {
        let Some(tags) = self.tags.get(page) else {
            return false;
        };
        let mut ended = false;
        for t in tags {
            match &t.tag {
                Tag::End(l, ..) if *l == loc => ended = true,
                Tag::Start(elem, _) if ended && !t.carried && is_body_kind(elem) => return true,
                _ => {}
            }
        }
        false
    }

    fn end_page(&self, loc: Location) -> Option<usize> {
        self.tags.iter().position(|page| {
            page.iter()
                .any(|t| matches!(t.tag, Tag::End(l, ..) if l == loc))
        })
    }
}

/// The `// @zerkalo-page-limit: N` line near the top of `text`.
pub fn parse_limit_comment(text: &str) -> Option<u32> {
    text.lines().take(20).find_map(|line| {
        line.trim()
            .strip_prefix(LIMIT_COMMENT)
            .and_then(|rest| rest.trim().parse::<u32>().ok())
            .filter(|n| *n > 0)
    })
}

/// Where a generated title page ends: the byte offset just past the
/// `#pagebreak()` that closes the title block, when the block has one. Styles
/// with no separate title page (MLA, IEEE) reach the next section first.
pub fn generated_title_end(text: &str) -> Option<usize> {
    let mut offset = 0;
    let mut in_title = false;
    for line in text.split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        let l = line.trim_end();
        if !in_title {
            in_title = l.starts_with("// ── Title block");
            continue;
        }
        if l.starts_with("// ──") || l.starts_with("#outline") || l.starts_with('=') {
            return None;
        }
        if l.starts_with("#pagebreak(") {
            return Some(start + line.len());
        }
    }
    None
}

/// The text, tooltip and over-the-limit flag for the preview's page counter.
pub struct Indicator {
    pub text: String,
    pub tooltip: String,
    pub over: bool,
}

pub fn indicator(current: usize, parts: Option<&PageParts>, default_limit: u32) -> Indicator {
    let total = parts.map_or(0, |p| p.total);
    let mut text = format!("{} / {}", current + 1, total);
    let Some(parts) = parts else {
        return Indicator {
            text,
            tooltip: String::new(),
            over: false,
        };
    };
    let body = parts.body();
    let limit = parts.own_limit.unwrap_or(default_limit);
    if limit > 0 {
        text.push_str(&format!(" · {body} of {limit} body"));
    } else if body != total {
        text.push_str(&format!(" · {body} body"));
    }

    let plural =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let mut pieces = Vec::new();
    if parts.title > 0 {
        pieces.push(plural(parts.title, "title page", "title pages"));
    }
    if parts.contents > 0 {
        pieces.push(plural(parts.contents, "contents page", "contents pages"));
    }
    pieces.push(format!("{body} body"));
    if parts.bibliography > 0 {
        pieces.push(plural(
            parts.bibliography,
            "reference page",
            "reference pages",
        ));
    }
    if parts.appendices > 0 {
        pieces.push(plural(parts.appendices, "appendix page", "appendix pages"));
    }
    let mut tooltip = format!("{}: {}", plural(total, "page", "pages"), pieces.join(", "));
    let limit_body = body as u32;
    if limit > 0 {
        let line = match limit_body.cmp(&limit) {
            std::cmp::Ordering::Less => {
                let spare = (limit - limit_body) as usize;
                format!("{} to spare", plural(spare, "page", "pages"))
            }
            std::cmp::Ordering::Equal => "right at the limit".to_string(),
            std::cmp::Ordering::Greater => {
                let over = (limit_body - limit) as usize;
                format!("{} over", plural(over, "page", "pages"))
            }
        };
        tooltip.push_str(&format!("\nPage limit {limit}: {line}"));
    }
    Indicator {
        text,
        tooltip,
        over: limit > 0 && limit_body > limit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(src: &str) -> PageParts {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.typ");
        std::fs::write(&path, src).unwrap();
        let (_, _, sync) = crate::compiler::compile_to_rgba_pages(
            &path,
            0.1,
            &Default::default(),
            &Default::default(),
            None,
        )
        .unwrap();
        sync.page_parts(&path)
    }

    const PAGE: &str = "#lorem(500)\n\n";

    #[test]
    fn a_plain_document_is_all_body() {
        let p = parts(&format!("= Intro\n{PAGE}{PAGE}"));
        assert!(p.total >= 2);
        assert_eq!(p.body(), p.total);
    }

    #[test]
    fn the_title_page_contents_and_references_are_left_out() {
        let src = format!(
            "// ── Title block ──\n#align(center)[Title]\n#pagebreak()\n\
             #outline()\n#pagebreak()\n\
             = Intro\n{PAGE}= Method\n{PAGE}\
             #pagebreak()\n= References\nA. Author. _Book_. 2020.\n"
        );
        let p = parts(&src);
        assert_eq!(p.title, 1);
        assert_eq!(p.contents, 1);
        assert_eq!(p.bibliography, 1);
        assert_eq!(p.body(), p.total - 3);
    }

    #[test]
    fn a_reference_list_starting_mid_page_leaves_that_page_in_the_body() {
        let p = parts("= Intro\nA short essay.\n\n= References\nA. Author. _Book_. 2020.\n");
        assert_eq!(p.total, 1);
        assert_eq!(p.bibliography, 0);
        assert_eq!(p.body(), 1);
    }

    #[test]
    fn a_page_split_paragraph_ending_above_the_references_counts_as_body() {
        let src = "= Intro\n#lorem(700)\n\n= References\nA. Author. _Book_. 2020.\n";
        let p = parts(src);
        assert!(p.total >= 2);
        assert_eq!(p.bibliography, 0, "{p:?}");
    }

    #[test]
    fn appendices_after_the_references_are_left_out() {
        let src = format!(
            "= Intro\n{PAGE}#pagebreak()\n= References\nA. Author.\n#pagebreak()\n\
             = Appendix A: Survey\n{PAGE}"
        );
        let p = parts(&src);
        assert_eq!(p.bibliography, 1);
        assert!(p.appendices >= 1);
        assert_eq!(p.body() + p.bibliography + p.appendices, p.total);
    }

    #[test]
    fn the_body_start_label_marks_a_hand_made_title_page() {
        let src = format!("#align(center)[My Title]\n#pagebreak()\n= Intro <body-start>\n{PAGE}");
        let p = parts(&src);
        assert_eq!(p.title, 1);
    }

    #[test]
    fn a_title_block_without_its_own_page_is_not_a_title_page() {
        let src = "// ── Title block ──\n#align(center)[Title]\n\n// ── Document body ──\n= Intro\nText\n#pagebreak()\nMore\n";
        assert_eq!(generated_title_end(src), None);
    }

    #[test]
    fn the_page_limit_line_is_read_from_the_top() {
        assert_eq!(
            parse_limit_comment("// @zerkalo-page-limit: 10\n= A\n"),
            Some(10)
        );
        assert_eq!(parse_limit_comment("// @zerkalo-page-limit: none\n"), None);
        assert_eq!(parse_limit_comment("= A\n"), None);
    }

    #[test]
    fn the_counter_names_the_limit_and_flags_going_over() {
        let p = PageParts {
            total: 12,
            title: 1,
            bibliography: 2,
            ..Default::default()
        };
        let i = indicator(2, Some(&p), 0);
        assert_eq!(i.text, "3 / 12 · 9 body");
        assert!(!i.over);
        let i = indicator(0, Some(&p), 8);
        assert_eq!(i.text, "1 / 12 · 9 of 8 body");
        assert!(i.over);
        assert!(i.tooltip.contains("1 page over"), "{}", i.tooltip);
        let all_body = PageParts {
            total: 4,
            ..Default::default()
        };
        assert_eq!(indicator(0, Some(&all_body), 0).text, "1 / 4");
    }
}
