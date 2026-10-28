//! The quiet "Updated to 0.55 — what's new" note in the status bar.
//!
//! After an update Zerkalo used to open the What's new window by itself, on
//! top of whatever the person sat down to do. The note says the same thing
//! without asking for anything: it waits in the status bar, across launches,
//! until it is clicked or closed.

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Label, Orientation};

use super::welcome_window::WelcomeWindow;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn build(parent: &gtk4::Window, last_seen: Option<String>) -> GtkBox {
    let note = GtkBox::new(Orientation::Horizontal, 0);
    note.set_margin_start(8);

    let label = Label::new(Some(&format!("Updated to {VERSION} — what's new")));
    label.add_css_class("caption");
    let open = Button::new();
    open.set_child(Some(&label));
    open.add_css_class("flat");
    open.add_css_class("status-toggle");
    open.set_tooltip_text(Some("Read what changed in this version"));

    let close = Button::from_icon_name("window-close-symbolic");
    close.add_css_class("flat");
    close.add_css_class("dim-label");
    close.set_valign(gtk4::Align::Center);
    close.set_tooltip_text(Some("Close this note"));
    close.update_property(&[gtk4::accessible::Property::Label("Close this note")]);

    note.append(&open);
    note.append(&close);

    {
        let note = note.clone();
        let parent = parent.clone();
        open.connect_clicked(move |_| {
            WelcomeWindow::mark_shown();
            note.set_visible(false);
            WelcomeWindow::new(&parent, false, last_seen.clone()).present();
        });
    }
    {
        let note = note.clone();
        close.connect_clicked(move |_| {
            WelcomeWindow::mark_shown();
            note.set_visible(false);
        });
    }
    note
}
