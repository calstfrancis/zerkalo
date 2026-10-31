//! The writing history window: the record of how a project was written, to
//! read on screen and save as a file for a teacher. The text itself is built in
//! `writing_history`; this shows it, lets the person pick the whole project or
//! one document, and saves it.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

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
    let win = adw::Window::builder()
        .title("Writing history")
        .default_width(860)
        .default_height(720)
        .width_request(420)
        .transient_for(parent)
        .build();

    let header = adw::HeaderBar::new();
    header.add_css_class("fond-chrome");

    let whole = gtk4::ToggleButton::with_label("Whole project");
    let single = gtk4::ToggleButton::with_label("This document");
    single.set_group(Some(&whole));
    whole.set_active(true);
    let scope = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    scope.add_css_class("linked");
    scope.append(&whole);
    scope.append(&single);
    scope.set_visible(active_file.is_some());
    header.set_title_widget(Some(&scope));

    let save_btn = gtk4::Button::with_label("Save as file…");
    save_btn.add_css_class("suggested-action");
    save_btn.set_sensitive(false);
    save_btn.set_tooltip_text(Some(
        "Save this record as a text file you can hand to a teacher or attach to your work",
    ));
    let copy_btn = gtk4::Button::from_icon_name("edit-copy-symbolic");
    copy_btn.set_sensitive(false);
    copy_btn.set_tooltip_text(Some("Copy the whole record"));
    copy_btn.update_property(&[gtk4::accessible::Property::Label("Copy the whole record")]);
    header.pack_end(&save_btn);
    header.pack_end(&copy_btn);

    let summary = gtk4::Label::new(None);
    summary.set_xalign(0.0);
    summary.set_wrap(true);
    summary.set_width_chars(30);
    summary.set_margin_start(18);
    summary.set_margin_end(18);
    summary.set_margin_top(10);
    summary.set_margin_bottom(10);

    let notice = adw::Banner::new("");

    let buffer = gtk4::TextBuffer::new(None);
    let colors = super::theme::diff_colors();
    buffer.create_tag(
        Some("added"),
        &[
            ("foreground", &colors.added_fg),
            ("paragraph-background", &colors.added_bg),
        ],
    );
    buffer.create_tag(
        Some("removed"),
        &[
            ("foreground", &colors.removed_fg),
            ("paragraph-background", &colors.removed_bg),
        ],
    );
    buffer.create_tag(
        Some("version"),
        &[
            ("weight", &700),
            ("scale", &1.15),
            ("pixels-above-lines", &14),
        ],
    );
    buffer.create_tag(
        Some("file"),
        &[("weight", &700), ("pixels-above-lines", &8)],
    );
    buffer.create_tag(Some("quiet"), &[("foreground", &"gray")]);

    let view = gtk4::TextView::with_buffer(&buffer);
    view.set_editable(false);
    view.set_cursor_visible(false);
    view.set_monospace(true);
    view.set_wrap_mode(gtk4::WrapMode::WordChar);
    view.set_left_margin(18);
    view.set_right_margin(18);
    view.set_top_margin(8);
    view.set_bottom_margin(18);
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_child(Some(&view));

    let spinner = gtk4::Spinner::new();
    spinner.start();
    spinner.set_halign(gtk4::Align::Center);
    spinner.set_valign(gtk4::Align::Center);
    spinner.set_width_request(36);
    spinner.set_height_request(36);
    let empty = adw::StatusPage::new();
    empty.set_icon_name(Some("document-open-recent-symbolic"));
    empty.set_title("No history yet");

    let stack = gtk4::Stack::new();
    stack.add_named(&spinner, Some("loading"));
    stack.add_named(&scroll, Some("text"));
    stack.add_named(&empty, Some("empty"));
    stack.set_vexpand(true);

    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    body.append(&summary);
    body.append(&notice);
    body.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    body.append(&stack);

    let toolbar = adw::ToolbarView::new();
    toolbar.set_top_bar_style(adw::ToolbarStyle::RaisedBorder);
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&body));
    win.set_content(Some(&toolbar));

    let report: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let generation = Rc::new(Cell::new(0u32));
    let root = project_root.to_path_buf();

    let load: Rc<dyn Fn()> =
        {
            let (stack, summary, notice, buffer) = (
                stack.clone(),
                summary.clone(),
                notice.clone(),
                buffer.clone(),
            );
            let (save_btn, copy_btn, empty) = (save_btn.clone(), copy_btn.clone(), empty.clone());
            let (report, generation, root) = (report.clone(), generation.clone(), root.clone());
            let (single, active_file) = (single.clone(), active_file.clone());
            Rc::new(move || {
                generation.set(generation.get() + 1);
                let mine = generation.get();
                stack.set_visible_child_name("loading");
                save_btn.set_sensitive(false);
                copy_btn.set_sensitive(false);
                summary.set_text("");
                notice.set_revealed(false);
                let only = if single.is_active() {
                    active_file.clone()
                } else {
                    None
                };
                let (root, today) = (
                    root.clone(),
                    chrono::Local::now().format("%-d %B %Y").to_string(),
                );
                let (stack, summary, notice, buffer) = (
                    stack.clone(),
                    summary.clone(),
                    notice.clone(),
                    buffer.clone(),
                );
                let (save_btn, copy_btn, empty) =
                    (save_btn.clone(), copy_btn.clone(), empty.clone());
                let (report, generation) = (report.clone(), generation.clone());
                gtk4::glib::spawn_future_local(async move {
                    let built = gtk4::gio::spawn_blocking(move || {
                        crate::writing_history::build(&root, only.as_deref(), &today)
                    })
                    .await;
                    if generation.get() != mine {
                        return;
                    }
                    match built {
                        Ok(Ok(h)) => {
                            fill(&buffer, &h.text);
                            *report.borrow_mut() = h.text.clone();
                            summary.set_text(&format!(
                            "{} saved {} over {} {} · {} lines written, {} taken out or rewritten",
                            h.versions,
                            if h.versions == 1 { "version" } else { "versions" },
                            h.days,
                            if h.days == 1 { "day" } else { "days" },
                            h.added,
                            h.removed
                        ));
                            if h.unsaved_files > 0 {
                                notice.set_title(
                                "Your latest changes haven't been backed up yet, so they aren't in \
                                 this record. Back up first to include them.",
                            );
                                notice.set_revealed(true);
                            }
                            stack.set_visible_child_name("text");
                            save_btn.set_sensitive(true);
                            copy_btn.set_sensitive(true);
                        }
                        Ok(Err(why)) => {
                            empty.set_description(Some(&why));
                            stack.set_visible_child_name("empty");
                        }
                        Err(_) => {
                            empty.set_description(Some("Couldn't read the history."));
                            stack.set_visible_child_name("empty");
                        }
                    }
                });
            })
        };

    {
        let load = load.clone();
        whole.connect_toggled(move |_| load());
    }
    {
        let report = report.clone();
        let win = win.clone();
        copy_btn.connect_clicked(move |b| {
            b.display().clipboard().set_text(&report.borrow());
            let _ = &win;
        });
    }
    {
        let (report, win, toasts) = (report.clone(), win.clone(), toasts.clone());
        let (root, active_file, single) = (root.clone(), active_file.clone(), single.clone());
        save_btn.connect_clicked(move |_| {
            let stem = if single.is_active() {
                active_file.as_deref()
            } else {
                None
            }
            .and_then(|f| f.file_stem())
            .or_else(|| root.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "project".into());
            let dialog = gtk4::FileDialog::builder()
                .title("Save your writing history")
                .modal(true)
                .initial_name(format!("{stem}-writing-history.txt"))
                .build();
            let (report, toasts) = (report.clone(), toasts.clone());
            dialog.save(Some(&win), None::<&gtk4::gio::Cancellable>, move |result| {
                let Ok(file) = result else { return };
                let Some(out) = file.path() else { return };
                let message = match crate::error::atomic_write(&out, report.borrow().as_bytes()) {
                    Ok(()) => "Saved your writing history.".to_string(),
                    Err(e) => format!(
                        "Couldn't save the file there — {}.",
                        crate::error::io_reason(&e)
                    ),
                };
                toasts.say(adw::Toast::new(&message));
            });
        });
    }

    win.present();
    load();
}

/// The record with its changes coloured: added lines green, removed red,
/// each version's heading and each file's name bold, the rest plain.
fn fill(buffer: &gtk4::TextBuffer, text: &str) {
    buffer.set_text("");
    let mut iter = buffer.start_iter();
    let table = buffer.tag_table();
    for line in text.lines() {
        let tag = if line.starts_with("Version ") && line.contains(" of ") {
            "version"
        } else if line.starts_with("diff --git") {
            "file"
        } else if line.starts_with("+++") || line.starts_with("---") {
            "quiet"
        } else if line.starts_with("@@") || line.starts_with("index ") || line.starts_with("====") {
            "quiet"
        } else if line.starts_with('+') {
            "added"
        } else if line.starts_with('-') {
            "removed"
        } else {
            ""
        };
        let with_newline = format!("{line}\n");
        match table.lookup(tag) {
            Some(t) => buffer.insert_with_tags(&mut iter, &with_newline, &[&t]),
            None => buffer.insert(&mut iter, &with_newline),
        }
    }
}
