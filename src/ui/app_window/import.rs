//! Document import: pandoc-backed conversion (LaTeX, DOCX, Markdown, …), PDF
//! text extraction, the batch/folder queue, and the dialogs that front them.
//! Split out of `app_window.rs`, which was 8,302 lines.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Entry, Label, Orientation, ScrolledWindow,
};
use libadwaita as adw;
use adw::prelude::*;

use crate::config::Config;
use super::super::editor_pane::EditorPane;
use super::show_alert;

fn strip_pandoc_preamble(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let n = lines.len();
    let mut i = 0;
    while i < n {
        let t = lines[i].trim();
        if t.is_empty() || t.starts_with("//") {
            i += 1;
            continue;
        }
        // Strip #set rules (paren-depth aware for multi-line blocks)
        if t.starts_with("#set ") {
            let mut depth: i32 = 0;
            loop {
                for c in lines[i].chars() {
                    match c { '(' => depth += 1, ')' => depth -= 1, _ => {} }
                }
                i += 1;
                if depth <= 0 || i >= n { break; }
            }
            continue;
        }
        // Strip #show rules (bracket-depth aware) — pandoc emits #show heading: etc.
        if t.starts_with("#show ") {
            let mut depth: i32 = 0;
            loop {
                depth += lines[i].chars().filter(|&c| c == '[').count() as i32;
                depth -= lines[i].chars().filter(|&c| c == ']').count() as i32;
                i += 1;
                if depth <= 0 || i >= n { break; }
            }
            continue;
        }
        // Strip standalone #import / #let lines in the preamble region.
        if t.starts_with("#import ") || t.starts_with("#let ") {
            i += 1;
            continue;
        }
        break;
    }
    // Trim leading blank lines before actual content
    while i < n && lines[i].trim().is_empty() { i += 1; }
    if i >= n { return String::new(); }
    let result = lines[i..].join("\n");
    if result.ends_with('\n') { result } else { result + "\n" }
}

// ── Document import via pandoc (LaTeX, Word, Markdown, OpenDocument Text) ──────

pub(super) struct ImportFormat {
    pub(super) label: &'static str,
    pub(super) icon: &'static str,
    /// File-glob patterns, e.g. `&["*.html", "*.htm"]`.
    pub(super) patterns: &'static [&'static str],
    /// Bare extensions (no dot), used to match dropped files — kept separate
    /// from `patterns` since drop-matching compares against `Path::extension()`.
    pub(super) extensions: &'static [&'static str],
    pub(super) filter_name: &'static str,
    pub(super) pandoc_from: &'static str,
}

pub(super) const IMPORT_FORMATS: &[ImportFormat] = &[
    ImportFormat {
        label: "LaTeX (.tex)",
        icon: "text-x-generic-symbolic",
        patterns: &["*.tex"],
        extensions: &["tex"],
        filter_name: "LaTeX files (*.tex)",
        pandoc_from: "latex",
    },
    ImportFormat {
        label: "Word (.docx)",
        icon: "x-office-document-symbolic",
        patterns: &["*.docx"],
        extensions: &["docx"],
        filter_name: "Word documents (*.docx)",
        pandoc_from: "docx",
    },
    ImportFormat {
        label: "Markdown (.md)",
        icon: "text-x-generic-symbolic",
        patterns: &["*.md", "*.markdown"],
        extensions: &["md", "markdown"],
        filter_name: "Markdown files (*.md)",
        pandoc_from: "markdown",
    },
    ImportFormat {
        label: "OpenDocument Text (.odt)",
        icon: "x-office-document-symbolic",
        patterns: &["*.odt"],
        extensions: &["odt"],
        filter_name: "OpenDocument Text (*.odt)",
        pandoc_from: "odt",
    },
    ImportFormat {
        label: "HTML (.html)",
        icon: "text-x-generic-symbolic",
        patterns: &["*.html", "*.htm"],
        extensions: &["html", "htm"],
        filter_name: "HTML files (*.html)",
        pandoc_from: "html",
    },
    ImportFormat {
        label: "EPUB (.epub)",
        icon: "x-office-document-symbolic",
        patterns: &["*.epub"],
        extensions: &["epub"],
        filter_name: "EPUB files (*.epub)",
        pandoc_from: "epub",
    },
    ImportFormat {
        label: "Rich Text (.rtf)",
        icon: "x-office-document-symbolic",
        patterns: &["*.rtf"],
        extensions: &["rtf"],
        filter_name: "Rich Text files (*.rtf)",
        pandoc_from: "rtf",
    },
];

/// Read-only list of past import attempts (`ImportLog`), reached from the
/// history icon in the Import picker dialog.
/// Find the `ImportFormat` a history record's stored label refers to, so
/// "Retry" can re-run the same pipeline without the user re-picking a format.
fn find_import_format_by_label(label: &str) -> Option<&'static ImportFormat> {
    IMPORT_FORMATS.iter().find(|f| f.label == label)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn show_import_history_dialog(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    work_dir: &std::path::Path,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
) {
    show_import_history_dialog_filtered(window, editor, work_dir, cfg, toast_overlay, false);
}

#[allow(clippy::too_many_arguments)]
fn show_import_history_dialog_filtered(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    work_dir: &std::path::Path,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
    initial_failed_only: bool,
) {
    let log = crate::import_log::ImportLog::load();

    let dlg = adw::Window::new();
    dlg.set_title(Some("Import History"));
    dlg.set_default_size(480, 460);
    dlg.set_transient_for(Some(window));
    dlg.set_modal(true);

    let header = adw::HeaderBar::new();
    let title_lbl = gtk4::Label::new(Some("Import History"));
    title_lbl.add_css_class("heading");
    header.set_title_widget(Some(&title_lbl));

    if log.records.is_empty() {
        let empty = adw::StatusPage::new();
        empty.set_icon_name(Some("document-open-recent-symbolic"));
        empty.set_title("No Imports Yet");
        empty.set_description(Some("Documents you import will be listed here."));
        empty.set_vexpand(true);
        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&empty));
        dlg.set_content(Some(&toolbar_view));
        dlg.present();
        return;
    }

    let clear_btn = Button::from_icon_name("user-trash-symbolic");
    clear_btn.add_css_class("flat");
    clear_btn.set_tooltip_text(Some("Clear History"));
    header.pack_end(&clear_btn);
    {
        let win_c = window.clone();
        let ep_c = editor.clone();
        let work_dir_c = work_dir.to_path_buf();
        let cfg_c = cfg.clone();
        let toast_c = toast_overlay.clone();
        let dlg_c = dlg.clone();
        clear_btn.connect_clicked(move |_| {
            let mut log = crate::import_log::ImportLog::load();
            log.clear();
            dlg_c.close();
            show_import_history_dialog(&win_c, &ep_c, &work_dir_c, &cfg_c, &toast_c);
        });
    }

    let failed_only_btn = gtk4::ToggleButton::new();
    failed_only_btn.set_icon_name("dialog-warning-symbolic");
    failed_only_btn.set_tooltip_text(Some("Show only failed imports"));
    failed_only_btn.add_css_class("flat");
    failed_only_btn.set_active(initial_failed_only);
    header.pack_end(&failed_only_btn);

    let search_entry = gtk4::SearchEntry::new();
    search_entry.set_placeholder_text(Some("Filter by filename, format, or message…"));
    search_entry.set_margin_start(12);
    search_entry.set_margin_end(12);
    search_entry.set_margin_top(8);

    let outer_box = GtkBox::new(Orientation::Vertical, 0);
    outer_box.append(&search_entry);

    let list_box = gtk4::ListBox::new();
    list_box.add_css_class("boxed-list");
    list_box.set_selection_mode(gtk4::SelectionMode::None);

    let total = log.records.len();
    for (display_idx, record) in log.records.iter().rev().enumerate() {
        let record_idx = total - 1 - display_idx;
        let row = adw::ActionRow::new();
        let name = record.source.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        row.set_title(name);
        row.set_subtitle(&format!("{} · {} · {}", record.date, record.format, record.message));
        row.set_widget_name(&format!(
            "{}|{} {} {}",
            if record.success { "ok" } else { "fail" },
            name, record.format, record.message
        ).to_lowercase());

        let prefix = if record.success {
            let img = gtk4::Image::from_icon_name("emblem-ok-symbolic");
            img.add_css_class("success");
            img
        } else {
            let img = gtk4::Image::from_icon_name("dialog-warning-symbolic");
            img.add_css_class("warning");
            img
        };
        row.add_prefix(&prefix);

        if let Some(output) = &record.output {
            if output.exists() {
                let reveal_btn = Button::from_icon_name("folder-open-symbolic");
                reveal_btn.add_css_class("flat");
                reveal_btn.set_valign(Align::Center);
                reveal_btn.set_tooltip_text(Some("Show containing folder"));
                let output_dir = output.parent().map(|p| p.to_path_buf());
                reveal_btn.connect_clicked(move |_| {
                    if let Some(dir) = &output_dir {
                        let _ = crate::git_sync::host_command("xdg-open").arg(dir).spawn();
                    }
                });
                row.add_suffix(&reveal_btn);
            }
        }

        if !record.success && record.source.exists() {
            let retry_btn = Button::from_icon_name("view-refresh-symbolic");
            retry_btn.add_css_class("flat");
            retry_btn.set_valign(Align::Center);
            retry_btn.set_tooltip_text(Some("Retry"));
            let win_c = window.clone();
            let ep_c = editor.clone();
            let work_dir_c = work_dir.to_path_buf();
            let cfg_c = cfg.clone();
            let toast_c = toast_overlay.clone();
            let dlg_c = dlg.clone();
            let source = record.source.clone();
            let format_label = record.format.clone();
            retry_btn.connect_clicked(move |_| {
                dlg_c.close();
                if format_label == "PDF (.pdf)" {
                    run_pdf_import(&win_c, &ep_c, source.clone());
                } else if let Some(fmt) = find_import_format_by_label(&format_label) {
                    run_pandoc_import(&win_c, &ep_c, &cfg_c, &toast_c, &work_dir_c, source.clone(), fmt);
                }
            });
            row.add_suffix(&retry_btn);
        }

        let delete_btn = Button::from_icon_name("edit-delete-symbolic");
        delete_btn.add_css_class("flat");
        delete_btn.set_valign(Align::Center);
        delete_btn.set_tooltip_text(Some("Remove from history"));
        let win_c = window.clone();
        let ep_c = editor.clone();
        let work_dir_c = work_dir.to_path_buf();
        let cfg_c = cfg.clone();
        let toast_c = toast_overlay.clone();
        let dlg_c = dlg.clone();
        delete_btn.connect_clicked(move |_| {
            let mut log = crate::import_log::ImportLog::load();
            log.remove(record_idx);
            dlg_c.close();
            show_import_history_dialog(&win_c, &ep_c, &work_dir_c, &cfg_c, &toast_c);
        });
        row.add_suffix(&delete_btn);

        list_box.append(&row);
    }

    let search_text: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    {
        let search_text_c = search_text.clone();
        let failed_only_c = failed_only_btn.clone();
        list_box.set_filter_func(move |row| {
            let wn = row.widget_name().to_string();
            let Some((status, text)) = wn.split_once('|') else { return true };
            if failed_only_c.is_active() && status != "fail" {
                return false;
            }
            let query = search_text_c.borrow();
            query.is_empty() || text.contains(query.as_str())
        });
    }
    {
        let lb = list_box.clone();
        let search_text_c = search_text.clone();
        search_entry.connect_search_changed(move |e| {
            *search_text_c.borrow_mut() = e.text().to_lowercase();
            lb.invalidate_filter();
        });
    }
    {
        let lb = list_box.clone();
        failed_only_btn.connect_toggled(move |_| {
            lb.invalidate_filter();
        });
    }

    let scroll = ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_child(Some(&list_box));
    scroll.set_margin_start(12);
    scroll.set_margin_end(12);
    scroll.set_margin_top(8);
    scroll.set_margin_bottom(12);
    outer_box.append(&scroll);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&outer_box));
    dlg.set_content(Some(&toolbar_view));
    dlg.present();
}

