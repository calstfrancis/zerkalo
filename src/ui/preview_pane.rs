use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, ContentFit, Label, Orientation, Picture, ScrolledWindow, Spinner, Stack,
};

// ── Result sent from the compile thread ──────────────────────────────────────

enum CompileResult {
    Success(PathBuf),
    Error(String),
}

// ── Widget ───────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PreviewPane {
    root_widget: GtkBox,
    stack: Stack,
    picture: Picture,
    spinner: Spinner,
    error_label: Label,
    output_dir: Rc<PathBuf>,
    extra_args: Rc<Vec<String>>,
    root_file: Rc<RefCell<Option<PathBuf>>>,
    on_compile_done: Rc<RefCell<Option<Box<dyn Fn(Option<String>)>>>>,
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

        // ── Page: empty ──────────────────────────────────────────────────────
        let empty_lbl = Label::new(Some("No preview\nCtrl+Shift+P to compile"));
        empty_lbl.add_css_class("dim-label");
        empty_lbl.set_justify(gtk4::Justification::Center);
        stack.add_named(&empty_lbl, Some("empty"));

        // ── Page: compiling ──────────────────────────────────────────────────
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

        // ── Page: ready (rendered image) ─────────────────────────────────────
        let img_scroll = ScrolledWindow::new();
        img_scroll.set_hexpand(true);
        img_scroll.set_vexpand(true);
        let picture = Picture::new();
        picture.set_can_shrink(true);
        picture.set_content_fit(ContentFit::Contain);
        img_scroll.set_child(Some(&picture));
        stack.add_named(&img_scroll, Some("ready"));

        // ── Page: error ──────────────────────────────────────────────────────
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

        Self {
            root_widget,
            stack,
            picture,
            spinner,
            error_label,
            output_dir: Rc::new(
                output_dir.unwrap_or_else(|| PathBuf::from("/tmp/zerkalo_preview")),
            ),
            extra_args: Rc::new(extra_args),
            root_file: Rc::new(RefCell::new(root_file)),
            on_compile_done: Rc::new(RefCell::new(None)),
        }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.root_widget
    }

    pub fn set_root_file(&self, path: PathBuf) {
        *self.root_file.borrow_mut() = Some(path);
    }

    /// Called with `None` on success, `Some(stderr)` on compile error.
    pub fn set_on_compile_done(&self, f: impl Fn(Option<String>) + 'static) {
        *self.on_compile_done.borrow_mut() = Some(Box::new(f));
    }

    /// Spawn a background compile and render the result once it completes.
    pub fn trigger_compile(&self) {
        let root_file = match self.root_file.borrow().clone() {
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
            let result = compile_and_render(&root_file, &output_dir, &extra_args);
            tx.send(result).ok();
        });

        // Poll every 50 ms on the main thread until the thread sends a result.
        let rx = Rc::new(rx);
        let picture = self.picture.clone();
        let stack = self.stack.clone();
        let spinner = self.spinner.clone();
        let error_label = self.error_label.clone();
        let on_compile_done = self.on_compile_done.clone();

        glib::timeout_add_local(Duration::from_millis(50), move || {
            match rx.try_recv() {
                Ok(result) => {
                    spinner.set_spinning(false);
                    match result {
                        CompileResult::Success(png_path) => {
                            // Clear stale image before loading the new one
                            picture.set_file(None::<&gtk4::gio::File>);
                            picture.set_file(Some(&gtk4::gio::File::for_path(&png_path)));
                            stack.set_visible_child_name("ready");
                            if let Some(f) = on_compile_done.borrow().as_ref() {
                                f(None);
                            }
                        }
                        CompileResult::Error(msg) => {
                            error_label.set_label(&msg);
                            stack.set_visible_child_name("error");
                            if let Some(f) = on_compile_done.borrow().as_ref() {
                                f(Some(msg));
                            }
                        }
                    }
                    glib::ControlFlow::Break
                }
                Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(TryRecvError::Disconnected) => {
                    spinner.set_spinning(false);
                    glib::ControlFlow::Break
                }
            }
        });
    }
}

// ── Background worker ────────────────────────────────────────────────────────

fn compile_and_render(root_file: &Path, output_dir: &Path, extra_args: &[String]) -> CompileResult {
    if let Err(e) = std::fs::create_dir_all(output_dir) {
        return CompileResult::Error(format!("Cannot create output dir: {e}"));
    }

    let pdf_path = output_dir.join("preview.pdf");
    let png_prefix = output_dir.join("preview");

    // Step 1: typst compile [extra_args…] root.typ preview.pdf
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
        Err(_) => {
            return CompileResult::Error(
                "Could not run 'typst'.\n\nInstall it from https://typst.app\nor via your package manager (e.g. zypper install typst).".into(),
            );
        }
        Ok(out) if !out.status.success() => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            return CompileResult::Error(if !stderr.is_empty() { stderr } else { stdout });
        }
        Ok(_) => {}
    }

    // Step 2: pdftoppm -singlefile -r 150 -png preview.pdf preview
    let pdftoppm = std::process::Command::new("pdftoppm")
        .args([
            "-singlefile",
            "-r",
            "150",
            "-png",
            pdf_path.to_str().unwrap_or(""),
            png_prefix.to_str().unwrap_or(""),
        ])
        .output();

    match pdftoppm {
        Err(_) => {
            return CompileResult::Error(
                "Could not run 'pdftoppm'.\n\nInstall poppler-tools:\n  zypper install poppler-tools".into(),
            );
        }
        Ok(out) if !out.status.success() => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return CompileResult::Error(format!("pdftoppm failed: {stderr}"));
        }
        Ok(_) => {}
    }

    // -singlefile writes: prefix.png
    let single = output_dir.join("preview.png");
    if single.exists() {
        return CompileResult::Success(single);
    }

    // Fallback: paged naming without -singlefile
    for candidate in &[
        output_dir.join("preview-1.png"),
        output_dir.join("preview-01.png"),
    ] {
        if candidate.exists() {
            return CompileResult::Success(candidate.clone());
        }
    }

    CompileResult::Error("Preview image not found after conversion.".into())
}
