//! Typst's HTML output → a small paragraph/run model that `package` writes as OOXML.
//!
//! Everything semantic Typst already worked out is taken as-is: citations arrive
//! formatted by the same engine as the PDF, every note (citation or `#footnote`)
//! arrives as a `doc-noteref` marker pointing into the `doc-endnotes` section, and the
//! bibliography arrives as its own section.

use std::collections::HashMap;

use base64::Engine as _;

use super::html::{Element, Node};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PStyle {
    Title,
    Heading(u8),
    Body,
    FirstBody,
    Quote,
    Caption,
    Bibliography,
    ListItem,
    ListContinue,
    Figure,
    NoteText,
    NotesHeading,
    Centered,
    Right,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Fmt {
    pub italic: bool,
    pub bold: bool,
    pub sup: bool,
    pub sub: bool,
    pub smallcaps: bool,
    pub underline: bool,
    pub strike: bool,
    pub code: bool,
    pub link: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum Run {
    Text(String, Fmt),
    NoteRef(usize),
    Break,
    Image(usize),
}

#[derive(Debug, Clone)]
pub struct Para {
    pub style: PStyle,
    pub runs: Vec<Run>,
    /// (numbering instance, level) for a list item's first paragraph.
    pub list: Option<(u32, u8)>,
}

#[derive(Debug, Clone)]
pub struct Cell {
    pub paras: Vec<Para>,
    pub header: bool,
    pub colspan: u32,
}

#[derive(Debug, Clone)]
pub enum Block {
    Para(Para),
    Table(Vec<Vec<Cell>>),
}

#[derive(Debug, Clone)]
pub struct Note {
    /// The note's number as Typst displayed it.
    pub number: String,
    pub paras: Vec<Para>,
}

#[derive(Debug, Clone)]
pub struct Image {
    pub data: Vec<u8>,
    pub ext: &'static str,
    pub width_pt: f64,
    pub height_pt: f64,
}

/// A numbering instance: bullets, or a decimal list restarting at 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ListKind {
    Bullet,
    Decimal,
}

/// The body's resolved style, from the `zk-style` probe (see `source::STYLE_PROBE`).
#[derive(Debug, Clone)]
pub struct BodyStyle {
    pub font: String,
    pub size: f64,
    pub leading: f64,
    pub spacing: f64,
    pub indent: f64,
    pub justify: bool,
    pub lang: String,
    pub page_width: f64,
    pub page_height: f64,
}

impl Default for BodyStyle {
    fn default() -> Self {
        BodyStyle {
            font: "Libertinus Serif".into(),
            size: 11.0,
            leading: 0.65 * 11.0,
            spacing: 1.2 * 11.0,
            indent: 0.0,
            justify: false,
            lang: "en".into(),
            page_width: 595.28,
            page_height: 841.89,
        }
    }
}

#[derive(Debug, Default)]
pub struct Doc {
    pub title: Option<String>,
    pub style: BodyStyle,
    pub blocks: Vec<Block>,
    pub notes: Vec<Note>,
    pub images: Vec<Image>,
    pub links: Vec<String>,
    pub lists: Vec<ListKind>,
    pub warnings: Vec<String>,
}

pub fn convert(root: &Element) -> Doc {
    let mut cx = Cx {
        doc: Doc::default(),
        note_bodies: HashMap::new(),
        note_index: HashMap::new(),
        prev_was_para: false,
        unsupported: 0,
        dropped_images: 0,
    };
    let mut probe = None;
    find_meta(root, &mut probe);
    if let Some(p) = probe {
        cx.doc.style = parse_probe(&p);
    }
    cx.doc.title = find(root, &|e| e.tag == "title")
        .map(|t| t.text().trim().to_string())
        .filter(|t| !t.is_empty());
    if let Some(section) = find(root, &|e| e.attr("role") == Some("doc-endnotes")) {
        cx.collect_note_bodies(section);
    }
    let body = find(root, &|e| e.tag == "body").unwrap_or(root);
    let mut blocks = Vec::new();
    cx.blocks(&body.children, PStyle::Body, &mut blocks);
    cx.doc.blocks = blocks;
    if cx.unsupported > 0 {
        cx.doc.warnings.push(format!(
            "{} piece(s) of math or drawing were converted to pictures",
            cx.unsupported
        ));
    }
    if cx.dropped_images > 0 {
        cx.doc.warnings.push(format!(
            "{} image(s) in a format Word can't hold were left out",
            cx.dropped_images
        ));
    }
    cx.doc
}

struct Cx {
    doc: Doc,
    /// Endnote-section entries by id, not yet numbered into `doc.notes`.
    note_bodies: HashMap<String, (String, Vec<Node>)>,
    /// Endnote id → index in `doc.notes`, assigned on first reference.
    note_index: HashMap<String, usize>,
    prev_was_para: bool,
    unsupported: usize,
    dropped_images: usize,
}

fn find<'a>(e: &'a Element, pred: &dyn Fn(&Element) -> bool) -> Option<&'a Element> {
    if pred(e) {
        return Some(e);
    }
    e.children.iter().find_map(|c| match c {
        Node::Element(child) => find(child, pred),
        Node::Text(_) => None,
    })
}

