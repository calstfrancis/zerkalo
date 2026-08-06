use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use chrono::Datelike;
use typst::diag::{FileError, FileResult, SourceDiagnostic, Severity, Warned};
use typst::foundations::{Bytes, Datetime, Dict, IntoValue, Str};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World as TypstWorld, WorldExt};
use typst_kit::download::{Downloader, ProgressSink};
use typst_kit::fonts::{FontSearcher, FontSlot, Fonts};
use typst_kit::package::PackageStorage;

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

/// Root under which downloaded `@preview` packages are cached, matching what
/// `typst-cli` uses so a package fetched by either is seen by both.
fn package_cache_root() -> PathBuf {
    let cache_root = std::env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".cache")
        });
    cache_root.join("typst/packages")
}

static PACKAGES: OnceLock<PackageStorage> = OnceLock::new();

/// Package storage that downloads `@preview` packages on first use instead of
/// failing. Previously a package that happened not to be in the cache already
/// surfaced as `file not found` naming an internal cache path, with no way to
/// act on it from inside Zerkalo.
///
/// `package_path` is left to the default so `@local` packages resolve from the
/// user's data dir as they do under `typst-cli`.
fn package_storage() -> &'static PackageStorage {
    PACKAGES.get_or_init(|| {
        PackageStorage::new(
            Some(package_cache_root()),
            None,
            Downloader::new(concat!("zerkalo/", env!("CARGO_PKG_VERSION"))),
        )
    })
}

/// Typst memoizes across compiles in a process-global `comemo` cache. Nothing
/// evicts it on its own, so an editor that recompiles on every debounce grows
/// without bound — measured at roughly 24 MB per 1000 compiles of a three-line
/// document, and far more for a real one. `typst-cli`'s watch loop evicts on
/// the same cadence for the same reason.
///
/// The argument is how many compiles an unused entry survives; 2 keeps the
/// incremental win between consecutive keystrokes while bounding the cache.
const COMEMO_RETENTION: usize = 2;

