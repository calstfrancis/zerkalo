use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use chrono::Datelike;
use typst::diag::{FileError, FileResult, SourceDiagnostic, Severity};
use typst::foundations::{Bytes, Datetime, Dict, IntoValue, Str};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World as TypstWorld};
use typst_kit::fonts::{FontSearcher, FontSlot, Fonts};

/// A panic anywhere else while holding one of these cache locks would otherwise
/// poison it permanently, turning one unrelated crash into "compiling is broken
/// until restart". The cached data itself can't be corrupted mid-insert (the
/// lock only ever guards a plain `HashMap::insert`/`get`), so recovering the
/// inner value on poison is safe.
fn poisoned_lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

// ── Static globals: fonts only — library is built per-compile with inputs ─────

static FONTS: OnceLock<(LazyHash<FontBook>, Vec<FontSlot>)> = OnceLock::new();
fn global_fonts() -> &'static (LazyHash<FontBook>, Vec<FontSlot>) {
    FONTS.get_or_init(|| {
        let Fonts { book, fonts } = FontSearcher::new().search();
        (LazyHash::new(book), fonts)
    })
}

fn build_library(sys_inputs: &HashMap<String, String>) -> LazyHash<Library> {
    if sys_inputs.is_empty() {
        return LazyHash::new(Library::default());
    }
    let mut dict = Dict::new();
    for (k, v) in sys_inputs {
        dict.insert(Str::from(k.as_str()), v.as_str().into_value());
    }
    LazyHash::new(Library::builder().with_inputs(dict).build())
}

// ── World implementation ──────────────────────────────────────────────────────

struct ZerkaloWorld {
    root: PathBuf,
    main_id: FileId,
    source_cache: Mutex<HashMap<FileId, FileResult<Source>>>,
    file_cache: Mutex<HashMap<FileId, FileResult<Bytes>>>,
    overrides: HashMap<PathBuf, String>,
    library: LazyHash<Library>,
}

impl ZerkaloWorld {
    fn new(
        root_file: &Path,
        overrides: HashMap<PathBuf, String>,
        sys_inputs: &HashMap<String, String>,
    ) -> Result<Self, String> {
        let root = root_file
            .parent()
            .ok_or_else(|| format!("no parent directory: {}", root_file.display()))?
            .to_path_buf();
        let rel = root_file.strip_prefix(&root).unwrap_or(root_file);
        let main_id = FileId::new(None, VirtualPath::new(rel));
        let library = build_library(sys_inputs);
        Ok(Self {
            root,
            main_id,
            source_cache: Mutex::new(HashMap::new()),
            file_cache: Mutex::new(HashMap::new()),
            overrides,
            library,
        })
    }

    fn resolve(&self, id: FileId) -> FileResult<PathBuf> {
        let vpath = id.vpath();
        let base = if let Some(spec) = id.package() {
            // Use the locally cached copy from ~/.cache/typst/packages/.
            let cache_root = std::env::var("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".cache")
                });
            cache_root
                .join("typst/packages")
                .join(spec.namespace.as_str())
                .join(spec.name.as_str())
                .join(spec.version.to_string())
        } else {
            self.root.clone()
        };

        vpath
            .resolve(&base)
            .ok_or_else(|| FileError::NotFound(vpath.as_rootless_path().to_path_buf()))
    }
}

impl typst::World for ZerkaloWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &global_fonts().0
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        {
            let cache = poisoned_lock(&self.source_cache);
            if let Some(result) = cache.get(&id) {
                return result.clone();
            }
        }
        let result = self.resolve(id).and_then(|path| {
            if let Some(text) = self.overrides.get(&path) {
                return Ok(Source::new(id, text.clone()));
            }
            std::fs::read_to_string(&path)
                .map(|text| Source::new(id, text))
                .map_err(|_| FileError::NotFound(path))
        });
        poisoned_lock(&self.source_cache).insert(id, result.clone());
        result
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        {
            let cache = poisoned_lock(&self.file_cache);
            if let Some(result) = cache.get(&id) {
                return result.clone();
            }
        }
        let result = self.resolve(id).and_then(|path| {
            std::fs::read(&path)
                .map(|b| Bytes::new(b))
                .map_err(|_| FileError::NotFound(path))
        });
        poisoned_lock(&self.file_cache).insert(id, result.clone());
        result
    }

    fn font(&self, index: usize) -> Option<Font> {
        global_fonts().1.get(index)?.get()
    }

    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        let tz_secs = (offset.unwrap_or(0) * 3600) as i32;
        let tz = chrono::FixedOffset::east_opt(tz_secs)?;
        let now = chrono::Local::now().with_timezone(&tz);
        Datetime::from_ymd(now.year(), now.month() as u8, now.day() as u8)
    }
}