fn find_meta(e: &Element, out: &mut Option<String>) {
    if e.tag == "meta" && e.attr("name") == Some("zk-style") {
        *out = e.attr("content").map(str::to_string);
    }
    for c in &e.children {
        if let Node::Element(child) = c {
            find_meta(child, out);
        }
    }
}

fn parse_probe(probe: &str) -> BodyStyle {
    let mut s = BodyStyle::default();
    for pair in probe.split(';') {
        let Some((k, v)) = pair.split_once('=') else {
            continue;
        };
        let num = v.parse::<f64>().ok().filter(|n| *n > 0.0);
        match k {
            "font" => {
                if let Some(first) = v.split('|').find(|f| !f.is_empty()) {
                    s.font = first.to_string();
                }
            }
            "size" => s.size = num.unwrap_or(s.size),
            "leading" => s.leading = num.unwrap_or(s.leading),
            "spacing" => s.spacing = num.unwrap_or(s.spacing),
            "indent" => s.indent = v.parse().unwrap_or(0.0),
            "justify" => s.justify = v == "true",
            "lang" if !v.is_empty() => s.lang = v.to_string(),
            "pw" => s.page_width = num.unwrap_or(s.page_width),
            "ph" => s.page_height = num.unwrap_or(s.page_height),
            _ => {}
        }
    }
    s
}

const BLOCK_TAGS: &[&str] = &[
    "p",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "blockquote",
    "ul",
    "ol",
    "table",
    "figure",
    "div",
    "section",
    "article",
    "main",
    "header",
    "footer",
    "aside",
    "nav",
    "hr",
    "pre",
];

fn is_block(node: &Node) -> bool {
    matches!(node, Node::Element(e) if BLOCK_TAGS.contains(&e.tag.as_str()))
}

impl Cx {
    fn collect_note_bodies(&mut self, section: &Element) {
        let mut items = Vec::new();
        gather(section, "li", &mut items);
        for li in items {
            let Some(id) = li.attr("id") else { continue };
            let mut number = String::new();
            let mut children = Vec::new();
            for c in &li.children {
                match c {
                    Node::Element(e) if e.attr("role") == Some("doc-backlink") => {
                        number = e.text().trim().to_string();
                    }
                    other => children.push(other.clone()),
                }
            }
            self.note_bodies.insert(id.to_string(), (number, children));
        }
    }

    fn blocks(&mut self, nodes: &[Node], style: PStyle, out: &mut Vec<Block>) {
        let mut pending: Vec<Node> = Vec::new();
        for node in nodes {
            if is_block(node) {
                self.flush_inline(&mut pending, style, out);
                if let Node::Element(e) = node {
                    self.block(e, style, out);
                }
            } else if let Node::Element(e) = node {
                match e.tag.as_str() {
                    "meta" | "title" | "script" | "style" | "link" | "head" => {}
                    "img" | "svg" if pending.iter().all(is_blank) => {
                        let mut runs = Vec::new();
                        self.inline(std::slice::from_ref(node), Fmt::default(), &mut runs);
                        self.push_para(PStyle::Figure, runs, None, out);
                    }
                    _ => pending.push(node.clone()),
                }
            } else {
                pending.push(node.clone());
            }
        }
        self.flush_inline(&mut pending, style, out);
    }

