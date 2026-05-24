use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Orientation};
use libadwaita as adw;

/// Modal dialog asking the user for a GitHub remote URL.
/// Shows when Sync is clicked and no remote is configured.
pub struct SyncDialog {
    pub window: adw::Window,
    url_row: adw::EntryRow,
    on_confirm: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
}

impl SyncDialog {
    pub fn new(parent: &impl IsA<gtk4::Window>) -> Self {
        let window = adw::Window::builder()
            .title("Link to GitHub")
            .transient_for(parent)
            .modal(true)
            .default_width(440)
            .resizable(false)
            .build();

        // ── Header bar ──────────────────────────────────────────────────────

        let header = adw::HeaderBar::new();
        header.set_show_end_title_buttons(false);

        let cancel_btn = Button::with_label("Cancel");
        cancel_btn.add_css_class("flat");
        header.pack_start(&cancel_btn);

        // ── Content ──────────────────────────────────────────────────────────

        let group = adw::PreferencesGroup::new();
        group.set_description(Some(
            "No GitHub remote found. Paste your repository URL to push.",
        ));

        let url_row = adw::EntryRow::new();
        url_row.set_title("Repository URL");
        group.add(&url_row);

        let confirm_btn = Button::with_label("Add Remote & Push");
        confirm_btn.add_css_class("suggested-action");
        confirm_btn.add_css_class("pill");
        confirm_btn.set_halign(Align::Center);
        confirm_btn.set_margin_top(8);

        let content = GtkBox::new(Orientation::Vertical, 0);
        content.set_margin_top(16);
        content.set_margin_bottom(24);
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.append(&group);
        content.append(&confirm_btn);

        // ── Toolbar view ─────────────────────────────────────────────────────

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&content));
        window.set_content(Some(&toolbar_view));

        // ── Wiring ──────────────────────────────────────────────────────────

        let win_cancel = window.clone();
        cancel_btn.connect_clicked(move |_| win_cancel.close());

        let on_confirm: Rc<RefCell<Option<Box<dyn Fn(String)>>>> = Rc::new(RefCell::new(None));

        let fire = {
            let on_confirm = on_confirm.clone();
            let url_row = url_row.clone();
            let window = window.clone();
            Rc::new(move || {
                let url = url_row.text().trim().to_string();
                if url.is_empty() {
                    return;
                }
                if let Some(f) = on_confirm.borrow().as_ref() {
                    f(url);
                }
                window.close();
            })
        };

        let fire_btn = fire.clone();
        confirm_btn.connect_clicked(move |_| fire_btn());

        let fire_entry = fire.clone();
        url_row.connect_entry_activated(move |_| fire_entry());

        Self { window, url_row, on_confirm }
    }

    pub fn set_on_confirm(&self, f: impl Fn(String) + 'static) {
        *self.on_confirm.borrow_mut() = Some(Box::new(f));
    }

    pub fn present(&self) {
        self.url_row.grab_focus();
        self.window.present();
    }
}