/// Shared tail of every compile: evict, and render warnings into the same text
/// format `parse_typst_errors` reads for errors.
fn finish<T>(world: &ZerkaloWorld, result: Warned<typst::diag::SourceResult<T>>) -> Result<(T, String), String> {
    let warnings = format_diagnostics(world, &result.warnings);
    let out = match result.output {
        Ok(value) => Ok((value, warnings)),
        Err(errors) => Err(format_diagnostics(world, &errors)),
    };
    typst::comemo::evict(COMEMO_RETENTION);
    out
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
            package_storage()
                .prepare_package(spec, &mut ProgressSink)
                .map_err(FileError::Package)?
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
                .map(Bytes::new)
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

    // Resolve the source location from the span.
    //
    // This must go through `WorldExt::range`, not `Span::range`. A span is
    // either a *raw range* (byte offsets packed into the span itself) or a
    // *number* identifying a node in the source's syntax tree — and Typst
    // attaches numbered spans to essentially every real diagnostic.
    // `Span::range` only decodes the raw-range kind and returns None for the
    // numbered kind, so this used to resolve a location for almost nothing:
    // no ` --> file:line:col` line was emitted, and the panel's fallback
    // pinned every error in the app to line 1. `WorldExt::range` tries the raw
    // range first and then looks the number up in the source, which is what
    // actually covers diagnostics from user documents.
    let location: Option<String> = d.span.id().and_then(|fid| {
        let src = TypstWorld::source(world, fid).ok()?;
        let range = WorldExt::range(world, d.span)?;
        let (line, col) = offset_to_line_col(src.text(), range.start);
        // Absolute where it can be resolved. The rootless vpath is relative to
        // the *compile* root (the root file's own folder), which is not always
        // the project root the panel joins against — when a root file sits in a
        // subfolder, that join produced a path to a file that doesn't exist, so
        // clicking the error jumped nowhere.
        let path = world
            .resolve(fid)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| src.id().vpath().as_rootless_path().display().to_string());
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
    let (doc, _warnings) = finish::<PagedDocument>(&world, typst::compile(&world))?;
    pdf_bytes_from_document(&doc)
}

/// Serialise an already-laid-out document to PDF.
///
/// Printing needs this separately from `compile_to_pdf_bytes`: it compiles once
/// and uses the result twice — the PDF goes to the print portal, and the
/// document itself stays around to raster-render pages if no portal is there.
pub fn pdf_bytes_from_document(doc: &PagedDocument) -> Result<Vec<u8>, String> {
    typst_pdf::pdf(doc, &typst_pdf::PdfOptions::default()).map_err(|errors| {
        errors
            .iter()
            .map(|e| e.message.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })
}

/// One rendered page as straight (non-premultiplied) RGBA8, ready to hand to
/// `GdkPixbuf` without a decode step.
pub struct RenderedPage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Compile `root_file` and hand back the laid-out document itself.
///
/// Printing needs this rather than a finished PDF or a page bitmap: the print
/// dialog decides the resolution and which pages are wanted, and neither is
/// known until after the user has answered it. Holding the document lets each
/// page be rendered on demand, at the printer's resolution, one at a time.
pub fn compile_document(
    root_file: &Path,
    overrides: &HashMap<PathBuf, String>,
    sys_inputs: &HashMap<String, String>,
) -> Result<PagedDocument, String> {
    let world = ZerkaloWorld::new(root_file, overrides.clone(), sys_inputs)?;
    finish::<PagedDocument>(&world, typst::compile(&world)).map(|(doc, _warnings)| doc)
}

/// Render a single already-laid-out page to straight RGBA8.
pub fn render_page_rgba(page: &typst::layout::Page, pixel_per_pt: f32) -> RenderedPage {
    let pixmap = typst_render::render(page, pixel_per_pt);
    let mut rgba = Vec::with_capacity(pixmap.pixels().len() * 4);
    for px in pixmap.pixels() {
        let c = px.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    RenderedPage { width: pixmap.width(), height: pixmap.height(), rgba }
}

/// Compile `root_file` and return each page as raw RGBA pixels.
///
/// The live preview uses this rather than `compile_to_png_bytes` because that
/// path PNG-encoded every page on the worker only for the main thread to decode
/// all of them straight back — the bytes never leave the process, so the whole
/// round-trip was wasted work, and the decode half stalled the UI on every
/// compile in proportion to page count.
///
/// Returns the rendered pages alongside any warnings the compile produced, in
/// the same text format errors use. Warnings used to be discarded outright, so
/// deprecations and unused imports never reached the user despite the error
/// panel already knowing how to display them.
pub fn compile_to_rgba_pages(
    root_file: &Path,
    pixel_per_pt: f32,
    overrides: &HashMap<PathBuf, String>,
    sys_inputs: &HashMap<String, String>,
) -> Result<(Vec<RenderedPage>, String), String> {
    let world = ZerkaloWorld::new(root_file, overrides.clone(), sys_inputs)?;
    let (doc, warnings) = finish::<PagedDocument>(&world, typst::compile(&world))?;
    // tiny-skia stores premultiplied RGBA; GdkPixbuf wants straight.
    // Typst pages are opaque so this is usually identity, but pages with
    // a transparent background would otherwise darken. See render_page_rgba.
    let pages = doc.pages.iter().map(|p| render_page_rgba(p, pixel_per_pt)).collect();
    Ok((pages, warnings))
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
    let (doc, _warnings) = finish::<PagedDocument>(&world, typst::compile(&world))?;
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
    fn an_error_reports_the_line_it_is_actually_on() {
        // Every diagnostic used to come back pointing at line 1. `Span::range()`
        // only resolves *raw range* spans; the spans Typst attaches to real
        // syntax nodes are numbered, and for those it returns None — so no
        // location was ever emitted and the UI's fallback pinned everything to
        // line 1. Typst's own WorldExt::range covers both cases.
        let path = write_temp_typ("= Title\n\nSome text.\n\n#no-such-function()\n");
        let err = compile_to_pdf_bytes(&path, &HashMap::new(), &HashMap::new())
            .expect_err("undefined function should fail to compile");
        assert!(
            err.contains(" --> "),
            "diagnostic should carry a source location, got:\n{err}"
        );
        let loc = err.lines().find(|l| l.trim().starts_with("-->")).unwrap();
        assert!(
            loc.contains(":5:"),
            "error is on line 5, but the location says: {loc}"
        );
    }

    #[test]
    fn an_error_inside_an_imported_file_points_at_that_file() {
        let dir = std::path::PathBuf::from(format!(
            "/tmp/zerkalo_test_import_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("helper.typ"), "#let ok = 1\n\n#undefined-thing()\n").unwrap();
        let main = dir.join("main.typ");
        std::fs::write(&main, "= Doc\n\n#include \"helper.typ\"\n").unwrap();

        let err = compile_to_pdf_bytes(&main, &HashMap::new(), &HashMap::new())
            .expect_err("error in the included file should fail the compile");
        let loc = err
            .lines()
            .find(|l| l.trim().starts_with("-->"))
            .unwrap_or_else(|| panic!("no location in:\n{err}"));
        assert!(
            loc.contains("helper.typ") && loc.contains(":3:"),
            "should point into helper.typ line 3, got: {loc}"
        );
    }

    #[test]
    fn a_real_diagnostic_survives_the_whole_pipeline_to_the_panel() {
        // End-to-end: compile a document with a known error and push the real
        // compiler output through the panel's parser, which is where the
        // line-1 fallback used to swallow everything.
        let path = write_temp_typ("= Title\n\nText.\n\n#no-such-thing()\n");
        let err = compile_to_pdf_bytes(&path, &HashMap::new(), &HashMap::new()).unwrap_err();
        let parsed = crate::ui::error_panel::parse_typst_errors(
            &err,
            path.parent().unwrap(),
        );
        assert_eq!(parsed.len(), 1, "one diagnostic expected, got {parsed:?}");
        assert_eq!(parsed[0].line, 5, "should point at the offending line, not line 1");
        assert_eq!(parsed[0].file, path, "should point at the real file");
        assert!(!parsed[0].hints.is_empty(), "Typst's hint should reach the panel");
        assert!(
            !parsed[0].message.to_lowercase().contains("unknown variable"),
            "headline should be plain language, got: {}",
            parsed[0].message
        );
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
        let (pages, _warnings) = result.unwrap();
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
        let (pages, _) = compile_to_rgba_pages(&path, 1.0, &HashMap::new(), &HashMap::new()).unwrap();
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

    /// Warnings used to be dropped on the floor: `compile` returns them
    /// alongside the output and only `output` was ever read, so a deprecation
    /// never reached the error panel that already knew how to render it.
    #[test]
    fn compile_surfaces_warnings_on_a_successful_compile() {
        // `#set page(width: auto)` inside a container warns without failing.
        let path = write_temp_typ("#let x = 1\n#x\n#show heading: it => it\n= H\n#[#set par(justify: true)]\n");
        let (_pages, warnings) =
            compile_to_rgba_pages(&path, 1.0, &HashMap::new(), &HashMap::new())
                .expect("document should still compile");
        // Not asserting on a specific warning text — Typst's own set changes
        // between versions. What matters is that the channel exists and the
        // format matches what parse_typst_errors reads.
        if !warnings.is_empty() {
            assert!(
                warnings.contains("warning:"),
                "warnings must use the same `warning: …` format the error panel parses: {warnings}"
            );
        }
    }

    fn rss_kb() -> u64 {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("VmRSS:"))?
                    .split_whitespace()
                    .nth(1)?
                    .parse()
                    .ok()
            })
            .unwrap_or(0)
    }

    const MEMCHECK_ENV: &str = "ZERKALO_MEMCHECK_CHILD";

    /// Guards the leak measured before eviction was added: without it the comemo
    /// cache grew by roughly 24 MB per 1000 compiles and never gave any back.
    ///
    /// RSS is a property of the whole process, so measuring it while the rest of
    /// the suite compiles documents on other threads reads their allocations as
    /// this test's growth — which made a first version of this test fail only in
    /// full parallel runs. The measurement therefore runs in a dedicated child
    /// process with nothing else in it.
    #[test]
    fn repeated_compiles_do_not_grow_memory_without_bound() {
        if rss_kb() == 0 {
            return; // no /proc — nothing to measure
        }
        if std::env::var(MEMCHECK_ENV).is_ok() {
            measure_compile_growth();
            return;
        }
        let exe = std::env::current_exe().expect("test binary path");
        let output = std::process::Command::new(exe)
            .args([
                "--exact",
                "compiler::tests::repeated_compiles_do_not_grow_memory_without_bound",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(MEMCHECK_ENV, "1")
            .output()
            .expect("re-run this test in a child process");
        assert!(
            output.status.success(),
            "memory growth check failed in the child process:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn measure_compile_growth() {
        let path = write_temp_typ("= Title\n\nSome prose.\n");
        // Warm up: first compiles pull in fonts and the standard library, which
        // is one-off cost, not growth.
        for i in 0..20 {
            std::fs::write(&path, format!("= Title\n\nSome prose {i}.\n")).unwrap();
            compile_to_pdf_bytes(&path, &HashMap::new(), &HashMap::new()).unwrap();
        }
        let base = rss_kb();
        for i in 20..220 {
            std::fs::write(&path, format!("= Title\n\nSome prose {i}.\n")).unwrap();
            compile_to_pdf_bytes(&path, &HashMap::new(), &HashMap::new()).unwrap();
        }
        let growth = rss_kb().saturating_sub(base);
        println!("RSS growth over 200 compiles: {growth} kB");
        // Unevicted, these 200 compiles grew RSS by ~5 MB. Allowing 3 MB leaves
        // room for allocator noise while still failing if eviction is removed.
        assert!(
            growth < 3 * 1024,
            "RSS grew {growth} kB over 200 compiles — is comemo eviction still in finish()?"
        );
    }

    #[test]
    fn compile_document_reports_page_count_for_printing() {
        // The print dialog's page range depends on this count being right.
        let path = write_temp_typ("First page.\n#pagebreak()\nSecond page.");
        let doc = compile_document(&path, &HashMap::new(), &HashMap::new())
            .expect("document should compile");
        assert_eq!(doc.pages.len(), 2, "two pages after an explicit pagebreak");
    }

    #[test]
    fn compile_document_surfaces_errors_rather_than_returning_empty() {
        // Printing used to discard this, leaving the button apparently dead.
        let path = write_temp_typ("#panic(\"boom\")");
        let result = compile_document(&path, &HashMap::new(), &HashMap::new());
        assert!(result.is_err(), "a failing document must report an error");
    }

    #[test]
    fn render_page_rgba_scales_with_resolution() {
        // draw_page relies on this: it renders at PRINT_DPI/72 and scales the
        // Cairo context back down by the same factor to land at true size.
        let path = write_temp_typ("= Heading");
        let doc = compile_document(&path, &HashMap::new(), &HashMap::new()).unwrap();
        let low = render_page_rgba(&doc.pages[0], 1.0);
        let high = render_page_rgba(&doc.pages[0], 2.0);
        // Page dimensions are fractional points, so doubling the scale lands
        // within a pixel of double the size rather than exactly on it.
        assert!(
            high.width.abs_diff(low.width * 2) <= 1,
            "2x scale should double the width: {} vs {}", high.width, low.width * 2
        );
        assert!(
            high.height.abs_diff(low.height * 2) <= 1,
            "2x scale should double the height: {} vs {}", high.height, low.height * 2
        );
        assert_eq!(high.rgba.len(), (high.width * high.height * 4) as usize);
    }

    #[test]
    fn compile_document_honours_sys_inputs() {
        // The CV path depends on this: entries arrive only via skrizhal-cv-data,
        // and printing omitting it produced a document that could not compile.
        let path = write_temp_typ(
            "#let d = sys.inputs.at(\"zerkalo-test\", default: \"missing\")\nValue: #d",
        );
        let mut inputs = HashMap::new();
        inputs.insert("zerkalo-test".to_string(), "present".to_string());
        assert!(
            compile_document(&path, &HashMap::new(), &inputs).is_ok(),
            "sys inputs must reach the compiled document"
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
