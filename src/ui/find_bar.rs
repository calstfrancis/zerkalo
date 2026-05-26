use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Entry, EventControllerKey, Label, Orientation, PropagationPhase,
    Separator, ToggleButton,
};

#[derive(Clone)]
pub struct FindBar {
    widget: GtkBox,
    pub find_entry: Entry,
    result_label: Label,
    on_search: Rc<RefCell<Option<Box<dyn Fn(&str, bool, bool)>>>>,
    on_replace_one: Rc<RefCell<Option<Box<dyn Fn(&str, &str, bool)>>>>,
    on_replace_all: Rc<RefCell<Option<Box<dyn Fn(&str, &str, bool)>>>>,
}

impl FindBar {
    pub fn new() -> Self {
        let bar = GtkBox::new(Orientation::Horizontal, 4);
        bar.set_margin_start(8);
        bar.set_margin_end(8);
        bar.set_margin_top(4);
        bar.set_margin_bottom(4);

        let find_entry = Entry::new();
        find_entry.set_placeholder_text(Some("Find…"));
        find_entry.set_hexpand(true);

        let prev_btn = Button::from_icon_name("go-up-symbolic");
        prev_btn.add_css_class("flat");
        prev_btn.set_tooltip_text(Some("Previous match"));

        let next_btn = Button::from_icon_name("go-down-symbolic");
        next_btn.add_css_class("flat");
        next_btn.set_tooltip_text(Some("Next match"));

        let result_label = Label::new(Some(""));
        result_label.add_css_class("dim-label");
        result_label.set_width_chars(12);

        let whole_word_btn = ToggleButton::new();
        whole_word_btn.set_label("W");
        whole_word_btn.add_css_class("flat");
        whole_word_btn.set_tooltip_text(Some("Match whole words only"));

        let sep = Separator::new(Orientation::Vertical);

        let replace_entry = Entry::new();
        replace_entry.set_placeholder_text(Some("Replace…"));
        replace_entry.set_hexpand(true);

        let replace_btn = Button::with_label("Replace");
        replace_btn.add_css_class("flat");

        let replace_all_btn = Button::with_label("All");
        replace_all_btn.add_css_class("flat");
        replace_all_btn.set_tooltip_text(Some("Replace all occurrences"));

        bar.append(&find_entry);
        bar.append(&prev_btn);
        bar.append(&next_btn);
        bar.append(&result_label);
        bar.append(&whole_word_btn);
        bar.append(&sep);
        bar.append(&replace_entry);
        bar.append(&replace_btn);
        bar.append(&replace_all_btn);

        let whole_word: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let on_search: Rc<RefCell<Option<Box<dyn Fn(&str, bool, bool)>>>> =
            Rc::new(RefCell::new(None));
        let on_replace_one: Rc<RefCell<Option<Box<dyn Fn(&str, &str, bool)>>>> =
            Rc::new(RefCell::new(None));
        let on_replace_all: Rc<RefCell<Option<Box<dyn Fn(&str, &str, bool)>>>> =
            Rc::new(RefCell::new(None));

        // Whole-word toggle
        {
            let ww = whole_word.clone();
            whole_word_btn.connect_toggled(move |btn| {
                *ww.borrow_mut() = btn.is_active();
            });
        }

        {
            let cb = on_search.clone();
            let e = find_entry.clone();
            let ww = whole_word.clone();
            next_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() { f(&e.text(), true, *ww.borrow()); }
            });
        }
        {
            let cb = on_search.clone();
            let e = find_entry.clone();
            let ww = whole_word.clone();
            prev_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() { f(&e.text(), false, *ww.borrow()); }
            });
        }
        {
            let cb = on_search.clone();
            let ww = whole_word.clone();
            find_entry.connect_activate(move |e| {
                if let Some(f) = cb.borrow().as_ref() { f(&e.text(), true, *ww.borrow()); }
            });
        }
        {
            let cb = on_replace_one.clone();
            let fe = find_entry.clone();
            let re = replace_entry.clone();
            let ww = whole_word.clone();
            replace_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() { f(&fe.text(), &re.text(), *ww.borrow()); }
            });
        }
        {
            let cb = on_replace_all.clone();
            let fe = find_entry.clone();
            let re = replace_entry.clone();
            let ww = whole_word.clone();
            replace_all_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() { f(&fe.text(), &re.text(), *ww.borrow()); }
            });
        }
        // Escape: clear entry
        {
            let entry = find_entry.clone();
            let kc = EventControllerKey::new();
            kc.set_propagation_phase(PropagationPhase::Capture);
            kc.connect_key_pressed(move |_, key, _, _| {
                if key == gtk4::gdk::Key::Escape {
                    entry.set_text("");
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            });
            find_entry.add_controller(kc);
        }

        Self {
            widget: bar,
            find_entry,
            result_label,
            on_search,
            on_replace_one,
            on_replace_all,
        }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.widget
    }

    pub fn show(&self) {
        self.find_entry.grab_focus();
    }

    pub fn set_result(&self, text: &str) {
        self.result_label.set_text(text);
    }

    pub fn set_on_search(&self, f: impl Fn(&str, bool, bool) + 'static) {
        *self.on_search.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_replace_one(&self, f: impl Fn(&str, &str, bool) + 'static) {
        *self.on_replace_one.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_replace_all(&self, f: impl Fn(&str, &str, bool) + 'static) {
        *self.on_replace_all.borrow_mut() = Some(Box::new(f));
    }
}
