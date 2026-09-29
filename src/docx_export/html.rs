//! A reader for the HTML Typst's own exporter writes — not a general HTML parser.
//! That output is regular: attributes are always double-quoted, text is escaped with
//! the five named entities plus hex references, and void elements are the only
//! unclosed tags. Inline SVG (Typst frames) closes its children with `/>`.

#[derive(Debug, Clone)]
pub enum Node {
    Element(Element),
    Text(String),
}

#[derive(Debug, Clone)]
pub struct Element {
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
    /// The element's source text, kept only for `<svg>`, which is rasterised whole.
    pub raw: Option<String>,
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn text(&self) -> String {
        let mut out = String::new();
        collect_text(&self.children, &mut out);
        out
    }
}

fn collect_text(nodes: &[Node], out: &mut String) {
    for n in nodes {
        match n {
            Node::Text(t) => out.push_str(t),
            Node::Element(e) => collect_text(&e.children, out),
        }
    }
}

const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr",
];

pub fn parse(html: &str) -> Element {
    let mut root = Element {
        tag: "#root".into(),
        attrs: Vec::new(),
        children: Vec::new(),
        raw: None,
    };
    let mut stack: Vec<(Element, usize)> = Vec::new();
    let bytes = html.as_bytes();
    let mut i = 0;
    let mut text_start = 0;

    macro_rules! current {
        () => {
            match stack.last_mut() {
                Some((e, _)) => e,
                None => &mut root,
            }
        };
    }

    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        if i > text_start {
            let t = decode_entities(&html[text_start..i]);
            if !t.is_empty() {
                current!().children.push(Node::Text(t));
            }
        }
        let tag_start = i;
        if html[i..].starts_with("<!--") {
            i = html[i..]
                .find("-->")
                .map(|p| i + p + 3)
                .unwrap_or(bytes.len());
            text_start = i;
            continue;
        }
        if html[i..].starts_with("<!") {
            i = html[i..]
                .find('>')
                .map(|p| i + p + 1)
                .unwrap_or(bytes.len());
            text_start = i;
            continue;
        }
        let Some(end) = find_tag_end(html, i) else {
            break;
        };
        let inner = &html[i + 1..end];
        i = end + 1;
        text_start = i;

        if let Some(name) = inner.strip_prefix('/') {
            let name = name.trim().to_ascii_lowercase();
            if let Some(pos) = stack.iter().rposition(|(e, _)| e.tag == name) {
                while stack.len() > pos {
                    let (mut e, open_at) = stack.pop().unwrap();
                    if e.tag == "svg" {
                        e.raw = Some(html[open_at..i].to_string());
                    }
                    current!().children.push(Node::Element(e));
                }
            }
            continue;
        }

        let self_closing = inner.ends_with('/');
        let inner = inner.trim_end_matches('/');
        let (name, attrs) = parse_tag(inner);
        let element = Element {
            tag: name.clone(),
            attrs,
            children: Vec::new(),
            raw: None,
        };
        if self_closing || VOID.contains(&name.as_str()) {
            current!().children.push(Node::Element(element));
        } else if name == "script" || name == "style" {
            let close = format!("</{name}");
            let body_end = html[i..].find(&close).map(|p| i + p).unwrap_or(bytes.len());
            let mut element = element;
            element
                .children
                .push(Node::Text(html[i..body_end].to_string()));
            current!().children.push(Node::Element(element));
            i = html[body_end..]
                .find('>')
                .map(|p| body_end + p + 1)
                .unwrap_or(bytes.len());
            text_start = i;
        } else {
            stack.push((element, tag_start));
        }
    }
    if text_start < bytes.len() {
        let t = decode_entities(&html[text_start..]);
        if !t.is_empty() {
            current!().children.push(Node::Text(t));
        }
    }
    while let Some((e, _)) = stack.pop() {
        current!().children.push(Node::Element(e));
    }
    root
}

/// The `>` closing the tag that opens at `start`, skipping any inside quoted values.
fn find_tag_end(html: &str, start: usize) -> Option<usize> {
    let mut in_quote = false;
    for (off, c) in html[start..].char_indices() {
        match c {
            '"' => in_quote = !in_quote,
            '>' if !in_quote => return Some(start + off),
            _ => {}
        }
    }
    None
}

fn parse_tag(inner: &str) -> (String, Vec<(String, String)>) {
    let inner = inner.trim();
    let name_end = inner
        .find(|c: char| c.is_whitespace())
        .unwrap_or(inner.len());
    let name = inner[..name_end].to_ascii_lowercase();
    let mut attrs = Vec::new();
    let mut rest = inner[name_end..].trim_start();
    while !rest.is_empty() {
        let key_end = rest
            .find(|c: char| c == '=' || c.is_whitespace())
            .unwrap_or(rest.len());
        let key = rest[..key_end].to_string();
        rest = rest[key_end..].trim_start();
        let value = if let Some(after) = rest.strip_prefix('=') {
            let after = after.trim_start();
            if let Some(q) = after.strip_prefix('"') {
                let close = q.find('"').unwrap_or(q.len());
                rest = q.get(close + 1..).unwrap_or("").trim_start();
                decode_entities(&q[..close])
            } else {
                let close = after
                    .find(|c: char| c.is_whitespace())
                    .unwrap_or(after.len());
                rest = after[close..].trim_start();
                decode_entities(&after[..close])
            }
        } else {
            String::new()
        };
        if !key.is_empty() {
            attrs.push((key, value));
        }
    }
    (name, attrs)
}

pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        rest = &rest[pos..];
        let Some(semi) = rest.find(';').filter(|&p| p <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..semi];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            e if e.starts_with("#x") || e.starts_with("#X") => u32::from_str_radix(&e[2..], 16)
                .ok()
                .and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first(e: &Element) -> &Element {
        match &e.children[0] {
            Node::Element(e) => e,
            other => panic!("expected element, got {other:?}"),
        }
    }

    #[test]
    fn nests_elements_and_decodes_text() {
        let root = parse(r#"<p class="a b">Tom &amp; Jerry&#x2019;s <em>big</em> day</p>"#);
        let p = first(&root);
        assert_eq!(p.tag, "p");
        assert_eq!(p.attr("class"), Some("a b"));
        assert_eq!(p.text(), "Tom & Jerry\u{2019}s big day");
    }

    #[test]
    fn void_and_self_closing_tags_do_not_swallow_siblings() {
        let root = parse(r#"<p>a<br>b<img src="x">c</p><svg><path d="M0"/></svg>"#);
        assert_eq!(first(&root).text(), "abc");
        let Node::Element(svg) = &root.children[1] else {
            panic!()
        };
        assert_eq!(svg.raw.as_deref(), Some(r#"<svg><path d="M0"/></svg>"#));
    }

    #[test]
    fn quoted_gt_in_attribute_does_not_end_the_tag() {
        let root = parse(r##"<a title="1 > 0" href="#x">t</a>"##);
        let a = first(&root);
        assert_eq!(a.attr("title"), Some("1 > 0"));
        assert_eq!(a.text(), "t");
    }
}