/// If `path` already exists, find the next free "`stem` (N).typ" instead of
/// silently overwriting it — mirrors the "Untitled 2.typ" collision-avoidance
/// convention in `library_window.rs::create_new_from_template`.
fn unique_typ_path(path: std::path::PathBuf) -> std::path::PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("output").to_string();
    let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let mut n = 1;
    loop {
        let candidate = dir.join(format!("{stem} ({n}).typ"));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

/// Translate a couple of common pandoc failure signatures into a plain-language
/// message; anything else falls back to the raw stderr (first 5 lines).
fn describe_pandoc_failure(stderr: &str) -> String {
    let lower = stderr.to_lowercase();
    if lower.contains("unknown writer") || lower.contains("unrecognized output format")
        || lower.contains("unknown output format")
    {
        return "Your pandoc version doesn't support Typst output. Zerkalo needs \
                pandoc 3.1 or later — you have an older version installed."
            .to_string();
    }
    format!("pandoc error:\n{}", stderr.lines().take(5).collect::<Vec<_>>().join("\n"))
}

/// Best-effort detection of Zotero/Mendeley/EndNote field codes inside a
/// `.docx`'s `word/document.xml` — these citation managers store citations as
/// proprietary custom-XML field codes that pandoc's docx reader doesn't
/// understand, so such citations silently convert to nothing rather than a
/// Typst `@key`, unlike plain typed citations. Requires `unzip`; if it's
/// missing or the file can't be read, this just reports no signatures found
/// rather than blocking the import on a missing optional tool.
fn docx_has_citation_manager_fields(path: &std::path::Path) -> bool {
    let Ok(output) = crate::git_sync::host_command("unzip")
        .arg("-p").arg(path).arg("word/document.xml")
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let xml = String::from_utf8_lossy(&output.stdout).to_lowercase();
    xml.contains("zotero") || xml.contains("mendeley") || xml.contains("endnote")
}

/// Build the base pandoc invocation for converting `input_name` (a bare
/// filename, relative to `input_dir`) to Typst. `.current_dir()` on the outer
/// Command only moves `flatpak-spawn`'s own cwd inside the sandbox, not the
/// host pandoc process's — flatpak-spawn needs an explicit `--directory=`,
/// the same reason git_sync's `git_cmd` uses `-C <repo>` instead of relying
/// on `.current_dir()`.
fn build_pandoc_command(
    input_dir: &std::path::Path,
    input_name: &str,
    pandoc_from: &str,
    out_name: &str,
    media_name: &str,
) -> std::process::Command {
    let mut cmd = if crate::git_sync::in_flatpak() {
        let mut c = std::process::Command::new("flatpak-spawn");
        c.arg("--host").arg(format!("--directory={}", input_dir.display())).arg("pandoc");
        c
    } else {
        let mut c = std::process::Command::new("pandoc");
        c.current_dir(input_dir);
        c
    };
    cmd.arg(input_name)
        .arg("-f").arg(pandoc_from)
        .arg("-t").arg("typst")
        .arg("--standalone")
        .arg(format!("--extract-media={media_name}"))
        .arg("-o").arg(out_name)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    cmd
}

/// Recursively copy a directory (used to relocate pandoc's `--extract-media`
/// output when the user chooses a different destination than the source's
/// own folder in the import-preview dialog).
fn copy_dir_recursive(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

/// Recursively collect files under `dir` matching any of `extensions`, for
/// "Include subfolders" in batch import. Skips hidden directories (`.git`
/// and similar) and any `*_media` directory — pandoc's own `--extract-media`
/// output, not a source document folder.
fn scan_files_recursive(dir: &std::path::Path, extensions: &[&str], out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if name.starts_with('.') || name.ends_with("_media") {
                continue;
            }
            scan_files_recursive(&path, extensions, out);
        } else if path.extension().and_then(|e| e.to_str())
            .map(|ext| extensions.iter().any(|want| want.eq_ignore_ascii_case(ext)))
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
}

/// First `.bib`/`.yaml`/`.yml` file directly inside `dir`, if any — the same
/// matching rule as the project-root auto-detect at startup (`app_window.rs`,
/// "Auto-detect .bib when no bib is configured"), reused here to offer the
/// same convenience right after importing a document that likely cites one.
fn find_bib_like_file(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    std::fs::read_dir(dir).ok()?.find_map(|e| {
        let path = e.ok()?.path();
        let ext = path.extension().and_then(|x| x.to_str())?;
        if ext.eq_ignore_ascii_case("bib") || ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml") {
            Some(path)
        } else {
            None
        }
    })
}

/// If `cfg.bib_path` isn't already set, look for a bibliography file next to
/// the just-imported source and offer to use it via a toast action.
/// Returns `true` if a bibliography-like file was found next to the import
/// (whether or not a toast ended up being shown for it), so callers can
/// decide whether a *different* nudge (see `warn_if_citations_without_bib`)
/// still applies.
fn offer_bib_autodetect(
    toast_overlay: &adw::ToastOverlay,
    cfg: &Rc<RefCell<Config>>,
    input_dir: &std::path::Path,
) -> bool {
    let Some(bib_path) = find_bib_like_file(input_dir) else { return false };
    if cfg.borrow().bib_path.is_some() {
        return true;
    }
    let name = bib_path.file_name().and_then(|n| n.to_str()).unwrap_or("bibliography file").to_string();
    let toast = adw::Toast::new(&format!("Found {name} — use it as your bibliography?"));
    toast.set_button_label(Some("Set"));
    toast.set_timeout(6);
    let cfg_c = cfg.clone();
    toast.connect_button_clicked(move |_| {
        let mut c = cfg_c.borrow_mut();
        c.bib_path = Some(bib_path.clone());
        let _ = c.save();
    });
    toast_overlay.add_toast(toast);
    true
}

/// Shown after a successful pandoc conversion, before anything is written
/// permanently: a read-only preview of the generated Typst (matching the
/// "Preview Code" window in `template_dialog.rs`) plus a destination choice.
/// "Import" writes the chosen destination and opens it; "Discard" deletes the
/// temporary files pandoc already wrote and does nothing further.
#[allow(clippy::too_many_arguments)]
/// A rough, at-a-glance read of what pandoc produced, shown above the preview
/// text so a user can judge conversion fidelity before committing — especially
/// useful for math-heavy sources, where LaTeX-to-Typst equation syntax
/// sometimes needs manual cleanup that a silent word count wouldn't hint at.
/// Counts probable Typst `@key` citations — an `@` not preceded by a word
/// character and followed by an identifier-starting letter, which excludes
/// email addresses and other incidental `@` uses.
fn count_citations(text: &str) -> usize {
    let bytes = text.as_bytes();
    bytes.iter().enumerate().filter(|(i, &b)| {
        if b != b'@' { return false; }
        let prev_is_wordchar = *i > 0 && (bytes[*i - 1] as char).is_alphanumeric();
        let next_is_ident_start = text[*i + 1..].chars().next().map(|c| c.is_alphabetic()).unwrap_or(false);
        !prev_is_wordchar && next_is_ident_start
    }).count()
}

/// If the converted document cites sources but no bibliography is configured
/// and none was found next to it, nudge the user. This is deliberately just a
/// nudge, not extraction: DOCX/ODT documents with Zotero/Mendeley-managed
/// citations carry that data in proprietary field codes, not something
/// pandoc's CLI can export as a standalone `.bib` file.
fn warn_if_citations_without_bib(
    toast_overlay: &adw::ToastOverlay,
    cfg: &Rc<RefCell<Config>>,
    processed: &str,
    found_nearby_bib: bool,
) {
    if found_nearby_bib || cfg.borrow().bib_path.is_some() {
        return;
    }
    if count_citations(processed) == 0 {
        return;
    }
    let toast = adw::Toast::new(
        "This document cites sources but no bibliography is set. If it used \
         Zotero, Mendeley, or EndNote, export your library to a .bib file and \
         place it alongside this document.",
    );
    toast.set_timeout(8);
    toast_overlay.add_toast(toast);
}

