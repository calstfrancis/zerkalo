use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation, ScrolledWindow, Separator};
use libadwaita as adw;
use adw::prelude::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASE_NAME: &str = "Clear Glass";

pub struct WelcomeWindow {
    window: adw::Window,
    on_dismissed: Rc<RefCell<Option<Box<dyn Fn()>>>>,
}

impl WelcomeWindow {
    /// True when no marker exists — the very first launch.
    pub fn is_first_run() -> bool {
        !glib::user_data_dir().join("zerkalo/.welcome_version").exists()
    }

    pub fn new(parent: &impl IsA<gtk4::Window>, is_first_run: bool) -> Self {
        let on_dismissed: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));

        let title = if is_first_run { "Welcome to Zerkalo" } else { "What's New" };
        let window = adw::Window::builder()
            .title(title)
            .transient_for(parent)
            .modal(true)
            .default_width(480)
            .default_height(580)
            .build();

        let header = adw::HeaderBar::new();

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
                 compiled automatically as you type."
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
                 │   (Typst text)  │  │            │  │\n\
                 │                 │  └────────────┘  │\n\
                 ├─────────────────┴──────────────────┤\n\
                 │  Status bar  (word count, cursor)   │\n\
                 └────────────────────────────────────┘"
            ));
            diagram.add_css_class("monospace");
            diagram.add_css_class("dim-label");
            diagram.add_css_class("caption");
            diagram.set_xalign(0.0);
            diagram.set_margin_top(4);
            diagram.set_margin_bottom(4);
            body.append(&diagram);

            body.append(&Separator::new(Orientation::Horizontal));
            body.append(&section_label("Getting Started"));
            for item in [
                "Open or create a .typ file from the title-bar dropdown",
                "Press Ctrl+S to save — the preview on the right updates immediately",
                "Use the formatting bar above the editor for Bold, Italic, and Headings",
                "Type @ to insert a citation (configure your .bib file in Settings ≡)",
                "Type # for Typst function completions — e.g. #figure(), #bibliography()",
                "Use Update Template Settings in the ≡ menu to change title, author, and style",
                "The Outline panel on the left shows your document structure — click to navigate",
            ] {
                body.append(&bullet_row(item));
            }

            body.append(&Separator::new(Orientation::Horizontal));
            body.append(&section_label("Simple Mode"));
            for item in [
                "Zerkalo hides the Typst front-matter so you can focus on writing prose",
                "To change template settings use Update Template Settings in the ≡ menu",
                "Turn Simple Mode off with the SIMPLE button in the status bar",
            ] {
                body.append(&bullet_row(item));
            }
        } else {
            body.append(&section_label(&format!("What's New in {VERSION}")));
            for item in [
                "Added: Default Fonts step in Setup & Onboarding — pick a default sans and serif font, used for new documents and template previews until you choose something else per-document",
                "Added: default fonts are soft-locked in Font Management — disabling one is blocked with a warning to pick a replacement first",
                "Added: descriptions in the in-document CV style switcher (Modern/Academic/Classic/Two-Column), matching the New from Template gallery",
                "Changed: the formatting toolbar now collapses lower-priority controls into a trailing \"more\" menu as the pane narrows, instead of forcing the editor to overflow underneath the sidebar",
                "Changed: in CV mode, New from Template's Metadata group now shows CV-relevant fields (Email, Location, Phone, Links) instead of academic-paper fields that were silently ignored for CVs",
                "Fixed: switching CV style away from Two-Column kept the old two-column layout — the style switcher now regenerates the document body, not just the style label",
                "Fixed: CV — Two-Column's Award entries ran title and organization together on one line, instead of stacking title / organization / date the way Education entries already did",
                "Fixed: a single-line description on any CV entry (Employment, Education, Award, etc.) failed to compile in every CV style, not just Two-Column",
                "Fixed: editing metadata (Email/Location/Website/etc.) via Update Template Settings on an existing CV could crash with \"unknown variable: section\"",
                "Fixed: Setup & Onboarding could open 2-3x wider than intended when status labels or install hints were long",
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
            ("Ctrl+Shift+G", "Git sync"),
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
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&outer));
        window.set_content(Some(&toolbar_view));

        let win_c = window.clone();
        let cb = on_dismissed.clone();
        ok_btn.connect_clicked(move |_| {
            win_c.close();
            if let Some(f) = cb.borrow().as_ref() { f(); }
        });

        Self { window, on_dismissed }
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
        let marker = glib::user_data_dir().join("zerkalo/.welcome_version");
        std::fs::read_to_string(&marker)
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
            != VERSION
    }

    /// Record that the welcome window has been shown for this version.
    pub fn mark_shown() {
        let marker = glib::user_data_dir().join("zerkalo/.welcome_version");
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
