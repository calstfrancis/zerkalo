//! Status file for the Porog launcher (`~/.local/share/fond/status/zerkalo.json`); all disk work is on one worker thread.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::OnceLock;
use std::time::Duration;

use fond_status::{ResumeEntry, Status};

const APP: &str = "zerkalo";

enum Msg {
    Active { path: PathBuf, title: String },
    Flush(Sender<()>),
}

fn worker() -> Option<&'static Sender<Msg>> {
    static WORKER: OnceLock<Option<Sender<Msg>>> = OnceLock::new();
    WORKER
        .get_or_init(|| {
            let dir = fond_status::status_dir()?;
            let (tx, rx) = mpsc::channel();
            std::thread::Builder::new()
                .name("porog-status".into())
                .spawn(move || run(&dir, rx))
                .ok()?;
            Some(tx)
        })
        .as_ref()
}

pub fn active(path: &Path, title: &str) {
    if let Some(tx) = worker() {
        let _ = tx.send(Msg::Active {
            path: path.to_path_buf(),
            title: title.to_string(),
        });
    }
}

pub fn flush() {
    if let Some(tx) = worker() {
        let (done, wait) = mpsc::channel();
        if tx.send(Msg::Flush(done)).is_ok() {
            let _ = wait.recv_timeout(Duration::from_millis(500));
        }
    }
}

fn run(dir: &Path, rx: mpsc::Receiver<Msg>) {
    let mut status = Status::read(dir, APP)
        .ok()
        .flatten()
        .unwrap_or_else(|| Status::new(APP, ""));
    status.app = APP.to_string();

    for msg in rx {
        let (path, title) = match msg {
            Msg::Active { path, title } => (path, title),
            Msg::Flush(done) => {
                let _ = done.send(());
                continue;
            }
        };
        let Some(mut entry) = entry_for(&path, &title) else {
            continue;
        };
        if status
            .resume
            .first()
            .is_some_and(|top| top.args == entry.args && top.title == entry.title)
        {
            continue;
        }
        let now = fond_status::now_utc();
        entry.at = Some(now.clone());
        status.push_resume(entry);
        status.written = now;
        if let Err(e) = status.write(dir) {
            eprintln!("zerkalo: could not write the Porog status file: {e}");
        }
    }
}

// Porog passes `args` back as Zerkalo's command line, which takes one existing `.typ` path.
fn entry_for(path: &Path, title: &str) -> Option<ResumeEntry> {
    if !path.is_absolute() || !path.is_file() {
        return None;
    }
    let arg = path.to_str()?.to_string();
    let file_name = path.file_name()?.to_str()?.to_string();
    let stem = file_name.strip_suffix(".typ").unwrap_or(&file_name);
    let title = title.trim();
    let (title, detail) = if title.is_empty() || title == stem {
        (file_name, None)
    } else {
        (title.to_string(), Some(file_name))
    };
    Some(ResumeEntry {
        title,
        detail,
        at: None,
        args: vec![arg],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("zerkalo-status-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_titled_document_shows_its_title_then_its_file_name() {
        let dir = scratch("titled");
        let file = dir.join("essay-2.typ");
        std::fs::write(&file, "").unwrap();

        let titled = entry_for(&file, "An Example Essay").unwrap();
        assert_eq!(titled.title, "An Example Essay");
        assert_eq!(titled.detail.as_deref(), Some("essay-2.typ"));
        assert_eq!(titled.args, [file.to_str().unwrap()]);

        let untitled = entry_for(&file, "essay-2").unwrap();
        assert_eq!(untitled.title, "essay-2.typ");
        assert_eq!(untitled.detail, None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_buffer_with_no_file_behind_it_is_not_offered() {
        let dir = scratch("missing");
        assert!(entry_for(&dir.join("gone.typ"), "Gone").is_none());
        assert!(entry_for(Path::new("untitled.typ"), "Untitled").is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_worker_keeps_the_newest_first_and_ignores_a_repeat() {
        let dir = scratch("worker");
        let a = dir.join("a.typ");
        let b = dir.join("b.typ");
        std::fs::write(&a, "").unwrap();
        std::fs::write(&b, "").unwrap();
        let status_dir = dir.join("status");

        let (tx, rx) = mpsc::channel();
        let active = |path: &Path| Msg::Active {
            path: path.to_path_buf(),
            title: String::new(),
        };
        tx.send(active(&a)).unwrap();
        tx.send(active(&b)).unwrap();
        tx.send(active(&b)).unwrap();
        tx.send(active(&a)).unwrap();
        drop(tx);
        run(&status_dir, rx);

        let status = Status::read(&status_dir, APP).unwrap().unwrap();
        let titles: Vec<_> = status.resume.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, ["a.typ", "b.typ"]);
        assert!(status.resume[0].at.is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