fn summarize_import_content(text: &str) -> String {
    let words = crate::writing_log::count_words(text);
    let headings = text.lines().filter(|l| l.trim_start().starts_with('=')).count();
    let images = text.matches("image(").count();
    let citations = count_citations(text);

    // Rough: Typst inline/block math is `$...$`; count paired delimiters.
    let equations = text.matches('$').count() / 2;

    let mut parts = vec![format!("{words} word{}", if words == 1 { "" } else { "s" })];
    if headings > 0 { parts.push(format!("{headings} heading{}", if headings == 1 { "" } else { "s" })); }
    if images > 0 { parts.push(format!("{images} image{}", if images == 1 { "" } else { "s" })); }
    if citations > 0 { parts.push(format!("{citations} citation{}", if citations == 1 { "" } else { "s" })); }
    if equations > 0 { parts.push(format!("~{equations} equation{} — review math syntax", if equations == 1 { "" } else { "s" })); }
    parts.join(" · ")
}

// Each argument is a distinct widget or piece of state this flow needs; a
// struct here would just be a bag with one caller.
#[allow(clippy::too_many_arguments)]
fn show_import_preview_dialog(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
    input_path: std::path::PathBuf,
    fmt_label: &'static str,
    processed: String,
    temp_out_path: std::path::PathBuf,
    media_name: String,
    work_dir: std::path::PathBuf,
    pandoc_warnings: String,
) {
    let input_dir = input_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let out_name = temp_out_path.file_name().and_then(|s| s.to_str()).unwrap_or("output.typ").to_string();

    let dlg = adw::Window::new();
    dlg.set_title(Some("Import Preview"));
    dlg.set_default_size(680, 560);
    dlg.set_transient_for(Some(window));
    dlg.set_modal(false);

    let header = adw::HeaderBar::new();
    let discard_btn = Button::with_label("Discard");
    discard_btn.add_css_class("flat");
    header.pack_start(&discard_btn);
    let import_btn = Button::with_label("Import");
    import_btn.add_css_class("suggested-action");
    header.pack_end(&import_btn);

    let outer = GtkBox::new(Orientation::Vertical, 0);

    let dest_group = adw::PreferencesGroup::new();
    dest_group.set_margin_start(12);
    dest_group.set_margin_end(12);
    dest_group.set_margin_top(8);
    dest_group.set_margin_bottom(8);
    let dest_row = adw::ComboRow::new();
    dest_row.set_title("Save to");
    let same_as_project = input_dir == work_dir;
    dest_row.set_model(Some(&gtk4::StringList::new(&[
        "This project",
        "Same folder as source file",
    ])));
    dest_row.set_selected(if same_as_project { 0 } else { 1 });
    dest_group.add(&dest_row);
    outer.append(&dest_group);

    let summary_lbl = gtk4::Label::new(Some(&summarize_import_content(&processed)));
    summary_lbl.add_css_class("dim-label");
    summary_lbl.add_css_class("caption");
    summary_lbl.set_halign(Align::Start);
    summary_lbl.set_margin_start(16);
    summary_lbl.set_margin_bottom(6);
    summary_lbl.set_wrap(true);
    outer.append(&summary_lbl);

    let warning_lines: Vec<&str> = pandoc_warnings.lines().filter(|l| !l.trim().is_empty()).collect();
    if !warning_lines.is_empty() {
        let warn_lbl = gtk4::Label::new(Some(&format!(
            "pandoc reported {} warning{} during conversion:\n{}",
            warning_lines.len(),
            if warning_lines.len() == 1 { "" } else { "s" },
            warning_lines.iter().take(5).copied().collect::<Vec<&str>>().join("\n"),
        )));
        warn_lbl.add_css_class("warning");
        warn_lbl.add_css_class("caption");
        warn_lbl.set_halign(Align::Start);
        warn_lbl.set_xalign(0.0);
        warn_lbl.set_margin_start(16);
        warn_lbl.set_margin_end(16);
        warn_lbl.set_margin_bottom(6);
        warn_lbl.set_wrap(true);
        outer.append(&warn_lbl);
    }

    let is_docx = input_path.extension().and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("docx")).unwrap_or(false);
    if is_docx && count_citations(&processed) == 0 && docx_has_citation_manager_fields(&input_path) {
        let zotero_lbl = gtk4::Label::new(Some(
            "This document appears to use Zotero/Mendeley/EndNote-linked citations, \
             which pandoc can't read directly — that's likely why no citations \
             converted. In Word, use your citation manager's \"Unlink Citations\" \
             (or equivalent) first, then re-import.",
        ));
        zotero_lbl.add_css_class("warning");
        zotero_lbl.add_css_class("caption");
        zotero_lbl.set_halign(Align::Start);
        zotero_lbl.set_xalign(0.0);
        zotero_lbl.set_margin_start(16);
        zotero_lbl.set_margin_end(16);
        zotero_lbl.set_margin_bottom(6);
        zotero_lbl.set_wrap(true);
        outer.append(&zotero_lbl);
    }

    let tv = gtk4::TextView::new();
    tv.set_editable(false);
    tv.set_monospace(true);
    tv.set_left_margin(12);
    tv.set_right_margin(12);
    tv.set_top_margin(8);
    tv.set_bottom_margin(8);
    tv.buffer().set_text(&processed);
    let scroll = ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_hexpand(true);
    scroll.set_child(Some(&tv));
    outer.append(&scroll);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&outer));
    dlg.set_content(Some(&toolbar_view));

    {
        let dlg_c = dlg.clone();
        let temp_out = temp_out_path.clone();
        let temp_media = input_dir.join(&media_name);
        let input_path_c = input_path.clone();
        discard_btn.connect_clicked(move |_| {
            let _ = std::fs::remove_file(&temp_out);
            let _ = std::fs::remove_dir_all(&temp_media);
            let mut log = crate::import_log::ImportLog::load();
            log.record(input_path_c.clone(), fmt_label, None, false, "Discarded by user");
            dlg_c.close();
        });
    }

    {
        let dlg_c = dlg.clone();
        let editor_c = editor.clone();
        let cfg_c = cfg.clone();
        let toast_overlay_c = toast_overlay.clone();
        let input_path_c = input_path.clone();
        let input_dir_c = input_dir.clone();
        let temp_out = temp_out_path.clone();
        let out_name_c = out_name.clone();
        let media_name_c = media_name.clone();
        let dest_row_c = dest_row.clone();
        let processed_c = processed.clone();
        import_btn.connect_clicked(move |_| {
            let final_dir = if dest_row_c.selected() == 0 { work_dir.clone() } else { input_dir_c.clone() };
            let final_path = unique_typ_path(final_dir.join(&out_name_c));
            let _ = std::fs::write(&final_path, &processed_c);

            if final_dir != input_dir_c {
                let src_media = input_dir_c.join(&media_name_c);
                if src_media.is_dir() {
                    let dst_media = final_dir.join(&media_name_c);
                    let _ = copy_dir_recursive(&src_media, &dst_media);
                    let _ = std::fs::remove_dir_all(&src_media);
                }
                let _ = std::fs::remove_file(&temp_out);
            }

            editor_c.open_file(final_path.clone(), &processed_c);
            let found_bib = offer_bib_autodetect(&toast_overlay_c, &cfg_c, &input_dir_c);
            warn_if_citations_without_bib(&toast_overlay_c, &cfg_c, &processed_c, found_bib);

            let mut log = crate::import_log::ImportLog::load();
            log.record(input_path_c.clone(), fmt_label, Some(final_path.clone()), true, "Imported successfully");

            let name = final_path.file_name().and_then(|n| n.to_str()).unwrap_or("document").to_string();
            let imported_toast = adw::Toast::new(&format!("Imported {name}"));
            imported_toast.set_button_label(Some("Undo"));
            imported_toast.set_timeout(6);
            let ep_undo = editor_c.clone();
            let final_path_undo = final_path.clone();
            imported_toast.connect_button_clicked(move |_| {
                ep_undo.close_file_if_open(&final_path_undo);
                let _ = std::fs::remove_file(&final_path_undo);
                let mut log = crate::import_log::ImportLog::load();
                log.record(final_path_undo.clone(), fmt_label, None, false, "Undone by user");
            });
            toast_overlay_c.add_toast(imported_toast);

            dlg_c.close();
        });
    }

    dlg.present();
}

/// Shared entry point for all pandoc-based document import (LaTeX/DOCX/
/// Markdown/ODT/HTML/EPUB). Runs pandoc as a killable child process polled
/// from the main thread (no background thread/channel needed, since `Child`
/// isn't shared across threads), extracts embedded media instead of silently
/// dropping it, and never overwrites an existing `.typ` file.
pub(super) fn import_via_pandoc(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    work_dir: &std::path::Path,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
    fmt: &'static ImportFormat,
) {
    let dialog = gtk4::FileDialog::new();
    dialog.set_title(&format!("Import {}", fmt.label));
    let filter = gtk4::FileFilter::new();
    filter.set_name(Some(fmt.filter_name));
    for p in fmt.patterns { filter.add_pattern(p); }
    let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&filter);
    dialog.set_filters(Some(&filters));
    dialog.set_initial_folder(Some(&gtk4::gio::File::for_path(work_dir)));

    let win = window.clone();
    let ep = editor.clone();
    let cfg = cfg.clone();
    let toast_overlay = toast_overlay.clone();
    let work_dir = work_dir.to_path_buf();
    let win_ref = win.clone();
    // Multi-select: a single file keeps the interactive preview flow below;
    // several files route through the same sequential batch queue folder
    // import uses (same-folder-as-source destination, no per-file preview —
    // reviewing N files individually would defeat the point of multi-select).
    dialog.open_multiple(Some(&win_ref), None::<&gtk4::gio::Cancellable>, move |result| {
        let Ok(list) = result else { return };
        let paths: Vec<std::path::PathBuf> = (0..list.n_items())
            .filter_map(|i| list.item(i))
            .filter_map(|obj| obj.downcast::<gtk4::gio::File>().ok())
            .filter_map(|f| f.path())
            .collect();
        match paths.len() {
            0 => {}
            1 => {
                run_pandoc_import(&win, &ep, &cfg, &toast_overlay, &work_dir, paths.into_iter().next().unwrap(), fmt);
            }
            n => {
                let queue: std::collections::VecDeque<std::path::PathBuf> = paths.into_iter().collect();
                run_batch_import_queue(win.clone(), ep.clone(), cfg.clone(), toast_overlay.clone(), work_dir.clone(), false, queue, fmt, n);
            }
        }
    });
}