    fn flush_inline(&mut self, pending: &mut Vec<Node>, style: PStyle, out: &mut Vec<Block>) {
        if pending.iter().all(is_blank) {
            pending.clear();
            return;
        }
        let mut runs = Vec::new();
        self.inline(pending, Fmt::default(), &mut runs);
        pending.clear();
        let style = self.body_style(style);
        self.push_para(style, runs, None, out);
    }

    /// `Body` becomes `FirstBody` unless the previous block was a body paragraph —
    /// the same rule Typst uses for which paragraphs get a first-line indent.
    fn body_style(&self, style: PStyle) -> PStyle {
        if style == PStyle::Body && !self.prev_was_para {
            PStyle::FirstBody
        } else {
            style
        }
    }

    fn push_para(
        &mut self,
        style: PStyle,
        runs: Vec<Run>,
        list: Option<(u32, u8)>,
        out: &mut Vec<Block>,
    ) {
        let runs = trim_runs(runs);
        if runs.is_empty() && list.is_none() {
            return;
        }
        self.prev_was_para = matches!(style, PStyle::Body | PStyle::FirstBody);
        out.push(Block::Para(Para { style, runs, list }));
    }

    fn block(&mut self, e: &Element, style: PStyle, out: &mut Vec<Block>) {
        match e.tag.as_str() {
            "h1" => {
                let mut runs = Vec::new();
                self.inline(&e.children, Fmt::default(), &mut runs);
                self.push_para(PStyle::Title, runs, None, out);
            }
            h if h.len() == 2 && h.starts_with('h') => {
                let level = h[1..].parse::<u8>().unwrap_or(2).saturating_sub(1).max(1);
                let mut runs = Vec::new();
                self.inline(&e.children, Fmt::default(), &mut runs);
                self.push_para(PStyle::Heading(level), runs, None, out);
            }
            "p" => {
                let mut runs = Vec::new();
                self.inline(&e.children, Fmt::default(), &mut runs);
                let style = self.body_style(style);
                self.push_para(style, runs, None, out);
            }
            "pre" => {
                let text = e.text();
                for line in text.trim_end_matches('\n').lines() {
                    let fmt = Fmt {
                        code: true,
                        ..Fmt::default()
                    };
                    self.push_para(style, vec![Run::Text(line.to_string(), fmt)], None, out);
                }
            }
            "blockquote" => {
                if e.children.iter().any(is_block) {
                    self.blocks(&e.children, PStyle::Quote, out);
                } else {
                    let mut runs = Vec::new();
                    self.inline(&e.children, Fmt::default(), &mut runs);
                    self.push_para(PStyle::Quote, runs, None, out);
                }
                self.prev_was_para = false;
            }
            "ul" | "ol" => {
                self.list(e, 0, out);
                self.prev_was_para = false;
            }
            "table" => {
                let rows = self.table(e);
                if !rows.is_empty() {
                    out.push(Block::Table(rows));
                }
                self.prev_was_para = false;
            }
            "figure" => {
                for c in &e.children {
                    match c {
                        Node::Element(cap) if cap.tag == "figcaption" => {
                            let mut runs = Vec::new();
                            self.inline(&cap.children, Fmt::default(), &mut runs);
                            self.push_para(PStyle::Caption, runs, None, out);
                        }
                        Node::Element(inner) if is_block(c) => {
                            self.block(inner, PStyle::Figure, out)
                        }
                        other => {
                            let mut runs = Vec::new();
                            self.inline(std::slice::from_ref(other), Fmt::default(), &mut runs);
                            self.push_para(PStyle::Figure, runs, None, out);
                        }
                    }
                }
                self.prev_was_para = false;
            }
            "section" if e.attr("role") == Some("doc-endnotes") => {}
            "section" if e.attr("role") == Some("doc-bibliography") => {
                for c in &e.children {
                    match c {
                        Node::Element(h) if h.tag.len() == 2 && h.tag.starts_with('h') => {
                            self.block(h, style, out)
                        }
                        Node::Element(list) if list.tag == "ul" || list.tag == "ol" => {
                            for item in &list.children {
                                if let Node::Element(li) = item {
                                    let mut runs = Vec::new();
                                    self.inline(&li.children, Fmt::default(), &mut runs);
                                    self.push_para(PStyle::Bibliography, runs, None, out);
                                }
                            }
                        }
                        Node::Element(other) => self.block(other, PStyle::Bibliography, out),
                        Node::Text(_) => {}
                    }
                }
                self.prev_was_para = false;
            }
            "hr" => {}
            // A bibliography kept only so citations resolve (see `compiler::HiddenBib`).
            "div" if e.attr("hidden").is_some() => {}
            "div" if matches!(e.attr("class"), Some("zk-center") | Some("zk-right")) => {
                let aligned = if e.attr("class") == Some("zk-center") {
                    PStyle::Centered
                } else {
                    PStyle::Right
                };
                // Headings and other block kinds inside keep their own style; only
                // what would have been body text takes the alignment.
                let mut inner = Vec::new();
                self.blocks(&e.children, aligned, &mut inner);
                for b in inner {
                    match b {
                        Block::Para(mut p)
                            if matches!(p.style, PStyle::Body | PStyle::FirstBody) =>
                        {
                            p.style = aligned;
                            out.push(Block::Para(p));
                        }
                        other => out.push(other),
                    }
                }
                self.prev_was_para = false;
            }
            _ => self.blocks(&e.children, style, out),
        }
    }

