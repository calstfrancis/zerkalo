//! Shared polling helper for the `thread::spawn` + `mpsc::sync_channel` +
//! `glib::timeout_add_local` shape used across the UI wherever a background
//! job (compile, package fetch/install, …) produces a `Result<T, String>`
//! that a `glib` timer then has to poll for and hand to the main thread —
//! see `WINDOWS-HARDENING-PLAN.md` Phase 6b. Several call sites hand-rolled
//! this identically; this is the shared shape, not a forced fit for every
//! spawn+poll site in the codebase — a few (multi-message channels, custom
//! result enums) genuinely differ and are left as they were.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use gtk4::glib;

/// Polls `rx` on a `glib` timer every `interval` until it yields a value,
/// then calls `on_ok`/`on_err` once and stops. A disconnected channel (the
/// spawned thread panicked before sending) silently breaks the poll with no
/// callback — matching what every site using this shape already did.
pub fn poll_result<T: 'static>(
    rx: Receiver<Result<T, String>>,
    interval: Duration,
    on_ok: impl FnOnce(T) + 'static,
    on_err: impl FnOnce(String) + 'static,
) {
    let rx = Rc::new(rx);
    let on_ok = Rc::new(RefCell::new(Some(on_ok)));
    let on_err = Rc::new(RefCell::new(Some(on_err)));
    glib::timeout_add_local(interval, move || match rx.try_recv() {
        Ok(Ok(value)) => {
            if let Some(f) = on_ok.borrow_mut().take() {
                f(value);
            }
            glib::ControlFlow::Break
        }
        Ok(Err(e)) => {
            if let Some(f) = on_err.borrow_mut().take() {
                f(e);
            }
            glib::ControlFlow::Break
        }
        Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(TryRecvError::Disconnected) => glib::ControlFlow::Break,
    });
}