/// Entry point for single-file pandoc import (from the picker, drag-drop,
/// multi-select, or Retry). Warns first if this exact source was already
/// imported successfully before, in case the user picked the wrong file or
/// forgot they'd already converted it; otherwise proceeds immediately.
pub(super) fn run_pandoc_import(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
    work_dir: &std::path::Path,
    input_path: std::path::PathBuf,
    fmt: &'static ImportFormat,
) {
    let log = crate::import_log::ImportLog::load();
    let prior = log.records.iter().rev().find(|r| r.success && r.source == input_path).cloned();

    let Some(prior) = prior else {
        run_pandoc_import_confirmed(window, editor, cfg, toast_overlay, work_dir, input_path, fmt);
        return;
    };

    let dlg = adw::MessageDialog::new(
        Some(window),
        Some("Already Imported"),
        Some(&format!(
            "You already imported this file on {}. Import it again?",
            prior.date
        )),
    );
    dlg.add_response("cancel", "Cancel");
    dlg.add_response("ok", "Import Anyway");
    dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
    dlg.set_default_response(Some("cancel"));
    dlg.set_close_response("cancel");

    let win = window.clone();
    let ep = editor.clone();
    let cfg = cfg.clone();
    let toast_overlay = toast_overlay.clone();
    let work_dir = work_dir.to_path_buf();
    dlg.connect_response(None, move |_, resp| {
        if resp == "ok" {
            run_pandoc_import_confirmed(&win, &ep, &cfg, &toast_overlay, &work_dir, input_path.clone(), fmt);
        }
    });
    dlg.present();
}

/// Spawns pandoc for a single input file and wires up progress/cancel/result
/// handling. Split out from `import_via_pandoc` so batch/folder import (which
/// already has its file list, no picker dialog needed) can call it directly.
fn run_pandoc_import_confirmed(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
    work_dir: &std::path::Path,
    input_path: std::path::PathBuf,
    fmt: &'static ImportFormat,
) {
    let stem = input_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output").to_string();
    let out_path = unique_typ_path(input_path.with_file_name(format!("{stem}.typ")));
    // Typst resolves `/`-rooted paths against the project root, not the OS
    // filesystem — so pandoc must be run with cwd = the input's directory and
    // given bare relative names, or `--extract-media`/`-o` with absolute paths
    // makes it emit `#image("/abs/os/path...")`, which won't resolve as an
    // image path inside the document (verified against a real pandoc run).
    let out_stem = out_path.file_stem().and_then(|s| s.to_str()).unwrap_or(&stem).to_string();
    let out_name = out_path.file_name().and_then(|s| s.to_str()).unwrap_or("output.typ").to_string();
    let media_name = format!("{out_stem}_media");
    let input_dir = input_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let input_name = input_path.file_name().and_then(|s| s.to_str()).unwrap_or("input").to_string();

    let mut cmd = build_pandoc_command(&input_dir, &input_name, fmt.pandoc_from, &out_name, &media_name);
    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => {
            show_alert(window, "Import Failed", &format!(
                "pandoc was not found. Install it to use {} import:\n\
                 \n  zypper install pandoc\
                 \n  apt   install pandoc\
                 \n  brew  install pandoc\
                 \n  dnf   install pandoc\
                 \nVersion 3.1 or later is required.",
                fmt.label
            ));
            let mut log = crate::import_log::ImportLog::load();
            log.record(input_path, fmt.label, None, false, "pandoc not found");
            return;
        }
    };
    let child = Rc::new(RefCell::new(Some(child)));

    let toast = adw::Toast::new(&format!("Importing {}…", fmt.label));
    toast.set_priority(adw::ToastPriority::High);
    toast.set_timeout(0);
    toast.set_button_label(Some("Cancel"));
    {
        // Killing the local `flatpak-spawn` client process does not always
        // guarantee the host-side pandoc process it launched also terminates
        // immediately — best-effort, but this is the only cancellation lever
        // available without a portal-level process-tracking API.
        let child_for_cancel = child.clone();
        let toast_for_cancel = toast.clone();
        let toast_overlay_for_cancel = toast_overlay.clone();
        let input_path_for_cancel = input_path.clone();
        toast.connect_button_clicked(move |_| {
            if let Some(mut c) = child_for_cancel.borrow_mut().take() {
                let _ = c.kill();
            }
            toast_for_cancel.dismiss();
            let cancelled = adw::Toast::new("Import cancelled");
            cancelled.set_timeout(3);
            toast_overlay_for_cancel.add_toast(cancelled);
            let mut log = crate::import_log::ImportLog::load();
            log.record(input_path_for_cancel.clone(), fmt.label, None, false, "Cancelled by user");
        });
    }
    toast_overlay.add_toast(toast.clone());

    let started = std::time::Instant::now();
    let child_poll = child.clone();
    let win = window.clone();
    let ep = editor.clone();
    let cfg = cfg.clone();
    let toast_overlay = toast_overlay.clone();
    let work_dir = work_dir.to_path_buf();
    let out_path = out_path.clone();
    glib::timeout_add_local(Duration::from_millis(150), move || {
        let mut guard = child_poll.borrow_mut();
        let Some(c) = guard.as_mut() else {
            // Already taken (and killed) by the Cancel button above.
            return glib::ControlFlow::Break;
        };
        match c.try_wait() {
            Ok(Some(status)) => {
                let stdout = c.stdout.take();
                let stderr = c.stderr.take();
                drop(guard);
                toast.dismiss();
                use std::io::Read;
                let mut stderr_text = String::new();
                if let Some(mut s) = stderr { let _ = s.read_to_string(&mut stderr_text); }
                let _ = stdout;

                if status.success() {
                    if let Ok(raw) = std::fs::read_to_string(&out_path) {
                        let bib_path = cfg.borrow().bib_path.clone();
                        let processed = post_process_latex_import(&raw, bib_path.as_deref());
                        show_import_preview_dialog(
                            &win, &ep, &cfg, &toast_overlay,
                            input_path.clone(), fmt.label, processed,
                            out_path.clone(), media_name.clone(), work_dir.clone(),
                            stderr_text.clone(),
                        );
                    } else {
                        show_alert(&win, "Import Failed", "pandoc reported success but the output file could not be read.");
                        let mut log = crate::import_log::ImportLog::load();
                        log.record(input_path.clone(), fmt.label, None, false, "Output file unreadable");
                    }
                } else {
                    let description = describe_pandoc_failure(&stderr_text);
                    show_alert(&win, "Import Failed", &description);
                    let mut log = crate::import_log::ImportLog::load();
                    log.record(input_path.clone(), fmt.label, None, false, &description);
                }
                glib::ControlFlow::Break
            }
            Ok(None) => {
                let secs = started.elapsed().as_secs();
                if secs > 0 {
                    toast.set_title(&format!("Importing {}… ({secs}s)", fmt.label));
                }
                glib::ControlFlow::Continue
            }
            Err(_) => {
                drop(guard);
                toast.dismiss();
                show_alert(&win, "Import Failed", "Failed to check the import process's status.");
                glib::ControlFlow::Break
            }
        }
    });
}

/// Small picker for batch import: choose a format, a folder, and a destination,
/// then convert every matching file in that folder one at a time (not in
/// parallel — avoids launching many concurrent pandoc processes). Unlike the
/// single-file flow, batch import skips the per-file preview dialog; the
/// dialog says so up front.
/// "Paste as Document": reads plain text off the clipboard (not rich HTML —
/// that would need mime-type negotiation via `read_value_async`, out of scope
/// here) and runs it through the same markdown pandoc path as a file import,
/// via stdin instead of a saved file.
pub(super) fn paste_as_document(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    work_dir: &std::path::Path,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
) {
    let clipboard = window.clipboard();
    let win = window.clone();
    let editor = editor.clone();
    let work_dir = work_dir.to_path_buf();
    let cfg = cfg.clone();
    let toast_overlay = toast_overlay.clone();
    clipboard.read_text_async(None::<&gtk4::gio::Cancellable>, move |result| {
        let Ok(Some(text)) = result else {
            show_alert(&win, "Nothing to Paste", "The clipboard doesn't contain any text.");
            return;
        };
        prompt_paste_filename(&win, &editor, &work_dir, &cfg, &toast_overlay, text.to_string());
    });
}

