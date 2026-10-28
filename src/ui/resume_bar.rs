//! The quiet line above the text when a document opens: when you last wrote in
//! it, and your note to yourself. It goes at the first keystroke, and never
//! asks for anything.

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Label, Orientation};

#[derive(Clone)]
pub struct ResumeBar {
    root: GtkBox,
    label: Label,
}

impl ResumeBar {
    pub fn new() -> Self {
        let root = GtkBox::new(Orientation::Horizontal, 0);
        root.set_visible(false);
        root.add_css_class("resume-bar");
        let label = Label::new(None);
        label.add_css_class("caption");
        label.add_css_class("dim-label");
        label.set_xalign(0.0);
        label.set_hexpand(true);
        label.set_wrap(true);
        label.set_margin_start(12);
        label.set_margin_end(12);
        label.set_margin_top(4);
        label.set_margin_bottom(4);
        label.set_selectable(false);
        root.append(&label);
        Self { root, label }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.root
    }

    pub fn show(&self, line: &str) {
        self.label.set_text(line);
        self.root.set_visible(true);
    }

    pub fn hide(&self) {
        if self.root.is_visible() {
            self.root.set_visible(false);
        }
    }
}

impl Default for ResumeBar {
    fn default() -> Self {
        Self::new()
    }
}
