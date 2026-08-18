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
use super::super::comments_panel::CommentsPanel;
use super::super::editor_pane::EditorPane;
use super::super::file_tree::FileTree;
use super::super::outline_panel::OutlinePanel;
use super::super::package_browser::PackageBrowser;
use super::super::preview_pane::PreviewPane;
use super::import::{IMPORT_FORMATS, run_pandoc_import, run_pdf_import};

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
    pub(super) package_browser: PackageBrowser,
    pub(super) comments_panel: CommentsPanel,
    pub(super) current_config: Rc<RefCell<Config>>,
    pub(super) project_root: PathBuf,
    pub(super) left_paned_holder: Rc<RefCell<Option<GtkBox>>>,
    pub(super) toast_overlay: adw::ToastOverlay,
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
        let toast_for_gost = ctx.toast_overlay.clone();
        let ep_for_gost = ctx.editor_pane.clone();
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
                // Without the font installed the CSS above is a silent no-op,
                // which reads as a broken toggle. Say so instead.
                if !ep_for_gost.is_gost_restoring() && !ep_for_gost.gost_font_available() {
                    let t = adw::Toast::new("GOST type B isn't installed — the UI font is unchanged");
                    t.set_timeout(5);
                    toast_for_gost.add_toast(t);
                }
            } else {
                ui_prov.load_from_data("* {}");
            }
            let mut cfg = current_config_for_gost.borrow_mut();
            if cfg.gost_font != enabled {
                cfg.gost_font = enabled;
                let _ = cfg.save();
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
        let cfg_ut = ctx.current_config.clone();
        let preview_ut = ctx.preview_pane.clone();
        let toast_ut = ctx.toast_overlay.clone();
        update_template_btn.connect_clicked(move |_| {
            super::open_template_for_active_document(
                &win_ut, &ep_ut, &preview_ut, &toast_ut, &root_ut, &cfg_ut,
            );
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
    left_box.append(&Separator::new(Orientation::Horizontal));
    left_box.append(ctx.package_browser.widget());
    left_box.append(&Separator::new(Orientation::Horizontal));
    left_box.append(ctx.comments_panel.widget());
    *ctx.left_paned_holder.borrow_mut() = Some(left_box.clone());


    (left_box, update_template_btn)
}
