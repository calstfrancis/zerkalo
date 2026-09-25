use std::cell::RefCell;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::Stdio;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use adw::prelude::*;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CheckButton, Label, Orientation, ScrolledWindow, Separator,
    TextView, WrapMode,
};
use libadwaita as adw;

// ── Export formats ────────────────────────────────────────────────────────────

const FORMATS: &[(&str, &str)] = &[
    ("PDF", "pdf"),
    ("HTML", "html"),
    ("DOCX", "docx"),
    ("ODT", "odt"),
    ("LaTeX", "tex"),
    ("EPUB", "epub"),
];

// ── Message type for the worker thread ───────────────────────────────────────

enum ExportMsg {
    Log(String),
    Done(String, PathBuf), // format label, written file
    Err(String),
}

const PEREPLYOT_APP_ID: &str = "io.github.calstfrancis.Pereplyot";

#[derive(Clone, Copy)]
enum Pereplyot {
    Flatpak,
    Binary,
}

fn detect_pereplyot() -> Option<Pereplyot> {
    let quiet = |mut c: std::process::Command| {
        c.stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    let mut flatpak = crate::git_sync::host_command("flatpak");
    flatpak.args(["info", "--show-ref", PEREPLYOT_APP_ID]);
    if quiet(flatpak) {
        return Some(Pereplyot::Flatpak);
    }
    let mut which = crate::git_sync::host_command("sh");
    which.args(["-c", "command -v pereplyot"]);
    quiet(which).then_some(Pereplyot::Binary)
}

/// What the Export dialog remembers between runs (persisted by the caller).
#[derive(Clone, Debug)]
pub struct ExportPrefs {
    pub format: u32,
    pub dir: Option<PathBuf>,
    pub open_after: bool,
    pub open_in_pereplyot: bool,
    /// 0 = don't export, 1 = `.bib`, 2 = `.yaml`.
    pub cited_refs: u32,
}

// ── Dialog ────────────────────────────────────────────────────────────────────

pub struct ExportDialog {
    window: adw::Window,
}

impl ExportDialog {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        parent: &adw::ApplicationWindow,
        root_file: Option<PathBuf>,
        output_dir: PathBuf,
        project_root: PathBuf,
        cv_elements_path: Option<PathBuf>,
        bib_path: Option<PathBuf>,
        prefs: ExportPrefs,
        on_save_prefs: impl Fn(ExportPrefs) + 'static,
    ) -> Self {
        let window = adw::Window::new();
        window.set_title(Some("Export"));
        window.set_default_width(460);
        window.set_default_height(640);
        window.set_transient_for(Some(parent));
        window.set_modal(true);
        window.set_resizable(true);

        let header = adw::HeaderBar::new();
        header.add_css_class("fond-chrome");

        let content = GtkBox::new(Orientation::Vertical, 0);

        // ── Format checkboxes ─────────────────────────────────────────────────
        let prefs_group = adw::PreferencesGroup::new();
        prefs_group.set_title("Export Formats");
        prefs_group.set_margin_start(16);
        prefs_group.set_margin_end(16);
        prefs_group.set_margin_top(16);
        prefs_group.set_margin_bottom(8);

        // Checked once up front rather than only discovered at export time —
        // PDF (index 0) and HTML (index 1) both compile in-process (the
        // embedded Typst compiler, and Typst's own HTML exporter — see
        // compiler::compile_to_html) and never need this; DOCX/ODT/LaTeX/EPUB
        // still shell out to the host's pandoc, which a flatpak install may
        // not have.
        let pandoc_available = crate::git_sync::host_command("pandoc")
            .arg("--version")
            .output()
            .is_ok();

        // One CheckButton per format; the remembered format is pre-checked
        let check_boxes: Vec<CheckButton> = FORMATS
            .iter()
            .enumerate()
            .map(|(i, (label, _))| {
                let cb = CheckButton::with_label(label);
                let needs_pandoc = i != 0 && i != 1;
                if needs_pandoc && !pandoc_available {
                    cb.set_active(false);
                    cb.set_sensitive(false);
                    cb.set_tooltip_text(Some(
                        "Needs pandoc, which isn't installed — click Install Dependencies below",
                    ));
                } else {
                    cb.set_active(i == prefs.format as usize);
                }
                cb
            })
            .collect();

        let fmt_box = GtkBox::new(Orientation::Horizontal, 8);
        fmt_box.set_halign(Align::Center);
        fmt_box.set_margin_top(4);
        fmt_box.set_margin_bottom(4);
        for cb in &check_boxes {
            fmt_box.append(cb);
        }
        prefs_group.add(&fmt_box);
        content.append(&prefs_group);

        if !pandoc_available {
            let note = Label::new(Some(
                "Only PDF and HTML are available until pandoc is installed — it's \
                 needed for DOCX, ODT, LaTeX, and EPUB export.",
            ));
            note.add_css_class("caption");
            note.add_css_class("dim-label");
            note.set_wrap(true);
            note.set_xalign(0.0);
            note.set_margin_start(16);
            note.set_margin_end(16);
            note.set_margin_bottom(4);
            content.append(&note);
        }

        // ── Destination and after-export options ──────────────────────────────
        let source_dir = root_file
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or(output_dir);
        let export_dir = Rc::new(RefCell::new(
            prefs
                .dir
                .clone()
                .filter(|d| d.is_dir())
                .unwrap_or(source_dir.clone()),
        ));

        let options_group = adw::PreferencesGroup::new();
        options_group.set_title("Options");
        options_group.set_margin_start(16);
        options_group.set_margin_end(16);
        options_group.set_margin_top(8);
        options_group.set_margin_bottom(8);

        let dest_row = adw::ActionRow::builder()
            .title("Save to")
            .subtitle(display_dir(&export_dir.borrow()))
            .build();
        dest_row.set_subtitle_lines(2);
        let choose_btn = Button::with_label("Choose…");
        choose_btn.set_valign(Align::Center);
        dest_row.add_suffix(&choose_btn);
        options_group.add(&dest_row);

        let open_row = adw::SwitchRow::builder()
            .title("Open when finished")
            .active(prefs.open_after)
            .build();
        options_group.add(&open_row);

        let viewer_row = adw::ComboRow::builder()
            .title("Open PDFs and EPUBs in")
            .model(&gtk4::StringList::new(&["Pereplyot", "Default app"]))
            .selected(if prefs.open_in_pereplyot { 0 } else { 1 })
            .visible(false)
            .build();
        viewer_row.set_sensitive(prefs.open_after);
        options_group.add(&viewer_row);
        {
            let vr = viewer_row.clone();
            open_row.connect_active_notify(move |r| vr.set_sensitive(r.is_active()));
        }

        let refs_source = root_file
            .as_deref()
            .and_then(|r| crate::cited_refs::resolve_source(r, bib_path.as_deref()));
        let refs_row = adw::ComboRow::builder()
            .title("Cited references")
            .model(&gtk4::StringList::new(&[
                "Don't export",
                "BibTeX (.bib)",
                "Hayagriva (.yaml)",
            ]))
            .selected(prefs.cited_refs.min(2))
            .build();
        match &refs_source {
            Some(src) => refs_row.set_subtitle(&format!(
                "Only the entries this document cites, from {}",
                src.file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default()
            )),
            None => {
                refs_row.set_subtitle("This document has no bibliography to export from");
                refs_row.set_selected(0);
                refs_row.set_sensitive(false);
            }
        }
        options_group.add(&refs_row);
        content.append(&options_group);

        // Detected off the main thread — `flatpak info` through
        // flatpak-spawn can take a noticeable moment.
        let pereplyot: Rc<RefCell<Option<Pereplyot>>> = Rc::new(RefCell::new(None));
        {
            let (tx, rx) = mpsc::sync_channel::<Result<Option<Pereplyot>, String>>(1);
            std::thread::spawn(move || {
                tx.send(Ok(detect_pereplyot())).ok();
            });
            let vr = viewer_row.clone();
            let slot = pereplyot.clone();
            super::async_poll::poll_result(
                rx,
                Duration::from_millis(100),
                move |found| {
                    vr.set_visible(found.is_some());
                    *slot.borrow_mut() = found;
                },
                |_| {},
            );
        }

        content.append(&Separator::new(Orientation::Horizontal));

        // ── Progress log area ─────────────────────────────────────────────────
        let log_view = TextView::new();
        log_view.set_editable(false);
        log_view.set_wrap_mode(WrapMode::WordChar);
        log_view.add_css_class("monospace");
        let log_scroll = ScrolledWindow::new();
        log_scroll.set_child(Some(&log_view));
        log_scroll.set_vexpand(true);
        log_scroll.set_min_content_height(100);
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
        install_btn.set_tooltip_text(Some(
            "Open Tools to see what's missing and how to install it",
        ));

        let folder_btn = Button::with_label("Show in Folder");
        folder_btn.set_visible(false);

        let export_btn = Button::with_label("Export");
        export_btn.add_css_class("suggested-action");
        export_btn.set_width_request(100);

        btn_row.append(&install_btn);
        btn_row.append(&folder_btn);
        btn_row.append(&export_btn);
        content.append(&btn_row);

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.set_top_bar_style(adw::ToolbarStyle::RaisedBorder);
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&content));
        window.set_content(Some(&toolbar_view));

        // Wire install button → open Tools. Installing Skrizhal is a tools
        // question, not a setup one; Tools also now checks pandoc for real
        // rather than assuming it's bundled, so this shows accurate install
        // instructions instead of a false checkmark.
        let parent_clone = parent.clone();
        let project_root_for_cv = project_root.clone();
        install_btn.connect_clicked(move |_| {
            super::tools_window::ToolsWindow::new(&parent_clone).present();
        });

        {
            let dir = export_dir.clone();
            let row = dest_row.clone();
            let win = window.clone();
            choose_btn.connect_clicked(move |_| {
                let fd = gtk4::FileDialog::new();
                fd.set_title("Export To");
                fd.set_initial_folder(Some(&gtk4::gio::File::for_path(&*dir.borrow())));
                let dir = dir.clone();
                let row = row.clone();
                fd.select_folder(Some(&win), None::<&gtk4::gio::Cancellable>, move |res| {
                    if let Some(path) = res.ok().and_then(|f| f.path()) {
                        row.set_subtitle(&display_dir(&path));
                        *dir.borrow_mut() = path;
                    }
                });
            });
        }

        {
            let dir = export_dir.clone();
            let win = window.clone();
            folder_btn.connect_clicked(move |_| {
                gtk4::FileLauncher::new(Some(&gtk4::gio::File::for_path(&*dir.borrow()))).launch(
                    Some(&win),
                    None::<&gtk4::gio::Cancellable>,
                    |_| {},
                );
            });
        }

        // Wire export button
        {
            let on_save_prefs = Rc::new(on_save_prefs);
            let checks = check_boxes.clone();
            let status_c = status_lbl.clone();
            let log_buf = log_view.buffer();
            let window_c = window.clone();

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
                let refs_format = match refs_row.selected() {
                    1 => Some(crate::cited_refs::RefFormat::Bib),
                    2 => Some(crate::cited_refs::RefFormat::Yaml),
                    _ => None,
                }
                .filter(|_| refs_source.is_some());

                if selected.is_empty() && refs_format.is_none() {
                    status_c.set_text("No formats selected.");
                    return;
                }

                let out_dir = export_dir.borrow().clone();
                let prefs_now = ExportPrefs {
                    format: selected.first().copied().unwrap_or(0) as u32,
                    dir: (out_dir != source_dir).then(|| out_dir.clone()),
                    open_after: open_row.is_active(),
                    open_in_pereplyot: viewer_row.selected() == 0,
                    cited_refs: refs_row.selected(),
                };

                // Clear log
                log_buf.set_text("");
                let total = selected.len() + usize::from(refs_format.is_some());
                status_c.set_text(&format!("Exporting {total} file(s)…"));
                btn.set_sensitive(false);
                folder_btn.set_visible(false);

                let (tx, rx) = mpsc::sync_channel::<ExportMsg>(64);

                let input_owned = input.clone();
                let export_dir_owned = out_dir.clone();
                let source_dir_owned = source_dir.clone();
                let selected_owned = selected.clone();
                let refs_source_owned = refs_source.clone();
                let (cv_overrides_owned, cv_sys_inputs_owned) = crate::cv_mode::cv_mode_compile_extras(
                    &project_root_for_cv,
                    cv_elements_path.as_deref(),
                );
                let bib_path_owned = bib_path.clone();

                std::thread::spawn(move || {
                    // Ensure the output directory exists before writing anything.
                    if let Err(e) = std::fs::create_dir_all(&export_dir_owned) {
                        tx.send(ExportMsg::Err(format!("Cannot create output directory: {e}"))).ok();
                        return;
                    }
                    let stem = input_owned
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("output")
                        .to_string();

                    for fmt_idx in &selected_owned {
                        let (label, ext) = FORMATS[*fmt_idx];
                        let out_path = export_dir_owned.join(format!("{stem}.{ext}"));

                        tx.send(ExportMsg::Log(format!("── Exporting {label}…"))).ok();

                        let result = match fmt_idx {
                            0 => {
                                // PDF via embedded compiler — runs in-process, no host tool needed.
                                match crate::compiler::compile_to_pdf_bytes(
                                    &input_owned,
                                    &cv_overrides_owned,
                                    &cv_sys_inputs_owned,
                                    bib_path_owned.as_deref(),
                                ) {
                                    Ok(bytes) => std::fs::write(&out_path, &bytes)
                                        .map_err(|e| format!("Write error: {e}")),
                                    Err(e) => Err(e),
                                }
                            }
                            1 => {
                                // HTML via Typst's own exporter — in-process,
                                // no pandoc, and self-contained (images embed
                                // as data URIs rather than writing loose
                                // files next to the output).
                                match crate::compiler::compile_to_html(
                                    &input_owned,
                                    &cv_overrides_owned,
                                    &cv_sys_inputs_owned,
                                    bib_path_owned.as_deref(),
                                ) {
                                    Ok(html) => std::fs::write(&out_path, html.as_bytes())
                                        .map_err(|e| format!("Write error: {e}")),
                                    Err(e) => Err(e),
                                }
                            }
                            _ => {
                                // DOCX, ODT, LaTeX, EPUB — pandoc reads typst
                                // natively; no Typst-native writer for these.
                                let pandoc_fmt = match fmt_idx {
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

                                // pandoc runs on the host (flatpak-spawn), which
                                // can't see a portal-granted folder outside home.
                                // Have it write next to the source, then move.
                                let staged = export_dir_owned.starts_with("/run/user");
                                let pandoc_out = if staged {
                                    source_dir_owned.join(format!(".zerkalo-export-{stem}.{ext}"))
                                } else {
                                    out_path.clone()
                                };

                                let result = run_command_logged(
                                    crate::git_sync::host_command("pandoc"),
                                    &[
                                        "-f", "typst",
                                        actual_input.to_str().unwrap_or(""),
                                        "-o", pandoc_out.to_str().unwrap_or(""),
                                        "--standalone",
                                        "--to", pandoc_fmt,
                                    ],
                                    &tx,
                                    &format!("pandoc not found. Install pandoc to export {label}.\n  apt install pandoc\n  dnf install pandoc\n  zypper install pandoc"),
                                );
                                if let Some(tmp) = tmp_path {
                                    let _ = std::fs::remove_file(tmp);
                                }
                                let result = if staged {
                                    let moved = result.and_then(|()| {
                                        std::fs::copy(&pandoc_out, &out_path)
                                            .map(|_| ())
                                            .map_err(|e| format!("Write error: {e}"))
                                    });
                                    let _ = std::fs::remove_file(&pandoc_out);
                                    moved
                                } else {
                                    result
                                };
                                result
                            }
                        };

                        match result {
                            Ok(()) => {
                                tx.send(ExportMsg::Done(label.to_string(), out_path)).ok();
                            }
                            Err(e) => {
                                tx.send(ExportMsg::Err(format!("[{label}] {e}"))).ok();
                            }
                        }
                    }

                    if let (Some(fmt), Some(src)) = (refs_format, refs_source_owned) {
                        tx.send(ExportMsg::Log("── Exporting cited references…".into())).ok();
                        let keys = crate::cited_refs::collect_cited_keys(&input_owned);
                        let out_path = export_dir_owned
                            .join(format!("{stem}-references.{}", fmt.extension()));
                        let result = crate::cited_refs::export_cited(&src, &keys, fmt).and_then(|r| {
                            std::fs::write(&out_path, r.text)
                                .map(|()| r.count)
                                .map_err(|e| format!("Write error: {e}"))
                        });
                        match result {
                            Ok(0) => {
                                let _ = std::fs::remove_file(&out_path);
                                tx.send(ExportMsg::Err(
                                    "[References] This document doesn't cite anything from its bibliography yet.".into(),
                                ))
                                .ok();
                            }
                            Ok(n) => {
                                tx.send(ExportMsg::Log(format!(
                                    "{n} cited entr{} written.",
                                    if n == 1 { "y" } else { "ies" }
                                )))
                                .ok();
                                tx.send(ExportMsg::Done("References".into(), out_path)).ok();
                            }
                            Err(e) => {
                                tx.send(ExportMsg::Err(format!("[References] {e}"))).ok();
                            }
                        }
                    }
                });

                let rx = Rc::new(rx);
                let btn_p = btn.clone();
                let folder_btn_p = folder_btn.clone();
                let status_p = status_c.clone();
                let log_buf_p = log_buf.clone();
                let on_save_prefs_inner = on_save_prefs.clone();
                let pereplyot_p = pereplyot.clone();
                let win_p = window_c.clone();
                let mut done_count = 0usize;
                let mut had_error = false;
                let mut written: Vec<PathBuf> = Vec::new();

                glib::timeout_add_local(Duration::from_millis(50), move || {
                    use std::sync::mpsc::TryRecvError;
                    loop {
                        match rx.try_recv() {
                            Ok(ExportMsg::Log(line)) => {
                                append_log(&log_buf_p, &line);
                            }
                            Ok(ExportMsg::Done(label, path)) => {
                                done_count += 1;
                                append_log(&log_buf_p, &format!("✓ {label} done."));
                                written.push(path);
                            }
                            Ok(ExportMsg::Err(e)) => {
                                append_log(&log_buf_p, &format!("✗ {e}"));
                                done_count += 1;
                                had_error = true;
                            }
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                btn_p.set_sensitive(true);
                                return glib::ControlFlow::Break;
                            }
                        }
                        if done_count >= total {
                            btn_p.set_sensitive(true);
                            on_save_prefs_inner(prefs_now.clone());
                            let where_ = display_dir(&out_dir);
                            status_p.set_text(&if had_error {
                                format!("Finished with errors (see log above). Saved files are in {where_}.")
                            } else {
                                format!("Saved to {where_}.")
                            });
                            folder_btn_p.set_visible(!written.is_empty());
                            if prefs_now.open_after {
                                if let Some(first) = written.first() {
                                    open_exported(
                                        &win_p,
                                        first,
                                        prefs_now.open_in_pereplyot.then(|| *pereplyot_p.borrow()).flatten(),
                                    );
                                }
                            }
                            return glib::ControlFlow::Break;
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

fn display_dir(dir: &std::path::Path) -> String {
    let home = glib::home_dir();
    match dir.strip_prefix(&home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => dir.display().to_string(),
    }
}

/// Opens an exported file: PDFs and EPUBs in Pereplyot when it's installed
/// and preferred, everything else (and the fallback) in the desktop's
/// default app via the OpenURI portal.
fn open_exported(window: &adw::Window, path: &std::path::Path, pereplyot: Option<Pereplyot>) {
    let readable = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf") || e.eq_ignore_ascii_case("epub"));
    if let (true, Some(how)) = (readable, pereplyot) {
        let mut cmd = match how {
            Pereplyot::Flatpak => {
                let mut c = crate::git_sync::host_command("flatpak");
                c.args(["run", PEREPLYOT_APP_ID]);
                c
            }
            Pereplyot::Binary => crate::git_sync::host_command("pereplyot"),
        };
        if let Ok(mut child) = cmd
            .arg(path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            std::thread::spawn(move || child.wait());
            return;
        }
    }
    gtk4::FileLauncher::new(Some(&gtk4::gio::File::for_path(path))).launch(
        Some(window),
        None::<&gtk4::gio::Cancellable>,
        |_| {},
    );
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
        let f = if num_fmt.is_empty() {
            "1.".to_string()
        } else {
            num_fmt
        };
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
        let content = format!("#set heading(numbering: \"I.A.1.\")\n{OLD_CONSTRUCT}\n");
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
