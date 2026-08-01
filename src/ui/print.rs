use std::cell::Cell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{PrintOperation, PrintOperationAction, Window};
use typst::layout::PagedDocument;

/// Resolution pages are rasterised at when falling back to GTK printing.
///
/// Only used when no desktop print portal is available. The portal path sends
/// the PDF itself, so it stays vector and this doesn't apply.
const PRINT_DPI: f64 = 300.0;

/// Typst's layout unit. Cairo print contexts are scaled so one unit is one
/// point, so this converts between the two.
const POINTS_PER_INCH: f64 = 72.0;

// Guards against a second print run starting while one is still compiling.
// Typst offers no way to abort a compile, so without this an impatient
// double-press on Ctrl+P starts a second full compile racing the first.
thread_local! {
    static PRINT_IN_FLIGHT: Cell<bool> = const { Cell::new(false) };
}

pub struct PrintRequest {
    pub root: PathBuf,
    pub overrides: HashMap<PathBuf, String>,
    pub sys_inputs: HashMap<String, String>,
    /// Shown as the job name in the printer queue.
    pub job_name: String,
}

pub enum PrintStatus {
    Preparing,
    AlreadyRunning,
    Failed(String),
    Cancelled,
    Sent,
}

/// What the compile thread hands back: the PDF for the portal, and the
/// laid-out document for the raster fallback. Compiling once serves both.
struct Prepared {
    pdf: Vec<u8>,
    doc: PagedDocument,
}

/// Compile the document and print it.
///
/// Prefers the desktop print portal, which takes the compiled PDF and so keeps
/// text as vectors at the printer's own resolution. Falls back to GTK's print
/// dialog with pages raster-rendered at [`PRINT_DPI`] when no portal answers —
/// GTK draws through Cairo and Typst has no Cairo backend, so that path cannot
/// preserve vectors.
///
/// `on_status` reports progress and failures to the caller's toast/error UI —
/// every failure path here goes through it, because the previous implementation
/// discarded compile and write errors alike and left the button looking dead.
pub fn print_document<F>(parent: &Window, request: PrintRequest, on_status: F)
where
    F: Fn(PrintStatus) + 'static,
{
    if PRINT_IN_FLIGHT.with(|f| f.get()) {
        on_status(PrintStatus::AlreadyRunning);
        return;
    }
    PRINT_IN_FLIGHT.with(|f| f.set(true));
    on_status(PrintStatus::Preparing);

    let on_status: Rc<dyn Fn(PrintStatus)> = Rc::new(on_status);
    let (tx, rx) = mpsc::sync_channel::<Result<Prepared, String>>(1);
    let PrintRequest { root, overrides, sys_inputs, job_name } = request;

    std::thread::spawn(move || {
        let prepared = crate::compiler::compile_document(&root, &overrides, &sys_inputs)
            .and_then(|doc| {
                crate::compiler::pdf_bytes_from_document(&doc).map(|pdf| Prepared { pdf, doc })
            });
        tx.send(prepared).ok();
    });

    let parent = parent.clone();
    glib::timeout_add_local(Duration::from_millis(100), move || {
        match rx.try_recv() {
            Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(TryRecvError::Disconnected) => {
                PRINT_IN_FLIGHT.with(|f| f.set(false));
                on_status(PrintStatus::Failed(
                    "The compiler stopped unexpectedly while preparing to print.".into(),
                ));
                glib::ControlFlow::Break
            }
            Ok(Err(msg)) => {
                PRINT_IN_FLIGHT.with(|f| f.set(false));
                on_status(PrintStatus::Failed(msg));
                glib::ControlFlow::Break
            }
            Ok(Ok(prepared)) => {
                PRINT_IN_FLIGHT.with(|f| f.set(false));
                start_portal_print(&parent, prepared, &job_name, &on_status);
                glib::ControlFlow::Break
            }
        }
    });
}

/// Outcome of the portal attempt, reported back to the GTK thread.
enum PortalOutcome {
    Sent,
    Cancelled,
    /// No portal answered — fall back to GTK's own print dialog.
    Unavailable(String),
    Failed(String),
}

