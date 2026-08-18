use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use fond_vault::VaultEvent;

/// Starts a live watch on a Kartoteka vault directory. Fires `on_change` on
/// the GTK main thread whenever anything under the vault changes, debounced
/// so a burst of writes (e.g. Kartoteka regenerating `library.yml` after an
/// edit) triggers one reload rather than several.
///
/// `fond_vault::watch` hands back a plain `mpsc::Receiver` fed by `notify`'s
/// own background thread, so bridging it into the GTK main loop is a drain
/// poll — the same idiom `file_watcher.rs` uses for `.typ` files, just
/// without needing its own path-filtering/dedup layer since events here are
/// only used as a reload trigger, not inspected individually.
pub fn start(vault_dir: PathBuf, on_change: impl Fn() + 'static) -> Option<fond_vault::VaultWatch> {
    let (watch, rx) = fond_vault::watch(&vault_dir).ok()?;

    glib::timeout_add_local(Duration::from_millis(300), move || {
        if drain_has_change(&rx) {
            on_change();
        }
        glib::ControlFlow::Continue
    });

    Some(watch)
}

fn drain_has_change(rx: &Receiver<VaultEvent>) -> bool {
    let mut changed = false;
    while let Ok(event) = rx.try_recv() {
        if let VaultEvent::Changed(_) = event {
            changed = true;
        }
    }
    changed
}
