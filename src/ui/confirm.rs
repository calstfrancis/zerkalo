//! Shared message dialogs.
//!
//! Every small "are you sure?" and "here's what happened" box in the app goes
//! through here. Before this they were split between `gtk4::AlertDialog` and
//! `adw::MessageDialog`, which look like siblings from different families —
//! most visibly on the destructive confirmations, where looking trustworthy
//! matters most. `adw::MessageDialog` is the right one at the libadwaita
//! version this build pins (v1_4; `AlertDialog` only arrives in 1.5).

use adw::prelude::*;
use gtk4::prelude::*;
use libadwaita as adw;

/// A confirmation whose accept button is styled destructive. `on_confirm` runs
/// only when that button is chosen; Escape and the close button cancel.
pub fn confirm_destructive(
    parent: Option<&gtk4::Window>,
    heading: &str,
    body: &str,
    action_label: &str,
    on_confirm: impl Fn() + 'static,
) {
    let dlg = adw::MessageDialog::new(parent, Some(heading), Some(body));
    dlg.add_response("cancel", "Cancel");
    dlg.add_response("confirm", action_label);
    dlg.set_response_appearance("confirm", adw::ResponseAppearance::Destructive);
    dlg.set_default_response(Some("cancel"));
    dlg.set_close_response("cancel");
    dlg.connect_response(None, move |_, response| {
        if response == "confirm" {
            on_confirm();
        }
    });
    dlg.present();
}

/// A plain acknowledgement, with a Copy button alongside OK so the message —
/// often a raw error — can be pasted elsewhere instead of retyped by hand.
/// `adw::MessageDialog`'s body label isn't selectable, so without this the
/// only way to get an error out of the dialog was a screenshot.
pub fn notice(parent: Option<&gtk4::Window>, heading: &str, body: &str) {
    let dlg = adw::MessageDialog::new(parent, Some(heading), Some(body));
    dlg.add_response("copy", "Copy");
    dlg.add_response("ok", "OK");
    dlg.set_default_response(Some("ok"));
    dlg.set_close_response("ok");
    let text = body.to_string();
    dlg.connect_response(None, move |_, response| {
        if response == "copy" {
            if let Some(display) = gtk4::gdk::Display::default() {
                display.clipboard().set_text(&text);
            }
        }
    });
    dlg.present();
}

/// Moves `path` to the system trash after confirming, then runs `after`.
///
/// The file tree's row menu and the open-dropdown's per-row delete button both
/// offer this and had grown separate copies of the same dialog.
pub fn confirm_trash(
    parent: Option<&gtk4::Window>,
    path: std::path::PathBuf,
    after: impl Fn(&std::path::Path) + 'static,
) {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("this file")
        .to_string();
    confirm_destructive(
        parent,
        "Move to trash?",
        &format!("'{name}' will be moved to the system trash."),
        "Move to Trash",
        move || {
            let _ = gtk4::gio::File::for_path(&path).trash(None::<&gtk4::gio::Cancellable>);
            after(&path);
        },
    );
}
