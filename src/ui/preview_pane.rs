use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;

use gtk4::gdk::prelude::GdkCairoContextExt;
use gtk4::gdk_pixbuf::Pixbuf;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, DrawingArea, Label, Orientation, ScrolledWindow, Spinner, Stack,
};

// ── Result sent from compile thread ──────────────────────────────────────────

enum CompileResult {
    Success,
    Error(String),
}

// ── Widget ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PreviewPane {
    root_widget: GtkBox,
    stack: Stack,
    img_scroll: ScrolledWindow,
    drawing_area: DrawingArea,
    spinner: Spinner,
    error_label: Label,
    output_dir: Rc<PathBuf>,
    extra_args: Rc<Vec<String>>,
    root_file: Rc<RefCell<Option<PathBuf>>>,
    zoom: Rc<RefCell<f64>>,
    auto_fit: Rc<RefCell<bool>>,
    on_compile_done: Rc<RefCell<Option<Box<dyn Fn(Option<String>)>>>>,
    on_zoom_changed: Rc<RefCell<Option<Box<dyn Fn(f64)>>>>,
    page_pixbufs: Rc<RefCell<Vec<Pixbuf>>>,
    watch_child: Rc<RefCell<Option<std::process::Child>>>,
}

impl PreviewPane {
    pub fn new(
        root_file: Option<PathBuf>,
        output_dir: Option<PathBuf>,
        extra_args: Vec<String>,
    ) -> Self {
        let root_widget = GtkBox::new(Orientation::Vertical, 0);
        root_widget.set_hexpand(true);
        root_widget.set_vexpand(true);

        let stack = Stack::new();
        stack.set_hexpand(true);
        stack.set_vexpand(true);

        // ── empty page ────────────────────────────────────────────────────────
        let empty_lbl = Label::new(Some("No preview\nCtrl+Shift+P to compile"));
        empty_lbl.add_css_class("dim-label");
        empty_lbl.set_justify(gtk4::Justification::Center);
        stack.add_named(&empty_lbl, Some("empty"));

        // ── compiling page ────────────────────────────────────────────────────
        let spin_box = GtkBox::new(Orientation::Vertical, 12);
        spin_box.set_halign(Align::Center);
        spin_box.set_valign(Align::Center);
        let spinner = Spinner::new();
        spinner.set_size_request(48, 48);
        let spin_lbl = Label::new(Some("Compiling\u{2026}"));
        spin_lbl.add_css_class("dim-label");
        spin_box.append(&spinner);
        spin_box.append(&spin_lbl);
        stack.add_named(&spin_box, Some("compiling"));

        // ── ready page: DrawingArea inside ScrolledWindow ─────────────────────
        let img_scroll = ScrolledWindow::new();
        img_scroll.set_hexpand(true);
        img_scroll.set_vexpand(true);

        let drawing_area = DrawingArea::new();
        drawing_area.set_halign(Align::Center);
        drawing_area.set_valign(Align::Start);
        img_scroll.set_child(Some(&drawing_area));
        stack.add_named(&img_scroll, Some("ready"));

        // ── error page ────────────────────────────────────────────────────────
        let err_scroll = ScrolledWindow::new();
        err_scroll.set_hexpand(true);
        err_scroll.set_vexpand(true);
        let error_label = Label::new(None);
        error_label.set_wrap(true);
        error_label.set_selectable(true);
        error_label.set_halign(Align::Start);
        error_label.set_valign(Align::Start);
        error_label.set_margin_top(12);
        error_label.set_margin_start(12);
        error_label.set_margin_end(12);
        error_label.add_css_class("error");
        err_scroll.set_child(Some(&error_label));
        stack.add_named(&err_scroll, Some("error"));

        stack.set_visible_child_name("empty");
        root_widget.append(&stack);

        let page_pixbufs: Rc<RefCell<Vec<Pixbuf>>> = Rc::new(RefCell::new(Vec::new()));

        // Wire up draw function
        let pixbufs_draw = page_pixbufs.clone();
        let zoom_draw: Rc<RefCell<f64>> = Rc::new(RefCell::new(1.0));
        let zoom_draw2 = zoom_draw.clone();

        drawing_area.set_draw_func(move |_area, ctx, _w, _h| {
            let z = *zoom_draw.borrow();
            let pbs = pixbufs_draw.borrow();

            // White background
            ctx.set_source_rgb(1.0, 1.0, 1.0);
            ctx.paint().ok();

            let mut y = 0.0f64;
            for pb in pbs.iter() {
                ctx.save().ok();
                ctx.scale(z, z);
                ctx.set_source_pixbuf(pb, 0.0, y / z);
                ctx.paint().ok();
                ctx.restore().ok();
                y += pb.height() as f64 * z + 8.0;
            }
        });

        let pane = Self {
            root_widget,
            stack,
            img_scroll,
            drawing_area,
            spinner,
            error_label,
            output_dir: Rc::new(
                output_dir.unwrap_or_else(|| PathBuf::from("/tmp/zerkalo_preview")),
            ),
            extra_args: Rc::new(extra_args),
            root_file: Rc::new(RefCell::new(root_file)),
            zoom: zoom_draw2,
            auto_fit: Rc::new(RefCell::new(true)),
            on_compile_done: Rc::new(RefCell::new(None)),
            on_zoom_changed: Rc::new(RefCell::new(None)),
            page_pixbufs,
            watch_child: Rc::new(RefCell::new(None)),
        };

        pane
    }