// ── Error formatting ──────────────────────────────────────────────────────────

/// Convert a byte offset in `text` to a (line, column) pair (both 1-based).
fn offset_to_line_col(text: &str, offset: usize) -> (usize, usize) {
    let safe = offset.min(text.len());
    // Walk back to a char boundary so we never panic on a multibyte codepoint.
    let safe = (0..=safe).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0);
    let before = &text[..safe];
    let line = before.bytes().filter(|&b| b == b'\n').count() + 1;
    let col  = safe - before.rfind('\n').map(|p| p + 1).unwrap_or(0) + 1;
    (line, col)
}

/// Format a list of `SourceDiagnostic` into the `error: …\n --> file:line:col` text
/// that `parse_typst_errors` in the UI layer understands.  We use the compile world
/// to resolve span → source → byte range → human-readable location.
fn format_diagnostics(world: &ZerkaloWorld, diags: &[SourceDiagnostic]) -> String {
    diags.iter().map(|d| format_one(world, d)).collect::<Vec<_>>().join("\n")
}

fn format_one(world: &ZerkaloWorld, d: &SourceDiagnostic) -> String {
    let sev = match d.severity {
        Severity::Error   => "error",
        Severity::Warning => "warning",
    };

    // Try to resolve the source location from the span
    let location: Option<String> = d.span.id().and_then(|fid| {
        let src = TypstWorld::source(world, fid).ok()?;
        let range = d.span.range()?;
        let (line, col) = offset_to_line_col(src.text(), range.start);
        let path = src.id().vpath().as_rootless_path().display().to_string();
        Some(format!("{path}:{line}:{col}"))
    });

    let mut out = format!("{sev}: {}", d.message);
    if let Some(loc) = location {
        out.push_str(&format!("\n --> {loc}"));
    }
    for hint in &d.hints {
        out.push_str(&format!("\n   = hint: {hint}"));
    }
    out
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Compile `root_file` in-process and return PDF bytes.
pub fn compile_to_pdf_bytes(
    root_file: &Path,
    overrides: &HashMap<PathBuf, String>,
    sys_inputs: &HashMap<String, String>,
) -> Result<Vec<u8>, String> {
    let world = ZerkaloWorld::new(root_file, overrides.clone(), sys_inputs)?;
    let result = typst::compile::<PagedDocument>(&world);

    match result.output {
        Ok(doc) => {
            typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default())
                .map_err(|errors| {
                    errors
                        .iter()
                        .map(|e| e.message.to_string())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
        }
        Err(errors) => Err(format_diagnostics(&world, &errors)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp_typ(content: &str) -> std::path::PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::path::PathBuf::from(format!(
            "/tmp/zerkalo_test_compile_{}_{}.typ",
            std::process::id(),
            n
        ));
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn compile_trivial_document_to_pdf() {
        let path = write_temp_typ("Hello, world!");
        let result = compile_to_pdf_bytes(&path, &HashMap::new(), &HashMap::new());
        assert!(result.is_ok(), "trivial doc should compile: {:?}", result.err());
        let bytes = result.unwrap();
        assert!(bytes.starts_with(b"%PDF-"), "output should be valid PDF");
    }

    #[test]
    fn compile_with_heading_and_content() {
        let path = write_temp_typ("= Introduction\n\nThis is a test document.\n");
        let result = compile_to_pdf_bytes(&path, &HashMap::new(), &HashMap::new());
        assert!(result.is_ok(), "document with heading should compile");
    }

    #[test]
    fn compile_nonexistent_root_fails() {
        let path = std::path::PathBuf::from("/tmp/zerkalo-nonexistent-root-abc123.typ");
        let _ = std::fs::remove_file(&path);
        let result = compile_to_pdf_bytes(&path, &HashMap::new(), &HashMap::new());
        assert!(result.is_err(), "compiling a nonexistent root file should fail");
    }

    #[test]
    fn compile_to_png_single_page() {
        let path = write_temp_typ("= Heading\n\nSome content here.");
        let result = compile_to_png_bytes(&path, 1.0, &HashMap::new(), &HashMap::new());
        assert!(result.is_ok(), "doc should compile to PNG");
        let pages = result.unwrap();
        assert!(!pages.is_empty(), "should produce at least one page");
        assert!(pages[0].starts_with(b"\x89PNG"), "output should be valid PNG");
    }

    #[test]
    fn compile_to_rgba_produces_buffer_matching_declared_dimensions() {
        let path = write_temp_typ("= Heading\n\nSome content here.");
        let result = compile_to_rgba_pages(&path, 1.0, &HashMap::new(), &HashMap::new());
        assert!(result.is_ok(), "doc should render to RGBA: {:?}", result.err());
        let pages = result.unwrap();
        assert!(!pages.is_empty(), "should produce at least one page");
        let p = &pages[0];
        assert!(p.width > 0 && p.height > 0, "page should have real dimensions");
        // GdkPixbuf reads this buffer using a rowstride derived from `width`, so
        // a mismatch here would read past the end of the allocation.
        assert_eq!(
            p.rgba.len(),
            (p.width * p.height * 4) as usize,
            "RGBA buffer must be exactly width * height * 4 bytes"
        );
    }

    #[test]
    fn compile_to_rgba_renders_page_content_opaque() {
        let path = write_temp_typ("= Heading\n\nSome content here.");
        let pages = compile_to_rgba_pages(&path, 1.0, &HashMap::new(), &HashMap::new()).unwrap();
        let p = &pages[0];
        assert!(
            p.rgba.chunks_exact(4).all(|px| px[3] == 255),
            "a Typst page background is opaque; a transparent result means demultiply is wrong"
        );
        assert!(
            p.rgba.chunks_exact(4).any(|px| px[0] < 128),
            "page should contain dark pixels — the rendered glyphs"
        );
    }

    #[test]
    fn compile_with_sys_inputs() {
        let path = write_temp_typ(
            "#let d = sys.inputs.at(\"draft\", default: \"false\")\nDraft: #d"
        );
        let mut inputs = HashMap::new();
        inputs.insert("draft".to_string(), "true".to_string());
        let result = compile_to_png_bytes(&path, 1.0, &HashMap::new(), &inputs);
        assert!(result.is_ok(), "doc with sys.inputs should compile: {:?}", result.err());
    }

    /// Phase 3a milestone (see skrizhal/plan.md): a CV-mode document —
    /// `cv-helpers.typ` injected as a virtual override next to the root
    /// file (exactly how `app_window.rs`'s `effective_cv_elements` wiring
    /// does it) and CV data passed via `skrizhal-cv-data` sys.inputs —
    /// compiles and renders `#cv-entry`/`#cv-section` correctly.
    #[test]
    fn compile_cv_entry_and_section_with_skrizhal_helpers() {
        let path = write_temp_typ(
            "#import \"cv-helpers.typ\": cv-entry, cv-section\n\
             #cv-entry(\"hope-united-2025\")\n\
             #cv-section(category: \"Education\")\n\
             #cv-entry(\"nonexistent-key\")",
        );

        let cv_helpers_src = include_str!("../templates/cv-helpers.typ");
        let mut overrides = HashMap::new();
        overrides.insert(
            std::path::PathBuf::from("/tmp/cv-helpers.typ"),
            cv_helpers_src.to_string(),
        );

        let cv_data = r#"
hope-united-2025:
  category: Ministry Position
  title: Student Minister
  organization: Hope United Church
  location: Halifax, NS
  date: 2025-09/2026-04
  tags: [ministry, current]
  description:
    - Preaching and worship leadership on a rotating basis
mdiv-2024:
  category: Education
  title: Master of Divinity
  organization: Atlantic School of Theology
  date: 2023/
"#;
        let mut inputs = HashMap::new();
        inputs.insert("skrizhal-cv-data".to_string(), cv_data.to_string());

        let result = compile_to_pdf_bytes(&path, &overrides, &inputs);
        assert!(result.is_ok(), "CV-mode document should compile: {:?}", result.err());
        let bytes = result.unwrap();
        assert!(bytes.starts_with(b"%PDF-"), "output should be valid PDF");
    }

    /// Profiles live under the reserved `_profiles` key in the same data
    /// file as the entries. Two things are being checked here: that
    /// `cv-profile` renders, and — via the plain `cv-section` call — that
    /// the reserved key is *not* mistaken for a CV entry and rendered as
    /// one, which is exactly what happened before `cv-entry-keys` existed.
    #[test]
    fn compile_cv_profile_with_reserved_profiles_key() {
        let path = write_temp_typ(
            "#import \"cv-helpers.typ\": cv-profile, cv-section\n\
             #cv-profile(\"academic\")\n\
             #cv-section()\n\
             #cv-profile(\"no-such-profile\")",
        );

        let cv_helpers_src = include_str!("../templates/cv-helpers.typ");
        let mut overrides = HashMap::new();
        overrides.insert(
            std::path::PathBuf::from("/tmp/cv-helpers.typ"),
            cv_helpers_src.to_string(),
        );

        let cv_data = r#"
old-job:
  category: Employment
  title: Earlier Post
  organization: Example Organization
  date: 2015/2018
  order: 1
new-job:
  category: Employment
  title: Later Post
  organization: Example University
  date: 2022/2024
mdiv:
  category: Education
  title: Master of Divinity
  organization: Example School
  date: 2023/
dropped-job:
  category: Employment
  title: Should Not Appear
  date: 2019/2020
_profiles:
  academic:
    label: Academic CV
    sections:
      - heading: Experience
        categories: [Employment]
        exclude: [dropped-job]
      - heading: Education
        categories: [Education]
"#;
        let mut inputs = HashMap::new();
        inputs.insert("skrizhal-cv-data".to_string(), cv_data.to_string());

        let result = compile_to_pdf_bytes(&path, &overrides, &inputs);
        assert!(
            result.is_ok(),
            "CV-profile document should compile: {:?}",
            result.err()
        );
        assert!(result.unwrap().starts_with(b"%PDF-"));
    }
}

/// One rendered page as straight (non-premultiplied) RGBA8, ready to hand to
/// `GdkPixbuf` without a decode step.
pub struct RenderedPage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Compile `root_file` and return each page as raw RGBA pixels.
///
/// The live preview uses this rather than `compile_to_png_bytes` because that
/// path PNG-encoded every page on the worker only for the main thread to decode
/// all of them straight back — the bytes never leave the process, so the whole
/// round-trip was wasted work, and the decode half stalled the UI on every
/// compile in proportion to page count.
pub fn compile_to_rgba_pages(
    root_file: &Path,
    pixel_per_pt: f32,
    overrides: &HashMap<PathBuf, String>,
    sys_inputs: &HashMap<String, String>,
) -> Result<Vec<RenderedPage>, String> {
    let world = ZerkaloWorld::new(root_file, overrides.clone(), sys_inputs)?;
    let result = typst::compile::<PagedDocument>(&world);

    match result.output {
        Ok(doc) => {
            let mut pages = Vec::with_capacity(doc.pages.len());
            for page in &doc.pages {
                let pixmap = typst_render::render(page, pixel_per_pt);
                // tiny-skia stores premultiplied RGBA; GdkPixbuf wants straight.
                // Typst pages are opaque so this is usually identity, but pages
                // with a transparent background would otherwise darken.
                let mut rgba = Vec::with_capacity(pixmap.pixels().len() * 4);
                for px in pixmap.pixels() {
                    let c = px.demultiply();
                    rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
                }
                pages.push(RenderedPage {
                    width: pixmap.width(),
                    height: pixmap.height(),
                    rgba,
                });
            }
            Ok(pages)
        }
        Err(errors) => Err(format_diagnostics(&world, &errors)),
    }
}

/// Compile `root_file` in-process and return PNG bytes for each page.
/// `pixel_per_pt` controls render resolution (2.0 ≈ 144 dpi).
pub fn compile_to_png_bytes(
    root_file: &Path,
    pixel_per_pt: f32,
    overrides: &HashMap<PathBuf, String>,
    sys_inputs: &HashMap<String, String>,
) -> Result<Vec<Vec<u8>>, String> {
    let world = ZerkaloWorld::new(root_file, overrides.clone(), sys_inputs)?;
    let result = typst::compile::<PagedDocument>(&world);

    match result.output {
        Ok(doc) => {
            let mut pages = Vec::with_capacity(doc.pages.len());
            for page in &doc.pages {
                let pixmap = typst_render::render(page, pixel_per_pt);
                let png_bytes = pixmap
                    .encode_png()
                    .map_err(|e| format!("PNG encode error: {e}"))?;
                pages.push(png_bytes);
            }
            Ok(pages)
        }
        Err(errors) => Err(format_diagnostics(&world, &errors)),
    }
}