fn start_portal_print(
    parent: &Window,
    prepared: Prepared,
    job_name: &str,
    on_status: &Rc<dyn Fn(PrintStatus)>,
) {
    if prepared.doc.pages.is_empty() {
        on_status(PrintStatus::Failed("The document has no pages to print.".into()));
        return;
    }

    let (tx, rx) = mpsc::sync_channel::<PortalOutcome>(1);
    let pdf = prepared.pdf;
    let title = job_name.to_string();
    // ashpd is async and the GTK main loop is glib, so the portal conversation
    // runs on its own current-thread tokio runtime and reports back by channel —
    // the same worker-plus-poll shape used elsewhere in the app.
    std::thread::spawn(move || {
        let outcome = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt.block_on(print_via_portal(&pdf, &title)),
            Err(e) => PortalOutcome::Unavailable(format!("no async runtime: {e}")),
        };
        tx.send(outcome).ok();
    });

    let parent = parent.clone();
    let on_status = on_status.clone();
    let doc = Rc::new(prepared.doc);
    let job_name = job_name.to_string();
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
                    run_gtk_print_dialog(&parent, &doc, &job_name, &on_status);
                }
            }
            glib::ControlFlow::Break
        }
    });
}

async fn print_via_portal(pdf: &[u8], title: &str) -> PortalOutcome {
    use ashpd::desktop::print::{PreparePrintOptions, PrintOptions, PrintProxy};

    let proxy = match PrintProxy::new().await {
        Ok(p) => p,
        Err(e) => return PortalOutcome::Unavailable(e.to_string()),
    };

    let file = match pdf_to_temp_file(pdf) {
        Ok(f) => f,
        Err(e) => return PortalOutcome::Failed(format!("Couldn't stage the document: {e}")),
    };

    // PreparePrint shows the settings dialog and hands back a token that
    // authorises the actual Print call with those settings.
    let prepared = match proxy
        .prepare_print(
            None,
            title,
            Default::default(),
            Default::default(),
            PreparePrintOptions::default().set_modal(true),
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
    use std::io::{Seek, SeekFrom, Write};

    let path = std::env::temp_dir().join(format!(
        "zerkalo-print-{}-{:?}.pdf",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&path)?;
    file.write_all(pdf)?;
    file.flush()?;
    file.seek(SeekFrom::Start(0))?;
    std::fs::remove_file(&path)?;
    Ok(file)
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
        let before = std::fs::read_dir(std::env::temp_dir())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("zerkalo-print-"))
            .count();
        let file = pdf_to_temp_file(b"%PDF-1.7\n").unwrap();
        let after = std::fs::read_dir(std::env::temp_dir())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("zerkalo-print-"))
            .count();
        drop(file);
        assert_eq!(before, after, "the staged file must not persist on disk");
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
}

// ── GTK fallback: raster printing when no portal is available ────────────────

fn run_gtk_print_dialog(
    parent: &Window,
    doc: &Rc<PagedDocument>,
    job_name: &str,
    on_status: &Rc<dyn Fn(PrintStatus)>,
) {
    let op = PrintOperation::new();
    op.set_job_name(job_name);
    op.set_n_pages(doc.pages.len() as i32);
    // The dialog's page-range, copies, collate and duplex controls are all
    // handled by GTK once n_pages is set.
    op.set_embed_page_setup(true);
    op.set_allow_async(true);

    let doc = doc.clone();
    op.connect_draw_page(move |_, ctx, page_nr| {
        let Some(page) = doc.pages.get(page_nr as usize) else { return };
        draw_page(&ctx.cairo_context(), page);
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

fn draw_page(cr: &gtk4::cairo::Context, page: &typst::layout::Page) {
    let scale = (PRINT_DPI / POINTS_PER_INCH) as f32;
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

    // The context is scaled in points; the bitmap is at PRINT_DPI. Scaling down
    // by the same factor it was rendered up by lands the page at its true size.
    let inv = POINTS_PER_INCH / PRINT_DPI;
    cr.save().ok();
    cr.scale(inv, inv);
    cr.set_source_pixbuf(&pixbuf, 0.0, 0.0);
    cr.paint().ok();
    cr.restore().ok();
}