    pub fn widget(&self) -> &GtkBox {
        &self.root_widget
    }

    pub fn set_root_file(&self, path: PathBuf) {
        *self.root_file.borrow_mut() = Some(path);
    }

    pub fn output_dir(&self) -> PathBuf {
        (*self.output_dir).clone()
    }

    pub fn root_file_path(&self) -> Option<PathBuf> {
        self.root_file.borrow().clone()
    }

    pub fn extra_args(&self) -> Vec<String> {
        (*self.extra_args).clone()
    }

    pub fn zoom(&self) -> f64 {
        *self.zoom.borrow()
    }

    pub fn set_zoom(&self, z: f64) {
        *self.zoom.borrow_mut() = z.clamp(0.25, 4.0);
        self.refit_drawing_area();
        let actual = *self.zoom.borrow();
        if let Some(f) = self.on_zoom_changed.borrow().as_ref() {
            f(actual);
        }
    }

    pub fn fit_width(&self) {
        *self.auto_fit.borrow_mut() = false;
        let scroll_w = self.img_scroll.allocated_width() as f64;
        let pb_w = self.page_pixbufs.borrow().first()
            .map(|pb| pb.width() as f64)
            .unwrap_or(0.0);
        if pb_w > 0.0 && scroll_w > 16.0 {
            self.set_zoom((scroll_w - 16.0) / pb_w);
        }
    }

    pub fn fit_page(&self) {
        *self.auto_fit.borrow_mut() = false;
        let scroll_w = self.img_scroll.allocated_width() as f64;
        let scroll_h = self.img_scroll.allocated_height() as f64;
        let pbs = self.page_pixbufs.borrow();
        let pb_w = pbs.first().map(|pb| pb.width() as f64).unwrap_or(0.0);
        let pb_h = pbs.first().map(|pb| pb.height() as f64).unwrap_or(0.0);
        drop(pbs);
        if pb_w > 0.0 && pb_h > 0.0 && scroll_w > 16.0 && scroll_h > 16.0 {
            let z = ((scroll_w - 16.0) / pb_w).min((scroll_h - 16.0) / pb_h);
            self.set_zoom(z);
        }
    }

