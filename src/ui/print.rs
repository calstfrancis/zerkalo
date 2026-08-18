use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{PrintOperation, PrintOperationAction, Window};
use typst::layout::PagedDocument;

use crate::config::{DuplexPref, PrintPrefs};
use crate::print_layout::{Imposition, PageNumbering, PaperSpec, physical_ranges_string};

/// Typst's layout unit. Cairo print contexts are scaled so one unit is one
/// point, so this converts between the two.
const POINTS_PER_INCH: f64 = 72.0;

/// Bounds on the resolution the GTK fallback rasterises at.
///
/// The printer's own DPI is used where it is sane. Below the floor, output
/// looks visibly soft; above the ceiling, a full-page bitmap costs hundreds of
/// megabytes for no visible gain — a 1200 dpi A4 page is around 560 MB of RGBA.
const MIN_PRINT_DPI: f64 = 150.0;
const MAX_PRINT_DPI: f64 = 600.0;
const FALLBACK_PRINT_DPI: f64 = 300.0;

pub struct PrintRequest {
    pub root: PathBuf,
    pub overrides: HashMap<PathBuf, String>,
    pub sys_inputs: HashMap<String, String>,
    /// The configured bibliography path, if any — needed so the compile can
    /// widen its sandbox root when it points outside the project (see
    /// `compiler::ZerkaloWorld::new`'s `extra_root`).
    pub bib_path: Option<PathBuf>,
    /// Shown as the job name in the printer queue.
    pub job_name: String,
}

impl PrintRequest {
    /// Identity of the *content* this request would compile, used to decide
    /// whether an earlier preparation still applies. Deliberately excludes the
    /// job name, which does not affect what gets compiled.
    fn cache_key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.root.hash(&mut hasher);
        // HashMap iteration order varies between runs, so hash the entries in
        // a fixed order or the key is useless as a cache key.
        let mut overrides: Vec<_> = self.overrides.iter().collect();
        overrides.sort_by_key(|(path, _)| *path);
        for (path, text) in overrides {
            path.hash(&mut hasher);
            text.hash(&mut hasher);
        }
        let mut inputs: Vec<_> = self.sys_inputs.iter().collect();
        inputs.sort_by_key(|(key, _)| *key);
        for (key, value) in inputs {
            key.hash(&mut hasher);
            value.hash(&mut hasher);
        }
        hasher.finish()
    }
}

/// What the print sheet needs to know once the document is compiled.
pub struct Prepared {
    pub pdf: Vec<u8>,
    /// Kept for the raster fallback, and for rendering the sheet's thumbnail.
    pub doc: PagedDocument,
    pub paper: PaperSpec,
    pub numbering: PageNumbering,
}

impl Prepared {
    fn from_document(doc: PagedDocument, pdf: Vec<u8>) -> Result<Self, String> {
        let sizes: Vec<(f64, f64)> = doc
            .pages
            .iter()
            .map(|p| {
                let size = p.frame.size();
                (size.x.to_pt(), size.y.to_pt())
            })
            .collect();
        let paper = PaperSpec::from_page_sizes(&sizes)
            .ok_or_else(|| "The document has no pages to print.".to_string())?;
        let numbering = PageNumbering::new(doc.pages.iter().map(|p| p.number).collect());
        Ok(Prepared { pdf, doc, paper, numbering })
    }
}

// The most recent successful preparation, kept so that reopening the print
// sheet — or changing an option in it and printing again — doesn't recompile a
// document that hasn't changed. Compiling a long document takes seconds, and
// adjusting the copy count shouldn't cost that.
//
// Only one entry: printing the same document repeatedly is the case worth
// serving, and holding several laid-out documents alive is real memory.
thread_local! {
    static LAST_PREPARED: RefCell<Option<(u64, Rc<Prepared>)>> = const { RefCell::new(None) };
}

/// Discard any cached preparation. Called when a document is closed or the
/// project changes, so a stale layout can't outlive the file it came from.
pub fn invalidate_cache() {
    LAST_PREPARED.with(|c| *c.borrow_mut() = None);
}

/// Handle to an in-progress preparation.
///
/// Typst offers no way to abort a compile, so cancelling detaches from the
/// result rather than stopping the work — the compile finishes into the cache,
/// where a later print picks it up for free instead of starting over.
pub struct Preparation {
    cancelled: Rc<std::cell::Cell<bool>>,
}

