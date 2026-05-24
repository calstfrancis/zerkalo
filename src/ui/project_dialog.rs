use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation};
use libadwaita as adw;

pub struct ProjectDialog {
    window: adw::Window,
    on_chosen: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>,
}

impl ProjectDialog {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::Window::new();
        window.set_application(Some(app));
        window.set_title(Some("Open Project"));
        window.set_modal(true);
        window.set_resizable(false);
        window.set_default_width(480);

        // ── Header bar ──────────────────────────────────────────────────────

        let header = adw::HeaderBar::new();
        header.set_show_end_title_buttons(false);

        // ── Path group ──────────────────────────────────────────────────────

        let group = adw::PreferencesGroup::new();
        group.set_title("Project folder");
        group.set_description(Some("Choose a folder for your Typst writing project"));

        let default_path = shellexpand::tilde("~/Documents/Zerkalo").into_owned();
        let path_row = adw::EntryRow::new();
        path_row.set_title("Path");
        path_row.set_text(&default_path);

        let browse_btn = Button::from_icon_name("document-open-symbolic");
        browse_btn.set_valign(Align::Center);
        browse_btn.add_css_class("flat");
        path_row.add_suffix(&browse_btn);
        group.add(&path_row);

        // ── Error label & open button ────────────────────────────────────────

        let error_lbl = Label::new(None);
        error_lbl.add_css_class("error");
        error_lbl.set_halign(Align::Center);
        error_lbl.set_margin_top(4);
        error_lbl.set_visible(false);
        error_lbl.set_wrap(true);

        let open_btn = Button::with_label("Open Project");
        open_btn.add_css_class("suggested-action");
        open_btn.add_css_class("pill");
        open_btn.set_halign(Align::Center);
        open_btn.set_margin_top(16);

        // ── Page layout ──────────────────────────────────────────────────────

        let content = GtkBox::new(Orientation::Vertical, 0);
        content.set_margin_top(16);
        content.set_margin_bottom(32);
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.append(&group);
        content.append(&error_lbl);
        content.append(&open_btn);

        // ── Status page ──────────────────────────────────────────────────────

        let status = adw::StatusPage::new();
        status.set_icon_name(Some("document-edit-symbolic"));
        status.set_title("Welcome to Зеркало");
        status.set_child(Some(&content));

        // ── Toolbar view ─────────────────────────────────────────────────────

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&status));
        window.set_content(Some(&toolbar_view));

        let on_chosen: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>> = Rc::new(RefCell::new(None));

        // Browse
        let win_browse = window.clone();
        let row_browse = path_row.clone();
        browse_btn.connect_clicked(move |_| {
            let file_dialog = gtk4::FileDialog::new();
            file_dialog.set_title("Select Project Folder");
            let initial = gtk4::gio::File::for_path(row_browse.text().as_str());
            file_dialog.set_initial_folder(Some(&initial));
            let row_cb = row_browse.clone();
            file_dialog.select_folder(
                Some(&win_browse),
                None::<&gtk4::gio::Cancellable>,
                move |result| {
                    if let Ok(file) = result {
                        if let Some(p) = file.path() {
                            row_cb.set_text(&p.to_string_lossy());
                        }
                    }
                },
            );
        });

        // Open project
        let win_open = window.clone();
        let row_open = path_row.clone();
        let err_open = error_lbl.clone();
        let cb_open = on_chosen.clone();
        let do_open = move || {
            let raw = row_open.text();
            let raw = raw.trim();
            if raw.is_empty() {
                err_open.set_label("Please enter a project folder path.");
                err_open.set_visible(true);
                return;
            }
            let path = PathBuf::from(shellexpand::tilde(raw).into_owned());
            match crate::project::init_project(&path) {
                Ok(()) => {
                    if let Some(f) = cb_open.borrow().as_ref() {
                        f(path);
                    }
                    win_open.close();
                }
                Err(e) => {
                    err_open.set_label(&e.to_string());
                    err_open.set_visible(true);
                }
            }
        };

        let do_open = Rc::new(do_open);
        let do_open_btn = do_open.clone();
        let do_open_entry = do_open.clone();
        open_btn.connect_clicked(move |_| do_open_btn());
        path_row.connect_entry_activated(move |_| do_open_entry());

        Self { window, on_chosen }
    }

    pub fn set_on_project_chosen(&self, f: impl Fn(PathBuf) + 'static) {
        *self.on_chosen.borrow_mut() = Some(Box::new(f));
    }

    pub fn present(&self) {
        self.window.present();
    }
}
