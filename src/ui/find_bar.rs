use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Entry, EventControllerKey, Label, Orientation, PropagationPhase,
    Revealer, RevealerTransitionType, Separator,
};

#[derive(Clone)]
pub struct FindBar {
    revealer: Revealer,
    pub find_entry: Entry,
    result_label: Label,
    on_search: Rc<RefCell<Option<Box<dyn Fn(&str, bool)>>>>,
    on_replace_one: Rc<RefCell<Option<Box<dyn Fn(&str, &str)>>>>,
    on_replace_all: Rc<RefCell<Option<Box<dyn Fn(&str, &str)>>>>,
    on_reveal_changed: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
}

impl FindBar {
    pub fn new() -> Self {
        let revealer = Revealer::new();
        revealer.set_transition_type(RevealerTransitionType::SlideDown);
        revealer.set_transition_duration(200);
        revealer.set_reveal_child(false);

        let bar_container = GtkBox::new(Orientation::Vertical, 0);
        bar_container.append(&Separator::new(Orientation::Horizontal));

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
        bar.append(&sep);
        bar.append(&replace_entry);
        bar.append(&replace_btn);
        bar.append(&replace_all_btn);
        bar_container.append(&bar);
        revealer.set_child(Some(&bar_container));

        let on_search: Rc<RefCell<Option<Box<dyn Fn(&str, bool)>>>> =
            Rc::new(RefCell::new(None));
        let on_replace_one: Rc<RefCell<Option<Box<dyn Fn(&str, &str)>>>> =
            Rc::new(RefCell::new(None));
        let on_replace_all: Rc<RefCell<Option<Box<dyn Fn(&str, &str)>>>> =
            Rc::new(RefCell::new(None));
        let on_reveal_changed: Rc<RefCell<Option<Box<dyn Fn(bool)>>>> =
            Rc::new(RefCell::new(None));

        {
            let cb = on_search.clone();
            let e = find_entry.clone();
            next_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() { f(&e.text(), true); }
            });
        }
        {
            let cb = on_search.clone();
            let e = find_entry.clone();
            prev_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() { f(&e.text(), false); }
            });
        }
        {
            let cb = on_search.clone();
            find_entry.connect_activate(move |e| {
                if let Some(f) = cb.borrow().as_ref() { f(&e.text(), true); }
            });
        }
        {
            let cb = on_replace_one.clone();
            let fe = find_entry.clone();
            let re = replace_entry.clone();
            replace_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() { f(&fe.text(), &re.text()); }
            });
        }
        {
            let cb = on_replace_all.clone();
            let fe = find_entry.clone();
            let re = replace_entry.clone();
            replace_all_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() { f(&fe.text(), &re.text()); }
            });
        }
        // Escape: clear entry and hide bar
        {
            let entry = find_entry.clone();
            let rev_c = revealer.clone();
            let orc = on_reveal_changed.clone();
            let kc = EventControllerKey::new();
            kc.set_propagation_phase(PropagationPhase::Capture);
            kc.connect_key_pressed(move |_, key, _, _| {
                if key == gtk4::gdk::Key::Escape {
                    entry.set_text("");
                    rev_c.set_reveal_child(false);
                    if let Some(f) = orc.borrow().as_ref() { f(false); }
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            });
            find_entry.add_controller(kc);
        }

        Self {
            revealer,
            find_entry,
            result_label,
            on_search,
            on_replace_one,
            on_replace_all,
            on_reveal_changed,
        }
    }

    pub fn widget(&self) -> &Revealer {
        &self.revealer
    }

    pub fn toggle(&self) {
        let reveal = !self.revealer.reveals_child();
        self.revealer.set_reveal_child(reveal);
        if reveal {
            self.find_entry.grab_focus();
        }
        if let Some(f) = self.on_reveal_changed.borrow().as_ref() { f(reveal); }
    }

    pub fn set_on_reveal_changed(&self, f: impl Fn(bool) + 'static) {
        *self.on_reveal_changed.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_result(&self, text: &str) {
        self.result_label.set_text(text);
    }

    pub fn set_on_search(&self, f: impl Fn(&str, bool) + 'static) {
        *self.on_search.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_replace_one(&self, f: impl Fn(&str, &str) + 'static) {
        *self.on_replace_one.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_replace_all(&self, f: impl Fn(&str, &str) + 'static) {
        *self.on_replace_all.borrow_mut() = Some(Box::new(f));
    }
}