impl Preparation {
    pub fn cancel(&self) {
        self.cancelled.set(true);
    }
}

/// Compile the document, or hand back the cached result immediately.
///
/// `on_ready` is called exactly once unless the preparation is cancelled first.
/// It is called synchronously on a cache hit, so callers must be ready to be
/// re-entered before this function returns.
pub fn prepare(request: &PrintRequest, on_ready: impl Fn(Result<Rc<Prepared>, String>) + 'static)
    -> Preparation
{
    let key = request.cache_key();
    let cancelled = Rc::new(std::cell::Cell::new(false));

    if let Some(hit) = LAST_PREPARED.with(|c| {
        c.borrow().as_ref().filter(|(cached, _)| *cached == key).map(|(_, p)| p.clone())
    }) {
        on_ready(Ok(hit));
        return Preparation { cancelled };
    }

    let (tx, rx) = mpsc::sync_channel::<Result<(PagedDocument, Vec<u8>), String>>(1);
    let root = request.root.clone();
    let overrides = request.overrides.clone();
    let sys_inputs = request.sys_inputs.clone();
    let bib_path = request.bib_path.clone();
    std::thread::spawn(move || {
        let prepared = crate::compiler::compile_document(&root, &overrides, &sys_inputs, bib_path.as_deref())
            .and_then(|doc| {
                crate::compiler::pdf_bytes_from_document(&doc).map(|pdf| (doc, pdf))
            });
        tx.send(prepared).ok();
    });

    let flag = cancelled.clone();
    glib::timeout_add_local(Duration::from_millis(80), move || {
        match rx.try_recv() {
            Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(TryRecvError::Disconnected) => {
                if !flag.get() {
                    on_ready(Err(
                        "The compiler stopped unexpectedly while preparing to print.".into()
                    ));
                }
                glib::ControlFlow::Break
            }
            Ok(result) => {
                let prepared = result.and_then(|(doc, pdf)| Prepared::from_document(doc, pdf));
                match prepared {
                    Ok(prepared) => {
                        let prepared = Rc::new(prepared);
                        // Cache even when cancelled: the work is already paid
                        // for, and the next print gets it instantly.
                        LAST_PREPARED
                            .with(|c| *c.borrow_mut() = Some((key, prepared.clone())));
                        if !flag.get() {
                            on_ready(Ok(prepared));
                        }
                    }
                    Err(msg) => {
                        if !flag.get() {
                            on_ready(Err(msg));
                        }
                    }
                }
                glib::ControlFlow::Break
            }
        }
    });

    Preparation { cancelled }
}

// ── Sending a prepared document to the printer ───────────────────────────────

/// Everything the user chose on the print sheet.
pub struct PrintJob {
    pub job_name: String,
    /// Physical page indices to print, in order.
    pub pages: Vec<usize>,
    pub imposition: Imposition,
    pub prefs: PrintPrefs,
}

pub enum PrintStatus {
    Failed(String),
    Cancelled,
    Sent,
}

/// Impose (if asked) and send to the printer.
///
/// Prefers the desktop print portal, which takes the PDF itself and so keeps
/// text as vectors at the printer's own resolution. Falls back to GTK's print
/// dialog with pages raster-rendered when no portal answers — GTK draws through
/// Cairo and Typst has no Cairo backend, so that path cannot preserve vectors.
pub fn send_to_printer(
    parent: &Window,
    prepared: &Rc<Prepared>,
    job: PrintJob,
    on_status: impl Fn(PrintStatus) + 'static,
) {
    let on_status: Rc<dyn Fn(PrintStatus)> = Rc::new(on_status);

    if job.pages.is_empty() {
        on_status(PrintStatus::Failed("No pages were selected to print.".into()));
        return;
    }

    let pdf = match crate::imposition::impose(&prepared.pdf, &job.pages, job.imposition) {
        Ok(pdf) => pdf,
        Err(e) => {
            on_status(PrintStatus::Failed(e));
            return;
        }
    };

    // Imposition already selected and reordered the pages, so the printer must
    // be told to print all of what it is given — passing the range as well
    // would apply it twice.
    let ranges = if job.imposition == Imposition::Off && job.pages.len() != prepared.numbering.len()
    {
        Some(physical_ranges_string(&job.pages))
    } else {
        None
    };
    // A booklet's sheets are landscape even though the document is portrait;
    // the dialog must open on the paper actually being fed.
    let paper = sheet_paper(prepared.paper, job.imposition);

    let (tx, rx) = mpsc::sync_channel::<PortalOutcome>(1);
    let title = job.job_name.clone();
    let prefs = job.prefs.clone();
    // ashpd is async and the GTK main loop is glib, so the portal conversation
    // runs on its own current-thread tokio runtime and reports back by channel —
    // the same worker-plus-poll shape used elsewhere in the app.
    std::thread::spawn(move || {
        let outcome = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt.block_on(print_via_portal(&pdf, &title, paper, &prefs, ranges.as_deref())),
            Err(e) => PortalOutcome::Unavailable(format!("no async runtime: {e}")),
        };
        tx.send(outcome).ok();
    });

    let parent = parent.clone();
    let prepared = prepared.clone();
    let job_name = job.job_name.clone();
    let pages = job.pages.clone();
    glib::timeout_add_local(Duration::from_millis(100), move || match rx.try_recv() {
        Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(TryRecvError::Disconnected) => {
            on_status(PrintStatus::Failed("The print helper stopped unexpectedly.".into()));
            glib::ControlFlow::Break
        }
        Ok(outcome) => {
            match outcome {
                PortalOutcome::Sent => on_status(PrintStatus::Sent),
                PortalOutcome::Cancelled => on_status(PrintStatus::Cancelled),
                PortalOutcome::Failed(msg) => on_status(PrintStatus::Failed(msg)),
                PortalOutcome::Unavailable(reason) => {
                    tracing::info!("print portal unavailable ({reason}); using GTK print dialog");
                    run_gtk_print_dialog(&parent, &prepared, &pages, paper, &job_name, &on_status);
                }
            }
            glib::ControlFlow::Break
        }
    });
}