fn prompt_paste_filename(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    work_dir: &std::path::Path,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
    text: String,
) {
    let has_open_doc = editor.get_active_path().is_some();

    let dlg = adw::MessageDialog::new(Some(window), Some("Paste as Document"), None);
    dlg.add_response("cancel", "Cancel");
    dlg.add_response("ok", "Import");
    dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
    dlg.set_default_response(Some("ok"));
    dlg.set_close_response("cancel");

    let container = GtkBox::new(Orientation::Vertical, 10);

    let dest_row = adw::ComboRow::new();
    let entry = Entry::new();
    entry.set_placeholder_text(Some("Untitled"));

    if has_open_doc {
        dest_row.set_title("Destination");
        dest_row.set_model(Some(&gtk4::StringList::new(&[
            "Insert into the current document",
            "Create a new document",
        ])));
        dest_row.set_selected(0);
        container.append(&dest_row);
        entry.set_visible(false);
        {
            let entry_c = entry.clone();
            dest_row.connect_selected_notify(move |row| {
                entry_c.set_visible(row.selected() == 1);
            });
        }
    } else {
        let lbl = Label::new(Some("Name the new document:"));
        lbl.set_halign(Align::Start);
        container.append(&lbl);
    }
    container.append(&entry);
    dlg.set_extra_child(Some(&container));

    let win = window.clone();
    let editor = editor.clone();
    let work_dir_c = work_dir.to_path_buf();
    let cfg = cfg.clone();
    let toast_overlay = toast_overlay.clone();
    let entry_c = entry.clone();
    dlg.connect_response(None, move |_, resp| {
        if resp != "ok" { return; }
        let insert_at_cursor = has_open_doc && dest_row.selected() == 0;
        let name = entry_c.text().to_string();
        let stem = if name.trim().is_empty() { "Untitled".to_string() } else { name.trim().to_string() };
        run_pandoc_import_from_stdin(&win, &editor, &cfg, &toast_overlay, &work_dir_c, text.clone(), &stem, insert_at_cursor);
    });
    dlg.present();
}

/// Like `run_pandoc_import`, but for content that isn't a file on disk yet —
/// pandoc reads from stdin (`-` as input) instead of a named file.
#[allow(clippy::too_many_arguments)]
fn run_pandoc_import_from_stdin(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
    work_dir: &std::path::Path,
    text: String,
    stem: &str,
    insert_at_cursor: bool,
) {
    let out_path = unique_typ_path(work_dir.join(format!("{stem}.typ")));
    let out_name = out_path.file_name().and_then(|s| s.to_str()).unwrap_or("output.typ").to_string();

    let mut cmd = if crate::git_sync::in_flatpak() {
        let mut c = std::process::Command::new("flatpak-spawn");
        c.arg("--host").arg(format!("--directory={}", work_dir.display())).arg("pandoc");
        c
    } else {
        let mut c = std::process::Command::new("pandoc");
        c.current_dir(work_dir);
        c
    };
    cmd.arg("-f").arg("markdown")
        .arg("-t").arg("typst")
        .arg("--standalone")
        .arg("-o").arg(&out_name)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => {
            show_alert(window, "Import Failed", "pandoc was not found. Install it to use Paste as Document.");
            return;
        }
    };
    {
        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
    }
    let child = Rc::new(RefCell::new(Some(child)));

    let toast = adw::Toast::new("Importing pasted text…");
    toast.set_priority(adw::ToastPriority::High);
    toast.set_timeout(0);
    toast_overlay.add_toast(toast.clone());

    let win = window.clone();
    let ep = editor.clone();
    let cfg = cfg.clone();
    let source_label = std::path::PathBuf::from(format!("Pasted text ({stem})"));
    glib::timeout_add_local(Duration::from_millis(150), move || {
        let mut guard = child.borrow_mut();
        let Some(c) = guard.as_mut() else { return glib::ControlFlow::Break };
        match c.try_wait() {
            Ok(Some(status)) => {
                let stderr = c.stderr.take();
                drop(guard);
                toast.dismiss();
                use std::io::Read;
                let mut stderr_text = String::new();
                if let Some(mut s) = stderr { let _ = s.read_to_string(&mut stderr_text); }

                let mut log = crate::import_log::ImportLog::load();
                if status.success() {
                    if let Ok(raw) = std::fs::read_to_string(&out_path) {
                        if insert_at_cursor {
                            // Body only — no Zerkalo preamble, since this is
                            // going into a document that (if templated) already
                            // has one.
                            let body = strip_pandoc_preamble(&raw);
                            let _ = std::fs::remove_file(&out_path);
                            ep.insert_at_cursor(&body);
                            log.record(source_label.clone(), "Paste as Document", None, true, "Inserted at cursor");
                        } else {
                            let bib_path = cfg.borrow().bib_path.clone();
                            let processed = post_process_latex_import(&raw, bib_path.as_deref());
                            let _ = std::fs::write(&out_path, &processed);
                            ep.open_file(out_path.clone(), &processed);
                            log.record(source_label.clone(), "Paste as Document", Some(out_path.clone()), true, "Imported successfully");
                        }
                    } else {
                        show_alert(&win, "Import Failed", "pandoc reported success but the output file could not be read.");
                        log.record(source_label.clone(), "Paste as Document", None, false, "Output file unreadable");
                    }
                } else {
                    let description = describe_pandoc_failure(&stderr_text);
                    show_alert(&win, "Import Failed", &description);
                    log.record(source_label.clone(), "Paste as Document", None, false, &description);
                }
                glib::ControlFlow::Break
            }
            Ok(None) => glib::ControlFlow::Continue,
            Err(_) => {
                drop(guard);
                toast.dismiss();
                glib::ControlFlow::Break
            }
        }
    });
}

pub(super) fn import_folder_via_pandoc(
    window: &adw::ApplicationWindow,
    editor: &EditorPane,
    work_dir: &std::path::Path,
    cfg: &Rc<RefCell<Config>>,
    toast_overlay: &adw::ToastOverlay,
) {
    let dlg = adw::Window::new();
    dlg.set_title(Some("Import Folder"));
    dlg.set_default_width(340);
    dlg.set_modal(true);
    dlg.set_transient_for(Some(window));

    let header = adw::HeaderBar::new();
    let title_lbl = gtk4::Label::new(Some("Import Folder"));
    title_lbl.add_css_class("heading");
    header.set_title_widget(Some(&title_lbl));

    let group = adw::PreferencesGroup::new();
    group.set_margin_start(12);
    group.set_margin_end(12);
    group.set_margin_top(8);
    group.set_description(Some("Every matching file is converted one at a time; each is opened without an individual preview step."));

    let format_row = adw::ComboRow::new();
    format_row.set_title("Format");
    let labels: Vec<&str> = IMPORT_FORMATS.iter().map(|f| f.label).collect();
    format_row.set_model(Some(&gtk4::StringList::new(&labels)));
    group.add(&format_row);

    let folder_row = adw::ActionRow::new();
    folder_row.set_title("Folder");
    folder_row.set_subtitle("Not selected");
    folder_row.set_activatable(true);
    folder_row.add_suffix(&gtk4::Image::from_icon_name("folder-open-symbolic"));
    group.add(&folder_row);

    let dest_row = adw::ComboRow::new();
    dest_row.set_title("Save to");
    dest_row.set_model(Some(&gtk4::StringList::new(&["This project", "Same folder as each source file"])));
    group.add(&dest_row);

    let recursive_row = adw::SwitchRow::new();
    recursive_row.set_title("Include subfolders");
    recursive_row.set_active(false);
    group.add(&recursive_row);

    let selected_folder: Rc<RefCell<Option<std::path::PathBuf>>> = Rc::new(RefCell::new(None));
    {
        let win_c = window.clone();
        let folder_row_c = folder_row.clone();
        let selected_folder_c = selected_folder.clone();
        folder_row.connect_activated(move |_| {
            let fd = gtk4::FileDialog::new();
            let folder_row2 = folder_row_c.clone();
            let selected_folder2 = selected_folder_c.clone();
            fd.select_folder(Some(&win_c), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        folder_row2.set_subtitle(&path.display().to_string());
                        *selected_folder2.borrow_mut() = Some(path);
                    }
                }
            });
        });
    }

    let import_btn = Button::with_label("Import Folder");
    import_btn.add_css_class("suggested-action");
    import_btn.set_margin_start(12);
    import_btn.set_margin_end(12);
    import_btn.set_margin_top(12);
    import_btn.set_margin_bottom(12);
    import_btn.set_halign(Align::End);

    let vbox = GtkBox::new(Orientation::Vertical, 0);
    vbox.append(&header);
    vbox.append(&group);
    vbox.append(&import_btn);
    dlg.set_content(Some(&vbox));

    {
        let dlg_c = dlg.clone();
        let win_c = window.clone();
        let ep_c = editor.clone();
        let cfg_c = cfg.clone();
        let toast_overlay_c = toast_overlay.clone();
        let work_dir_c = work_dir.to_path_buf();
        let format_row_c = format_row.clone();
        let dest_row_c = dest_row.clone();
        let recursive_row_c = recursive_row.clone();
        let selected_folder_c = selected_folder.clone();
        import_btn.connect_clicked(move |_| {
            let Some(folder) = selected_folder_c.borrow().clone() else { return };
            let idx = format_row_c.selected() as usize;
            let Some(fmt) = IMPORT_FORMATS.get(idx) else { return };
            let dest_this_project = dest_row_c.selected() == 0;
            dlg_c.close();

            let mut files: Vec<std::path::PathBuf> = Vec::new();
            if recursive_row_c.is_active() {
                scan_files_recursive(&folder, fmt.extensions, &mut files);
            } else {
                files.extend(std::fs::read_dir(&folder)
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        p.is_file() && p.extension().and_then(|e| e.to_str())
                            .map(|ext| fmt.extensions.iter().any(|want| want.eq_ignore_ascii_case(ext)))
                            .unwrap_or(false)
                    }));
            }
            files.sort();

            if files.is_empty() {
                show_alert(&win_c, "Nothing to Import", &format!("No {} files were found in that folder.", fmt.label));
                return;
            }

            let total = files.len();
            let queue: std::collections::VecDeque<std::path::PathBuf> = files.into_iter().collect();
            run_batch_import_queue(
                win_c.clone(), ep_c.clone(), cfg_c.clone(), toast_overlay_c.clone(),
                work_dir_c.clone(), dest_this_project, queue, fmt, total,
            );
        });
    }

    dlg.present();
}

