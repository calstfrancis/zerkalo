use super::*;

fn settings_for(key: &str) -> TemplateSettings {
    let mut s = sidecar_to_settings(&SidecarSettings::default());
    s.title = "A Study".into();
    s.author = "Jane Doe".into();
    s.style_idx = CITATION_STYLES.iter().position(|(_, k)| *k == key).unwrap();
    s.bib_path = Some(PathBuf::from("refs.bib"));
    s.include_bibliography = true;
    s
}

#[test]
fn a_new_document_names_its_reference_list_the_way_the_style_does() {
    for (key, title) in [
        ("apa", "References"),
        ("mla", "Works Cited"),
        ("harvard", "Reference list"),
        ("ieee", "References"),
        ("gost-r-705", "Список использованных источников"),
    ] {
        let doc = generate_typst_template(&settings_for(key));
        assert!(
            doc.contains(&format!("title: \"{title}\"")),
            "{key} should title its reference list \"{title}\""
        );
    }
    let sbl = generate_typst_template(&settings_for("sbl"));
    assert!(
        !sbl.contains("title:"),
        "SBL keeps Typst's \"Bibliography\""
    );
}

#[test]
fn each_style_uses_the_citation_style_it_is_named_for() {
    assert_eq!(bib_style("asa"), "american-sociological-association");
    assert_eq!(bib_style("harvard"), "harvard-cite-them-right");
    assert_eq!(bib_style("sbl"), "turabian-fullnote-8");
    assert_eq!(bib_style("turabian"), "turabian-fullnote-8");
    assert_eq!(
        bib_style("vancouver"),
        "taylor-and-francis-national-library-of-medicine"
    );
}

#[test]
fn headings_without_numbering_of_their_own_are_not_numbered() {
    let rules = inject_heading_numbering(&heading_styles("ieee"), true, "I.A.1.");
    assert!(rules.contains("#if it.numbering != none ["));
    let decimal = inject_heading_numbering(&heading_styles("apa"), true, "1.");
    assert!(decimal.contains("display(\"1.\")"));
}

#[test]
fn switching_style_rewrites_the_paragraph_rule_and_keeps_the_line_gap() {
    let block = "#set text(font: \"Libertinus Serif\", size: 12pt, lang: \"en\")\n\
                 #set par(leading: 1.65em, spacing: 1.65em, first-line-indent: 1em, justify: true)\n";
    let apa = update_par_for_style(block, "apa");
    assert!(apa.contains("leading: 1.65em, spacing: 1.65em, first-line-indent: (amount: 0.5in, all: true), justify: false"));
    let gost = update_par_for_style(block, "gost-r-705");
    assert!(gost.contains("lang: \"ru\""));
    assert!(gost.contains("justify: true"));
}

#[test]
fn mla_with_its_page_number_at_the_top_right_gets_the_surname_header() {
    let mut s = settings_for("mla");
    s.page_num_pos = 3;
    let doc = generate_typst_template(&s);
    assert!(
        doc.contains("#doc-author.split"),
        "MLA carries the surname header"
    );
}
