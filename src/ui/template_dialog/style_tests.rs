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

fn page_parts_for(s: &TemplateSettings) -> crate::page_parts::PageParts {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("refs.bib"),
        "@book{doe2020, author = {Doe, Jane}, title = {A Book}, year = {2020}, publisher = {Example Press}}\n",
    )
    .unwrap();
    let doc = generate_typst_template(s).replace(
        "Start writing here...",
        "#lorem(900) @doe2020\n\n#lorem(300)",
    );
    let path = dir.path().join("main.typ");
    std::fs::write(&path, &doc).unwrap();
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

#[test]
fn a_generated_apa_paper_counts_the_abstract_but_not_the_title_page_contents_or_references() {
    let mut s = settings_for("apa");
    s.include_title_page = true;
    s.include_abstract = true;
    s.abstract_text = "What this paper argues.".into();
    s.include_toc = true;
    let p = page_parts_for(&s);
    assert_eq!(p.title, 1, "{p:?}");
    assert_eq!(p.contents, 1, "{p:?}");
    assert_eq!(p.bibliography, 1, "{p:?}");
    assert!(
        p.body() >= 3,
        "abstract plus at least two body pages: {p:?}"
    );
}

#[test]
fn a_generated_mla_paper_has_no_title_page_to_leave_out() {
    let mut s = settings_for("mla");
    s.include_title_page = true;
    let p = page_parts_for(&s);
    assert_eq!(p.title, 0, "{p:?}");
    assert_eq!(p.bibliography, 1, "{p:?}");
    assert_eq!(p.body(), p.total - 1, "{p:?}");
}

#[test]
fn a_generated_chicago_paper_leaves_out_its_title_page() {
    let mut s = settings_for("chicago-notes");
    s.include_title_page = true;
    let p = page_parts_for(&s);
    assert_eq!(p.title, 1, "{p:?}");
    assert_eq!(p.bibliography, 1, "{p:?}");
}
