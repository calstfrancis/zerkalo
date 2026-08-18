//! Two pieces of `AppWindow::new`'s tail: persisting the paned positions, and
//! the filesystem watcher that reacts to `.typ` files changing outside the app.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::Paned;

use super::super::editor_pane::EditorPane;
use super::super::library_window::LibraryWindow;
use super::super::preview_pane::PreviewPane;
use crate::config::Config;
use crate::library::Library;

pub(super) struct PanePersistCtx {
    pub(super) current_config: Rc<RefCell<Config>>,
    pub(super) outer_paned: Paned,
    pub(super) inner_paned: Paned,
}

/// Pane positions are saved 400 ms after the last drag, with a guard against
/// persisting the initial layout pass on window realize.
pub(super) fn wire_pane_persistence(ctx: &PanePersistCtx) {
    // ── Persist pane positions (debounced, 400 ms after last drag) ────────
    // Use a flag so we ignore position-notify during initial GTK layout.
    {
        let cfg = ctx.current_config.clone();
        let ready = Rc::new(std::cell::Cell::new(false));
        let ready2 = ready.clone();
        ctx.outer_paned.connect_realize(move |_| {
            let r = ready2.clone();
            glib::idle_add_local_once(move || {
                r.set(true);
            });
        });
        let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        ctx.outer_paned.connect_position_notify(move |p| {
            if !ready.get() {
                return;
            }
            let pos = p.position();
            let cfg2 = cfg.clone();
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
                    c.sidebar_width = pos;
                    let _ = c.save();
                },
            ));
        });
    }
    {
        let cfg = ctx.current_config.clone();
        let ready = Rc::new(std::cell::Cell::new(false));
        let ready2 = ready.clone();
        ctx.inner_paned.connect_realize(move |_| {
            let r = ready2.clone();
            glib::idle_add_local_once(move || {
                r.set(true);
            });
        });
        let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        ctx.inner_paned.connect_position_notify(move |p| {
            if !ready.get() {
                return;
            }
            let pos = p.position();
            let cfg2 = cfg.clone();
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
                    c.preview_split = pos;
                    let _ = c.save();
                },
            ));
        });
    }
}

pub(super) struct WatcherCtx {
    pub(super) editor_pane: EditorPane,
    pub(super) preview_pane: PreviewPane,
    pub(super) project_root: PathBuf,
    pub(super) library: Rc<RefCell<Library>>,
    pub(super) library_window: LibraryWindow,
    pub(super) manual_compile_only: Rc<RefCell<bool>>,
}

pub(super) fn wire_file_watcher(ctx: &WatcherCtx) -> Option<notify::RecommendedWatcher> {
    // ── File-system watcher for external .typ changes ───────────────────
    // Fires when a .typ file in the project is written by an external tool
    // (e.g., a sync agent, another editor) so the preview stays current.
    let preview_for_watch = ctx.preview_pane.clone();
    let editor_for_watch = ctx.editor_pane.clone();
    let mco_for_watch = ctx.manual_compile_only.clone();
    let library_for_watch = ctx.library.clone();
    let lw_for_watch = ctx.library_window.clone();
    let file_watcher = crate::file_watcher::start(ctx.project_root.clone(), move |changed_path| {
        library_for_watch
            .borrow_mut()
            .upsert_document(&changed_path)
            .ok();
        if lw_for_watch.window().is_visible() {
            lw_for_watch.refresh();
        }
        // Only react to files we don't have open — those are handled by
        // the editor's own save path.
        let is_open = editor_for_watch.is_file_open(&changed_path);
        if !is_open && !*mco_for_watch.borrow() {
            preview_for_watch.trigger_compile();
        }
    });

    file_watcher
}
