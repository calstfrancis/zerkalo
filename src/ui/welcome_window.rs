use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation, ScrolledWindow, Separator};
use libadwaita as adw;

const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const RELEASE_NAME: &str = "True Anchor";

pub struct WelcomeWindow {
    window: adw::Window,
    on_dismissed: Rc<RefCell<Option<Box<dyn Fn()>>>>,
}

impl WelcomeWindow {
    /// True when no marker exists — the very first launch.
    pub fn is_first_run() -> bool {
        !crate::config::zerkalo_data_dir()
            .join(".welcome_version")
            .exists()
    }

    pub fn new(parent: &impl IsA<gtk4::Window>, is_first_run: bool) -> Self {
        let on_dismissed: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));

        let title = if is_first_run {
            "Welcome to Zerkalo"
        } else {
            "What's New"
        };
        let window = adw::Window::builder()
            .title(title)
            .transient_for(parent)
            .modal(true)
            .default_width(480)
            .default_height(580)
            .build();

        let header = adw::HeaderBar::new();
        header.add_css_class("fond-chrome");

        let outer = GtkBox::new(Orientation::Vertical, 0);

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);

        let body = GtkBox::new(Orientation::Vertical, 12);
        body.set_margin_start(24);
        body.set_margin_end(24);
        body.set_margin_top(20);
        body.set_margin_bottom(20);

        // Clamp caps the natural-width request so labels wrap within the window width
        // rather than forcing the window to expand to fit unwrapped text.
        let clamp = adw::Clamp::new();
        clamp.set_maximum_size(460);
        clamp.set_child(Some(&body));

        let app_title = Label::new(Some("Zerkalo"));
        app_title.add_css_class("title-1");
        app_title.set_halign(Align::Center);

        let sub_lbl = Label::new(Some(&format!("Version {VERSION} \"{RELEASE_NAME}\"")));
        sub_lbl.add_css_class("dim-label");
        sub_lbl.set_halign(Align::Center);
        sub_lbl.set_margin_bottom(4);

        body.append(&app_title);
        body.append(&sub_lbl);
        body.append(&Separator::new(Orientation::Horizontal));

        if is_first_run {
            body.append(&section_label("How Zerkalo Works"));
            let intro = Label::new(Some(
                "Zerkalo is a Typst editor with a live preview pane. You write in Typst markup \
                 on the left and see the formatted PDF on the right. Your document is saved and \
                 compiled automatically as you type.",
            ));
            intro.set_wrap(true);
            intro.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
            intro.set_xalign(0.0);
            intro.set_hexpand(true);
            intro.set_halign(Align::Fill);
            body.append(&intro);

            // ASCII layout diagram
            let diagram = Label::new(Some(
                "┌─────────────────┬──────────────────┐\n\
                 │  File tree  ≡   │  Live preview    │\n\
                 ├─────────────────┤                  │\n\
                 │                 │  ┌────────────┐  │\n\
                 │   Editor        │  │  PDF page  │  │\n\
                 │   (light markup)│  │            │  │\n\
                 │                 │  └────────────┘  │\n\
                 ├─────────────────┴──────────────────┤\n\
                 │  Status bar  (word count, cursor)   │\n\
                 └────────────────────────────────────┘",
            ));
            diagram.add_css_class("monospace");
            diagram.add_css_class("dim-label");
            diagram.add_css_class("caption");
            diagram.set_xalign(0.0);
            diagram.set_margin_top(4);
            diagram.set_margin_bottom(4);
            body.append(&diagram);

            let diagram_note = Label::new(Some(
                "You type short instructions like *bold* — the preview on the right shows \
                 the real formatting.",
            ));
            diagram_note.add_css_class("dim-label");
            diagram_note.add_css_class("caption");
            diagram_note.set_wrap(true);
            diagram_note.set_xalign(0.0);
            diagram_note.set_margin_bottom(4);
            body.append(&diagram_note);

            body.append(&Separator::new(Orientation::Horizontal));
            body.append(&section_label("Getting Started"));
            for item in [
                "Open or create a .typ file from the title-bar dropdown",
                "Press Ctrl+S to save — the preview on the right updates immediately",
                "Use the formatting bar above the editor for Bold, Italic, and Headings",
                "Use Change Document Style, under Document Tools in the ≡ menu, to change title, author, and style",
                "The Outline panel on the left shows your document structure — click to navigate",
            ] {
                body.append(&bullet_row(item));
            }

            body.append(&Separator::new(Orientation::Horizontal));
            body.append(&section_label("When You're Ready"));
            for item in [
                "Type @ to insert a citation, once you've added a bibliography from the Citations panel",
                "Type # to see suggestions for tables, figures, and other building blocks",
                "Zerkalo hides the technical setup lines at the top of the file — change them from Change Document Style (≡ → Document Tools), not by scrolling up",
                "Turn Simple Mode off with the SIMPLE button in the header, beside Library, if you ever want to see that setup section directly",
            ] {
                body.append(&bullet_row(item));
            }
        } else {
            body.append(&section_label(&format!("What's New in {VERSION}")));
            for item in [
                "Spell check no longer flags contractions and possessives as misspelled — \"doesn't\" was being checked as \"doesn\" (never a real word) because the apostrophe split it in two. Whole words with an apostrophe are now checked as themselves.",
                "Search, click-to-jump from the preview, and heading navigation now actually scroll the editor to show the result. The cursor and match count were always updating correctly — the view itself just wasn't scrolling to follow.",
            ] {
                body.append(&bullet_row(item));
            }
        }

        body.append(&Separator::new(Orientation::Horizontal));
        body.append(&section_label("Keyboard Shortcuts"));
        for (key, desc) in [
            ("Ctrl+S", "Save and snapshot"),
            ("Ctrl+Shift+P", "Compile and preview"),
            ("Ctrl+K", "Command palette"),
            ("Ctrl+F", "Find in document"),
            ("Ctrl+Tab", "Next open file"),
            ("Ctrl+Shift+G", "Save a version & back up"),
        ] {
            body.append(&shortcut_row(key, desc));
        }

        scroll.set_child(Some(&clamp));
        outer.append(&scroll);
        outer.append(&Separator::new(Orientation::Horizontal));

        let footer = GtkBox::new(Orientation::Horizontal, 0);
        footer.set_margin_start(16);
        footer.set_margin_end(16);
        footer.set_margin_top(8);
        footer.set_margin_bottom(12);
        let spacer = GtkBox::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        footer.append(&spacer);
        let btn_label = if is_first_run { "Get Started" } else { "Close" };
        let ok_btn = Button::with_label(btn_label);
        ok_btn.add_css_class("suggested-action");
        ok_btn.add_css_class("pill");
        footer.append(&ok_btn);
        outer.append(&footer);

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.set_top_bar_style(adw::ToolbarStyle::RaisedBorder);
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&outer));
        window.set_content(Some(&toolbar_view));

        let win_c = window.clone();
        let cb = on_dismissed.clone();
        ok_btn.connect_clicked(move |_| {
            win_c.close();
            if let Some(f) = cb.borrow().as_ref() {
                f();
            }
        });

        Self {
            window,
            on_dismissed,
        }
    }

    /// Called after "Get Started" is clicked (after the window closes).
    pub fn set_on_dismissed(&self, f: impl Fn() + 'static) {
        *self.on_dismissed.borrow_mut() = Some(Box::new(f));
    }

    pub fn present(&self) {
        self.window.present();
    }

    /// Returns true when the welcome window should be shown (new install or version upgrade).
    pub fn should_show() -> bool {
        let marker = crate::config::zerkalo_data_dir().join(".welcome_version");
        std::fs::read_to_string(&marker)
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
            != VERSION
    }

    /// Record that the welcome window has been shown for this version.
    pub fn mark_shown() {
        let marker = crate::config::zerkalo_data_dir().join(".welcome_version");
        if let Some(parent) = marker.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(marker, VERSION);
    }
}

