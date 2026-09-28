//! Linked images are copied into an `assets/` folder beside the document, so
//! the document keeps working wherever the original picture lives (or after
//! it's moved or deleted) and the image travels with the document's backups.

use std::path::{Path, PathBuf};

pub const ASSETS_DIR: &str = "assets";

/// Copies `src` into `<doc_dir>/assets/` and returns the path to use in
/// `image("…")`, relative to `doc_dir` with forward slashes. An image already
/// inside `doc_dir` is used where it is. A same-named file in `assets/` with
/// identical bytes is reused; a different one gets a numbered name instead of
/// being overwritten.
pub fn import_image(src: &Path, doc_dir: &Path) -> std::io::Result<String> {
    let doc_dir_c = doc_dir.canonicalize()?;
    let src_c = src.canonicalize()?;
    if let Ok(rel) = src_c.strip_prefix(&doc_dir_c) {
        return Ok(to_typst_path(rel));
    }

    let assets = doc_dir.join(ASSETS_DIR);
    std::fs::create_dir_all(&assets)?;
    let name = src
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image.png");
    let dest = free_destination(&assets, name, &std::fs::read(&src_c)?)?;
    if !dest.exists() {
        std::fs::copy(&src_c, &dest)?;
    }
    Ok(to_typst_path(dest.strip_prefix(doc_dir).unwrap_or(&dest)))
}

/// `assets/name`, or `assets/name-2`, `-3`, … — the first that is either
/// free or already holds exactly `bytes`.
fn free_destination(assets: &Path, name: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };
    for n in 1.. {
        let candidate = if n == 1 {
            assets.join(name)
        } else {
            assets.join(format!("{stem}-{n}{ext}"))
        };
        match std::fs::read(&candidate) {
            Ok(existing) if existing == bytes => return Ok(candidate),
            Ok(_) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(candidate),
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}

fn to_typst_path(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// A `#figure` snippet for an image path, escaped for a Typst string.
pub fn figure_snippet(path: &str) -> String {
    let escaped = path.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "\n#figure(\n  image(\"{escaped}\", width: 80%),\n  caption: [Caption],\n) <fig:label>\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("zerkalo_assets_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn external_image_is_copied_into_assets() {
        let (doc, out) = (tmp("doc1"), tmp("out1"));
        std::fs::write(out.join("cat.png"), b"cat").unwrap();
        let rel = import_image(&out.join("cat.png"), &doc).unwrap();
        assert_eq!(rel, "assets/cat.png");
        assert_eq!(std::fs::read(doc.join("assets/cat.png")).unwrap(), b"cat");
    }

    #[test]
    fn identical_image_is_reused_and_different_one_renamed() {
        let (doc, out) = (tmp("doc2"), tmp("out2"));
        std::fs::write(out.join("cat.png"), b"cat").unwrap();
        assert_eq!(
            import_image(&out.join("cat.png"), &doc).unwrap(),
            "assets/cat.png"
        );
        assert_eq!(
            import_image(&out.join("cat.png"), &doc).unwrap(),
            "assets/cat.png"
        );
        std::fs::write(out.join("cat.png"), b"other cat").unwrap();
        assert_eq!(
            import_image(&out.join("cat.png"), &doc).unwrap(),
            "assets/cat-2.png"
        );
        assert_eq!(std::fs::read(doc.join("assets/cat.png")).unwrap(), b"cat");
    }

    #[test]
    fn image_already_beside_the_document_is_used_in_place() {
        let doc = tmp("doc3");
        std::fs::create_dir_all(doc.join("pics")).unwrap();
        std::fs::write(doc.join("pics/dog.png"), b"dog").unwrap();
        assert_eq!(
            import_image(&doc.join("pics/dog.png"), &doc).unwrap(),
            "pics/dog.png"
        );
        assert!(!doc.join(ASSETS_DIR).exists());
    }

    #[test]
    fn snippet_escapes_quotes() {
        assert!(figure_snippet(r#"assets/a"b.png"#).contains(r#"image("assets/a\"b.png""#));
    }
}