/// Processes one file from the batch queue, then recurses for the next once
/// pandoc exits — sequential by design, to avoid many concurrent pandoc
/// processes and many simultaneous "Importing…" toasts.
/// Entry point for batch import: starts up to `cfg.batch_import_concurrency`
/// workers pulling from a shared queue, each recursing into the next file on
/// its own completion — bounded parallelism rather than strictly one-at-a-time,
/// with one shared progress toast updated as files finish.
#[allow(clippy::too_many_arguments)]
fn run_batch_import_queue(
    window: adw::ApplicationWindow,
    editor: EditorPane,
    cfg: Rc<RefCell<Config>>,
    toast_overlay: adw::ToastOverlay,
    work_dir: std::path::PathBuf,
    dest_this_project: bool,
    queue: std::collections::VecDeque<std::path::PathBuf>,
    fmt: &'static ImportFormat,
    total: usize,
) {
    let queue = Rc::new(RefCell::new(queue));
    let done = Rc::new(std::cell::Cell::new(0usize));
    let failed = Rc::new(std::cell::Cell::new(0usize));
    let active = Rc::new(std::cell::Cell::new(0usize));
    let written: Rc<RefCell<Vec<std::path::PathBuf>>> = Rc::new(RefCell::new(Vec::new()));

    let progress = adw::Toast::new(&format!("Importing… (0 of {total} done)"));
    progress.set_priority(adw::ToastPriority::High);
    progress.set_timeout(0);
    toast_overlay.add_toast(progress.clone());
    let progress = Rc::new(progress);

    let concurrency = cfg.borrow().batch_import_concurrency.max(1) as usize;
    let n_workers = concurrency.min(total.max(1));
    for _ in 0..n_workers {
        run_next_batch_worker(
            window.clone(), editor.clone(), cfg.clone(), toast_overlay.clone(),
            work_dir.clone(), dest_this_project, queue.clone(), fmt,
            done.clone(), failed.clone(), active.clone(), total, progress.clone(), written.clone(),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn run_next_batch_worker(
    window: adw::ApplicationWindow,
    editor: EditorPane,
    cfg: Rc<RefCell<Config>>,
    toast_overlay: adw::ToastOverlay,
    work_dir: std::path::PathBuf,
    dest_this_project: bool,
    queue: Rc<RefCell<std::collections::VecDeque<std::path::PathBuf>>>,
    fmt: &'static ImportFormat,
    done: Rc<std::cell::Cell<usize>>,
    failed: Rc<std::cell::Cell<usize>>,
    active: Rc<std::cell::Cell<usize>>,
    total: usize,
    progress: Rc<adw::Toast>,
    written: Rc<RefCell<Vec<std::path::PathBuf>>>,
) {
    let Some(input_path) = queue.borrow_mut().pop_front() else {
        // No more work for this worker slot. Once every worker has reached
        // this point (none still active), the batch is finished.
        if active.get() == 0 {
            progress.dismiss();
            let has_failures = failed.get() > 0;
            let has_successes = done.get() > 0;
            let summary = if has_failures {
                format!("Imported {} of {} files ({} failed)", done.get(), total, failed.get())
            } else {
                format!("Imported {} of {} files", done.get(), total)
            };
            let toast = adw::Toast::new(&summary);
            toast.set_timeout(5);
            if has_successes {
                // Only one action button fits on a toast — undoing the batch is
                // the more time-sensitive action when there's something to undo.
                toast.set_button_label(Some("Undo All"));
                let editor_c = editor.clone();
                let written_c = written.clone();
                let fmt_label = fmt.label;
                toast.connect_button_clicked(move |_| {
                    let mut log = crate::import_log::ImportLog::load();
                    for path in written_c.borrow().iter() {
                        editor_c.close_file_if_open(path);
                        let _ = std::fs::remove_file(path);
                        log.record(path.clone(), fmt_label, None, false, "Undone by user (batch)");
                    }
                });
            } else if has_failures {
                toast.set_button_label(Some("View Failures"));
                let win_c = window.clone();
                let ep_c = editor.clone();
                let work_dir_c = work_dir.clone();
                let cfg_c = cfg.clone();
                let toast_overlay_c = toast_overlay.clone();
                toast.connect_button_clicked(move |_| {
                    show_import_history_dialog_filtered(&win_c, &ep_c, &work_dir_c, &cfg_c, &toast_overlay_c, true);
                });
            }
            toast_overlay.add_toast(toast);
        }
        return;
    };
    active.set(active.get() + 1);

    let stem = input_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output").to_string();
    let out_path = unique_typ_path(input_path.with_file_name(format!("{stem}.typ")));
    let out_stem = out_path.file_stem().and_then(|s| s.to_str()).unwrap_or(&stem).to_string();
    let out_name = out_path.file_name().and_then(|s| s.to_str()).unwrap_or("output.typ").to_string();
    let media_name = format!("{out_stem}_media");
    let input_dir = input_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let input_name = input_path.file_name().and_then(|s| s.to_str()).unwrap_or("input").to_string();

    let mut cmd = build_pandoc_command(&input_dir, &input_name, fmt.pandoc_from, &out_name, &media_name);
    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => {
            let mut log = crate::import_log::ImportLog::load();
            log.record(input_path, fmt.label, None, false, "pandoc not found");
            failed.set(failed.get() + 1);
            active.set(active.get() - 1);
            show_alert(&window, "Import Failed", "pandoc was not found. Install it to use folder import.");
            run_next_batch_worker(window, editor, cfg, toast_overlay, work_dir, dest_this_project, queue, fmt, done, failed, active, total, progress, written);
            return;
        }
    };
    let child = Rc::new(RefCell::new(Some(child)));

    glib::timeout_add_local(Duration::from_millis(150), move || {
        let mut guard = child.borrow_mut();
        let Some(c) = guard.as_mut() else { return glib::ControlFlow::Break };
        match c.try_wait() {
            Ok(Some(status)) => {
                let stderr = c.stderr.take();
                drop(guard);
                use std::io::Read;
                let mut stderr_text = String::new();
                if let Some(mut s) = stderr { let _ = s.read_to_string(&mut stderr_text); }

                if status.success() {
                    if let Ok(raw) = std::fs::read_to_string(&out_path) {
                        let bib_path = cfg.borrow().bib_path.clone();
                        let processed = post_process_latex_import(&raw, bib_path.as_deref());
                        let final_dir = if dest_this_project { work_dir.clone() } else { input_dir.clone() };
                        let final_path = unique_typ_path(final_dir.join(&out_name));
                        let _ = std::fs::write(&final_path, &processed);
                        if final_dir != input_dir {
                            let src_media = input_dir.join(&media_name);
                            if src_media.is_dir() {
                                let dst_media = final_dir.join(&media_name);
                                let _ = copy_dir_recursive(&src_media, &dst_media);
                                let _ = std::fs::remove_dir_all(&src_media);
                            }
                            let _ = std::fs::remove_file(&out_path);
                        }
                        let mut log = crate::import_log::ImportLog::load();
                        log.record(input_path.clone(), fmt.label, Some(final_path.clone()), true, "Imported successfully (batch)");
                        written.borrow_mut().push(final_path);
                        done.set(done.get() + 1);
                    } else {
                        let mut log = crate::import_log::ImportLog::load();
                        log.record(input_path.clone(), fmt.label, None, false, "Output file unreadable");
                        failed.set(failed.get() + 1);
                    }
                } else {
                    let description = describe_pandoc_failure(&stderr_text);
                    let mut log = crate::import_log::ImportLog::load();
                    log.record(input_path.clone(), fmt.label, None, false, &description);
                    failed.set(failed.get() + 1);
                }

                progress.set_title(&format!("Importing… ({} of {} done)", done.get() + failed.get(), total));
                active.set(active.get() - 1);
                run_next_batch_worker(
                    window.clone(), editor.clone(), cfg.clone(), toast_overlay.clone(),
                    work_dir.clone(), dest_this_project, queue.clone(), fmt,
                    done.clone(), failed.clone(), active.clone(), total, progress.clone(), written.clone(),
                );
                glib::ControlFlow::Break
            }
            Ok(None) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        }
    });
}

/// Post-process a pandoc-converted Typst file:
///
/// 1. Insert `#pagebreak()` between the title block and the body
///    (just before the first top-level `= Heading`).
/// 2. Insert `#pagebreak()` before the `#bibliography(...)` call.
/// 3. Fix the bibliography path to the configured `.bib` file if supplied;
///    add a commented-out bibliography stub if none exists.
fn post_process_latex_import(content: &str, bib_path: Option<&std::path::Path>) -> String {
    // ── Phase 1: single-pass classifier ───────────────────────────────────────
    //
    // Every line in the pandoc-converted content falls into one of three buckets:
    //
    //  DISCARDED  — formatting rules that Zerkalo's template block controls:
    //               #set page(...)  #set text(...)  #set par(...)
    //               #show heading*  #set heading(...)
    //
    //  MACROS     — definitions the body may depend on; placed after the template:
    //               #import "..."   #let name = ...
    //
    //  BODY       — all actual document content (headings, paragraphs, citations,
    //               #page(...) content blocks, #figure, #footnote, etc.)
    //
    // This approach handles content scattered throughout the file, not just at the
    // top, which is what pandoc produces for complex LaTeX sources.

    enum Scan { Body, SkipSet(i32), SkipShow(i32), CollectLet(i32) }

    let lines: Vec<&str> = content.lines().collect();
    let mut macro_defs: Vec<String> = Vec::new();
    let mut body: Vec<String> = Vec::new();
    let mut scan = Scan::Body;
    let mut let_buf = String::new();

    // Combined depth counting for all delimiter types
    let paren_depth = |s: &str| -> i32 {
        s.chars().fold(0i32, |d, c| match c {
            '(' => d + 1,
            ')' => d - 1,
            _ => d,
        })
    };
    // For #show heading blocks, which use block(...)[\n...\n] syntax, we must
    // track ALL delimiters together: the `(` opens before the `[` does.
    let total_depth = |s: &str| -> i32 {
        s.chars().fold(0i32, |d, c| match c {
            '(' | '[' | '{' => d + 1,
            ')' | ']' | '}' => d - 1,
            _ => d,
        })
    };
    for &line in &lines {
        let t = line.trim();
        scan = match scan {
            // ── Continuation: discarding a multi-line #set block ────────────────
            Scan::SkipSet(d) => {
                let d = d + paren_depth(t);
                if d > 0 { Scan::SkipSet(d) } else { Scan::Body }
            }

            // ── Continuation: discarding a multi-line #show heading block ────────
            // Uses total_depth (all delimiters) because show rules use block(...)[\n...\n]
            // where the `(` opens before the `[` does.
            Scan::SkipShow(d) => {
                let d = d + total_depth(t);
                if d > 0 { Scan::SkipShow(d) } else { Scan::Body }
            }

            // ── Continuation: collecting a multi-line #let definition ────────────
            Scan::CollectLet(d) => {
                let_buf.push('\n');
                let_buf.push_str(line);
                let d = d + total_depth(t);
                if d <= 0 {
                    macro_defs.push(std::mem::take(&mut let_buf));
                    Scan::Body
                } else {
                    Scan::CollectLet(d)
                }
            }

            // ── Normal body scan ─────────────────────────────────────────────────
            Scan::Body => {
                if t.starts_with("#set ") {
                    // Strip all #set rules pandoc generates (page, text, par, heading,
                    // list, table, math.equation, etc.); track depth for multi-line blocks.
                    let d = paren_depth(t);
                    if d > 0 { Scan::SkipSet(d) } else { Scan::Body }
                } else if t.starts_with("#show") {
                    // Strip all #show rules (#show heading:, #show:, #show terms:, etc.).
                    // Uses total_depth because show rules mix (), [], {} delimiters.
                    let d = total_depth(t);
                    if d > 0 { Scan::SkipShow(d) } else { Scan::Body }
                } else if t.starts_with("#import ") {
                    macro_defs.push(line.to_string());
                    Scan::Body
                } else if t.starts_with("#let ") {
                    // Use total_depth: pandoc's #let conf(...) = {...} uses () for
                    // function params before {} for the body.
                    let d = total_depth(t);
                    let_buf = line.to_string();
                    if d > 0 {
                        Scan::CollectLet(d)
                    } else {
                        macro_defs.push(std::mem::take(&mut let_buf));
                        Scan::Body
                    }
                } else {
                    body.push(line.to_string());
                    Scan::Body
                }
            }
        };
    }

    // ── Phase 2: process body — insert pagebreaks, fix bibliography ───────────

    // Trim leading blank lines from the body
    let skip = body.iter().position(|l| !l.trim().is_empty()).unwrap_or(body.len());
    let body = body[skip..].to_vec();

    let first_heading = body.iter().position(|l| {
        let t = l.trim();
        t.starts_with("= ") && !t.starts_with("==")
    });

    let bib_idx = body.iter().position(|l| l.trim().starts_with("#bibliography"));

    let bib_style = bib_idx
        .and_then(|bi| {
            let s = body[bi].trim();
            let start = s.find("style:")? + 6;
            let after = s[start..].trim_start().trim_start_matches('"');
            let end = after.find('"')?;
            Some(after[..end].to_string())
        })
        .unwrap_or_else(|| "chicago-author-date".to_string());

    let bib_call = match bib_path {
        Some(bp) => format!("#bibliography(\"{}\", style: \"{}\")", bp.display(), bib_style),
        None if bib_idx.is_some() => body[bib_idx.unwrap()].trim().to_string(),
        None => format!("// #bibliography(\"refs.bib\", style: \"{}\")", bib_style),
    };

    let trim_trailing = |v: &mut Vec<String>| {
        while v.last().map(|l: &String| l.trim().is_empty()).unwrap_or(false) {
            v.pop();
        }
    };

    let mut processed: Vec<String> = Vec::with_capacity(body.len() + 8);
    let mut pb_done = false;

    for (i, line) in body.iter().enumerate() {
        // Pagebreak before first top-level heading (separates title block from body)
        if Some(i) == first_heading && !pb_done && i > 0 {
            trim_trailing(&mut processed);
            processed.push(String::new());
            processed.push("#pagebreak()".to_string());
            processed.push(String::new());
            pb_done = true;
        }

        // Replace bibliography line with a clean, properly-placed version
        if Some(i) == bib_idx {
            trim_trailing(&mut processed);
            processed.push(String::new());
            processed.push("#pagebreak()".to_string());
            processed.push(String::new());
            processed.push(bib_call.clone());
            continue;
        }

        processed.push(line.clone());
    }

    if bib_idx.is_none() {
        processed.push(String::new());
        processed.push(
            "// ── Bibliography ────────────────────────────────────────────────────"
                .to_string(),
        );
        processed.push(bib_call);
    }

    // ── Phase 3: assemble a well-formed Zerkalo document ─────────────────────

    let preamble = super::super::template_dialog::default_import_preamble();
    let mut out = preamble;
    out.push('\n');

    if !macro_defs.is_empty() {
        out.push_str(
            "// ── Imported macros ─────────────────────────────────────────────────────\n",
        );
        for def in &macro_defs {
            out.push_str(def);
            out.push('\n');
        }
        out.push('\n');
    }

    out.push_str(
        "// ── Document body ───────────────────────────────────────────────────────\n\n",
    );
    out.push_str(&processed.join("\n"));
    if !out.ends_with('\n') {
        out.push('\n');
    }

    out
}

/// Wrap plain text extracted from a PDF into a Typst document managed by Zerkalo's template system.
/// A line extracted from a PDF is treated as a probable section heading (and
/// promoted to `== Heading`) when it's short, isn't sentence-ending
/// punctuation, and sits alone between blank lines — the closest signal
/// `pdftotext`'s plain-text output gives us to the source PDF's actual
/// heading styling, which is lost entirely once text is extracted.
fn is_probable_pdf_heading(line: &str, prev_blank: bool, next_blank: bool) -> bool {
    let t = line.trim();
    if t.is_empty() || !prev_blank || !next_blank {
        return false;
    }
    if t.chars().count() > 60 {
        return false;
    }
    !matches!(t.chars().last(), Some('.' | ',' | ';' | ':'))
}

/// Reflow pdftotext output, promoting probable headings (see
/// `is_probable_pdf_heading`) to `== Heading` lines.
fn format_pdf_body(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = String::new();
    let mut prev_blank = true;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            out.push('\n');
            prev_blank = true;
            continue;
        }
        let next_blank = lines.get(i + 1).map(|l| l.trim().is_empty()).unwrap_or(true);
        if is_probable_pdf_heading(trimmed, prev_blank, next_blank) {
            out.push_str("== ");
        }
        out.push_str(trimmed);
        out.push('\n');
        prev_blank = false;
    }
    out
}

