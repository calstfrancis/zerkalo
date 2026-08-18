//! Editor-adjacent wiring: the file tree's unsaved marker, tab-context delete,
//! image and document drag-and-drop, the GOST font toggle, and the sidebar's
//! Update Template button. Split out of `AppWindow::new`.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Orientation};
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

    // Outline / Citations / Packages / Comments used to be a plain stacked
    // Box — fixed proportions, no way to give one section more room. Three
    // nested vertical Paned dividers instead, each position persisted the
    // same debounced (400ms after drag stop) way the primary sidebar/preview
    // splits already are, per the suite's own "pane positions persist"
    // convention.
    let packages_comments_pane = gtk4::Paned::new(Orientation::Vertical);
    packages_comments_pane.set_start_child(Some(ctx.package_browser.widget()));
    packages_comments_pane.set_end_child(Some(ctx.comments_panel.widget()));
    packages_comments_pane.set_position(ctx.current_config.borrow().sidebar_packages_split);
    packages_comments_pane.set_resize_start_child(true);
    packages_comments_pane.set_resize_end_child(true);
    packages_comments_pane.set_shrink_start_child(false);
    packages_comments_pane.set_shrink_end_child(false);
    packages_comments_pane.set_vexpand(true);
    let packages_comments_suppress = persist_vertical_split(&packages_comments_pane, ctx.current_config.clone(), |c, pos| {
        c.sidebar_packages_split = pos;
    });

    let citations_packages_pane = gtk4::Paned::new(Orientation::Vertical);
    citations_packages_pane.set_start_child(Some(ctx.citation_panel.widget()));
    citations_packages_pane.set_end_child(Some(&packages_comments_pane));
    citations_packages_pane.set_position(ctx.current_config.borrow().sidebar_citations_split);
    citations_packages_pane.set_resize_start_child(true);
    citations_packages_pane.set_resize_end_child(true);
    citations_packages_pane.set_shrink_start_child(false);
    citations_packages_pane.set_shrink_end_child(false);
    citations_packages_pane.set_vexpand(true);
    persist_vertical_split(&citations_packages_pane, ctx.current_config.clone(), |c, pos| {
        c.sidebar_citations_split = pos;
    });

    let outline_rest_pane = gtk4::Paned::new(Orientation::Vertical);
    outline_rest_pane.set_start_child(Some(ctx.outline_panel.widget()));
    outline_rest_pane.set_end_child(Some(&citations_packages_pane));
    outline_rest_pane.set_position(ctx.current_config.borrow().sidebar_outline_split);
    outline_rest_pane.set_resize_start_child(true);
    outline_rest_pane.set_resize_end_child(true);
    outline_rest_pane.set_shrink_start_child(false);
    outline_rest_pane.set_shrink_end_child(false);
    outline_rest_pane.set_vexpand(true);
    persist_vertical_split(&outline_rest_pane, ctx.current_config.clone(), |c, pos| {
        c.sidebar_outline_split = pos;
    });

    // Packages and Comments can be collapsed to just their header row —
    // useful once a project's manuscript is stable and neither is needed
    // for a while. Collapsing actually reclaims the freed space for its
    // sibling in the shared Paned (see wire_collapse_reclaims_space) rather
    // than hiding the content and leaving a blank gap where it was.
    let packages_reclaim = wire_collapse_reclaims_space(
        &packages_comments_pane,
        true,
        packages_comments_suppress.clone(),
        ctx.current_config.clone(),
        |c| c.sidebar_packages_split,
    );
    let comments_reclaim = wire_collapse_reclaims_space(
        &packages_comments_pane,
        false,
        packages_comments_suppress,
        ctx.current_config.clone(),
        |c| c.sidebar_packages_split,
    );

    let initial_packages_collapsed = ctx.current_config.borrow().sidebar_packages_collapsed;
    let initial_comments_collapsed = ctx.current_config.borrow().sidebar_comments_collapsed;
    ctx.package_browser.set_collapsed(initial_packages_collapsed);
    ctx.comments_panel.set_collapsed(initial_comments_collapsed);
    if initial_packages_collapsed {
        packages_reclaim(true);
    } else if initial_comments_collapsed {
        comments_reclaim(true);
    }
    {
        let cfg = ctx.current_config.clone();
        ctx.package_browser.set_on_collapse_toggle(move |collapsed| {
            let mut c = cfg.borrow_mut();
            c.sidebar_packages_collapsed = collapsed;
            let _ = c.save();
            drop(c);
            packages_reclaim(collapsed);
        });
    }
    {
        let cfg = ctx.current_config.clone();
        ctx.comments_panel.set_on_collapse_toggle(move |collapsed| {
            let mut c = cfg.borrow_mut();
            c.sidebar_comments_collapsed = collapsed;
            let _ = c.save();
            drop(c);
            comments_reclaim(collapsed);
        });
    }

    let left_box = GtkBox::new(Orientation::Vertical, 0);
    left_box.set_hexpand(false);
    left_box.set_vexpand(true);
    left_box.set_overflow(gtk4::Overflow::Hidden);
    left_box.add_css_class("zerkalo-sidebar");
    left_box.append(&outline_rest_pane);
    *ctx.left_paned_holder.borrow_mut() = Some(left_box.clone());


    (left_box, update_template_btn)
}

