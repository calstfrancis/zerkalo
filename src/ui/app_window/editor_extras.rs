//! Editor-adjacent wiring: the file tree's unsaved marker, tab-context delete,
//! image and document drag-and-drop, the GOST font toggle, and the sidebar's
//! Update Template button. Split out of `AppWindow::new`.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Orientation, Separator};
use libadwaita as adw;

use crate::config::Config;
use super::super::citation_panel::CitationPanel;
use super::super::editor_pane::EditorPane;
use super::super::file_tree::FileTree;
use super::super::outline_panel::OutlinePanel;
use super::super::preview_pane::PreviewPane;
use super::super::template_dialog::TemplateDialog;
use super::import::{IMPORT_FORMATS, run_pandoc_import, run_pdf_import};
use super::apply_template_result;

pub(super) struct EditorExtrasCtx {
    pub(super) window: adw::ApplicationWindow,
    pub(super) editor_pane: EditorPane,
    pub(super) file_tree: FileTree,
    pub(super) toast_overlay: adw::ToastOverlay,
    pub(super) current_config: Rc<RefCell<Config>>,
    pub(super) project_root: PathBuf,
}

pub(super) fn wire_editor_extras(ctx: &EditorExtrasCtx) {
    // ── Unsaved-file indicator in file tree ─────────────────────────────
    {
        let ft = ctx.file_tree.clone();
        ctx.editor_pane.set_on_file_dirty(move |path, dirty| {
            ft.set_file_modified(&path, dirty);
        });
    }

    // ── Delete file from tab context menu ───────────────────────────────────
    {
        let ft = ctx.file_tree.clone();
        ctx.editor_pane.set_on_delete_file(move |_path| {
            ft.refresh();
        });
    }

    // ── Image drag-and-drop handler ──────────────────────────────────────────
    {
        let root = ctx.project_root.clone();
        let ep = ctx.editor_pane.clone();
        let ft = ctx.file_tree.clone();
        ctx.editor_pane.set_on_image_drop(move |src_path| {
            let fname = src_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("image.png")
                .to_string();
            let dest = root.join(&fname);
            if dest != src_path {
                if let Err(e) = std::fs::copy(&src_path, &dest) {
                    tracing::warn!("Failed to copy image: {e}");
                    return;
                }
            }
            ft.refresh();
            ep.insert_at_cursor(&format!(
                "\n#figure(\n  image(\"{fname}\"),\n  caption: [],\n)\n"
            ));
        });
    }

    // ── Document drag-and-drop handler ────────────────────────────────────────
    {
        let win = ctx.window.clone();
        let ep = ctx.editor_pane.clone();
        let cfg = ctx.current_config.clone();
        let toast = ctx.toast_overlay.clone();
        ctx.editor_pane.set_on_document_drop(move |src_path| {
            let ext = src_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if ext == "pdf" {
                run_pdf_import(&win, &ep, src_path);
                return;
            }
            if let Some(fmt) = IMPORT_FORMATS.iter().find(|f| f.extensions.contains(&ext.as_str())) {
                let work_dir = ep.get_active_path()
                    .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                    .unwrap_or_else(|| src_path.parent().map(|d| d.to_path_buf()).unwrap_or_default());
                run_pandoc_import(&win, &ep, &cfg, &toast, &work_dir, src_path, fmt);
            }
        });
    }

    // (Refs and Files panels removed — refs/file-tree callbacks kept for
    //  compile-error marking, dirty indicators, and image-drop insertion)

}

pub(super) struct SidebarToolbarCtx {
    pub(super) window: adw::ApplicationWindow,
    pub(super) editor_pane: EditorPane,
    pub(super) preview_pane: PreviewPane,
    pub(super) outline_panel: OutlinePanel,
    pub(super) citation_panel: CitationPanel,
    pub(super) current_config: Rc<RefCell<Config>>,
    pub(super) project_root: PathBuf,
    pub(super) left_paned_holder: Rc<RefCell<Option<GtkBox>>>,
}

