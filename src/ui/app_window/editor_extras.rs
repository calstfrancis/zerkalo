//! Editor-adjacent wiring: the file tree's unsaved marker, tab-context delete,
//! image and document drag-and-drop, the GOST font toggle, and the sidebar's
//! Update Template button. Split out of `AppWindow::new`.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Orientation};
use libadwaita as adw;

use super::super::citation_panel::CitationPanel;
use super::super::comments_panel::CommentsPanel;
use super::super::editor_pane::EditorPane;
use super::super::file_tree::FileTree;
use super::super::outline_panel::OutlinePanel;
use super::super::package_browser::PackageBrowser;
use super::super::preview_pane::PreviewPane;
use super::import::{run_pandoc_import, run_pdf_import, IMPORT_FORMATS};
use crate::config::Config;

pub(super) struct EditorExtrasCtx {
    pub(super) window: adw::ApplicationWindow,
    pub(super) editor_pane: EditorPane,
    pub(super) file_tree: FileTree,
    pub(super) toast_overlay: adw::ToastOverlay,
    pub(super) current_config: Rc<RefCell<Config>>,
    pub(super) project_root: PathBuf,
    pub(super) sync_badge: gtk4::Label,
}

pub(super) fn wire_editor_extras(ctx: &EditorExtrasCtx) {
    // ── Unsaved-file indicator in file tree, and an immediate sync-badge
    // refresh ─────────────────────────────────────────────────────────────
    //
    // `on_file_dirty` fires `(path, false)` from `mark_saved` right after any
    // successful disk write — Ctrl+S, the Save button, or Save All — so it
    // doubles as "just saved" without a new callback. Without this, the
    // badge only reflected reality on the 30s auto-backup poll (see
    // `lifecycle.rs`), so saving and immediately checking the badge could
    // show stale "all backed up" for up to half a minute.
    {
        let ft = ctx.file_tree.clone();
        let badge = ctx.sync_badge.clone();
        let root_fallback = ctx.project_root.clone();
        ctx.editor_pane.set_on_file_dirty(move |path, dirty| {
            ft.set_file_modified(&path, dirty);
            if !dirty {
                let root = path
                    .parent()
                    .and_then(crate::git_sync::git_repo_root)
                    .unwrap_or_else(|| root_fallback.clone());
                super::sync::refresh_badge(&root, &badge);
            }
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
            let fname = src_path
                .file_name()
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
            let ext = src_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if ext == "pdf" {
                run_pdf_import(&win, &ep, src_path);
                return;
            }
            if let Some(fmt) = IMPORT_FORMATS
                .iter()
                .find(|f| f.extensions.contains(&ext.as_str()))
            {
                let work_dir = ep
                    .get_active_path()
                    .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                    .unwrap_or_else(|| {
                        src_path
                            .parent()
                            .map(|d| d.to_path_buf())
                            .unwrap_or_default()
                    });
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
                    let t =
                        adw::Toast::new("GOST type B isn't installed — the UI font is unchanged");
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
                &win_ut,
                &ep_ut,
                &preview_ut,
                &toast_ut,
                &root_ut,
                &cfg_ut,
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
    // Packages (start) keeps the exact pixel height the user last set it to,
    // regardless of anything above it changing size — resize_start_child
    // false means this pane never redistributes space *into* it on its own;
    // only Comments (end) absorbs a size change imposed from outside (the
    // window resizing, or Citations' own divider moving above). Both used to
    // be true, which had GTK split any such change proportionally across
    // *both* Packages and Comments — meaning dragging Citations' divider (or
    // resizing the window) visibly nudged a Packages/Comments split the user
    // had deliberately set, with no drag on that divider involved at all.
    packages_comments_pane.set_resize_start_child(false);
    packages_comments_pane.set_resize_end_child(true);
    packages_comments_pane.set_shrink_start_child(false);
    packages_comments_pane.set_shrink_end_child(false);
    packages_comments_pane.set_vexpand(true);
    let packages_comments_suppress = persist_vertical_split(
        &packages_comments_pane,
        ctx.current_config.clone(),
        |c, pos| {
            c.sidebar_packages_split = pos;
        },
    );

    let citations_packages_pane = gtk4::Paned::new(Orientation::Vertical);
    citations_packages_pane.set_start_child(Some(ctx.citation_panel.widget()));
    citations_packages_pane.set_end_child(Some(&packages_comments_pane));
    citations_packages_pane.set_position(ctx.current_config.borrow().sidebar_citations_split);
    // Same reasoning as packages_comments_pane above: Citations (start)
    // keeps its set height regardless of the window resizing; the
    // Packages+Comments block (end) absorbs that instead — which itself
    // only passes the change on to Comments, per its own resize flags,
    // leaving Packages' height untouched too.
    citations_packages_pane.set_resize_start_child(false);
    citations_packages_pane.set_resize_end_child(true);
    citations_packages_pane.set_shrink_start_child(false);
    citations_packages_pane.set_shrink_end_child(false);
    citations_packages_pane.set_vexpand(true);
    let citations_packages_suppress = persist_vertical_split(
        &citations_packages_pane,
        ctx.current_config.clone(),
        |c, pos| {
            c.sidebar_citations_split = pos;
        },
    );

    let outline_rest_pane = gtk4::Paned::new(Orientation::Vertical);
    outline_rest_pane.set_start_child(Some(ctx.outline_panel.widget()));
    outline_rest_pane.set_end_child(Some(&citations_packages_pane));
    outline_rest_pane.set_position(ctx.current_config.borrow().sidebar_outline_split);
    // Outline (start) is the one section that *does* flex with the window —
    // it's the natural "everything else is fixed, this fills the rest" pane
    // at the top of the stack, so it's the only one with resize_start_child
    // true. Citations/Packages/Comments (all inside the end child here) each
    // keep the exact height last configured, window resizes and all.
    outline_rest_pane.set_resize_start_child(true);
    outline_rest_pane.set_resize_end_child(false);
    outline_rest_pane.set_shrink_start_child(false);
    outline_rest_pane.set_shrink_end_child(false);
    outline_rest_pane.set_vexpand(true);
    let outline_rest_suppress =
        persist_vertical_split(&outline_rest_pane, ctx.current_config.clone(), |c, pos| {
            c.sidebar_outline_split = pos;
        });

    // Citations, Packages and Comments can each be collapsed to just their
    // header row — useful once a project's manuscript is stable and a
    // section isn't needed for a while. A single collapse reclaims the
    // freed space for its sibling in the shared Paned (see
    // wire_collapse_reclaims_space), same as before. What's new here is
    // `reflow_outer_sections`: previously, collapsing *both* Packages and
    // Comments only reclaimed space within their own shared Paned — the
    // Citations/Packages+Comments divider above them never moved, so the
    // freed space sat as a dead gap below the two collapsed headers instead
    // of flowing up to Citations, and the only way to actually close that
    // gap was to also drag the divider by hand (in a specific order, or the
    // sizes visibly fought each other). `reflow_outer_sections` re-derives
    // both outer dividers from the three sections' live collapsed states —
    // relying on `shrink_start_child(false)`/`shrink_end_child(false)`
    // (already set on every pane here) making each Paned's own minimum size
    // the sum of its children's minimums, so asking for an extreme position
    // is enough for GTK to clamp it to exactly "both collapsed, no gap"
    // without measuring anything by hand — so it's called after every
    // toggle of any of the three, not just its own.
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

    let reflow_outer_sections: Rc<dyn Fn()> = Rc::new({
        let citation_panel = ctx.citation_panel.clone();
        let package_browser = ctx.package_browser.clone();
        let comments_panel = ctx.comments_panel.clone();
        let citations_packages_pane = citations_packages_pane.clone();
        let outline_rest_pane = outline_rest_pane.clone();
        let packages_comments_pane = packages_comments_pane.clone();
        let cfg = ctx.current_config.clone();
        let citations_packages_suppress = citations_packages_suppress.clone();
        let outline_rest_suppress = outline_rest_suppress.clone();
        move || {
            // Deferred one main-loop turn: this runs in the same call stack
            // as the Revealer's `set_reveal_child`, whose effect on its
            // ancestors' minimum-size caches GTK only finishes processing on
            // the next iteration. Reading/setting positions synchronously
            // here worked for a single Paned's own collapse (the existing
            // `wire_collapse_reclaims_space` calls above), but not through
            // this second hop — `citations_packages_pane` reacting to its
            // end child's (`packages_comments_pane`) minimum shrinking. And
            // extreme sentinel values (`i32::MAX`) turned out not to be safe
            // to rely on either: measured directly, an un-clamped `i32::MAX`
            // survived as the literal stored position rather than getting
            // clamped against the live minimum, so this computes an exact
            // target from real, current measurements instead.
            let citation_panel = citation_panel.clone();
            let package_browser = package_browser.clone();
            let comments_panel = comments_panel.clone();
            let citations_packages_pane = citations_packages_pane.clone();
            let outline_rest_pane = outline_rest_pane.clone();
            let packages_comments_pane = packages_comments_pane.clone();
            let cfg = cfg.clone();
            let citations_packages_suppress = citations_packages_suppress.clone();
            let outline_rest_suppress = outline_rest_suppress.clone();
            glib::idle_add_local_once(move || {
                let ci = citation_panel.is_collapsed();
                let pk = package_browser.is_collapsed();
                let cm = comments_panel.is_collapsed();

                let cp_total = citations_packages_pane.height();
                if cp_total > 0 {
                    let pc_min = packages_comments_pane.measure(Orientation::Vertical, -1).1;
                    citations_packages_suppress.set(true);
                    if ci {
                        let ci_min = citation_panel.widget().measure(Orientation::Vertical, -1).1;
                        citations_packages_pane.set_position(ci_min);
                    } else if pk && cm {
                        citations_packages_pane.set_position((cp_total - pc_min).max(0));
                    } else {
                        citations_packages_pane.set_position(cfg.borrow().sidebar_citations_split);
                    }
                    citations_packages_suppress.set(false);
                }

                let or_total = outline_rest_pane.height();
                if or_total > 0 {
                    outline_rest_suppress.set(true);
                    if ci && pk && cm {
                        let cp_min = citations_packages_pane.measure(Orientation::Vertical, -1).1;
                        outline_rest_pane.set_position((or_total - cp_min).max(0));
                    } else {
                        outline_rest_pane.set_position(cfg.borrow().sidebar_outline_split);
                    }
                    outline_rest_suppress.set(false);
                }
            });
        }
    });

    // The pane has zero allocated height until the window is actually shown,
    // so a reflow requested before then (below, for a section that starts
    // collapsed per persisted config) has nothing real to measure yet —
    // retry once realization gives it real geometry.
    {
        let reflow = reflow_outer_sections.clone();
        citations_packages_pane.connect_realize(move |_| reflow());
    }

    let initial_citations_collapsed = ctx.current_config.borrow().sidebar_citations_collapsed;
    let initial_packages_collapsed = ctx.current_config.borrow().sidebar_packages_collapsed;
    let initial_comments_collapsed = ctx.current_config.borrow().sidebar_comments_collapsed;
    ctx.citation_panel
        .set_collapsed(initial_citations_collapsed);
    ctx.package_browser
        .set_collapsed(initial_packages_collapsed);
    ctx.comments_panel.set_collapsed(initial_comments_collapsed);
    if initial_packages_collapsed {
        packages_reclaim(true);
    } else if initial_comments_collapsed {
        comments_reclaim(true);
    }
    reflow_outer_sections();
    {
        let cfg = ctx.current_config.clone();
        let reflow = reflow_outer_sections.clone();
        ctx.citation_panel.set_on_collapse_toggle(move |collapsed| {
            let mut c = cfg.borrow_mut();
            c.sidebar_citations_collapsed = collapsed;
            let _ = c.save();
            drop(c);
            reflow();
        });
    }
    {
        let cfg = ctx.current_config.clone();
        let reflow = reflow_outer_sections.clone();
        ctx.package_browser
            .set_on_collapse_toggle(move |collapsed| {
                let mut c = cfg.borrow_mut();
                c.sidebar_packages_collapsed = collapsed;
                let _ = c.save();
                drop(c);
                packages_reclaim(collapsed);
                reflow();
            });
    }
    {
        let cfg = ctx.current_config.clone();
        let reflow = reflow_outer_sections;
        ctx.comments_panel.set_on_collapse_toggle(move |collapsed| {
            let mut c = cfg.borrow_mut();
            c.sidebar_comments_collapsed = collapsed;
            let _ = c.save();
            drop(c);
            comments_reclaim(collapsed);
            reflow();
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
fn persist_vertical_split(
    paned: &gtk4::Paned,
    cfg: Rc<RefCell<Config>>,
    setter: impl Fn(&mut Config, i32) + 'static,
) -> Rc<std::cell::Cell<bool>> {
    let setter = Rc::new(setter);
    let ready = Rc::new(std::cell::Cell::new(false));
    let ready2 = ready.clone();
    paned.connect_realize(move |_| {
        let r = ready2.clone();
        glib::idle_add_local_once(move || {
            r.set(true);
        });
    });
    let suppress = Rc::new(std::cell::Cell::new(false));
    let suppress2 = suppress.clone();
    let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    paned.connect_position_notify(move |p| {
        if !ready.get() || suppress2.get() {
            return;
        }
        let pos = p.position();
        let cfg2 = cfg.clone();
        let setter2 = setter.clone();
        let pending_for_cb = pending.clone();
        let mut slot = pending.borrow_mut();
        if let Some(id) = slot.take() {
            id.remove();
        }
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
