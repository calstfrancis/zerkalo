use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Entry, EventControllerKey, Label, Orientation, PropagationPhase,
    Revealer, RevealerTransitionType, Separator, ToggleButton,
};

use crate::text_search::Options;

/// What asked for a search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindStep {
    /// The find text or an option changed: look from where the search started.
    Typed,
    Next,
    Previous,
}

/// Search-as-you-type waits for this many characters; one or two letters
/// match nearly everywhere, and searching a long document for each of them
/// made the view jump and typing lag. Enter searches for anything.
pub const MIN_TYPED: usize = 3;

const TYPING_PAUSE: std::time::Duration = std::time::Duration::from_millis(150);

type Callback<T> = Rc<RefCell<Option<Box<T>>>>;

#[derive(Clone)]
pub struct FindBar {
    revealer: Revealer,
    pub find_entry: Entry,
    replace_entry: Entry,
    replace_toggle: ToggleButton,
    result_label: Label,
    on_search: Callback<dyn Fn(&str, FindStep)>,
    on_replace_one: Callback<dyn Fn(&str, &str)>,
    on_replace_all: Callback<dyn Fn(&str, &str)>,
    on_reveal_changed: Callback<dyn Fn(bool)>,
    on_all_files: Callback<dyn Fn(&str)>,
    case_btn: ToggleButton,
    word_btn: ToggleButton,
    regex_btn: ToggleButton,
    accent_btn: ToggleButton,
}

fn option_toggle(label: &str, tooltip: &str) -> ToggleButton {
    let b = ToggleButton::with_label(label);
    b.add_css_class("flat");
    b.set_tooltip_text(Some(tooltip));
    b.update_property(&[gtk4::accessible::Property::Label(tooltip)]);
    b
}

fn flat_icon(icon: &str, tooltip: &str) -> Button {
    let b = Button::from_icon_name(icon);
    b.add_css_class("flat");
    b.set_tooltip_text(Some(tooltip));
    b.update_property(&[gtk4::accessible::Property::Label(tooltip)]);
    b
}

/// Has a screen reader say `text` (GTK 4.14 and later; older GTK says
/// nothing). Looked up at run time, since the build supports older GTK.
fn announce(widget: &impl IsA<gtk4::Accessible>, text: &str) {
    #[cfg(unix)]
    {
        use glib::translate::ToGlibPtr;
        use std::sync::OnceLock;
        type Announce = unsafe extern "C" fn(
            *mut gtk4::ffi::GtkAccessible,
            *const std::ffi::c_char,
            std::ffi::c_int,
        );
        static ANNOUNCE: OnceLock<Option<Announce>> = OnceLock::new();
        let f = ANNOUNCE.get_or_init(|| {
            // SAFETY: dlsym with a NUL-terminated name only looks a symbol up.
            let p = unsafe { libc::dlsym(libc::RTLD_DEFAULT, c"gtk_accessible_announce".as_ptr()) };
            // SAFETY: when present, the symbol has exactly this C signature
            // (gtk/gtkaccessible.h, GTK 4.14).
            (!p.is_null()).then(|| unsafe { std::mem::transmute::<*mut libc::c_void, Announce>(p) })
        });
        let (Some(f), Ok(msg)) = (f, std::ffi::CString::new(text)) else {
            return;
        };
        let accessible: &gtk4::Accessible = widget.upcast_ref();
        // SAFETY: a live GtkAccessible and a valid C string; 1 is
        // GTK_ACCESSIBLE_ANNOUNCEMENT_PRIORITY_MEDIUM.
        unsafe { f(accessible.to_glib_none().0, msg.as_ptr(), 1) };
    }
    #[cfg(not(unix))]
    let _ = (widget, text);
}

