use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Entry, Label, Orientation, Window};

pub struct ProjectDialog {
    window: Window,
    #[allow(dead_code)]
    path_entry: Entry,
    on_chosen: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>,
}

impl ProjectDialog {
    pub fn new(app: &gtk4::Application) -> Self {
        let window = Window::new();
        window.set_application(Some(app));
        window.set_title(Some("Зеркало"));
        window.set_modal(true);
        window.set_resizable(false);
        window.set_default_width(480);

        let vbox = GtkBox::new(Orientation::Vertical, 0);
        vbox.set_margin_top(32);
        vbox.set_margin_bottom(28);
        vbox.set_margin_start(32);
        vbox.set_margin_end(32);

        let title_lbl = Label::new(Some("Welcome to Зеркало"));
        title_lbl.add_css_class("title-1");
        title_lbl.set_halign(Align::Start);
        vbox.append(&title_lbl);

        let sub_lbl = Label::new(Some("Choose a folder for your Typst writing project"));
        sub_lbl.add_css_class("dim-label");
        sub_lbl.set_halign(Align::Start);
        sub_lbl.set_margin_top(6);
        sub_lbl.set_margin_bottom(24);
        vbox.append(&sub_lbl);

        let folder_lbl = Label::new(Some("Project folder"));
        folder_lbl.set_halign(Align::Start);
        folder_lbl.set_margin_bottom(6);
        vbox.append(&folder_lbl);

        let path_row = GtkBox::new(Orientation::Horizontal, 8);
        let default_path = shellexpand::tilde("~/Documents/Zerkalo").into_owned();
        let path_entry = Entry::new();
        path_entry.set_text(&default_path);
        path_entry.set_hexpand(true);
        path_row.append(&path_entry);

        let browse_btn = Button::with_label("Browse…");
        path_row.append(&browse_btn);
        vbox.append(&path_row);

        let error_lbl = Label::new(None);
        error_lbl.add_css_class("error");
        error_lbl.set_halign(Align::Start);
        error_lbl.set_margin_top(8);
        error_lbl.set_visible(false);
        error_lbl.set_wrap(true);
        vbox.append(&error_lbl);

        let btn_row = GtkBox::new(Orientation::Horizontal, 8);
        btn_row.set_halign(Align::End);
        btn_row.set_margin_top(28);

        let cancel_btn = Button::with_label("Cancel");
        let open_btn = Button::with_label("Open Project");
        open_btn.add_css_class("suggested-action");
        btn_row.append(&cancel_btn);
        btn_row.append(&open_btn);
        vbox.append(&btn_row);

        window.set_child(Some(&vbox));

        let on_chosen: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>> = Rc::new(RefCell::new(None));

        // Cancel
        let win_cancel = window.clone();
        cancel_btn.connect_clicked(move |_| win_cancel.close());

        // Browse — open folder chooser
        let win_browse = window.clone();
        let entry_browse = path_entry.clone();
        browse_btn.connect_clicked(move |_| {
            let file_dialog = gtk4::FileDialog::new();
            file_dialog.set_title("Select Project Folder");
            let initial = gtk4::gio::File::for_path(entry_browse.text().as_str());
            file_dialog.set_initial_folder(Some(&initial));
            let entry_cb = entry_browse.clone();
            file_dialog.select_folder(
                Some(&win_browse),
                None::<&gtk4::gio::Cancellable>,
                move |result| {
                    if let Ok(file) = result {
                        if let Some(p) = file.path() {
                            entry_cb.set_text(&p.to_string_lossy());
                        }
                    }
                },
            );
        });

        // Open Project — validate, init, fire callback
        let win_open = window.clone();
        let entry_open = path_entry.clone();
        let err_open = error_lbl.clone();
        let cb_open = on_chosen.clone();
        let do_open = move || {
            let raw = entry_open.text();
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

        // Trigger open from button click
        let do_open_btn = Rc::new(do_open);
        let do_open_entry = do_open_btn.clone();
        open_btn.connect_clicked(move |_| do_open_btn());
        // Also trigger on Enter in the path entry
        path_entry.connect_activate(move |_| do_open_entry());

        Self {
            window,
            path_entry,
            on_chosen,
        }
    }

    pub fn set_on_project_chosen(&self, f: impl Fn(PathBuf) + 'static) {
        *self.on_chosen.borrow_mut() = Some(Box::new(f));
    }

    pub fn present(&self) {
        self.window.present();
    }
}
