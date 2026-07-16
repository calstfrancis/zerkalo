use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, LinkButton, Orientation};
use libadwaita as adw;
use adw::prelude::*;

use crate::github_auth::{self, DeviceCodeResponse, GithubAuthError};

enum FlowUpdate {
    Code(DeviceCodeResponse),
    Done(Result<(String, String), GithubAuthError>),
}

/// Shows a modal "Sign in with GitHub" window that drives the device flow
/// end-to-end: requests a code, displays it alongside a link to open the
/// verification page, polls for approval, stores the resulting token in the
/// system keyring, and reports the connected username back to `on_connected`.
pub fn present(parent: &impl IsA<gtk4::Window>, on_connected: impl Fn(String) + 'static) {
    let dialog = adw::Window::builder()
        .title("Sign in with GitHub")
        .transient_for(parent)
        .modal(true)
        .default_width(420)
        .default_height(260)
        .build();

    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    let cancel_btn = Button::with_label("Cancel");
    cancel_btn.add_css_class("flat");
    header.pack_start(&cancel_btn);

    let status_lbl = Label::new(Some("Requesting a sign-in code from GitHub…"));
    status_lbl.set_wrap(true);
    status_lbl.set_xalign(0.0);
    status_lbl.set_margin_top(16);
    status_lbl.set_margin_start(16);
    status_lbl.set_margin_end(16);

    let code_lbl = Label::new(None);
    code_lbl.add_css_class("title-1");
    code_lbl.set_selectable(true);
    code_lbl.set_margin_top(12);
    code_lbl.set_visible(false);

    let open_link = LinkButton::with_label("https://github.com/login/device", "Open github.com/login/device ↗");
    open_link.set_halign(Align::Center);
    open_link.set_margin_top(8);
    open_link.set_visible(false);

    let spinner = gtk4::Spinner::new();
    spinner.set_spinning(true);
    spinner.set_margin_top(16);
    spinner.set_halign(Align::Center);

    let vbox = GtkBox::new(Orientation::Vertical, 4);
    vbox.append(&header);
    vbox.append(&status_lbl);
    vbox.append(&code_lbl);
    vbox.append(&open_link);
    vbox.append(&spinner);
    dialog.set_content(Some(&vbox));

    let cancelled = Arc::new(AtomicBool::new(false));

    let dialog_cancel = dialog.clone();
    let cancelled_btn = cancelled.clone();
    cancel_btn.connect_clicked(move |_| {
        cancelled_btn.store(true, Ordering::Relaxed);
        dialog_cancel.close();
    });

    let (tx, rx) = sync_channel::<FlowUpdate>(2);
    let cancelled_thread = cancelled.clone();
    std::thread::spawn(move || {
        let device = match github_auth::request_device_code(github_auth::CLIENT_ID) {
            Ok(d) => d,
            Err(e) => {
                let _ = tx.send(FlowUpdate::Done(Err(e)));
                return;
            }
        };
        let _ = tx.send(FlowUpdate::Code(device.clone()));

        let result = github_auth::poll_for_access_token(github_auth::CLIENT_ID, &device, &cancelled_thread)
            .and_then(|token| github_auth::fetch_username(&token).map(|user| (token, user)));
        let _ = tx.send(FlowUpdate::Done(result));
    });

    let dialog_poll = dialog.clone();
    glib::timeout_add_local(Duration::from_millis(200), move || {
        loop {
            match rx.try_recv() {
                Ok(FlowUpdate::Code(device)) => {
                    status_lbl.set_label("Enter this code at github.com to connect your account:");
                    code_lbl.set_label(&device.user_code);
                    code_lbl.set_visible(true);
                    open_link.set_uri(&device.verification_uri);
                    open_link.set_visible(true);
                }
                Ok(FlowUpdate::Done(Ok((token, username)))) => {
                    // Guards against the rare race where approval lands just as
                    // Cancel is clicked — don't save the token or report success.
                    if cancelled.load(Ordering::Relaxed) {
                        return glib::ControlFlow::Break;
                    }
                    if let Err(e) = crate::secret_store::save_github_token(&token) {
                        status_lbl.set_label(&format!("Signed in, but couldn't store the token: {e}"));
                        spinner.set_spinning(false);
                        return glib::ControlFlow::Break;
                    }
                    dialog_poll.close();
                    on_connected(username);
                    return glib::ControlFlow::Break;
                }
                Ok(FlowUpdate::Done(Err(e))) => {
                    status_lbl.set_label(&format!("Sign-in failed: {e}"));
                    spinner.set_spinning(false);
                    return glib::ControlFlow::Break;
                }
                Err(TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(TryRecvError::Disconnected) => return glib::ControlFlow::Break,
            }
        }
    });

    dialog.present();
}