/// The paper an imposed job is printed on, as opposed to the document's own
/// page size. Two-up and booklet turn a portrait page into a landscape sheet.
fn sheet_paper(page: PaperSpec, imposition: Imposition) -> PaperSpec {
    if imposition.rotates_sheet() {
        PaperSpec { width_pt: page.height_pt, height_pt: page.width_pt, uniform: page.uniform }
    } else {
        page
    }
}

/// Outcome of the portal attempt, reported back to the GTK thread.
enum PortalOutcome {
    Sent,
    Cancelled,
    /// No portal answered — fall back to GTK's own print dialog.
    Unavailable(String),
    Failed(String),
}

async fn print_via_portal(
    pdf: &[u8],
    title: &str,
    paper: PaperSpec,
    prefs: &PrintPrefs,
    ranges: Option<&str>,
) -> PortalOutcome {
    use ashpd::desktop::print::{
        Duplex, Orientation, PageSetup, PreparePrintOptions, PrintOptions, PrintPages, PrintProxy,
        Settings,
    };

    let proxy = match PrintProxy::new().await {
        Ok(p) => p,
        Err(e) => return PortalOutcome::Unavailable(e.to_string()),
    };

    let file = match pdf_to_temp_file(pdf) {
        Ok(f) => f,
        Err(e) => return PortalOutcome::Failed(format!("Couldn't stage the document: {e}")),
    };

    // Without this the dialog opens on the desktop's default paper whatever
    // the document is, so anything that isn't A4/Letter gets silently scaled
    // or clipped.
    let (width_mm, height_mm) = paper.portrait_mm();
    let page_setup = PageSetup::default()
        .set_width(width_mm)
        .set_height(height_mm)
        .set_orientation(if paper.is_landscape() {
            Orientation::Landscape
        } else {
            Orientation::Portrait
        })
        // Typst lays out margins inside the page, so the printable area is the
        // whole sheet; declaring margins here would inset the content twice.
        .set_margin_top(0.0)
        .set_margin_bottom(0.0)
        .set_margin_left(0.0)
        .set_margin_right(0.0);

    let mut settings = Settings::default()
        .set_n_copies(prefs.copies.max(1))
        .set_collate(prefs.collate)
        .set_use_color(prefs.color)
        .set_orientation(if paper.is_landscape() {
            Orientation::Landscape
        } else {
            Orientation::Portrait
        });
    settings = match prefs.duplex {
        DuplexPref::Printer => settings,
        DuplexPref::OneSided => settings.set_duplex(Duplex::Simplex),
        DuplexPref::LongEdge => settings.set_duplex(Duplex::Horizontal),
        DuplexPref::ShortEdge => settings.set_duplex(Duplex::Vertical),
    };
    if let Some(ranges) = ranges {
        settings = settings.set_print_pages(PrintPages::Ranges).set_page_ranges(ranges);
    }

    // PreparePrint shows the settings dialog and hands back a token that
    // authorises the actual Print call with those settings.
    let prepared = match proxy
        .prepare_print(
            None,
            title,
            settings,
            page_setup,
            // Zerkalo resolves ranges itself, in the document's own numbering,
            // before the dialog opens — so the dialog's own "current page" and
            // "selection" would contradict what was already chosen.
            PreparePrintOptions::default()
                .set_modal(true)
                .set_has_current_page(false)
                .set_has_selected_pages(false),
        )
        .await
    {
        Ok(request) => match request.response() {
            Ok(r) => r,
            Err(ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)) => {
                return PortalOutcome::Cancelled
            }
            Err(e) => return PortalOutcome::Failed(e.to_string()),
        },
        Err(e) => return PortalOutcome::Unavailable(e.to_string()),
    };

    use std::os::fd::AsFd;
    match proxy
        .print(
            None,
            title,
            &file.as_fd(),
            PrintOptions::default().set_token(prepared.token).set_modal(true),
        )
        .await
    {
        Ok(_) => PortalOutcome::Sent,
        Err(ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)) => {
            PortalOutcome::Cancelled
        }
        Err(e) => PortalOutcome::Failed(e.to_string()),
    }
}