    pub fn set_on_compile_done(&self, f: impl Fn(Option<String>) + 'static) {
        *self.on_compile_done.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_zoom_changed(&self, f: impl Fn(f64) + 'static) {
        *self.on_zoom_changed.borrow_mut() = Some(Box::new(f));
    }

    // ── Watch mode ────────────────────────────────────────────────────────────

    pub fn start_watch(&self) {
        self.stop_watch();

        let root = match self.root_file.borrow().clone() {
            Some(f) => f,
            None => return,
        };

        let _ = std::fs::create_dir_all(&*self.output_dir);
        let pdf_path = self.output_dir.join("preview.pdf");
        let extra_args = (*self.extra_args).clone();

        let child = std::process::Command::new("typst")
            .arg("watch")
            .args(&extra_args)
            .arg(&root)
            .arg(&pdf_path)
            .current_dir(root.parent().unwrap_or(Path::new(".")))
            .stderr(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .spawn()
            .ok();

        if let Some(child) = child {
            *self.watch_child.borrow_mut() = Some(child);

            let last_mtime: Rc<RefCell<Option<std::time::SystemTime>>> =
                Rc::new(RefCell::new(
                    std::fs::metadata(&pdf_path)
                        .and_then(|m| m.modified())
                        .ok(),
                ));

            let pane = self.clone();
            glib::timeout_add_local(Duration::from_millis(400), move || {
                if pane.watch_child.borrow().is_none() {
                    return glib::ControlFlow::Break;
                }
                let pdf = pane.output_dir.join("preview.pdf");
                let current_mtime = std::fs::metadata(&pdf)
                    .and_then(|m| m.modified())
                    .ok();

                let changed = match (*last_mtime.borrow(), current_mtime) {
                    (Some(old), Some(new)) => old != new,
                    (None, Some(_)) => true,
                    _ => false,
                };
                if changed {
                    *last_mtime.borrow_mut() = current_mtime;
                    pane.render_pdf_and_display(&pdf);
                }
                glib::ControlFlow::Continue
            });
        }
    }

    pub fn stop_watch(&self) {
        if let Some(mut child) = self.watch_child.borrow_mut().take() {
            let _ = child.kill();
        }
    }

    pub fn is_watching(&self) -> bool {
        self.watch_child.borrow().is_some()
    }

    // ── Compile ───────────────────────────────────────────────────────────────

    pub fn trigger_compile(&self) {
        let root = match self.root_file.borrow().clone() {
            Some(f) => f,
            None => {
                self.error_label
                    .set_label("No root file detected.\nCreate a main.typ file.");
                self.stack.set_visible_child_name("error");
                return;
            }
        };

        self.spinner.set_spinning(true);
        self.stack.set_visible_child_name("compiling");

        let (tx, rx) = mpsc::sync_channel::<CompileResult>(1);
        let output_dir = (*self.output_dir).clone();
        let extra_args = (*self.extra_args).clone();

        std::thread::spawn(move || {
            let result = run_typst_compile(&root, &output_dir, &extra_args);
            tx.send(result).ok();
        });

        let rx = Rc::new(rx);
        let pane = self.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            match rx.try_recv() {
                Ok(result) => {
                    pane.spinner.set_spinning(false);
                    match result {
                        CompileResult::Success => {
                            let pdf = pane.output_dir.join("preview.pdf");
                            pane.render_pdf_and_display(&pdf);
                            if let Some(f) = pane.on_compile_done.borrow().as_ref() {
                                f(None);
                            }
                        }
                        CompileResult::Error(msg) => {
                            pane.error_label.set_label(&msg);
                            pane.stack.set_visible_child_name("error");
                            if let Some(f) = pane.on_compile_done.borrow().as_ref() {
                                f(Some(msg));
                            }
                        }
                    }
                    glib::ControlFlow::Break
                }
                Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(TryRecvError::Disconnected) => {
                    pane.spinner.set_spinning(false);
                    glib::ControlFlow::Break
                }
            }
        });
    }

    /// Re-render the PDF from disk without recompiling. Called by pop-out window
    /// and by watch mode when the PDF changes.
    pub fn refresh_display(&self) {
        let pdf = self.output_dir.join("preview.pdf");
        if pdf.exists() {
            self.render_pdf_and_display(&pdf);
        }
    }

    // ── Internal ──────────────────────────────────────────────────────────────

    fn render_pdf_and_display(&self, pdf_path: &Path) {
        let dpi = (150.0 * self.zoom().max(1.0)).round() as u32;
        let pdf = pdf_path.to_path_buf();
        let output_dir = (*self.output_dir).clone();

        self.spinner.set_spinning(true);
        self.stack.set_visible_child_name("compiling");

        let (tx, rx) = mpsc::sync_channel::<Result<Vec<PathBuf>, String>>(1);
        std::thread::spawn(move || {
            tx.send(render_pdf_to_pngs(&pdf, &output_dir, dpi)).ok();
        });

        let rx = Rc::new(rx);
        let pane = self.clone();
        glib::timeout_add_local(Duration::from_millis(30), move || {
            match rx.try_recv() {
                Ok(Ok(pages)) => {
                    pane.spinner.set_spinning(false);
                    pane.load_pixbufs(&pages);
                    pane.stack.set_visible_child_name("ready");
                    glib::ControlFlow::Break
                }
                Ok(Err(_)) => {
                    pane.spinner.set_spinning(false);
                    pane.stack.set_visible_child_name("ready");
                    glib::ControlFlow::Break
                }
                Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(TryRecvError::Disconnected) => {
                    pane.spinner.set_spinning(false);
                    glib::ControlFlow::Break
                }
            }
        });
    }

