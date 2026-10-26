//! Snippets for setting verse with verseatile, inserted from the command
//! palette. Each brings its own import when the document hasn't got one, so
//! it works in an academic paper as well as in a poetry document.

const IMPORT: &str = "#import \"@preview/verseatile:0.2.2\": poem\n";

fn with_import(doc: &str, body: &str) -> String {
    if doc.contains("verseatile") {
        body.to_string()
    } else {
        format!("{IMPORT}{body}")
    }
}

pub fn poem_snippet(doc: &str) -> String {
    with_import(
        doc,
        "#poem[Title of the Poem][\n  First line of the first stanza \\\n  Second line of the first stanza\n\n  First line of the second stanza \\\n  Second line of the second stanza\n]\n",
    )
}

pub fn verse_quote_snippet(doc: &str) -> String {
    with_import(
        doc,
        "#quote(block: true)[\n  #poem[][\n    First quoted line \\\n    Second quoted line\n  ]\n]\n",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn compiles(src: &str) {
        let path = std::env::temp_dir().join(format!("zerkalo_poetry_{}.typ", std::process::id()));
        std::fs::write(&path, src).unwrap();
        let result =
            crate::compiler::compile_to_pdf_bytes(&path, &HashMap::new(), &HashMap::new(), None);
        let _ = std::fs::remove_file(&path);
        assert!(result.is_ok(), "{:?}\n{src}", result.err());
    }

    #[test]
    fn snippets_compile_inside_an_ordinary_document() {
        let prose = "= Essay\n\nSome prose.\n\n";
        compiles(&format!("{prose}{}", poem_snippet(prose)));
        compiles(&format!("{prose}{}", verse_quote_snippet(prose)));
    }

    #[test]
    fn import_is_added_once() {
        let first = poem_snippet("");
        assert!(first.starts_with("#import"));
        assert!(!poem_snippet(&first).contains("#import"));
        assert!(
            !verse_quote_snippet("#import \"@preview/verseatile:0.2.2\": *").contains("#import")
        );
    }
}