    fn list(&mut self, e: &Element, level: u8, out: &mut Vec<Block>) {
        let kind = if e.tag == "ol" {
            ListKind::Decimal
        } else {
            ListKind::Bullet
        };
        self.doc.lists.push(kind);
        let num_id = self.doc.lists.len() as u32;
        for item in &e.children {
            let Node::Element(li) = item else { continue };
            let mut inline_nodes = Vec::new();
            let mut first = true;
            let flush =
                |cx: &mut Cx, nodes: &mut Vec<Node>, first: &mut bool, out: &mut Vec<Block>| {
                    if nodes.iter().all(is_blank) && !*first {
                        nodes.clear();
                        return;
                    }
                    let mut runs = Vec::new();
                    cx.inline(nodes, Fmt::default(), &mut runs);
                    nodes.clear();
                    let (style, list) = if *first {
                        (PStyle::ListItem, Some((num_id, level)))
                    } else {
                        (PStyle::ListContinue, None)
                    };
                    cx.push_para(style, runs, list, out);
                    *first = false;
                };
            for c in &li.children {
                match c {
                    Node::Element(sub) if sub.tag == "ul" || sub.tag == "ol" => {
                        flush(self, &mut inline_nodes, &mut first, out);
                        self.list(sub, level.saturating_add(1).min(8), out);
                    }
                    Node::Element(p) if is_block(c) => {
                        flush(self, &mut inline_nodes, &mut first, out);
                        inline_nodes.extend(p.children.iter().cloned());
                    }
                    other => inline_nodes.push(other.clone()),
                }
            }
            flush(self, &mut inline_nodes, &mut first, out);
        }
    }

    fn table(&mut self, e: &Element) -> Vec<Vec<Cell>> {
        let mut rows = Vec::new();
        let mut trs = Vec::new();
        gather(e, "tr", &mut trs);
        for tr in trs {
            let mut row = Vec::new();
            for c in &tr.children {
                let Node::Element(cell) = c else { continue };
                if cell.tag != "td" && cell.tag != "th" {
                    continue;
                }
                let mut blocks = Vec::new();
                let saved = self.prev_was_para;
                self.prev_was_para = true;
                self.blocks(&cell.children, PStyle::Body, &mut blocks);
                self.prev_was_para = saved;
                let paras = blocks
                    .into_iter()
                    .filter_map(|b| match b {
                        Block::Para(p) => Some(p),
                        Block::Table(_) => None,
                    })
                    .collect();
                row.push(Cell {
                    paras,
                    header: cell.tag == "th",
                    colspan: cell
                        .attr("colspan")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(1),
                });
            }
            if !row.is_empty() {
                rows.push(row);
            }
        }
        rows
    }

    fn inline(&mut self, nodes: &[Node], fmt: Fmt, out: &mut Vec<Run>) {
        for node in nodes {
            match node {
                Node::Text(t) => {
                    let t = t.replace(['\n', '\r', '\t'], " ");
                    if !t.is_empty() {
                        out.push(Run::Text(t, fmt));
                    }
                }
                Node::Element(e) => self.inline_element(e, fmt, out),
            }
        }
    }

