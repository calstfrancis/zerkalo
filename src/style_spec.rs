//! One definition per citation style: the bibliography style and title it
//! uses and the page and paragraph layout it asks for. The template dialog,
//! the generator and the older `styles.rs` path all read from here so they
//! cannot drift apart.

pub(crate) struct StyleSpec {
    pub key: &'static str,
    pub csl: &'static str,
    /// Empty leaves Typst's own "Bibliography".
    pub bib_title: &'static str,
    /// Margin on every side in inches; `None` keeps whatever is chosen.
    pub margin_in: Option<&'static str>,
    /// Index into the dialog's line-spacing options (0 single, 1 one and a half, 2 double).
    pub spacing_idx: usize,
    /// Index into the dialog's font-size options (0 10 pt, 1 11 pt, 2 12 pt, 3 14 pt).
    pub font_size_idx: u32,
    /// Index into the dialog's page-number options (4 is none).
    pub page_num: u32,
    pub justify: bool,
    pub indent: &'static str,
    /// Indent the first paragraph after a heading too.
    pub indent_all: bool,
    pub lang: &'static str,
}

const DOUBLE: usize = 2;
const ONE_AND_HALF: usize = 1;
const SINGLE: usize = 0;
const BOTTOM_CENTER: u32 = 0;
const TOP_RIGHT: u32 = 3;
const NO_NUMBER: u32 = 4;

const fn spec(key: &'static str, csl: &'static str, bib_title: &'static str) -> StyleSpec {
    StyleSpec {
        key,
        csl,
        bib_title,
        margin_in: Some("1"),
        spacing_idx: DOUBLE,
        font_size_idx: 2,
        page_num: TOP_RIGHT,
        justify: false,
        indent: "0.5in",
        indent_all: true,
        lang: "en",
    }
}

const SPECS: &[StyleSpec] = &[
    StyleSpec {
        indent_all: false,
        ..spec("sbl", "turabian-fullnote-8", "")
    },
    spec("chicago-notes", "chicago-notes", ""),
    spec("chicago-author-date", "chicago-author-date", "References"),
    spec("mla", "mla", "Works Cited"),
    spec("apa", "apa", "References"),
    spec("asa", "american-sociological-association", "References"),
    spec("turabian", "turabian-fullnote-8", ""),
    StyleSpec {
        spacing_idx: ONE_AND_HALF,
        page_num: BOTTOM_CENTER,
        ..spec("harvard", "harvard-cite-them-right", "Reference list")
    },
    StyleSpec {
        margin_in: Some("0.75"),
        spacing_idx: SINGLE,
        font_size_idx: 0,
        page_num: NO_NUMBER,
        justify: true,
        indent: "0.15in",
        ..spec("ieee", "ieee", "References")
    },
    StyleSpec {
        margin_in: None,
        spacing_idx: ONE_AND_HALF,
        font_size_idx: 3,
        page_num: BOTTOM_CENTER,
        justify: true,
        indent: "12.5mm",
        lang: "ru",
        ..spec(
            "gost-r-705",
            "gost-r-705-2008-numeric",
            "Список использованных источников",
        )
    },
    StyleSpec {
        page_num: BOTTOM_CENTER,
        ..spec(
            "vancouver",
            "taylor-and-francis-national-library-of-medicine",
            "References",
        )
    },
    StyleSpec {
        margin_in: None,
        spacing_idx: SINGLE,
        page_num: BOTTOM_CENTER,
        justify: true,
        indent: "1.8em",
        ..spec("latex", "ieee", "References")
    },
];

pub(crate) fn style_spec(key: &str) -> &'static StyleSpec {
    SPECS.iter().find(|s| s.key == key).unwrap_or(&SPECS[1])
}

/// The Typst `#set par(...)` rule for a style at the given line gap.
pub(crate) fn par_rule(key: &str, leading: &str) -> String {
    let s = style_spec(key);
    let spacing = leading.to_string();
    let indent = if s.indent_all {
        format!("(amount: {}, all: true)", s.indent)
    } else {
        s.indent.to_string()
    };
    format!(
        "#set par(leading: {leading}, spacing: {spacing}, first-line-indent: {indent}, justify: {})",
        s.justify
    )
}

/// A citation as the citation panel builds it, with the page label dropped for
/// styles that write a bare number: Chicago, MLA and ASA give `14`, only APA,
/// Harvard and IEEE give `p. 14`.
pub(crate) fn adapt_locator(citation: &str, style_key: &str) -> String {
    if matches!(style_key, "apa" | "harvard" | "ieee" | "latex") {
        return citation.to_string();
    }
    for label in ["[pp. ", "[p. "] {
        if let Some(i) = citation.find(label) {
            return format!("{}[{}", &citation[..i], &citation[i + label.len()..]);
        }
    }
    citation.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_dialog_style_has_a_spec() {
        for key in [
            "sbl",
            "chicago-notes",
            "chicago-author-date",
            "mla",
            "apa",
            "asa",
            "turabian",
            "harvard",
            "ieee",
            "gost-r-705",
            "vancouver",
            "latex",
        ] {
            assert_eq!(style_spec(key).key, key);
        }
    }

    #[test]
    fn page_labels_stay_only_where_the_style_prints_them() {
        assert_eq!(adapt_locator("@a[p. 12]", "chicago-notes"), "@a[12]");
        assert_eq!(adapt_locator("@a[pp. 12–14]", "mla"), "@a[12–14]");
        assert_eq!(adapt_locator("@a[p. 12]", "apa"), "@a[p. 12]");
        assert_eq!(adapt_locator("@a[ch. 3]", "mla"), "@a[ch. 3]");
        assert_eq!(adapt_locator("@a", "mla"), "@a");
    }

    #[test]
    fn indent_rule_covers_first_paragraphs_only_where_the_style_says() {
        assert!(par_rule("apa", "1.65em").contains("all: true"));
        assert!(!par_rule("sbl", "1.65em").contains("all: true"));
    }
}