/// Runs the pdftotext-based PDF import pipeline for `input_path`, shared by
/// the ☰ → Import → PDF file picker and drag-and-drop.
pub(super) fn run_pdf_import(window: &adw::ApplicationWindow, editor: &EditorPane, input_path: std::path::PathBuf) {
    let stem = input_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output").to_string();
    let out_path = unique_typ_path(input_path.with_file_name(format!("{stem}.typ")));
    let output = crate::git_sync::host_command("pdftotext")
        .arg("-layout")
        .arg(&input_path)
        .arg("-")
        .output();
    let mut log = crate::import_log::ImportLog::load();
    match output {
        Ok(o) if o.status.success() => {
            let extracted = String::from_utf8_lossy(&o.stdout).to_string();
            let typst_doc = post_process_pdf_import(&extracted, stem.as_str());
            let _ = std::fs::write(&out_path, &typst_doc);
            editor.open_file(out_path.clone(), &typst_doc);
            log.record(input_path, "PDF (.pdf)", Some(out_path), true, "Imported successfully");
        }
        Ok(_) => {
            show_alert(window, "Import Failed", "pdftotext could not extract text from this PDF.");
            log.record(input_path, "PDF (.pdf)", None, false, "pdftotext could not extract text");
        }
        Err(_) => {
            show_alert(window, "Import Failed",
                "pdftotext was not found. Install poppler-utils to use PDF import:\n\
                 \n  zypper install poppler-tools\
                 \n  apt   install poppler-utils\
                 \n  brew  install poppler\
                 \n  dnf   install poppler-utils");
            log.record(input_path, "PDF (.pdf)", None, false, "pdftotext not found");
        }
    }
}

fn post_process_pdf_import(text: &str, title: &str) -> String {
    let escaped_title = title.replace('"', "\\\"");
    let preamble = super::super::template_dialog::default_import_preamble();
    let mut out = format!(
        "{preamble}\n\
         // ── Document body ───────────────────────────────────────────────────────\n\
         // Imported from PDF — plain text only. Section headings are guessed from\n\
         // short, isolated lines; review them, and other formatting (tables, math,\n\
         // images) is not preserved at all.\n\
         \n\
         = {escaped_title}\n\
         \n"
    );

    out.push_str(&format_pdf_body(text));

    // Bibliography stub so Zerkalo can locate it
    out.push_str(
        "\n// ── Bibliography ────────────────────────────────────────────────────\n\
         // #bibliography(\"refs.bib\", style: \"chicago-author-date\")\n",
    );

    out
}


#[cfg(test)]
mod tests {
    use super::{
        describe_pandoc_failure, format_pdf_body, post_process_latex_import,
        scan_files_recursive, strip_pandoc_preamble, summarize_import_content, unique_typ_path,
    };

    // ── document import helpers ───────────────────────────────────────────────

