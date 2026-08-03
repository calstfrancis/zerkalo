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
        cv_elements_path: Option<PathBuf>,
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
        let project_root_for_cv = project_root.clone();
        install_btn.connect_clicked(move |_| {
            let (sans, serif) = {
                let c = crate::config::shared();
                let c = c.borrow();
                (c.default_sans_font.clone(), c.default_serif_font.clone())
            };
            let wizard = super::setup_wizard::SetupWizard::new(&parent_clone, &project_root, &sans, &serif, |sans, serif| {
                let _ = crate::config::update(|c| {
                    c.default_sans_font = sans;
                    c.default_serif_font = serif;
                });
            });
            wizard.present();
        });

        // Wire export button
        {
            let on_save_format = Rc::new(on_save_format);
            let checks = check_boxes.clone();
            let status_c = status_lbl.clone();
            let log_buf = log_view.buffer();

            // Derive the folder where output files land.  External tools run on
            // the host via flatpak-spawn, so they cannot see sandbox-private /tmp
            // paths.  Writing next to the source file is always host-accessible.
            let export_dir = root_file
                .as_ref()
                .and_then(|p| p.parent())
                .map(|p| p.to_path_buf())
                .unwrap_or(output_dir);

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
                let export_dir_owned = export_dir.clone();
                let selected_owned = selected.clone();
                let (cv_overrides_owned, cv_sys_inputs_owned) = crate::cv_mode::cv_mode_compile_extras(
                    &project_root_for_cv,
                    cv_elements_path.as_deref(),
                );

                std::thread::spawn(move || {
                    // Ensure the output directory exists before writing anything.
                    if let Err(e) = std::fs::create_dir_all(&export_dir_owned) {
                        tx.send(ExportMsg::Err(format!("Cannot create output directory: {e}"))).ok();
                        return;
                    }

                    for fmt_idx in &selected_owned {
                        let (label, ext) = FORMATS[*fmt_idx];
                        let stem = input_owned
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("output")
                            .to_string();
                        let out_path = export_dir_owned.join(format!("{stem}.{ext}"));

                        tx.send(ExportMsg::Log(format!("── Exporting {label}…"))).ok();

                        let result = match fmt_idx {
                            0 => {
                                // PDF via embedded compiler — runs in-process, no host tool needed.
                                match crate::compiler::compile_to_pdf_bytes(
                                    &input_owned,
                                    &cv_overrides_owned,
                                    &cv_sys_inputs_owned,
                                ) {
                                    Ok(bytes) => std::fs::write(&out_path, &bytes)
                                        .map_err(|e| format!("Write error: {e}")),
                                    Err(e) => Err(e),
                                }
                            }
                            _ => {
                                // All other formats via pandoc.
                                // HTML, DOCX, ODT, LaTeX, EPUB — pandoc reads typst natively.
                                let pandoc_fmt = match fmt_idx {
                                    1 => "html",
                                    2 => "docx",
                                    3 => "odt",
                                    4 => "latex",
                                    5 => "epub",
                                    _ => "docx",
                                };

                                // Migrate legacy `it.numbering` pattern: Typst's non-PDF export
                                // pipeline doesn't expose element fields in show rules, so field
                                // access on heading elements fails. Write a patched temp file
                                // rather than touching the original on disk.
                                let tmp_path = migrate_for_pandoc(&input_owned);
                                let actual_input = tmp_path.as_ref().unwrap_or(&input_owned);

                                let result = run_command_logged(
                                    crate::git_sync::host_command("pandoc"),
                                    &[
                                        "-f", "typst",
                                        actual_input.to_str().unwrap_or(""),
                                        "-o", out_path.to_str().unwrap_or(""),
                                        "--standalone",
                                        "--to", pandoc_fmt,
                                    ],
                                    &tx,
                                    &format!("pandoc not found. Install pandoc to export {label}.\n  apt install pandoc\n  dnf install pandoc\n  zypper install pandoc"),
                                );
                                if let Some(tmp) = tmp_path {
                                    let _ = std::fs::remove_file(tmp);
                                }
                                result
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
                let export_dir_for_open = export_dir.clone();
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
                                    crate::git_sync::host_command("xdg-open")
                                        .arg(&export_dir_for_open)
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

// If the source file contains the legacy `it.numbering` pattern that Typst's
// non-PDF export pipeline can't handle, write a patched temp file and return
// its path. Returns None if no migration was needed (use the original file).
fn migrate_for_pandoc(source: &std::path::Path) -> Option<PathBuf> {
    const OLD: &str =
        "#if it.numbering != none [#context counter(heading).display(it.numbering)#h(0.3em)]";

    let content = std::fs::read_to_string(source).ok()?;
    if !content.contains(OLD) {
        return None;
    }

    // Detect whether heading numbering is active and what format is used.
    // Prefer scanning within the Zerkalo-generated template markers when
    // present (keeps detection scoped to the known preamble), but fall back
    // to scanning the whole document when they're missing — e.g. a
    // hand-edited or older document — rather than aborting the migration
    // and exporting with the pandoc-incompatible construct still in place.
    let scan_section: &str = match (
        content.find("// ZERKALO-TEMPLATE-BEGIN"),
        content.find("// ZERKALO-TEMPLATE-END"),
    ) {
        (Some(b), Some(e)) if b < e => &content[b..e],
        _ => &content,
    };
    let (num_on, num_fmt) = {
        let mut on = false;
        let mut fmt = String::new();
        for line in scan_section.lines() {
            if let Some(rest) = line.trim().strip_prefix("#set heading(numbering: \"") {
                if let Some(end) = rest.find('"') {
                    fmt = rest[..end].to_string();
                    on = true;
                    break;
                }
            }
        }
        (on, fmt)
    };

    let new_prefix = if num_on {
        let f = if num_fmt.is_empty() { "1.".to_string() } else { num_fmt };
        format!("#context counter(heading).display(\"{f}\")#h(0.3em)")
    } else {
        String::new()
    };

    let patched = content.replace(OLD, &new_prefix);
    let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("doc");
    let tmp = std::env::temp_dir().join(format!("zerkalo_pandoc_{stem}.typ"));
    std::fs::write(&tmp, patched).ok()?;
    Some(tmp)
}

fn append_log(buf: &gtk4::TextBuffer, text: &str) {
    let mut end = buf.end_iter();
    if buf.char_count() > 0 {
        buf.insert(&mut end, "\n");
    }
    buf.insert(&mut end, text);
}

fn run_command_logged(
    mut cmd: std::process::Command,
    args: &[&str],
    tx: &mpsc::SyncSender<ExportMsg>,
    not_found_msg: &str,
) -> Result<(), String> {
    let mut child = match cmd
        .args(args)
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(not_found_msg.to_string());
        }
        Err(e) => return Err(format!("Failed to start command: {e}")),
    };

    // Read both stderr and stdout concurrently to avoid deadlock and to capture
    // whichever stream the tool (or its Typst subprocess) writes errors to.
    let stderr = child.stderr.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let tx_err = tx.clone();
    let stderr_thread = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            tx_err.send(ExportMsg::Log(line)).ok();
        }
    });
    let tx_out = tx.clone();
    let stdout_thread = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            tx_out.send(ExportMsg::Log(line)).ok();
        }
    });
    stderr_thread.join().ok();
    stdout_thread.join().ok();

    let status = child.wait().map_err(|e| format!("Process error: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Command failed (see log above)".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD_CONSTRUCT: &str =
        "#if it.numbering != none [#context counter(heading).display(it.numbering)#h(0.3em)]";

    fn write_temp(name: &str, content: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "zerkalo_migrate_test_{name}_{}.typ",
            std::process::id()
        ));
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn migrate_for_pandoc_returns_none_when_construct_absent() {
        let path = write_temp("absent", "#set page(paper: \"a4\")\n");
        assert!(migrate_for_pandoc(&path).is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn migrate_for_pandoc_migrates_within_template_markers() {
        let content = format!(
            "// ZERKALO-TEMPLATE-BEGIN\n#set heading(numbering: \"1.1.\")\n// ZERKALO-TEMPLATE-END\n{OLD_CONSTRUCT}\n"
        );
        let path = write_temp("with_markers", &content);
        let out = migrate_for_pandoc(&path).expect("should migrate");
        let migrated = std::fs::read_to_string(&out).unwrap();
        assert!(!migrated.contains(OLD_CONSTRUCT));
        assert!(migrated.contains("counter(heading).display(\"1.1.\")"));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn migrate_for_pandoc_still_migrates_when_template_markers_are_missing() {
        // A hand-edited or older document without the ZERKALO-TEMPLATE markers
        // must still get the pandoc-incompatible construct patched out,
        // instead of migrate_for_pandoc bailing via `?` and leaving it in place.
        let content = format!(
            "#set heading(numbering: \"I.A.1.\")\n{OLD_CONSTRUCT}\n"
        );
        let path = write_temp("no_markers", &content);
        let out = migrate_for_pandoc(&path).expect("should still migrate without markers");
        let migrated = std::fs::read_to_string(&out).unwrap();
        assert!(!migrated.contains(OLD_CONSTRUCT));
        assert!(migrated.contains("counter(heading).display(\"I.A.1.\")"));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn migrate_for_pandoc_without_markers_and_without_numbering_strips_construct_blank() {
        let content = OLD_CONSTRUCT.to_string();
        let path = write_temp("no_markers_no_numbering", &content);
        let out = migrate_for_pandoc(&path).expect("should still migrate");
        let migrated = std::fs::read_to_string(&out).unwrap();
        assert!(!migrated.contains(OLD_CONSTRUCT));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&out);
    }
}
