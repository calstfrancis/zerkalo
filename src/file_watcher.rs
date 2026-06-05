use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// Starts a recursive filesystem watcher on `project_dir`.
/// Fires `on_change` on the GTK main thread whenever a `.typ` file is
/// written or created by an external process.
/// Returns the watcher handle; drop it to stop watching.
pub fn start(
    project_dir: PathBuf,
    on_change: impl Fn(PathBuf) + 'static,
) -> Option<RecommendedWatcher> {
    // Pending paths collected by the watcher thread, drained on the GTK thread.
    let pending: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(Vec::new()));
    let pending_watcher = pending.clone();

    let on_change = std::rc::Rc::new(on_change);

    // Poll the pending queue on GTK's main loop every 250 ms.
    glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
        let paths: Vec<PathBuf> = pending.lock().map(|mut g| g.drain(..).collect()).unwrap_or_default();
        for path in paths {
            on_change(path);
        }
        glib::ControlFlow::Continue
    });

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            let is_write = matches!(
                event.kind,
                EventKind::Modify(_) | EventKind::Create(_)
            );
            if is_write {
                if let Ok(mut guard) = pending_watcher.lock() {
                    for path in event.paths {
                        if path.extension().map_or(false, |e| e == "typ") {
                            guard.push(path);
                        }
                    }
                }
            }
        }
    })
    .ok()?;

    watcher.watch(&project_dir, RecursiveMode::Recursive).ok()?;
    Some(watcher)
}