/// Write the PDF somewhere the portal can read it by file descriptor, then
/// unlink it immediately. The descriptor stays valid for as long as the file is
/// open, so nothing is left on disk — the old print path wrote every document
/// to a predictable `~/.cache/zerkalo/<stem>.pdf` that persisted and collided
/// between projects.
fn pdf_to_temp_file(pdf: &[u8]) -> std::io::Result<std::fs::File> {
    stage_pdf_at(&staging_path(), pdf)
}

fn staging_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "zerkalo-print-{}-{:?}.pdf",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ))
}

/// Split out from `pdf_to_temp_file` so tests can name the path they are
/// checking. Asserting on a scan of the whole temp directory raced against the
/// other staging tests, which create and unlink files there at the same time.
fn stage_pdf_at(path: &std::path::Path, pdf: &[u8]) -> std::io::Result<std::fs::File> {
    use std::io::{Seek, SeekFrom, Write};

    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(path)?;
    file.write_all(pdf)?;
    file.flush()?;
    file.seek(SeekFrom::Start(0))?;
    std::fs::remove_file(path)?;
    Ok(file)
}

// ── GTK fallback: raster printing when no portal is available ────────────────

fn run_gtk_print_dialog(
    parent: &Window,
    prepared: &Rc<Prepared>,
    pages: &[usize],
    paper: PaperSpec,
    job_name: &str,
    on_status: &Rc<dyn Fn(PrintStatus)>,
) {
    let op = PrintOperation::new();
    op.set_job_name(job_name);
    op.set_n_pages(pages.len() as i32);
    op.set_embed_page_setup(true);
    op.set_allow_async(true);
    // Same reason as the portal path: without a page setup derived from the
    // document, GTK prints onto the desktop's default paper.
    op.set_default_page_setup(Some(&gtk_page_setup(paper)));

    // The GTK fallback draws pages one at a time and cannot compose several
    // onto a sheet, so imposition is silently unavailable here. It is only
    // reached when no portal answers, which on a Flatpak install means no
    // printing at all — see the note in `packaging/…Zerkalo.yml`.
    let prepared = prepared.clone();
    let pages = pages.to_vec();
    op.connect_draw_page(move |_, ctx, nth| {
        let Some(index) = pages.get(nth as usize) else { return };
        let Some(page) = prepared.doc.pages.get(*index) else { return };
        draw_page(&ctx.cairo_context(), page, raster_dpi(ctx));
    });

    // With allow_async set, `run` returns InProgress and the real outcome
    // arrives on `done` — so all reporting happens there, in both the sync and
    // async cases. `run` itself only reports a failure to start.
    let status_for_done = on_status.clone();
    op.connect_done(move |op, result| match result {
        gtk4::PrintOperationResult::Cancel => status_for_done(PrintStatus::Cancelled),
        gtk4::PrintOperationResult::Error => {
            let msg = op
                .error()
                .map(|e| e.to_string())
                .unwrap_or_else(|| "The print job failed.".into());
            status_for_done(PrintStatus::Failed(msg));
        }
        _ => status_for_done(PrintStatus::Sent),
    });

    if let Err(e) = op.run(PrintOperationAction::PrintDialog, Some(parent)) {
        on_status(PrintStatus::Failed(format!("Couldn't open the print dialog: {e}")));
    }
}