    fn inline_element(&mut self, e: &Element, fmt: Fmt, out: &mut Vec<Run>) {
        let mut f = fmt;
        match e.tag.as_str() {
            "sup" if e.attr("role") == Some("doc-noteref") => {
                let target = find(e, &|a| a.tag == "a")
                    .and_then(|a| a.attr("href"))
                    .map(|h| h.trim_start_matches('#').to_string());
                match target.and_then(|id| self.note(&id)) {
                    Some(idx) => {
                        if matches!(last_non_blank(out), Some(Run::NoteRef(_))) {
                            out.push(Run::Text(
                                ",".into(),
                                Fmt {
                                    sup: true,
                                    ..Fmt::default()
                                },
                            ));
                        }
                        out.push(Run::NoteRef(idx));
                    }
                    None => self.inline(&e.children, Fmt { sup: true, ..fmt }, out),
                }
                return;
            }
            "br" => {
                out.push(Run::Break);
                return;
            }
            "img" => {
                if let Some(idx) = self.image_from_img(e) {
                    out.push(Run::Image(idx));
                }
                return;
            }
            "svg" => {
                if let Some(idx) = self.image_from_svg(e) {
                    self.unsupported += 1;
                    out.push(Run::Image(idx));
                }
                return;
            }
            "math" => {
                self.unsupported += 1;
                out.push(Run::Text(
                    e.text(),
                    Fmt {
                        italic: true,
                        ..fmt
                    },
                ));
                return;
            }
            "em" | "i" => f.italic = true,
            "strong" | "b" => f.bold = true,
            "sup" => f.sup = true,
            "sub" => f.sub = true,
            "u" => f.underline = true,
            "s" | "del" | "strike" => f.strike = true,
            "code" | "kbd" | "samp" => f.code = true,
            "a" => {
                if let Some(href) = e.attr("href").filter(|h| !h.starts_with('#')) {
                    self.doc.links.push(href.to_string());
                    f.link = Some(self.doc.links.len() - 1);
                }
            }
            "script" | "style" | "meta" => return,
            _ => {}
        }
        if let Some(style) = e.attr("style") {
            let style = style.replace(' ', "");
            if style.contains("small-caps") {
                f.smallcaps = true;
            }
            if style.contains("font-style:italic") {
                f.italic = true;
            }
            if style.contains("font-weight:bold") || style.contains("font-weight:700") {
                f.bold = true;
            }
        }
        if is_block(&Node::Element(e.clone())) && !out.is_empty() {
            out.push(Run::Text(" ".into(), fmt));
        }
        self.inline(&e.children, f, out);
    }

    /// The note for endnote-section id `id`, numbered into `doc.notes` in order of
    /// first reference.
    fn note(&mut self, id: &str) -> Option<usize> {
        if let Some(&idx) = self.note_index.get(id) {
            return Some(idx);
        }
        let (number, children) = self.note_bodies.remove(id)?;
        let saved = self.prev_was_para;
        let mut blocks = Vec::new();
        self.blocks(&children, PStyle::NoteText, &mut blocks);
        self.prev_was_para = saved;
        let mut paras: Vec<Para> = blocks
            .into_iter()
            .filter_map(|b| match b {
                Block::Para(mut p) => {
                    p.style = PStyle::NoteText;
                    Some(p)
                }
                Block::Table(_) => None,
            })
            .collect();
        if paras.is_empty() {
            paras.push(Para {
                style: PStyle::NoteText,
                runs: Vec::new(),
                list: None,
            });
        }
        self.doc.notes.push(Note { number, paras });
        let idx = self.doc.notes.len() - 1;
        self.note_index.insert(id.to_string(), idx);
        Some(idx)
    }

    fn image_from_img(&mut self, e: &Element) -> Option<usize> {
        let src = e.attr("src")?;
        let (mime, data) = decode_data_uri(src)?;
        let (data, ext) = match mime.as_str() {
            "image/png" => (data, "png"),
            "image/jpeg" | "image/jpg" => (data, "jpeg"),
            "image/gif" => (data, "gif"),
            "image/svg+xml" => {
                let svg = String::from_utf8(data).ok()?;
                return self.rasterize(&svg);
            }
            _ => {
                self.dropped_images += 1;
                return None;
            }
        };
        let size = imagesize::blob_size(&data).ok()?;
        let (w, h) = (size.width as f64 * 0.75, size.height as f64 * 0.75);
        self.add_image(data, ext, w, h)
    }

