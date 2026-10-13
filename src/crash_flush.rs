use std::cell::RefCell;
use std::io::Write;
use std::path::Path;

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
    let crash_log = crate::config::zerkalo_data_dir().join(CRASH_LOG);
    std::panic::set_hook(Box::new(move |info| {
        // Written synchronously and first: zerkalo.log goes through a background writer an
        // abort can kill before it flushes, and stderr is lost when launched from a menu.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let backtrace = std::backtrace::Backtrace::force_capture().to_string();
            append_report(&crash_log, &crash_report(&info.to_string(), &backtrace))
        }));
        tracing::error!("{info}");

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

pub const CRASH_LOG: &str = "crash.log";
const CRASH_LOG_MAX_BYTES: u64 = 1024 * 1024;

fn crash_report(panic: &str, backtrace: &str) -> String {
    let thread = std::thread::current();
    format!(
        "=== {} · Zerkalo {} · thread {}\n{panic}\n\nBacktrace:\n{backtrace}\n\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S %z"),
        env!("CARGO_PKG_VERSION"),
        thread.name().unwrap_or("unnamed"),
    )
}

/// Appends to `path`, first moving a file past the size cap aside to `<path>.old`.
fn append_report(path: &Path, report: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if std::fs::metadata(path).is_ok_and(|m| m.len() > CRASH_LOG_MAX_BYTES) {
        std::fs::rename(path, path.with_extension("log.old"))?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    f.write_all(report.as_bytes())?;
    f.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crash_report_carries_the_message_location_version_and_backtrace() {
        let r = crash_report(
            "panicked at src/ui/x.rs:12:5:\nalready borrowed",
            "0: zerkalo::f",
        );
        assert!(r.contains("src/ui/x.rs:12:5"));
        assert!(r.contains("already borrowed"));
        assert!(r.contains(env!("CARGO_PKG_VERSION")));
        assert!(r.contains("0: zerkalo::f"));
    }

    #[test]
    fn reports_append_and_an_oversized_log_is_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("nested").join(CRASH_LOG);
        append_report(&log, "one\n").unwrap();
        append_report(&log, "two\n").unwrap();
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "one\ntwo\n");

        std::fs::write(&log, vec![b'x'; CRASH_LOG_MAX_BYTES as usize + 1]).unwrap();
        append_report(&log, "three\n").unwrap();
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "three\n");
        assert!(dir.path().join("nested/crash.log.old").exists());
    }
}