fn gtk_page_setup(paper: PaperSpec) -> gtk4::PageSetup {
    let (width_mm, height_mm) = paper.portrait_mm();
    let size = gtk4::PaperSize::new_custom(
        "zerkalo-document",
        "Document page size",
        width_mm,
        height_mm,
        gtk4::Unit::Mm,
    );
    let setup = gtk4::PageSetup::new();
    setup.set_paper_size(&size);
    setup.set_orientation(if paper.is_landscape() {
        gtk4::PageOrientation::Landscape
    } else {
        gtk4::PageOrientation::Portrait
    });
    // Typst puts the margins inside the page; the printable area is the sheet.
    setup.set_top_margin(0.0, gtk4::Unit::Mm);
    setup.set_bottom_margin(0.0, gtk4::Unit::Mm);
    setup.set_left_margin(0.0, gtk4::Unit::Mm);
    setup.set_right_margin(0.0, gtk4::Unit::Mm);
    setup
}

/// The resolution to rasterise at, taken from the printer rather than assumed.
///
/// A fixed 300 dpi downsampled every 600 dpi printer and made large-format
/// pages enormous. The context reports the real figure; the clamp keeps a
/// misreporting driver from asking for a bitmap that won't fit in memory.
fn raster_dpi(ctx: &gtk4::PrintContext) -> f64 {
    let reported = ctx.dpi_x().max(ctx.dpi_y());
    if reported.is_finite() && reported >= 1.0 {
        reported.clamp(MIN_PRINT_DPI, MAX_PRINT_DPI)
    } else {
        FALLBACK_PRINT_DPI
    }
}