/// Debounced (400ms after last drag), skipping the initial realize-triggered
/// notify — same shape as `startup.rs`'s `wire_pane_persistence` for the
/// primary sidebar/preview splits; a local copy here since these three
/// sidebar-section dividers are constructed and owned by this function, not
/// threaded back up to `AppWindow::new`.
/// Returns a `suppress` flag: set it `true` around a programmatic
/// `paned.set_position(...)` (e.g. driving a collapse/expand) so that move
/// isn't mistaken for a user drag and persisted over their real preferred
/// split — see `wire_collapse_reclaims_space` below.
fn persist_vertical_split(paned: &gtk4::Paned, cfg: Rc<RefCell<Config>>, setter: impl Fn(&mut Config, i32) + 'static) -> Rc<std::cell::Cell<bool>> {
    let setter = Rc::new(setter);
    let ready = Rc::new(std::cell::Cell::new(false));
    let ready2 = ready.clone();
    paned.connect_realize(move |_| {
        let r = ready2.clone();
        glib::idle_add_local_once(move || { r.set(true); });
    });
    let suppress = Rc::new(std::cell::Cell::new(false));
    let suppress2 = suppress.clone();
    let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    paned.connect_position_notify(move |p| {
        if !ready.get() || suppress2.get() { return; }
        let pos = p.position();
        let cfg2 = cfg.clone();
        let setter2 = setter.clone();
        let pending_for_cb = pending.clone();
        let mut slot = pending.borrow_mut();
        if let Some(id) = slot.take() { id.remove(); }
        *slot = Some(glib::timeout_add_local_once(
            std::time::Duration::from_millis(400),
            move || {
                *pending_for_cb.borrow_mut() = None;
                let mut c = cfg2.borrow_mut();
                setter2(&mut c, pos);
                let _ = c.save();
            },
        ));
    });
    suppress
}

/// Wires a Paned's start/end collapse toggles (from `PackageBrowser`/
/// `CommentsPanel`'s own collapse buttons) to actually reclaim the freed
/// space, not just hide content while the divider — and the blank gap
/// beneath the header it leaves — stays put. `shrink_start_child(false)`/
/// `shrink_end_child(false)` are already set on `paned`, so driving the
/// position to an extreme is enough: GTK clamps it to the collapsed side's
/// new minimum (now just its header, once the Revealer inside hides its
/// body) instead of actually shrinking past that — no manual height
/// measurement needed. `suppress` (from `persist_vertical_split`) stops
/// these programmatic moves from being saved as the user's real split
/// preference.
fn wire_collapse_reclaims_space(
    paned: &gtk4::Paned,
    is_start_child: bool,
    suppress: Rc<std::cell::Cell<bool>>,
    cfg: Rc<RefCell<Config>>,
    getter: impl Fn(&Config) -> i32 + 'static,
) -> impl Fn(bool) + 'static {
    let paned = paned.clone();
    move |collapsed| {
        suppress.set(true);
        if collapsed {
            paned.set_position(if is_start_child { 0 } else { i32::MAX });
        } else {
            paned.set_position(getter(&cfg.borrow()));
        }
        suppress.set(false);
    }
}
