//! "Export writing history": the project's history as one text file, for
//! showing a teacher how the writing came about. The file itself is built in
//! `writing_history`; this asks what to include and where to put it.

use std::path::{Path, PathBuf};

use adw::prelude::*;
use gtk4::prelude::*;
use libadwaita as adw;

use super::notify::Say;

pub fn run(
    parent: &impl IsA<gtk4::Window>,
    toasts: &adw::ToastOverlay,
    project_root: &Path,
    active_file: Option<PathBuf>,
) {
    let body = "A file showing each saved version of your writing, with the date it was saved \
                and exactly what changed. It is for showing a teacher how your writing grew \
                over time.\n\nIt comes from your project's backup history, so it only goes \
                back as far as your first backup.";
    let dlg = adw::MessageDialog::new(
        Some(parent),
        Some("Export your writing history"),
        Some(body),
    );
    dlg.add_response("cancel", "Not now");
    if active_file.is_some() {
        dlg.add_response("file", "Just this document");
    }
    dlg.add_response("all", "The whole project");
    dlg.set_response_appearance("all", adw::ResponseAppearance::Suggested);
    dlg.set_default_response(Some("all"));
    dlg.set_close_response("cancel");

    let parent: gtk4::Window = parent.clone().upcast();
    let toasts = toasts.clone();
    let root = project_root.to_path_buf();
    dlg.connect_response(None, move |_, response| {
        let only = match response {
            "all" => None,
            "file" => active_file.clone(),
            _ => return,
        };
        choose_where(&parent, &toasts, &root, only);
    });
    dlg.present();
}

fn choose_where(
    parent: &gtk4::Window,
    toasts: &adw::ToastOverlay,
    root: &Path,
    only: Option<PathBuf>,
) {
    let stem = only
        .as_deref()
        .and_then(|f| f.file_stem())
        .or_else(|| root.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".into());
    let dialog = gtk4::FileDialog::builder()
        .title("Save your writing history")
        .modal(true)
        .initial_name(format!("{stem}-writing-history.txt"))
        .build();
    let toasts = toasts.clone();
    let root = root.to_path_buf();
    dialog.save(
        Some(parent),
        None::<&gtk4::gio::Cancellable>,
        move |result| {
            let Ok(file) = result else { return };
            let Some(out) = file.path() else { return };
            let toasts = toasts.clone();
            let root = root.clone();
            let only = only.clone();
            gtk4::glib::spawn_future_local(async move {
                let today = chrono::Local::now().format("%-d %B %Y").to_string();
                let built = gtk4::gio::spawn_blocking(move || {
                    crate::writing_history::build(&root, only.as_deref(), &today)
                })
                .await;
                let message = match built {
                    Ok(Ok(text)) => match crate::error::atomic_write(&out, text.as_bytes()) {
                        Ok(()) => "Saved your writing history.".to_string(),
                        Err(e) => format!(
                            "Couldn't save the file there — {}.",
                            crate::error::io_reason(&e)
                        ),
                    },
                    Ok(Err(why)) => why,
                    Err(_) => "Couldn't build the writing history.".to_string(),
                };
                toasts.say(adw::Toast::new(&message));
            });
        },
    );
}