fn draw_page(cr: &gtk4::cairo::Context, page: &typst::layout::Page, dpi: f64) {
    let scale = (dpi / POINTS_PER_INCH) as f32;
    let rendered = crate::compiler::render_page_rgba(page, scale);
    if rendered.width == 0 || rendered.height == 0 {
        return;
    }

    // Cairo has no straight-RGBA input format, so hand the pixels to GdkPixbuf
    // (which does) and let gdk4 convert. The buffer is dropped as soon as the
    // page is painted, so only one page is ever resident.
    let rowstride = (rendered.width * 4) as i32;
    let pixbuf = gtk4::gdk_pixbuf::Pixbuf::from_bytes(
        &glib::Bytes::from_owned(rendered.rgba),
        gtk4::gdk_pixbuf::Colorspace::Rgb,
        true,
        8,
        rendered.width as i32,
        rendered.height as i32,
        rowstride,
    );

    // The context is scaled in points; the bitmap is at `dpi`. Scaling down by
    // the same factor it was rendered up by lands the page at its true size.
    let inv = POINTS_PER_INCH / dpi;
    cr.save().ok();
    cr.scale(inv, inv);
    cr.set_source_pixbuf(&pixbuf, 0.0, 0.0);
    cr.paint().ok();
    cr.restore().ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn staged_pdf_is_readable_after_being_unlinked() {
        // The portal receives only a file descriptor, so the staged file is
        // unlinked immediately and must stay readable through the fd. If this
        // broke, printing would silently send an empty document.
        let payload = b"%PDF-1.7\ntest body\n";
        let mut file = pdf_to_temp_file(payload).expect("staging should succeed");
        let mut back = Vec::new();
        file.read_to_end(&mut back).expect("fd should still be readable");
        assert_eq!(back, payload, "content must survive the unlink");
    }

    #[test]
    fn staged_pdf_leaves_nothing_behind() {
        // Checked by name rather than by counting files in the temp directory:
        // the sibling staging tests create and unlink files there at the same
        // time, so a before/after count failed at random.
        let path = std::env::temp_dir()
            .join(format!("zerkalo-print-leaves-nothing-{}.pdf", std::process::id()));
        let file = stage_pdf_at(&path, b"%PDF-1.7\n").unwrap();
        assert!(!path.exists(), "the staged file must not persist on disk");
        drop(file);
        assert!(!path.exists(), "and must not reappear when the handle is dropped");
    }

    #[test]
    fn staging_paths_do_not_collide() {
        // Two documents staged in the same moment must not land on the same
        // path — `create_new` would make the second fail rather than overwrite,
        // which would surface as a print that silently did nothing.
        let first = staging_path();
        let second = staging_path();
        assert_ne!(first, second);
    }

    #[test]
    fn staged_pdf_reads_from_the_start() {
        // Written then rewound: a missing seek would hand the portal a
        // descriptor positioned at EOF, i.e. a zero-length document.
        let mut file = pdf_to_temp_file(b"%PDF-1.7\nabc").unwrap();
        let mut first = [0u8; 5];
        file.read_exact(&mut first).expect("should read from offset 0");
        assert_eq!(&first, b"%PDF-");
    }

    // ── Cache keys ───────────────────────────────────────────────────────────

    fn request(root: &str, overrides: &[(&str, &str)]) -> PrintRequest {
        PrintRequest {
            root: PathBuf::from(root),
            overrides: overrides
                .iter()
                .map(|(p, t)| (PathBuf::from(p), (*t).to_string()))
                .collect(),
            sys_inputs: HashMap::new(),
            bib_path: None,
            job_name: "job".into(),
        }
    }

    #[test]
    fn the_same_content_yields_the_same_cache_key() {
        // Two requests built independently must agree, or the cache never hits
        // and every print recompiles.
        assert_eq!(
            request("a.typ", &[("a.typ", "hello")]).cache_key(),
            request("a.typ", &[("a.typ", "hello")]).cache_key()
        );
    }

    #[test]
    fn map_ordering_does_not_change_the_cache_key() {
        // HashMap iteration order varies run to run; hashing entries in that
        // order would make the key unstable and the cache useless.
        let many: Vec<(&str, &str)> =
            vec![("a.typ", "1"), ("b.typ", "2"), ("c.typ", "3"), ("d.typ", "4")];
        let mut reversed = many.clone();
        reversed.reverse();
        assert_eq!(request("a.typ", &many).cache_key(), request("a.typ", &reversed).cache_key());
    }

    #[test]
    fn edited_content_invalidates_the_cache_key() {
        // The whole point: an edit must not print the previous version.
        assert_ne!(
            request("a.typ", &[("a.typ", "hello")]).cache_key(),
            request("a.typ", &[("a.typ", "hello!")]).cache_key()
        );
        assert_ne!(
            request("a.typ", &[]).cache_key(),
            request("b.typ", &[]).cache_key()
        );
    }

    #[test]
    fn the_job_name_does_not_affect_the_cache_key() {
        let mut renamed = request("a.typ", &[]);
        let original = renamed.cache_key();
        renamed.job_name = "something else".into();
        assert_eq!(renamed.cache_key(), original, "renaming a job doesn't change its content");
    }

    #[test]
    fn sys_inputs_are_part_of_the_cache_key() {
        // CV documents reach the compiler entirely through sys inputs; missing
        // them here would print a stale CV after the data file changed.
        let mut with_cv = request("a.typ", &[]);
        with_cv.sys_inputs.insert("skrizhal-cv-data".into(), "name: A".into());
        let mut other = request("a.typ", &[]);
        other.sys_inputs.insert("skrizhal-cv-data".into(), "name: B".into());
        assert_ne!(with_cv.cache_key(), other.cache_key());
    }

    // ── Sheet paper ──────────────────────────────────────────────────────────

    const A4: PaperSpec = PaperSpec { width_pt: 595.28, height_pt: 841.89, uniform: true };

    #[test]
    fn unimposed_jobs_print_on_the_documents_own_paper() {
        assert_eq!(sheet_paper(A4, Imposition::Off), A4);
        assert_eq!(sheet_paper(A4, Imposition::FourUp), A4, "a 2×2 grid keeps the page shape");
    }

    #[test]
    fn two_up_and_booklets_print_on_a_rotated_sheet() {
        // The dialog has to open on the paper actually being fed, or a booklet
        // is set up as portrait and comes out cropped down the middle.
        for imp in [Imposition::TwoUp, Imposition::Booklet] {
            let sheet = sheet_paper(A4, imp);
            assert!(sheet.is_landscape(), "{imp:?} feeds a landscape sheet");
            assert_eq!(sheet.width_pt, A4.height_pt);
            assert_eq!(sheet.height_pt, A4.width_pt);
        }
    }
}
