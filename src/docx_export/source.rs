//! Preparing a document's source for the Word export's HTML compile.

use typst::syntax::ast::{self, AstNode};
use typst::syntax::{SyntaxKind, SyntaxNode};

/// What a document's own `#show heading` rule did to a heading's appearance, read off
/// the rule's source before the rule is removed (see [`prepare`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HeadingLook {
    pub center: bool,
    pub right: bool,
    pub italic: bool,
    pub bold: bool,
    pub upper: bool,
    pub smallcaps: bool,
    /// `size:` given in the rule, in points or as a multiple of the body size.
    pub size: Option<Size>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Size {
    Pt(f64),
    Em(f64),
}

/// Heading looks by level (1-based); `None` key means the rule named no level.
pub type HeadingLooks = Vec<(Option<u8>, HeadingLook)>;

/// Appended to the document so the body's resolved style comes back out of the HTML
/// compile as a `<meta name="zk-style">` — the export mirrors whatever the document's
/// own set rules produced, however they were written.
const STYLE_PROBE: &str = r#"
#context {
  let pt(l) = if type(l) == length { calc.round(l.to-absolute() / 1pt, digits: 2) } else { 0 }
  let fli = par.first-line-indent
  let fli = if type(fli) == dictionary { fli.at("amount", default: 0pt) } else { fli }
  let fonts = if type(text.font) == array { text.font } else { (text.font,) }
  let fam = fonts.map(f => if type(f) == dictionary { f.at("name", default: "") } else { str(f) })
  html.elem("meta", attrs: (name: "zk-style", content: (
    "font=" + fam.join("|"),
    "size=" + str(pt(text.size)),
    "leading=" + str(pt(par.leading)),
    "spacing=" + str(pt(par.spacing)),
    "indent=" + str(pt(fli)),
    "justify=" + repr(par.justify),
    "lang=" + text.lang,
    "pw=" + str(pt(page.width)),
    "ph=" + str(pt(page.height)),
  ).join(";")))
}
"#;

/// The document's source with its top-level `#show heading` rules removed and the
/// style probe appended, plus the looks those rules applied. The rules go because a
/// heading show rule replaces the heading with styled blocks, and the HTML compile
/// would then carry no heading at all — Word/InDesign need real headings, with the
/// look moved into the Heading styles instead.
pub fn prepare(content: &str) -> (String, HeadingLooks) {
    let root = typst::syntax::parse(content);
    let mut remove: Vec<(usize, usize)> = Vec::new();
    let mut looks = HeadingLooks::new();

    let mut offset = 0;
    let mut hash_at: Option<usize> = None;
    for child in root.children() {
        let len = child.len();
        match child.kind() {
            SyntaxKind::Hash => hash_at = Some(offset),
            SyntaxKind::ShowRule => {
                if let Some(level) = heading_rule_level(child) {
                    let text = &content[offset..offset + len];
                    looks.push((level, read_look(text)));
                    remove.push((hash_at.unwrap_or(offset), offset + len));
                }
                hash_at = None;
            }
            _ => hash_at = None,
        }
        offset += len;
    }

    let mut out = String::with_capacity(content.len() + STYLE_PROBE.len());
    let mut last = 0;
    for (start, end) in remove {
        out.push_str(&content[last..start]);
        last = end;
    }
    out.push_str(&content[last..]);
    out.push_str(STYLE_PROBE);
    (out, looks)
}

/// `Some(level)` for a show rule whose selector is `heading` or
/// `heading.where(level: N)`; `Some(None)` for a bare `heading`.
fn heading_rule_level(node: &SyntaxNode) -> Option<Option<u8>> {
    let rule = node.cast::<ast::ShowRule>()?;
    let selector = rule.selector()?.to_untyped().full_text();
    let selector = selector.trim();
    if !selector.starts_with("heading") {
        return None;
    }
    let level = selector.split("level:").nth(1).and_then(|rest| {
        rest.trim_start()
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .ok()
    });
    Some(level)
}

fn read_look(rule: &str) -> HeadingLook {
    let compact: String = rule.chars().filter(|c| !c.is_whitespace()).collect();
    HeadingLook {
        center: compact.contains("align(center") || compact.contains("align:center"),
        right: compact.contains("align(right") || compact.contains("align:right"),
        italic: compact.contains("style:\"italic\"") || compact.contains("emph("),
        bold: compact.contains("weight:\"bold\"") || compact.contains("strong("),
        upper: compact.contains("upper("),
        smallcaps: compact.contains("smallcaps("),
        size: compact.split("size:").nth(1).and_then(parse_size),
    }
}

fn parse_size(s: &str) -> Option<Size> {
    let num: String = s
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let value: f64 = num.parse().ok()?;
    let unit = &s[num.len()..];
    if unit.starts_with("pt") {
        Some(Size::Pt(value))
    } else if unit.starts_with("em") {
        Some(Size::Em(value))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_heading_show_rules_and_reads_their_look() {
        let doc = "#set text(size: 12pt)\n\
                   #show heading.where(level: 1): it => block[\n  #align(center)[#text(weight: \"bold\")[#it.body]]\n]\n\
                   #show heading.where(level: 2): it => text(style: \"italic\", size: 1.1em, it.body)\n\
                   #show link: underline\n\
                   = Title\nBody.\n";
        let (out, looks) = prepare(doc);
        assert!(!out.contains("show heading"), "{out}");
        assert!(out.contains("#show link: underline"));
        assert!(out.contains("= Title\nBody."));
        assert_eq!(looks.len(), 2);
        assert_eq!(looks[0].0, Some(1));
        assert!(looks[0].1.center && looks[0].1.bold && !looks[0].1.italic);
        assert_eq!(looks[1].0, Some(2));
        assert!(looks[1].1.italic);
        assert_eq!(looks[1].1.size, Some(Size::Em(1.1)));
    }

    #[test]
    fn leaves_nested_and_non_heading_rules_alone() {
        let doc = "#let f(body) = { show heading: set text(red); body }\n#show strong: emph\n";
        let (out, looks) = prepare(doc);
        assert!(out.starts_with(doc));
        assert!(looks.is_empty());
    }
}
