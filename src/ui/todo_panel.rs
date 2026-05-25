use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Label, Orientation, Revealer, RevealerTransitionType,
    ScrolledWindow, Separator, TextView, WrapMode,
};

#[derive(Clone)]
pub struct TodoPanel {
    widget: GtkBox,
    #[allow(dead_code)]
    global_buffer: gtk4::TextBuffer,
    file_buffer: gtk4::TextBuffer,
    file_header_label: Label,
    current_file: Rc<RefCell<Option<PathBuf>>>,
    is_loading: Rc<RefCell<bool>>,
}

impl TodoPanel {
    pub fn new() -> Self {
        let root = GtkBox::new(Orientation::Vertical, 0);

        // ── Global TODO header ────────────────────────────────────────────────
        let global_header = GtkBox::new(Orientation::Horizontal, 4);
        global_header.set_margin_start(8);
        global_header.set_margin_end(8);
        global_header.set_margin_top(6);
        global_header.set_margin_bottom(2);

        let toggle_btn = Button::new();
        toggle_btn.set_icon_name("pan-down-symbolic");
        toggle_btn.add_css_class("flat");
        toggle_btn.set_valign(gtk4::Align::Center);
        toggle_btn.set_tooltip_text(Some("Toggle global TODO"));

        let global_title = Label::new(Some("Global TODO"));
        global_title.set_hexpand(true);
        global_title.set_xalign(0.0);
        global_title.add_css_class("heading");

        global_header.append(&toggle_btn);
        global_header.append(&global_title);
        root.append(&global_header);

        // ── Global TODO revealer ──────────────────────────────────────────────
        let global_revealer = Revealer::new();
        global_revealer.set_transition_type(RevealerTransitionType::SlideDown);
        global_revealer.set_reveal_child(true);

        let global_scroll = ScrolledWindow::new();
        global_scroll.set_min_content_height(60);
        global_scroll.set_max_content_height(180);
        global_scroll.set_propagate_natural_height(true);
        global_scroll.set_margin_start(4);
        global_scroll.set_margin_end(4);
        global_scroll.set_margin_bottom(4);

        let global_buffer = gtk4::TextBuffer::new(None);
        let global_view = TextView::with_buffer(&global_buffer);
        global_view.set_wrap_mode(WrapMode::WordChar);
        global_view.set_left_margin(6);
        global_view.set_right_margin(6);
        global_view.set_top_margin(4);
        global_view.set_bottom_margin(4);
        global_scroll.set_child(Some(&global_view));
        global_revealer.set_child(Some(&global_scroll));
        root.append(&global_revealer);
        root.append(&Separator::new(Orientation::Horizontal));

        // Toggle wiring
        let rev_c = global_revealer.clone();
        let btn_c = toggle_btn.clone();
        toggle_btn.connect_clicked(move |_| {
            let revealed = !rev_c.reveals_child();
            rev_c.set_reveal_child(revealed);
            btn_c.set_icon_name(if revealed {
                "pan-down-symbolic"
            } else {
                "pan-end-symbolic"
            });
        });

        // ── Per-file TODO ─────────────────────────────────────────────────────
        let file_header = GtkBox::new(Orientation::Horizontal, 4);
        file_header.set_margin_start(8);
        file_header.set_margin_end(8);
        file_header.set_margin_top(6);
        file_header.set_margin_bottom(2);

        let file_header_label = Label::new(Some("File TODO"));
        file_header_label.set_hexpand(true);
        file_header_label.set_xalign(0.0);
        file_header_label.add_css_class("heading");
        file_header_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        file_header.append(&file_header_label);
        root.append(&file_header);

        let file_scroll = ScrolledWindow::new();
        file_scroll.set_min_content_height(60);
        file_scroll.set_max_content_height(180);
        file_scroll.set_propagate_natural_height(true);
        file_scroll.set_vexpand(true);
        file_scroll.set_margin_start(4);
        file_scroll.set_margin_end(4);
        file_scroll.set_margin_bottom(4);

        let file_buffer = gtk4::TextBuffer::new(None);
        let file_view = TextView::with_buffer(&file_buffer);
        file_view.set_wrap_mode(WrapMode::WordChar);
        file_view.set_left_margin(6);
        file_view.set_right_margin(6);
        file_view.set_top_margin(4);
        file_view.set_bottom_margin(4);
        file_scroll.set_child(Some(&file_view));
        root.append(&file_scroll);

        // ── State ─────────────────────────────────────────────────────────────
        let current_file: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
        let is_loading: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        // ── Auto-save: global buffer ──────────────────────────────────────────
        {
            let buf = global_buffer.clone();
            let loading = is_loading.clone();
            let gen: Rc<RefCell<u64>> = Rc::new(RefCell::new(0));
            let gen2 = gen.clone();
            global_buffer.connect_changed(move |_| {
                if *loading.borrow() {
                    return;
                }
                *gen2.borrow_mut() += 1;
                let my_gen = *gen2.borrow();
                let buf2 = buf.clone();
                let gen3 = gen2.clone();
                glib::timeout_add_local(Duration::from_millis(800), move || {
                    if *gen3.borrow() == my_gen {
                        let start = buf2.start_iter();
                        let end = buf2.end_iter();
                        let text = buf2.text(&start, &end, false);
                        let path = global_todo_path();
                        if let Some(dir) = path.parent() {
                            let _ = std::fs::create_dir_all(dir);
                        }
                        let _ = std::fs::write(&path, text.as_str());
                    }
                    glib::ControlFlow::Break
                });
            });
        }

        // ── Auto-save: file buffer ────────────────────────────────────────────
        {
            let buf = file_buffer.clone();
            let loading = is_loading.clone();
            let cf = current_file.clone();
            let gen: Rc<RefCell<u64>> = Rc::new(RefCell::new(0));
            let gen2 = gen.clone();
            file_buffer.connect_changed(move |_| {
                if *loading.borrow() {
                    return;
                }
                *gen2.borrow_mut() += 1;
                let my_gen = *gen2.borrow();
                let buf2 = buf.clone();
                let gen3 = gen2.clone();
                let cf2 = cf.clone();
                glib::timeout_add_local(Duration::from_millis(800), move || {
                    if *gen3.borrow() == my_gen {
                        if let Some(todo_path) = cf2.borrow().as_ref().map(todo_path_for) {
                            let start = buf2.start_iter();
                            let end = buf2.end_iter();
                            let text = buf2.text(&start, &end, false);
                            let _ = std::fs::write(&todo_path, text.as_str());
                        }
                    }
                    glib::ControlFlow::Break
                });
            });
        }

        // Load global TODO initial content
        {
            let path = global_todo_path();
            if let Ok(content) = std::fs::read_to_string(&path) {
                *is_loading.borrow_mut() = true;
                global_buffer.set_text(&content);
                *is_loading.borrow_mut() = false;
            }
        }

        Self {
            widget: root,
            global_buffer,
            file_buffer,
            file_header_label,
            current_file,
            is_loading,
        }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.widget
    }

    pub fn set_current_file(&self, path: Option<&PathBuf>) {
        *self.current_file.borrow_mut() = path.cloned();
        *self.is_loading.borrow_mut() = true;
        match path {
            None => {
                self.file_header_label.set_text("File TODO");
                self.file_buffer.set_text("");
            }
            Some(p) => {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                self.file_header_label.set_text(name);
                let content = std::fs::read_to_string(todo_path_for(p)).unwrap_or_default();
                self.file_buffer.set_text(&content);
            }
        }
        *self.is_loading.borrow_mut() = false;
    }
}

fn global_todo_path() -> PathBuf {
    let base = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(base).join(".local/share/zerkalo/global-todo.md")
}

fn todo_path_for(file: &PathBuf) -> PathBuf {
    let name = format!(
        "{}.todo",
        file.file_name().and_then(|n| n.to_str()).unwrap_or("_")
    );
    file.with_file_name(name)
}