    fn image_from_svg(&mut self, e: &Element) -> Option<usize> {
        let raw = e.raw.as_deref()?;
        self.rasterize(raw)
    }

    /// SVG → PNG at 300 dpi, sized from the SVG's own dimensions. Typst draws text in
    /// its SVG as outlines, so no fonts are needed here.
    fn rasterize(&mut self, svg: &str) -> Option<usize> {
        let svg = with_absolute_size(svg, self.doc.style.size);
        let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).ok()?;
        let size = tree.size();
        let (w_pt, h_pt) = (size.width() as f64, size.height() as f64);
        if w_pt <= 0.0 || h_pt <= 0.0 {
            return None;
        }
        let scale = 300.0 / 72.0;
        let mut pixmap = resvg::tiny_skia::Pixmap::new(
            (w_pt * scale).ceil().max(1.0) as u32,
            (h_pt * scale).ceil().max(1.0) as u32,
        )?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(scale as f32, scale as f32),
            &mut pixmap.as_mut(),
        );
        let png = pixmap.encode_png().ok()?;
        self.add_image(png, "png", w_pt, h_pt)
    }

    fn add_image(&mut self, data: Vec<u8>, ext: &'static str, w: f64, h: f64) -> Option<usize> {
        let max_w = (self.doc.style.page_width - 144.0).max(144.0);
        let (w, h) = if w > max_w {
            (max_w, h * max_w / w)
        } else {
            (w, h)
        };
        self.doc.images.push(Image {
            data,
            ext,
            width_pt: w,
            height_pt: h,
        });
        Some(self.doc.images.len() - 1)
    }
}

fn gather<'a>(e: &'a Element, tag: &str, out: &mut Vec<&'a Element>) {
    for c in &e.children {
        if let Node::Element(child) = c {
            if child.tag == tag {
                out.push(child);
            } else {
                gather(child, tag, out);
            }
        }
    }
}

fn is_blank(n: &Node) -> bool {
    matches!(n, Node::Text(t) if t.trim().is_empty())
}

fn last_non_blank(runs: &[Run]) -> Option<&Run> {
    runs.iter()
        .rev()
        .find(|r| !matches!(r, Run::Text(t, _) if t.trim().is_empty()))
}

/// Drops leading/trailing whitespace-only text and trims the outer ends of the rest.
fn trim_runs(mut runs: Vec<Run>) -> Vec<Run> {
    while matches!(runs.first(), Some(Run::Text(t, _)) if t.trim().is_empty()) {
        runs.remove(0);
    }
    while matches!(runs.last(), Some(Run::Text(t, _)) if t.trim().is_empty()) {
        runs.pop();
    }
    if let Some(Run::Text(t, _)) = runs.first_mut() {
        *t = t.trim_start().to_string();
    }
    if let Some(Run::Text(t, _)) = runs.last_mut() {
        *t = t.trim_end().to_string();
    }
    runs
}

fn decode_data_uri(src: &str) -> Option<(String, Vec<u8>)> {
    let rest = src.strip_prefix("data:")?;
    let (meta, payload) = rest.split_once(',')?;
    let mime = meta.split(';').next().unwrap_or("").to_ascii_lowercase();
    let data = if meta.ends_with(";base64") {
        base64::engine::general_purpose::STANDARD
            .decode(payload.trim())
            .ok()?
    } else {
        super::html::decode_entities(payload).into_bytes()
    };
    Some((mime, data))
}

