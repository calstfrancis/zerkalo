//! The status bar's live numbers — line and column, word counts, words this
//! session, words in this section — and when they change.
//!
//! Numbers that tick over with every keystroke are movement in the corner of
//! the eye while writing. By default (Settings → Comfort) they settle: a new
//! value waits until typing has paused for a moment, then all of them change
//! together. They can also change live, or be hidden.

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use gtk4::glib;
use gtk4::Label;

use crate::config::CounterTiming;

/// How long typing must pause before the numbers catch up — the same pause
/// that lets a new problem show.
pub(super) const SETTLE: Duration = Duration::from_millis(1500);

thread_local! {
    static LAST_TYPED: Cell<Option<Instant>> = const { Cell::new(None) };
    static PENDING: RefCell<Vec<(Label, String)>> = const { RefCell::new(Vec::new()) };
    static FLUSH: RefCell<Option<glib::SourceId>> = const { RefCell::new(None) };
}

pub(super) fn timing() -> CounterTiming {
    crate::config::shared().borrow().comfort.counters
}

/// Called for every edit, so the numbers know typing is going on.
pub(super) fn typed() {
    LAST_TYPED.with(|t| t.set(Some(Instant::now())));
}

/// Shows `text` on `label` — now, or once typing pauses.
pub(super) fn show(label: &Label, text: &str) {
    if timing() == CounterTiming::Hidden {
        if !label.text().is_empty() {
            label.set_text("");
        }
        return;
    }
    let since = LAST_TYPED.with(|t| t.get()).map(|t| t.elapsed());
    let waiting = timing() == CounterTiming::WhenPaused && since.is_some_and(|s| s < SETTLE);
    if !waiting {
        PENDING.with(|p| p.borrow_mut().retain(|(l, _)| l != label));
        if label.text() != text {
            label.set_text(text);
        }
        return;
    }
    PENDING.with(|p| {
        let mut p = p.borrow_mut();
        p.retain(|(l, _)| l != label);
        p.push((label.clone(), text.to_string()));
    });
    schedule_flush(SETTLE - since.unwrap_or_default());
}

/// Shows `text` straight away and drops anything waiting for `label` — for
/// something the person did on purpose, like selecting text.
pub(super) fn now(label: &Label, text: &str) {
    PENDING.with(|p| p.borrow_mut().retain(|(l, _)| l != label));
    label.set_text(text);
}

fn schedule_flush(after: Duration) {
    FLUSH.with(|f| {
        if let Some(id) = f.borrow_mut().take() {
            id.remove();
        }
        let id = glib::timeout_add_local_once(after, || {
            FLUSH.with(|f| *f.borrow_mut() = None);
            let since = LAST_TYPED.with(|t| t.get()).map(|t| t.elapsed());
            if let Some(s) = since.filter(|s| *s < SETTLE) {
                schedule_flush(SETTLE - s);
                return;
            }
            let pending = PENDING.with(|p| std::mem::take(&mut *p.borrow_mut()));
            for (label, text) in pending {
                label.set_text(&text);
            }
        });
        *f.borrow_mut() = Some(id);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pausing_matches_the_problem_grace_period() {
        assert_eq!(SETTLE, Duration::from_millis(1500));
    }
}