    #[test]
    fn unique_typ_path_passes_through_when_free() {
        let dir = std::env::temp_dir().join(format!("zerkalo-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("nonexistent.typ");
        assert_eq!(unique_typ_path(path.clone()), path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unique_typ_path_suffixes_on_collision() {
        let dir = std::env::temp_dir().join(format!("zerkalo-test-collide-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let taken = dir.join("essay.typ");
        std::fs::write(&taken, "").unwrap();
        let result = unique_typ_path(taken.clone());
        assert_eq!(result, dir.join("essay (1).typ"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn describe_pandoc_failure_recognizes_unknown_writer() {
        let msg = describe_pandoc_failure("Error: Unknown writer: typst");
        assert!(msg.contains("pandoc 3.1 or later"), "got: {msg}");
    }

    #[test]
    fn describe_pandoc_failure_falls_back_to_raw_stderr() {
        let msg = describe_pandoc_failure("some other pandoc error\nline two");
        assert!(msg.starts_with("pandoc error:\n"), "got: {msg}");
        assert!(msg.contains("some other pandoc error"));
    }

    // ── format_pdf_body ────────────────────────────────────────────────────────

    #[test]
    fn format_pdf_body_promotes_isolated_short_line_to_heading() {
        let input = "\nIntroduction\n\nSome body text here that goes on.\n";
        let result = format_pdf_body(input);
        assert!(result.contains("== Introduction"), "got: {result}");
    }

    #[test]
    fn format_pdf_body_does_not_promote_long_lines() {
        let long = "This is a much longer line of text that runs well past sixty characters total.";
        let input = format!("\n{long}\n\nMore text.\n");
        let result = format_pdf_body(&input);
        assert!(!result.contains(&format!("== {long}")), "got: {result}");
        assert!(result.contains(long));
    }

    #[test]
    fn format_pdf_body_does_not_promote_sentence_ending_lines() {
        let input = "\nThis looks short.\n\nMore text.\n";
        let result = format_pdf_body(input);
        assert!(!result.contains("== This looks short."), "got: {result}");
    }

    #[test]
    fn format_pdf_body_does_not_promote_lines_without_blank_neighbors() {
        let input = "Some heading\nfollowed immediately by body text.\n";
        let result = format_pdf_body(input);
        assert!(!result.contains("== Some heading"), "got: {result}");
    }

    // ── scan_files_recursive ──────────────────────────────────────────────────

    #[test]
    fn scan_files_recursive_finds_nested_matches_and_skips_media_dirs() {
        let dir = std::env::temp_dir().join(format!("zerkalo-scan-test-{}", std::process::id()));
        let sub = dir.join("chapter1");
        let media = dir.join("essay_media");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::create_dir_all(&media).unwrap();
        std::fs::write(dir.join("essay.tex"), "").unwrap();
        std::fs::write(sub.join("notes.tex"), "").unwrap();
        std::fs::write(sub.join("readme.txt"), "").unwrap();
        std::fs::write(media.join("stray.tex"), "").unwrap();

        let mut found = Vec::new();
        scan_files_recursive(&dir, &["tex"], &mut found);
        found.sort();

        assert_eq!(found.len(), 2, "got: {found:?}");
        assert!(found.iter().any(|p| p.ends_with("essay.tex")));
        assert!(found.iter().any(|p| p.ends_with("chapter1/notes.tex")));
        assert!(!found.iter().any(|p| p.to_string_lossy().contains("essay_media")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── summarize_import_content ──────────────────────────────────────────────

    #[test]
    fn summarize_counts_words_headings_images_citations() {
        let text = "= Title\n\nSome words here today @smith2020 and more.\n\n#figure(image(\"a.png\"))\n";
        let summary = summarize_import_content(text);
        assert!(summary.contains("heading"), "got: {summary}");
        assert!(summary.contains("1 image"), "got: {summary}");
        assert!(summary.contains("1 citation"), "got: {summary}");
    }

    #[test]
    fn summarize_ignores_email_like_at_signs() {
        let text = "Contact me at name@example.com for details.";
        let summary = summarize_import_content(text);
        assert!(!summary.contains("citation"), "got: {summary}");
    }

    #[test]
    fn summarize_omits_zero_counts() {
        let text = "Just plain prose with nothing special in it at all.";
        let summary = summarize_import_content(text);
        assert!(!summary.contains("heading"));
        assert!(!summary.contains("image"));
        assert!(!summary.contains("citation"));
        assert!(!summary.contains("equation"));
        assert!(summary.contains("word"));
    }

    // ── post_process_latex_import ─────────────────────────────────────────────

    #[test]
    fn import_discards_formatting_rules() {
        // Simulates a complex pandoc output with set/show rules throughout the file
        let input = "\
#set page(paper: \"a4\", margin: 1in)\n\
#set text(font: \"Arial\", size: 12pt)\n\
#set par(leading: 1em)\n\
#set heading(numbering: \"1.1.\")\n\
#show heading: it => block[#it.body]\n\
\n\
= Introduction\n\
\n\
Some text.\n\
\n\
#bibliography(\"refs.bib\", style: \"apa\")\n";

        let result = post_process_latex_import(input, None);

        // Template block is present
        assert!(result.contains("// ZERKALO-TEMPLATE-BEGIN"), "template block present");
        assert!(result.contains("// ZERKALO-TEMPLATE-END"), "template block closed");

        // Check only the section AFTER the template markers — that's where the
        // user's formatting rules would appear if they weren't discarded.
        // (The template block itself legitimately contains these directives.)
        let after_template = result
            .split("// ZERKALO-TEMPLATE-END")
            .nth(1)
            .unwrap_or("");
        assert!(!after_template.contains("#set page("), "set page not in body");
        assert!(!after_template.contains("#set text("), "set text not in body");
        assert!(!after_template.contains("#set par("), "set par not in body");
        assert!(!after_template.contains("#set heading("), "set heading not in body");
        assert!(!after_template.contains("#show heading"), "show heading not in body");

        // Body content is preserved
        assert!(result.contains("= Introduction"), "heading preserved");
        assert!(result.contains("Some text."), "body text preserved");

        // Bibliography is present
        assert!(after_template.contains("#bibliography("), "bibliography present");
    }

    #[test]
    fn import_moves_macros_to_section() {
        let input = "\
#set text(font: \"Arial\")\n\
#import \"@preview/droplet:0.3.1\": dropcap\n\
#let essay-par(body) = block(width: 100%, body)\n\
\n\
= Heading\n\
\n\
#essay-par[Some text.]\n";

        let result = post_process_latex_import(input, None);

        // Macros are placed after the template block, not discarded
        assert!(result.contains("#import \"@preview/droplet:0.3.1\""), "import preserved");
        assert!(result.contains("#let essay-par"), "let definition preserved");

        // Macros come AFTER the template block
        let template_end = result.find("// ZERKALO-TEMPLATE-END").unwrap();
        let import_pos = result.find("#import").unwrap();
        assert!(import_pos > template_end, "import is after template block");

        // Body content is preserved
        assert!(result.contains("= Heading"), "heading preserved");
        assert!(result.contains("#essay-par[Some text.]"), "macro usage preserved");
    }

    #[test]
    fn import_multiline_show_heading_discarded() {
        let input = "\
#show heading.where(level: 1): it => block(\n\
  width: 100%,\n\
  above: 1em,\n\
)[\n\
  #align(center)[#it.body]\n\
]\n\
\n\
= Body\n";

        let result = post_process_latex_import(input, None);
        let after_template = result
            .split("// ZERKALO-TEMPLATE-END")
            .nth(1)
            .unwrap_or("");
        // The user's custom show rule should not appear in the body
        assert!(!after_template.contains("#show heading"), "multi-line show heading discarded from body");
        // The body inside the show rule should also be gone
        assert!(!after_template.contains("#align(center)[#it.body]"), "show heading body discarded");
        // Actual document content is kept
        assert!(result.contains("= Body"), "actual content kept");
    }

    #[test]
    fn import_inserts_pagebreak_before_first_heading() {
        // When there is content before the first heading (a title block), a
        // pagebreak must be inserted between them.
        let input = "\
#set text(font: \"Arial\")\n\
\n\
Title material here\n\
\n\
= Introduction\n\
\n\
Body.\n";

        let result = post_process_latex_import(input, None);
        let pb = result.find("#pagebreak()").unwrap();
        let h1 = result.find("= Introduction").unwrap();
        assert!(pb < h1, "pagebreak before first heading");
    }

    #[test]
    fn import_body_marker_present() {
        let input = "= Heading\n\nText.\n";
        let result = post_process_latex_import(input, None);
        assert!(result.contains("// ── Document body"), "body marker present");
    }

    #[test]
    fn strip_pandoc_empty_input() {
        assert_eq!(strip_pandoc_preamble(""), "");
    }

    #[test]
    fn strip_pandoc_only_set_rules() {
        let input = "#set text(font: \"Arial\")\n#set page(paper: \"a4\")\n";
        assert_eq!(strip_pandoc_preamble(input), "");
    }

    #[test]
    fn strip_pandoc_preserves_body() {
        let input = "#set text(font: \"Arial\")\n\n= Introduction\n\nBody text.\n";
        let result = strip_pandoc_preamble(input);
        assert_eq!(result, "= Introduction\n\nBody text.\n");
    }

    #[test]
    fn strip_pandoc_multiline_set_rule() {
        let input = "#set text(\n  font: \"Arial\",\n  size: 12pt,\n)\n\n= Heading\n";
        let result = strip_pandoc_preamble(input);
        assert_eq!(result, "= Heading\n");
    }

    #[test]
    fn strip_pandoc_skips_leading_comments() {
        let input = "// Generated by pandoc\n#set text(font: \"Arial\")\n\n= Body\n";
        let result = strip_pandoc_preamble(input);
        assert_eq!(result, "= Body\n");
    }

    #[test]
    fn strip_pandoc_no_preamble() {
        let input = "= Just a heading\n\nSome text.\n";
        let result = strip_pandoc_preamble(input);
        assert_eq!(result, "= Just a heading\n\nSome text.\n");
    }
}