impl FindBar {
    pub fn new() -> Self {
        let revealer = Revealer::new();
        revealer.set_transition_type(RevealerTransitionType::SlideDown);
        revealer.set_transition_duration(250);
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
        find_entry.set_width_chars(8);
        find_entry.update_property(&[gtk4::accessible::Property::Label("Find")]);

        let case_btn = option_toggle(
            "Aa",
            "Match case (a capital letter in the search turns this on by itself)",
        );
        let accent_btn = option_toggle("á", "Match accents (off, “zizek” finds “Žižek”)");
        let word_btn = option_toggle("\\b", "Whole words only");
        let regex_btn = option_toggle(".*", "Regular expression");

        let prev_btn = flat_icon("go-up-symbolic", "Previous match (Shift+Enter)");
        let next_btn = flat_icon(
            "go-down-symbolic",
            crate::keybindings::shown("Next match (Enter or {find})"),
        );

        let result_label = Label::new(Some(""));
        result_label.add_css_class("dim-label");
        result_label.add_css_class("caption");

        let all_files_btn = Button::with_label("All files");
        all_files_btn.add_css_class("flat");
        all_files_btn.set_tooltip_text(Some(
            "Search every document in the project for this (Ctrl+Shift+F)",
        ));

        let replace_toggle = ToggleButton::new();
        replace_toggle.set_icon_name("edit-find-replace-symbolic");
        replace_toggle.add_css_class("flat");
        let replace_tip = crate::keybindings::shown("Replace ({replace})");
        replace_toggle.set_tooltip_text(Some(replace_tip));
        replace_toggle.update_property(&[gtk4::accessible::Property::Label("Replace")]);

        let replace_entry = Entry::new();
        replace_entry.set_placeholder_text(Some("Replace with…"));
        replace_entry.set_hexpand(true);
        replace_entry.set_width_chars(8);
        replace_entry.update_property(&[gtk4::accessible::Property::Label("Replace with")]);

        let replace_btn = Button::with_label("Replace");
        replace_btn.add_css_class("flat");
        replace_btn.set_tooltip_text(Some("Replace this match and go to the next (Enter)"));

        let replace_all_btn = Button::with_label("All");
        replace_all_btn.add_css_class("flat");
        replace_all_btn.set_tooltip_text(Some("Replace every match in this document"));

        // Its own line under the search, so the bar never needs more width
        // than the editor has.
        let replace_row = GtkBox::new(Orientation::Horizontal, 4);
        replace_row.set_margin_start(8);
        replace_row.set_margin_end(8);
        replace_row.set_margin_bottom(4);
        replace_row.append(&replace_entry);
        replace_row.append(&replace_btn);
        replace_row.append(&replace_all_btn);
        let replace_revealer = Revealer::new();
        replace_revealer.set_transition_type(RevealerTransitionType::SlideDown);
        replace_revealer.set_child(Some(&replace_row));
        replace_toggle
            .bind_property("active", &replace_revealer, "reveal-child")
            .sync_create()
            .build();

        bar.append(&find_entry);
        bar.append(&case_btn);
        bar.append(&accent_btn);
        bar.append(&word_btn);
        bar.append(&regex_btn);
        bar.append(&prev_btn);
        bar.append(&next_btn);
        bar.append(&result_label);
        bar.append(&all_files_btn);
        bar.append(&replace_toggle);
        bar_container.append(&bar);
        bar_container.append(&replace_revealer);
        revealer.set_child(Some(&bar_container));

        let fb = Self {
            revealer: revealer.clone(),
            find_entry: find_entry.clone(),
            replace_entry: replace_entry.clone(),
            replace_toggle: replace_toggle.clone(),
            result_label,
            on_search: Rc::new(RefCell::new(None)),
            on_replace_one: Rc::new(RefCell::new(None)),
            on_replace_all: Rc::new(RefCell::new(None)),
            on_reveal_changed: Rc::new(RefCell::new(None)),
            on_all_files: Rc::new(RefCell::new(None)),
            case_btn: case_btn.clone(),
            word_btn: word_btn.clone(),
            regex_btn: regex_btn.clone(),
            accent_btn: accent_btn.clone(),
        };

        {
            let f = fb.clone();
            next_btn.connect_clicked(move |_| f.search(FindStep::Next));
            let f = fb.clone();
            prev_btn.connect_clicked(move |_| f.search(FindStep::Previous));
            let f = fb.clone();
            find_entry.connect_activate(move |_| f.search(FindStep::Next));
        }
        // An option changed: search again from where the search started.
        for b in [&case_btn, &accent_btn, &word_btn, &regex_btn] {
            let f = fb.clone();
            b.connect_toggled(move |_| f.search(FindStep::Typed));
        }
        // Search as you type once there's enough to go on, after a short
        // pause so a quick typist isn't searched for at every letter. Below
        // that, the highlights clear at once.
        {
            let f = fb.clone();
            let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
            find_entry.connect_changed(move |e| {
                if let Some(id) = pending.borrow_mut().take() {
                    id.remove();
                }
                if e.text().chars().count() < MIN_TYPED {
                    f.search(FindStep::Typed);
                    return;
                }
                let (f, slot) = (f.clone(), pending.clone());
                let id = glib::timeout_add_local_once(TYPING_PAUSE, move || {
                    slot.borrow_mut().take();
                    f.search(FindStep::Typed);
                });
                *pending.borrow_mut() = Some(id);
            });
        }
        {
            let f = fb.clone();
            replace_btn.connect_clicked(move |_| f.replace_one());
            let f = fb.clone();
            replace_entry.connect_activate(move |_| f.replace_one());
            let f = fb.clone();
            replace_all_btn.connect_clicked(move |_| {
                if let Some(cb) = f.on_replace_all.borrow().as_ref() {
                    cb(&f.find_entry.text(), &f.replace_entry.text());
                }
            });
        }
        {
            let f = fb.clone();
            replace_toggle.connect_toggled(move |t| {
                if t.is_active() && f.revealer.reveals_child() {
                    f.replace_entry.grab_focus();
                }
            });
        }
        {
            let f = fb.clone();
            all_files_btn.connect_clicked(move |_| {
                if let Some(cb) = f.on_all_files.borrow().as_ref() {
                    cb(&f.find_entry.text());
                }
            });
        }
        // Escape closes the bar, keeping the search for next time.
        // Shift+Enter in the find box goes back a match.
        {
            let f = fb.clone();
            let kc = EventControllerKey::new();
            kc.set_propagation_phase(PropagationPhase::Capture);
            kc.connect_key_pressed(move |_, key, _, mods| {
                use gtk4::gdk::Key;
                if matches!(key, Key::Return | Key::KP_Enter)
                    && mods.contains(gtk4::gdk::ModifierType::SHIFT_MASK)
                    && f.find_entry.has_focus()
                {
                    f.search(FindStep::Previous);
                    return glib::Propagation::Stop;
                }
                if key == Key::Escape {
                    f.close();
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
            bar_container.add_controller(kc);
        }
        fb
    }

    pub fn widget(&self) -> &Revealer {
        &self.revealer
    }

    pub fn is_open(&self) -> bool {
        self.revealer.reveals_child()
    }

    fn search(&self, step: FindStep) {
        if let Some(f) = self.on_search.borrow().as_ref() {
            f(&self.find_entry.text(), step);
        }
    }

    fn replace_one(&self) {
        if let Some(f) = self.on_replace_one.borrow().as_ref() {
            f(&self.find_entry.text(), &self.replace_entry.text());
        }
    }

    /// Opens the bar. `prefill` (the editor's selection) replaces the search;
    /// otherwise the last search is kept, selected so typing replaces it.
    fn open(&self, prefill: Option<String>) {
        self.revealer.set_reveal_child(true);
        if let Some(f) = self.on_reveal_changed.borrow().as_ref() {
            f(true);
        }
        match prefill {
            Some(text) if text != self.find_entry.text() => self.find_entry.set_text(&text),
            _ => self.search(FindStep::Typed),
        }
        self.find_entry.grab_focus();
        self.find_entry.select_region(0, -1);
    }

    fn close(&self) {
        self.revealer.set_reveal_child(false);
        if let Some(f) = self.on_reveal_changed.borrow().as_ref() {
            f(false);
        }
    }

    pub fn toggle(&self, prefill: Option<String>) {
        if self.is_open() {
            self.close();
        } else {
            self.open(prefill);
        }
    }

    /// Ctrl+F: opens the bar; once it is open, the next match.
    pub fn open_or_next(&self, prefill: Option<String>) {
        if !self.is_open() {
            self.open(prefill);
        } else if self.find_entry.text().is_empty() {
            self.find_entry.grab_focus();
        } else {
            self.search(FindStep::Next);
        }
    }

    /// Ctrl+H: opens the bar with Replace showing, ready to type the
    /// replacement (or the search, when there isn't one yet).
    pub fn open_replace(&self, prefill: Option<String>) {
        if !self.is_open() {
            self.open(prefill);
        }
        self.replace_toggle.set_active(true);
        if self.find_entry.text().is_empty() {
            self.find_entry.grab_focus();
        } else {
            self.replace_entry.grab_focus();
        }
    }

    pub fn set_on_reveal_changed(&self, f: impl Fn(bool) + 'static) {
        *self.on_reveal_changed.borrow_mut() = Some(Box::new(f));
    }

    /// Shows `text` beside the search; `say` also reads it out to a screen
    /// reader, for results the person asked for rather than typing noise.
    pub fn set_result(&self, text: &str, say: bool) {
        self.result_label.set_text(text);
        if say && !text.is_empty() {
            announce(&self.find_entry, text);
        }
    }

    pub fn set_on_search(&self, f: impl Fn(&str, FindStep) + 'static) {
        *self.on_search.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_replace_one(&self, f: impl Fn(&str, &str) + 'static) {
        *self.on_replace_one.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_replace_all(&self, f: impl Fn(&str, &str) + 'static) {
        *self.on_replace_all.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_all_files(&self, f: impl Fn(&str) + 'static) {
        *self.on_all_files.borrow_mut() = Some(Box::new(f));
    }

    pub fn options(&self) -> Options {
        Options {
            case_sensitive: self.case_btn.is_active(),
            whole_word: self.word_btn.is_active(),
            regex: self.regex_btn.is_active(),
            match_accents: self.accent_btn.is_active(),
        }
    }

    pub fn set_entry_error(&self, error: bool) {
        if error {
            self.find_entry.add_css_class("error");
        } else {
            self.find_entry.remove_css_class("error");
        }
    }
}
