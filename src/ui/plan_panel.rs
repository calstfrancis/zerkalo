use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Label, Orientation, ScrolledWindow, TextView, WrapMode,
};

#[derive(Clone)]
pub struct PlanPanel {
    widget: GtkBox,
    text_view: TextView,
    save_path: Rc<RefCell<Option<PathBuf>>>,
    is_loading: Rc<RefCell<bool>>,
    header_label: Label,
    work_dir: Rc<PathBuf>,
}

impl PlanPanel {
    pub fn new(work_dir: PathBuf) -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 0);
        widget.set_vexpand(true);

        let header_box = GtkBox::new(Orientation::Horizontal, 0);
        header_box.set_margin_start(8);
        header_box.set_margin_end(8);
        header_box.set_margin_top(6);
        header_box.set_margin_bottom(4);

        let header_label = Label::new(Some("Project Notes"));
        header_label.set_hexpand(true);
        header_label.set_xalign(0.0);
        header_label.add_css_class("heading");
        header_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        header_box.append(&header_label);
        widget.append(&header_box);

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_hexpand(true);
        scroll.set_margin_start(8);
        scroll.set_margin_end(8);
        scroll.set_margin_bottom(8);
        scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);

        let text_view = TextView::new();
        text_view.set_vexpand(true);
        text_view.set_wrap_mode(WrapMode::Word);
        text_view.set_left_margin(4);
        text_view.set_right_margin(4);
        text_view.set_top_margin(4);
        text_view.set_bottom_margin(4);
        text_view.add_css_class("monospace");
        scroll.set_child(Some(&text_view));
        widget.append(&scroll);

        let save_path: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
        let is_loading: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let work_dir = Rc::new(work_dir);

        let panel = Self { widget, text_view, save_path, is_loading, header_label, work_dir };

        // Load project notes immediately
        let project_plan = panel.work_dir.join("project.plan");
        let text = std::fs::read_to_string(&project_plan).unwrap_or_default();
        *panel.is_loading.borrow_mut() = true;
        panel.text_view.buffer().set_text(&text);
        *panel.is_loading.borrow_mut() = false;
        *panel.save_path.borrow_mut() = Some(project_plan);

        {
            let p = panel.clone();
            panel.text_view.buffer().connect_changed(move |buf| {
                if *p.is_loading.borrow() { return; }
                let text = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
                let path_opt = p.save_path.borrow().clone();
                if let Some(path) = path_opt {
                    glib::timeout_add_local_once(Duration::from_millis(400), move || {
                        let _ = std::fs::write(&path, &text);
                    });
                }
            });
        }

        panel
    }

    pub fn widget(&self) -> &GtkBox { &self.widget }

    pub fn set_current_file(&self, path: Option<&PathBuf>) {
        match path {
            None => {
                self.header_label.set_text("Project Notes");
                let project_plan = self.work_dir.join("project.plan");
                let text = std::fs::read_to_string(&project_plan).unwrap_or_default();
                *self.is_loading.borrow_mut() = true;
                self.text_view.buffer().set_text(&text);
                *self.is_loading.borrow_mut() = false;
                *self.save_path.borrow_mut() = Some(project_plan);
            }
            Some(p) => {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                self.header_label.set_text(name);
                let plan_path = plan_path_for(p);
                let text = std::fs::read_to_string(&plan_path).unwrap_or_default();
                *self.is_loading.borrow_mut() = true;
                self.text_view.buffer().set_text(&text);
                *self.is_loading.borrow_mut() = false;
                *self.save_path.borrow_mut() = Some(plan_path);
            }
        }
    }
}

fn plan_path_for(file: &PathBuf) -> PathBuf {
    let name = format!(
        "{}.plan",
        file.file_name().and_then(|n| n.to_str()).unwrap_or("_")
    );
    file.with_file_name(name)
}
