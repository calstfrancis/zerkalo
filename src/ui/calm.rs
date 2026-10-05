//! One thing at a time at start-up.
//!
//! When Zerkalo opens it may have several things to say — a welcome, writing it
//! kept from a crash, newer writing waiting online. Showing them all at once
//! is a pile-up. [`when_calm`] holds each back until no other window is open
//! over the main one.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;

/// Whether any other window is currently open in front of `parent`.
fn something_is_open(parent: &gtk4::Window) -> bool {
    gtk4::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk4::Window>().ok())
        .any(|w| w.is_visible() && &w != parent && w.transient_for().as_ref() == Some(parent))
}

/// Runs `f` once no dialog or window is open over `parent` — now, if that's
/// already true, otherwise as soon as it becomes true.
pub fn when_calm(parent: &impl IsA<gtk4::Window>, f: impl FnOnce() + 'static) {
    let parent: gtk4::Window = parent.clone().upcast();
    let f = Rc::new(RefCell::new(Some(f)));
    let run = move || -> glib::ControlFlow {
        if something_is_open(&parent) {
            return glib::ControlFlow::Continue;
        }
        if let Some(f) = f.borrow_mut().take() {
            f();
        }
        glib::ControlFlow::Break
    };
    glib::timeout_add_local(Duration::from_millis(600), run);
}