fn section_label(text: &str) -> Label {
    let lbl = Label::new(Some(text));
    lbl.set_xalign(0.0);
    lbl.add_css_class("heading");
    lbl
}

fn bullet_row(text: &str) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 8);
    row.set_margin_start(4);
    row.set_hexpand(true);
    let dot = Label::new(Some("•"));
    dot.set_valign(Align::Start);
    dot.add_css_class("dim-label");
    let lbl = Label::new(Some(text));
    lbl.set_xalign(0.0);
    lbl.set_wrap(true);
    lbl.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    lbl.set_hexpand(true);
    lbl.set_halign(Align::Fill);
    row.append(&dot);
    row.append(&lbl);
    row
}

fn shortcut_row(key: &str, desc: &str) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 8);
    row.set_margin_start(4);
    row.set_hexpand(true);
    let key_lbl = Label::new(Some(key));
    key_lbl.set_width_chars(16);
    key_lbl.set_xalign(0.0);
    key_lbl.add_css_class("monospace");
    let desc_lbl = Label::new(Some(desc));
    desc_lbl.set_xalign(0.0);
    desc_lbl.set_hexpand(true);
    desc_lbl.set_halign(Align::Fill);
    desc_lbl.set_wrap(true);
    desc_lbl.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    desc_lbl.add_css_class("dim-label");
    row.append(&key_lbl);
    row.append(&desc_lbl);
    row
}
