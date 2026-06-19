use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation, ScrolledWindow, Separator};
use libadwaita as adw;
use adw::prelude::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");

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
            .default_width(500)
            .default_height(640)
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

        let app_title = Label::new(Some("Zerkalo"));
        app_title.add_css_class("title-1");
        app_title.set_halign(Align::Center);

        let sub_lbl = Label::new(Some(&format!("Version {VERSION}")));
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
            intro.set_xalign(0.0);
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
                "Project toggle in status bar — reveals root-file controls inline (left of SIMPLE)",
                "Citations sidebar — folder button picks a .bib file; filename shown in header",
                "Cursor movement scrolls the preview to the matching position",
                "Compile spinner shows elapsed seconds; status shows page count and timing",
                "Build Log panel — collapsible raw compiler output on error",
                "Simple Mode — hides line numbers, adds left margin, hides template marker comments",
                "Preview toolbar 'Help' toggle replaces the icon-only ? button",
                "Root-file suggestion banner only shown when project toggle is ON",
                "Fixed crash on cursor move (glib SourceId::remove panic)",
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

        scroll.set_child(Some(&body));
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
    let dot = Label::new(Some("•"));
    dot.set_valign(Align::Start);
    dot.add_css_class("dim-label");
    let lbl = Label::new(Some(text));
    lbl.set_xalign(0.0);
    lbl.set_wrap(true);
    lbl.set_hexpand(true);
    row.append(&dot);
    row.append(&lbl);
    row
}

fn shortcut_row(key: &str, desc: &str) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 0);
    row.set_margin_start(4);
    let key_lbl = Label::new(Some(key));
    key_lbl.set_width_chars(22);
    key_lbl.set_xalign(0.0);
    key_lbl.add_css_class("monospace");
    let desc_lbl = Label::new(Some(desc));
    desc_lbl.set_xalign(0.0);
    desc_lbl.add_css_class("dim-label");
    row.append(&key_lbl);
    row.append(&desc_lbl);
    row
}