    fn load_pixbufs(&self, pages: &[PathBuf]) {
        let mut pixbufs = Vec::new();
        for path in pages {
            match Pixbuf::from_file(path) {
                Ok(pb) => pixbufs.push(pb),
                Err(e) => eprintln!("Failed to load pixbuf {}: {e}", path.display()),
            }
        }
        *self.page_pixbufs.borrow_mut() = pixbufs;
        if *self.auto_fit.borrow() {
            // Defer fit_width so the scroll widget has been allocated its size
            let pane = self.clone();
            glib::idle_add_local_once(move || { pane.fit_width(); });
        } else {
            self.refit_drawing_area();
        }
    }

    fn refit_drawing_area(&self) {
        let z = *self.zoom.borrow();
        let pbs = self.page_pixbufs.borrow();
        let mut total_h = 0i32;
        let mut max_w = 0i32;
        for pb in pbs.iter() {
            let w = (pb.width() as f64 * z).round() as i32;
            let h = (pb.height() as f64 * z).round() as i32;
            max_w = max_w.max(w);
            total_h += h + 8;
        }
        drop(pbs);
        self.drawing_area.set_content_width(max_w.max(1));
        self.drawing_area.set_content_height(total_h.max(1));
        self.drawing_area.queue_draw();
        let scroll = self.img_scroll.clone();
        glib::idle_add_local_once(move || { scroll.hadjustment().set_value(0.0); });
    }
}

// ── Background workers ────────────────────────────────────────────────────────

fn run_typst_compile(
    root_file: &Path,
    output_dir: &Path,
    extra_args: &[String],
) -> CompileResult {
    if let Err(e) = std::fs::create_dir_all(output_dir) {
        return CompileResult::Error(format!("Cannot create output dir: {e}"));
    }

    let pdf_path = output_dir.join("preview.pdf");
    let typst = std::process::Command::new("typst")
        .arg("compile")
        .args(extra_args)
        .args([
            root_file.to_str().unwrap_or(""),
            pdf_path.to_str().unwrap_or(""),
        ])
        .current_dir(root_file.parent().unwrap_or(Path::new(".")))
        .output();

    match typst {
        Err(_) => CompileResult::Error(
            "Could not run 'typst'.\n\nInstall it from https://typst.app\nor via your package manager.".into(),
        ),
        Ok(out) if !out.status.success() => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            CompileResult::Error(if !stderr.is_empty() { stderr } else { stdout })
        }
        Ok(_) => CompileResult::Success,
    }
}

fn render_pdf_to_pngs(
    pdf_path: &Path,
    output_dir: &Path,
    dpi: u32,
) -> Result<Vec<PathBuf>, String> {
    // Remove stale preview PNGs before rendering new ones
    if let Ok(entries) = std::fs::read_dir(output_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("preview-") && name.ends_with(".png") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    let dpi_str = dpi.to_string();
    let png_prefix = output_dir.join("preview");
    let out = std::process::Command::new("pdftoppm")
        .args([
            "-r", &dpi_str,
            "-png",
            pdf_path.to_str().unwrap_or(""),
            png_prefix.to_str().unwrap_or(""),
        ])
        .output();

    match out {
        Err(_) => Err("Could not run 'pdftoppm'.\n\nInstall poppler-tools:\n  zypper install poppler-tools".to_string()),
        Ok(o) if !o.status.success() => {
            Err(format!("pdftoppm failed: {}", String::from_utf8_lossy(&o.stderr).trim()))
        }
        Ok(_) => {
            let pages = collect_preview_pngs(output_dir);
            if pages.is_empty() {
                Err("No preview pages generated.".to_string())
            } else {
                Ok(pages)
            }
        }
    }
}

fn collect_preview_pngs(output_dir: &Path) -> Vec<PathBuf> {
    let mut pages: Vec<PathBuf> = std::fs::read_dir(output_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("preview-") && name.ends_with(".png") {
                Some(e.path())
            } else {
                None
            }
        })
        .collect();
    pages.sort();
    pages
}
