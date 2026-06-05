use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::Stdio;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, CheckButton, Label, Orientation, ScrolledWindow,
           Separator, TextView, WrapMode};
use libadwaita as adw;
use adw::prelude::*;

// ── Export formats ────────────────────────────────────────────────────────────

const FORMATS: &[(&str, &str)] = &[
    ("PDF",   "pdf"),
    ("HTML",  "html"),
    ("DOCX",  "docx"),
    ("ODT",   "odt"),
    ("LaTeX", "tex"),
    ("EPUB",  "epub"),
];

// ── Message type for the worker thread ───────────────────────────────────────

enum ExportMsg {
    Log(String),
    Done(String),  // format label
    Err(String),
}

// ── Dialog ────────────────────────────────────────────────────────────────────

pub struct ExportDialog {
    window: adw::Window,
}

impl ExportDialog {
    pub fn new(
        parent: &adw::ApplicationWindow,
        root_file: Option<PathBuf>,
        output_dir: PathBuf,
        project_root: PathBuf,
        initial_format: u32,
        on_save_format: impl Fn(u32) + 'static,
    ) -> Self {
        let window = adw::Window::new();
        window.set_title(Some("Export"));
        window.set_default_width(420);
        window.set_default_height(460);
        window.set_transient_for(Some(parent));
        window.set_modal(true);
        window.set_resizable(true);

        let header = adw::HeaderBar::new();

        let content = GtkBox::new(Orientation::Vertical, 0);

        // ── Format checkboxes ─────────────────────────────────────────────────
        let prefs_group = adw::PreferencesGroup::new();
        prefs_group.set_title("Export Formats");
        prefs_group.set_margin_start(16);
        prefs_group.set_margin_end(16);
        prefs_group.set_margin_top(16);
        prefs_group.set_margin_bottom(8);

        // One CheckButton per format; the "initial_format" is pre-checked
        let check_boxes: Vec<CheckButton> = FORMATS.iter().enumerate().map(|(i, (label, _))| {
            let cb = CheckButton::with_label(label);
            cb.set_active(i == initial_format as usize);
            cb
        }).collect();

        let fmt_box = GtkBox::new(Orientation::Horizontal, 8);
        fmt_box.set_halign(Align::Center);
        fmt_box.set_margin_top(4);
        fmt_box.set_margin_bottom(4);
        for cb in &check_boxes {
            fmt_box.append(cb);
        }
        prefs_group.add(&fmt_box);
        content.append(&prefs_group);

        content.append(&Separator::new(Orientation::Horizontal));

        // ── Progress log area ─────────────────────────────────────────────────
        let log_view = TextView::new();
        log_view.set_editable(false);
        log_view.set_wrap_mode(WrapMode::WordChar);
        log_view.add_css_class("monospace");
        let log_scroll = ScrolledWindow::new();
        log_scroll.set_child(Some(&log_view));
        log_scroll.set_vexpand(true);
        log_scroll.set_min_content_height(120);
        log_scroll.set_margin_start(8);
        log_scroll.set_margin_end(8);
        log_scroll.set_margin_top(8);
        log_scroll.set_margin_bottom(4);
        content.append(&log_scroll);

        // ── Status label ──────────────────────────────────────────────────────
        let status_lbl = Label::new(Some("Select formats and click Export."));
        status_lbl.add_css_class("caption");
        status_lbl.add_css_class("dim-label");
        status_lbl.set_margin_start(16);
        status_lbl.set_margin_end(16);
        status_lbl.set_margin_bottom(4);
        status_lbl.set_wrap(true);
        status_lbl.set_xalign(0.0);
        content.append(&status_lbl);

        // ── Action buttons ────────────────────────────────────────────────────
        let btn_row = GtkBox::new(Orientation::Horizontal, 8);
        btn_row.set_halign(Align::End);
        btn_row.set_margin_start(16);
        btn_row.set_margin_end(16);
        btn_row.set_margin_top(4);
        btn_row.set_margin_bottom(16);

        let install_btn = Button::with_label("Install Dependencies…");
        install_btn.add_css_class("flat");
        install_btn.set_tooltip_text(Some("Open the System Check Wizard to install missing tools"));

        let export_btn = Button::with_label("Export");
        export_btn.add_css_class("suggested-action");
        export_btn.set_width_request(100);

        btn_row.append(&install_btn);
        btn_row.append(&export_btn);
        content.append(&btn_row);

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&content));
        window.set_content(Some(&toolbar_view));

        // Wire install button → open Setup Wizard
        let parent_clone = parent.clone();
        install_btn.connect_clicked(move |_| {
            let wizard = super::setup_wizard::SetupWizard::new(&parent_clone, &project_root);
            wizard.present();
        });

        // Wire export button
        {
            let on_save_format = Rc::new(on_save_format);
            let checks = check_boxes.clone();
            let status_c = status_lbl.clone();
            let log_buf = log_view.buffer();
            let out_dir = output_dir.clone();
            export_btn.connect_clicked(move |btn| {
                let Some(ref input) = root_file else {
                    status_c.set_text("No file is currently open.");
                    return;
                };

                // Collect selected formats
                let selected: Vec<usize> = checks.iter().enumerate()
                    .filter(|(_, cb)| cb.is_active())
                    .map(|(i, _)| i)
                    .collect();

                if selected.is_empty() {
                    status_c.set_text("No formats selected.");
                    return;
                }

                // Remember the last selected format (first checked one)
                let first_fmt = selected[0] as u32;

                // Clear log
                log_buf.set_text("");
                status_c.set_text(&format!("Exporting {} format(s)…", selected.len()));
                btn.set_sensitive(false);

                let (tx, rx) = mpsc::sync_channel::<ExportMsg>(64);

                let input_owned = input.clone();
                let out_dir_owned = out_dir.clone();
                let selected_owned = selected.clone();

                std::thread::spawn(move || {
                    for fmt_idx in &selected_owned {
                        let (label, ext) = FORMATS[*fmt_idx];
                        let stem = input_owned
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("output")
                            .to_string();
                        let out_path = out_dir_owned.join(format!("{stem}.{ext}"));

                        tx.send(ExportMsg::Log(format!("── Exporting {label}…"))).ok();

                        let result = match fmt_idx {
                            0 => {
                                // PDF via embedded compiler
                                match crate::compiler::compile_to_pdf_bytes(
                                    &input_owned,
                                    &std::collections::HashMap::new(),
                                ) {
                                    Ok(bytes) => std::fs::write(&out_path, &bytes)
                                        .map_err(|e| format!("Write error: {e}")),
                                    Err(e) => Err(e),
                                }
                            }
                            1 => {
                                // HTML via typst CLI
                                run_command_logged(
                                    "typst",
                                    &[
                                        "compile",
                                        "--format", "html",
                                        input_owned.to_str().unwrap_or(""),
                                        out_path.to_str().unwrap_or(""),
                                    ],
                                    &tx,
                                    "typst CLI not found. Install: cargo install typst-cli",
                                )
                            }
                            _ => {
                                // pandoc formats
                                let pandoc_fmt = match fmt_idx {
                                    2 => "docx",
                                    3 => "odt",
                                    4 => "latex",
                                    5 => "epub",
                                    _ => "docx",
                                };
                                run_command_logged(
                                    "pandoc",
                                    &[
                                        "-f", "typst",
                                        input_owned.to_str().unwrap_or(""),
                                        "-o", out_path.to_str().unwrap_or(""),
                                        "--standalone",
                                        if pandoc_fmt == "docx" { "--reference-doc" } else { "--to" },
                                        if pandoc_fmt == "docx" { "" } else { pandoc_fmt },
                                    ],
                                    &tx,
                                    &format!("pandoc not found. Install pandoc to export {label}.\n  apt install pandoc\n  dnf install pandoc\n  zypper install pandoc"),
                                )
                            }
                        };

                        match result {
                            Ok(()) => {
                                tx.send(ExportMsg::Done(label.to_string())).ok();
                            }
                            Err(e) => {
                                tx.send(ExportMsg::Err(format!("[{label}] {e}"))).ok();
                            }
                        }
                    }
                });

                let rx = Rc::new(rx);
                let btn_p = btn.clone();
                let status_p = status_c.clone();
                let log_buf_p = log_buf.clone();
                let out_dir_for_open = out_dir.clone();
                let on_save_fmt_inner = on_save_format.clone();
                let mut done_count = 0usize;
                let total = selected.len();

                glib::timeout_add_local(Duration::from_millis(50), move || {
                    use std::sync::mpsc::TryRecvError;
                    loop {
                        match rx.try_recv() {
                            Ok(ExportMsg::Log(line)) => {
                                append_log(&log_buf_p, &line);
                            }
                            Ok(ExportMsg::Done(label)) => {
                                done_count += 1;
                                append_log(&log_buf_p, &format!("✓ {label} done."));
                                if done_count >= total {
                                    status_p.set_text("All exports complete. Opening output folder…");
                                    btn_p.set_sensitive(true);
                                    on_save_fmt_inner(first_fmt);
                                    std::process::Command::new("xdg-open")
                                        .arg(&out_dir_for_open)
                                        .spawn().ok();
                                    return glib::ControlFlow::Break;
                                }
                            }
                            Ok(ExportMsg::Err(e)) => {
                                append_log(&log_buf_p, &format!("✗ {e}"));
                                done_count += 1;
                                if done_count >= total {
                                    status_p.set_text("Export finished with errors. See log above.");
                                    btn_p.set_sensitive(true);
                                    return glib::ControlFlow::Break;
                                }
                            }
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                btn_p.set_sensitive(true);
                                return glib::ControlFlow::Break;
                            }
                        }
                    }
                    glib::ControlFlow::Continue
                });
            });
        }

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn append_log(buf: &gtk4::TextBuffer, text: &str) {
    let mut end = buf.end_iter();
    if buf.char_count() > 0 {
        buf.insert(&mut end, "\n");
    }
    buf.insert(&mut end, text);
}

fn run_command_logged(
    cmd: &str,
    args: &[&str],
    tx: &mpsc::SyncSender<ExportMsg>,
    not_found_msg: &str,
) -> Result<(), String> {
    // Filter out empty args (used as placeholders for conditional flags)
    let args: Vec<&str> = args.iter().copied().filter(|a| !a.is_empty()).collect();

    let mut child = match std::process::Command::new(cmd)
        .args(&args)
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(not_found_msg.to_string());
        }
        Err(e) => return Err(format!("Failed to start {cmd}: {e}")),
    };

    // Read stderr in the same thread (we're already in a worker thread)
    let stderr = child.stderr.take().unwrap();
    let reader = BufReader::new(stderr);
    for line in reader.lines().flatten() {
        tx.send(ExportMsg::Log(line)).ok();
    }

    let status = child.wait().map_err(|e| format!("Process error: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Command failed (see log above)".to_string())
    }
}