/// Typst sizes the SVG it embeds in HTML in `em` (so it scales with surrounding text);
/// usvg needs absolute units, so `em` widths/heights become points at the body size.
fn with_absolute_size(svg: &str, em_pt: f64) -> String {
    let Some(tag_end) = svg.find('>') else {
        return svg.to_string();
    };
    let (open, rest) = svg.split_at(tag_end);
    let mut open = open.to_string();
    for attr in ["width", "height"] {
        let needle = format!(" {attr}=\"");
        if let Some(pos) = open.find(&needle) {
            let start = pos + needle.len();
            if let Some(len) = open[start..].find('"') {
                let value = open[start..start + len].to_string();
                if let Some(num) = value.strip_suffix("em").and_then(|n| n.parse::<f64>().ok()) {
                    open.replace_range(start..start + len, &format!("{}pt", num * em_pt));
                }
            }
        }
    }
    open + rest
}

#[cfg(test)]
mod tests {
    use super::super::html;
    use super::*;

    fn doc(body: &str) -> Doc {
        convert(&html::parse(&format!(
            "<html><head><title>T</title></head><body>{body}</body></html>"
        )))
    }

    fn paras(d: &Doc) -> Vec<&Para> {
        d.blocks
            .iter()
            .filter_map(|b| match b {
                Block::Para(p) => Some(p),
                Block::Table(_) => None,
            })
            .collect()
    }

    const NOTES: &str = r##"<section role="doc-endnotes"><ol>
        <li id="n1"><sup role="doc-backlink"><a href="#r1">1</a></sup>First <em>note</em>.</li>
        <li id="n2"><sup role="doc-backlink"><a href="#r2">2</a></sup>Second.</li>
        <li id="n3"><sup role="doc-backlink"><a href="#r3">3</a></sup>Third.</li></ol></section>"##;

    #[test]
    fn notes_are_referenced_in_order_with_commas_between_adjacent_marks() {
        let d = doc(&format!(
            r##"<h2>Head</h2><p>A<sup role="doc-noteref"><a href="#n1">1</a></sup> b<sup role="doc-noteref"><a href="#n2">2</a></sup><sup role="doc-noteref"><a href="#n3">3</a></sup>.</p>{NOTES}"##
        ));
        assert_eq!(d.notes.len(), 3);
        assert_eq!(d.notes[0].number, "1");
        let p = paras(&d);
        assert_eq!(p[0].style, PStyle::Heading(1));
        assert_eq!(p[1].style, PStyle::FirstBody);
        let marks: Vec<String> = p[1]
            .runs
            .iter()
            .map(|r| match r {
                Run::NoteRef(i) => format!("[{i}]"),
                Run::Text(t, _) => t.clone(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(marks.concat(), "A[0] b[1],[2].");
        let Run::Text(_, f) = &d.notes[0].paras[0].runs[1] else {
            panic!()
        };
        assert!(f.italic);
    }

    #[test]
    fn paragraphs_after_a_paragraph_are_plain_body() {
        let d = doc("<p>One</p><p>Two</p><blockquote>Q</blockquote><p>Three</p>");
        let styles: Vec<PStyle> = paras(&d).iter().map(|p| p.style).collect();
        assert_eq!(
            styles,
            [
                PStyle::FirstBody,
                PStyle::Body,
                PStyle::Quote,
                PStyle::FirstBody
            ]
        );
    }

    #[test]
    fn bibliography_and_lists_get_their_own_styles() {
        let d = doc(r#"<ul><li>one<ol><li>inner</li></ol></li><li>two</li></ul>
               <section role="doc-bibliography"><h2>Bibliography</h2><ul><li>Alpha, Ann.</li></ul></section>"#);
        let p = paras(&d);
        assert_eq!(p[0].list, Some((1, 0)));
        assert_eq!(p[1].list, Some((2, 1)));
        assert_eq!(p[2].list, Some((1, 0)));
        assert_eq!(d.lists, [ListKind::Bullet, ListKind::Decimal]);
        assert_eq!(p[3].style, PStyle::Heading(1));
        assert_eq!(p[4].style, PStyle::Bibliography);
    }

    #[test]
    fn style_probe_is_read() {
        let d = doc(
            r#"<meta name="zk-style" content="font=EB Garamond|Libertinus Serif;size=12;leading=7.8;spacing=14.4;indent=12;justify=true;lang=en;pw=612;ph=792">"#,
        );
        assert_eq!(d.style.font, "EB Garamond");
        assert_eq!(d.style.size, 12.0);
        assert_eq!(d.style.indent, 12.0);
        assert!(d.style.justify);
        assert_eq!(d.style.page_width, 612.0);
    }
}