/// Returns the sidebar's left column, which the layout assembly then packs.
/// Returns the sidebar column and the Template button, which the caller packs
/// into the header bar. The button used to be a full-width row above the
/// panels: it is the only thing in that column that is not a panel, and it
/// belongs with the other document-level actions.
pub(super) fn wire_sidebar_toolbar(ctx: &SidebarToolbarCtx) -> (GtkBox, Button) {
    // ── GOST font toggle (status bar button wired here) ───────────────────
    let current_config_for_gost = ctx.current_config.clone();
    let ui_font_provider = gtk4::CssProvider::new();
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &ui_font_provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
    }
    {
        let ui_prov = ui_font_provider.clone();
        ctx.editor_pane.set_on_gost_toggle(move |enabled| {
            if enabled {
                let cfg = current_config_for_gost.borrow();
                let editor_font = cfg.editor_font_family.clone();
                let size_clause = if cfg.editor_font_size > 0 {
                    format!("font-size: {}pt; ", cfg.editor_font_size)
                } else {
                    String::new()
                };
                ui_prov.load_from_data(&format!(
                    "* {{ font-family: 'GOST type B'; }} \
                     textview {{ font-family: '{editor_font}'; {size_clause}}}",
                ));
            } else {
                ui_prov.load_from_data("* {}");
            }
        });
    }

    // ── Sidebar toolbar: Update Template button ───────────────────────────
    let update_template_btn = Button::new();
    update_template_btn.set_label("Template");
    update_template_btn.add_css_class("flat");
    update_template_btn.set_tooltip_text(Some(
        "Change formatting style, margins, fonts for this document",
    ));

    {
        let win_ut = ctx.window.clone();
        let ep_ut = ctx.editor_pane.clone();
        let root_ut = ctx.project_root.clone();
        let current_config_for_ut = ctx.current_config.clone();
        let preview_ut = ctx.preview_pane.clone();
        update_template_btn.connect_clicked(move |_| {
            let Some(current_path) = ep_ut.get_active_path() else { return };
            let current_content = ep_ut.get_active_content().unwrap_or_default();
            let dlg = TemplateDialog::new(&win_ut, &root_ut, false);

            dlg.set_cv_elements_path(current_config_for_ut.borrow().cv_elements_path.clone());
            {
                let cfg = current_config_for_ut.clone();
                dlg.set_on_cv_elements_change(move |path| {
                    let mut c = cfg.borrow_mut();
                    c.cv_elements_path = Some(path);
                    let _ = c.save();
                });
            }

            if let Some(sidecar) = super::super::template_dialog::load_sidecar(&current_path) {
                dlg.preselect_from_sidecar(&sidecar);
            } else {
                let doc_kind = super::super::template_dialog::parse_doc_kind(&current_content);
                dlg.preselect_cv_mode(doc_kind.as_deref() == Some("cv"));
                dlg.preselect_body_kind(super::super::template_dialog::body_kind_from_key(
                    doc_kind.as_deref().unwrap_or(""),
                ));
                dlg.preselect_style(
                    &super::super::template_dialog::parse_style_key(&current_content)
                        .unwrap_or_default(),
                );
                // A CV document's @zerkalo-style marker is just the literal "cv"
                // (see generate_cv_template), so preselect_style above can't
                // recover the actual CV style (Modern/Academic/Classic/
                // Two-Column) from it — that's tracked separately via
                // @zerkalo-cv-style.
                if let Some(cv_style) = super::super::template_dialog::parse_cv_style(&current_content) {
                    if let Some(idx) = super::super::template_dialog::cv_style_index(&cv_style) {
                        dlg.preselect_cv_style_index(idx);
                    }
                }
                if let Some(f) = super::super::template_dialog::parse_font(&current_content) {
                    dlg.preselect_font(&f);
                }
                if let Some(p) = super::super::template_dialog::parse_paper(&current_content) {
                    dlg.preselect_paper(&p, "", "");
                }
                if let Some(s) = super::super::template_dialog::parse_spacing(&current_content) {
                    dlg.preselect_spacing(&s);
                }
                dlg.preselect_margin(super::super::template_dialog::parse_margin(&current_content), "");
                dlg.preselect_toc(
                    super::super::template_dialog::parse_has_toc(&current_content),
                    super::super::template_dialog::parse_toc_depth(&current_content),
                );
                dlg.preselect_abstract(
                    super::super::template_dialog::parse_has_abstract(&current_content),
                    &super::super::template_dialog::parse_abstract_text(&current_content),
                );
                dlg.preselect_keywords(
                    super::super::template_dialog::parse_has_keywords(&current_content),
                    &super::super::template_dialog::parse_keywords_text(&current_content),
                );
                if let Some(f) = super::super::template_dialog::parse_dropcap_font(&current_content) {
                    dlg.preselect_dropcap_font(&f);
                }
                if let Some(c) = super::super::template_dialog::parse_dropcap_color(&current_content) {
                    dlg.preselect_dropcap_color(&c);
                }
            }
            // The body is ground truth for CV-ness: if the sidecar/marker path above
            // disagrees with what the document's body actually calls (#cv-section, an
            // import of cv-helpers.typ), trust the body — see body_looks_like_cv's doc
            // comment. Without this, a document whose sidecar drifted to a non-CV kind
            // would keep regenerating a non-CV preamble onto its still-CV body forever,
            // producing a document that fails to compile ("unknown function: section").
            if super::super::template_dialog::body_looks_like_cv(&current_content) {
                dlg.preselect_cv_mode(true);
                dlg.preselect_body_kind(super::super::template_dialog::body_kind_from_key("cv"));
                // See the identical fallback earlier in this file (the
                // read-only "current document" path) for why this is needed:
                // the sidecar/marker path above may have left Style on a
                // stale or non-CV-meaningful selection.
                if let Some(cv_style) = super::super::template_dialog::parse_cv_style(&current_content) {
                    if let Some(idx) = super::super::template_dialog::cv_style_index(&cv_style) {
                        dlg.preselect_cv_style_index(idx);
                    }
                }
            }
            // If the user edited the abstract directly in the .typ file, that wins
            // over what the sidecar recorded last time. Override with doc's text.
            if let Some(doc_abstract) = super::super::template_dialog::parse_abstract_from_doc(&current_content) {
                dlg.override_abstract_text(&doc_abstract);
            }
            // Always read metadata from the document — the user may have edited the
            // #let doc-* variables directly, and the sidecar won't reflect those changes.
            dlg.preselect_metadata(
                &super::super::template_dialog::parse_meta(&current_content, "title"),
                &super::super::template_dialog::parse_meta(&current_content, "subtitle"),
                &super::super::template_dialog::parse_meta(&current_content, "author"),
                &super::super::template_dialog::parse_meta(&current_content, "affiliation"),
                &super::super::template_dialog::parse_meta(&current_content, "course"),
                &super::super::template_dialog::parse_meta(&current_content, "professor"),
                &super::super::template_dialog::parse_meta(&current_content, "date"),
            );

            let ep2 = ep_ut.clone();
            let win_ut2 = win_ut.clone();
            let preview_ut2 = preview_ut.clone();
            let current_content_for_apply = current_content.clone();
            let current_path_for_apply = current_path.clone();
            dlg.set_on_apply(move |new_content, sidecar| {
                apply_template_result(
                    &win_ut2,
                    &ep2,
                    &preview_ut2,
                    current_path_for_apply.clone(),
                    current_content_for_apply.clone(),
                    new_content,
                    sidecar,
                );
            });
            dlg.present();
        });
    }

    let left_box = GtkBox::new(Orientation::Vertical, 0);
    left_box.set_hexpand(false);
    left_box.set_vexpand(true);
    left_box.set_overflow(gtk4::Overflow::Hidden);
    left_box.add_css_class("zerkalo-sidebar");
    left_box.append(ctx.outline_panel.widget());
    left_box.append(&Separator::new(Orientation::Horizontal));
    left_box.append(ctx.citation_panel.widget());
    *ctx.left_paned_holder.borrow_mut() = Some(left_box.clone());


    (left_box, update_template_btn)
}
