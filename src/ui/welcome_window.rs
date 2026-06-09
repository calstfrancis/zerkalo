use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation, ScrolledWindow, Separator};
use libadwaita as adw;
use adw::prelude::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct WelcomeWindow {
    window: adw::Window,
}

impl WelcomeWindow {
    pub fn new(parent: &impl IsA<gtk4::Window>) -> Self {
        let window = adw::Window::builder()
            .title("Welcome to Zerkalo")
            .transient_for(parent)
            .modal(true)
            .default_width(500)
            .default_height(600)
            .build();

        let header = adw::HeaderBar::new();

        let outer = GtkBox::new(Orientation::Vertical, 0);

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);

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

        body.append(&section_label(&format!("What's New in {VERSION}")));
        for item in [
            "Completion popup now shows all snippets when # is typed and filters as you type — no more missing completions",
            "Template dialog: \"Numbering Format\" row lets you choose Decimal (1.1.1.), IEEE Roman (I.A.1.), or Alpha (a.a.a.)",
            "Template dialog: \"Preview Code…\" button shows the generated Typst preamble before you apply it",
            "Heading numbers now render correctly — fixed for all styles (GOST, Vancouver, IEEE)",
            "IEEE / GOST / Vancouver numbering is now user-controlled: the Numbered Headings toggle turns it on or off",
            "Outline and Symbols panel buttons now use symbolic icons instead of text labels",
        ] {
            body.append(&bullet_row(item));
        }

        body.append(&Separator::new(Orientation::Horizontal));
        body.append(&section_label("Quick Start"));
        for item in [
            "Open or create a .typ file from the title-bar dropdown",
            "Edit your document — the preview updates as you type",
            "Press Ctrl+Shift+P to compile manually at any time",
            "Type # for Typst function completions (requires tinymist LSP)",
            "Type @ for citation completions (configure .bib in Settings ≡)",
            "Click ⟳ in the toolbar to commit and push to Git",
            "The Outline panel shows headings; click one to jump there",
            "Toggle GOST Type B font and autocorrect in the status bar",
        ] {
            body.append(&bullet_row(item));
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
        let ok_btn = Button::with_label("Get Started");
        ok_btn.add_css_class("suggested-action");
        ok_btn.add_css_class("pill");
        footer.append(&ok_btn);
        outer.append(&footer);

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&outer));
        window.set_content(Some(&toolbar_view));

        let win_c = window.clone();
        ok_btn.connect_clicked(move |_| win_c.close());

        Self { window }
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
