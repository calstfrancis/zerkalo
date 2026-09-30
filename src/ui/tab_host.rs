//! The editor's open documents: an `adw::TabView` with an `adw::TabBar` above it.
//!
//! Exposes the handful of index-based calls the editor code was written
//! against (`page_num`, `current_page`, `set_current_page`, ...) so the rest of
//! `editor_pane.rs` doesn't care which container holds the pages.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gtk4::gio;
use libadwaita as adw;

#[derive(Clone)]
pub struct TabHost {
    pub view: adw::TabView,
    pub bar: adw::TabBar,
    bypass_close: Rc<Cell<bool>>,
}

impl TabHost {
    pub fn new() -> Self {
        let view = adw::TabView::new();
        view.set_hexpand(true);
        view.set_vexpand(true);
        let bar = adw::TabBar::new();
        bar.set_view(Some(&view));
        bar.set_autohide(false);
        bar.set_expand_tabs(true);
        Self {
            view,
            bar,
            bypass_close: Rc::new(Cell::new(false)),
        }
    }

    /// True while a page is being removed by code (not by the user), so the
    /// close-page handler skips its unsaved-changes dialog.
    pub fn bypassing_close(&self) -> bool {
        self.bypass_close.get()
    }

    pub fn n_pages(&self) -> u32 {
        self.view.n_pages().max(0) as u32
    }

    pub fn page_num(&self, child: &impl IsA<gtk4::Widget>) -> Option<u32> {
        let child = child.as_ref();
        (0..self.view.n_pages())
            .find(|&i| self.view.nth_page(i).child() == *child)
            .map(|i| i as u32)
    }

    pub fn nth_page(&self, n: Option<u32>) -> Option<gtk4::Widget> {
        let n = n?;
        (n < self.n_pages()).then(|| self.view.nth_page(n as i32).child())
    }

    pub fn current_page(&self) -> Option<u32> {
        let page = self.view.selected_page()?;
        Some(self.view.page_position(&page).max(0) as u32)
    }

    pub fn set_current_page(&self, n: Option<u32>) {
        if let Some(n) = n {
            if n < self.n_pages() {
                self.view.set_selected_page(&self.view.nth_page(n as i32));
            }
        }
    }

    pub fn remove_page(&self, n: Option<u32>) {
        let Some(n) = n else { return };
        if n >= self.n_pages() {
            return;
        }
        let page = self.view.nth_page(n as i32);
        self.bypass_close.set(true);
        self.view.close_page(&page);
        self.bypass_close.set(false);
    }
}

/// One of the two marks a tab can carry, shown through its `TabPage`: the
/// unsaved-changes dot (page indicator) or the error icon (leading page icon).
#[derive(Clone, Copy)]
enum MarkKind {
    Unsaved,
    Error,
}

#[derive(Clone)]
pub struct TabMark {
    page: adw::TabPage,
    view: adw::TabView,
    kind: MarkKind,
}

impl TabMark {
    pub fn unsaved(view: &adw::TabView, page: &adw::TabPage) -> Self {
        Self {
            page: page.clone(),
            view: view.clone(),
            kind: MarkKind::Unsaved,
        }
    }

    pub fn error(view: &adw::TabView, page: &adw::TabPage) -> Self {
        Self {
            page: page.clone(),
            view: view.clone(),
            kind: MarkKind::Error,
        }
    }

    pub fn set_visible(&self, on: bool) {
        match self.kind {
            MarkKind::Unsaved => {
                let icon = on.then(|| gio::ThemedIcon::new("media-record-symbolic"));
                self.page.set_indicator_icon(icon.as_ref());
                self.page
                    .set_indicator_tooltip(if on { "Unsaved changes" } else { "" });
            }
            MarkKind::Error => {
                let icon = on.then(|| gio::ThemedIcon::new("dialog-error-symbolic"));
                self.page.set_icon(icon.as_ref());
            }
        }
    }

    /// Any widget in the window, for theme-colour lookups.
    pub fn widget(&self) -> &adw::TabView {
        &self.view
    }
}
