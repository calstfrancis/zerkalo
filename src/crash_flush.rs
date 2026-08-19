use std::cell::RefCell;

use crate::ui::editor_pane::EditorPane;

thread_local! {
    static ACTIVE_EDITOR: RefCell<Option<EditorPane>> = const { RefCell::new(None) };
}

pub fn register(editor_pane: &EditorPane) {
    ACTIVE_EDITOR.with(|cell| *cell.borrow_mut() = Some(editor_pane.clone()));
}

/// Installs a panic hook that makes a best-effort attempt to write
/// crash-recovery autosave copies of any modified buffers before the
/// process aborts, so a panic loses at most as much work as a normal
/// autosave interval would. Wrapped in `catch_unwind` because the panic
/// may have left GTK/editor state mid-mutation (e.g. a `RefCell` already
/// borrowed) — a second panic here must not become a hard abort before
/// the original panic's own hook (logging, message) still runs.
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let flushed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ACTIVE_EDITOR.with(|cell| {
                cell.borrow().as_ref().map(|ep| {
                    ep.modified_buffers()
                        .into_iter()
                        .filter(|(path, content)| crate::auto_save::save(path, content))
                        .count()
                })
            })
        }));

        match flushed {
            Ok(Some(n)) => tracing::error!(
                "panic: crash-recovery autosave flushed {n} modified buffer(s) before exit"
            ),
            Ok(None) => {}
            Err(_) => tracing::error!(
                "panic: crash-recovery autosave flush itself failed; state may already have been mid-mutation"
            ),
        }

        default_hook(info);
    }));
}
