use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation, ScrolledWindow, Separator};
use libadwaita as adw;
use adw::prelude::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASE_NAME: &str = "True Type";

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
                "New: autocomplete suggests inline — type # and the best match appears dim after the cursor; Tab accepts it, and the (now much smaller) list only opens once you've typed two characters",
                "New: the status bar says what the current suggestion does and which keys take it, so the explanation never covers what you're writing",
                "New: suggestions are found by their Typst name and by fragments of it — #pagebreak works, and #break finds it too",
                "New: clicking anywhere else dismisses a completion or citation popup",
                "New: #cv-profile(\"name\") renders a whole CV profile — every section, in order, with its headings — built in Skrizhal's CV Profiles dialog",
                "Fixed: Zerkalo crashed when resizing the editor/preview split, dragging the sidebar edge, or toggling the sidebar",
                "Fixed: copying — especially from the right-click menu — threw the editor back to the top of the document",
                "Fixed: a CV elements file containing profiles could leave the ! autocomplete and CV panel completely empty",
                "Fixed: GitHub sync could send your sign-in token to non-GitHub backup remotes — token use is now scoped to github.com only",
                "Fixed: cancelling \"Sign in with GitHub\" didn't actually stop the background approval check",
                "Fixed: renaming a citation key could silently overwrite a different, already-existing key",
                "Fixed: hyphenated citation keys (e.g. smith-2020) weren't renamed in the document text, only in the bibliography file",
                "Fixed: snapshot/version history could mix together unrelated files or projects that happened to share a name",
                "Fixed: a CV's style selector could show the wrong style when reopening Update Template Settings",
                "Fixed: dragging to reorder files in the sidebar could show a \"rejected\" bounce-back even though the reorder succeeded",
                "Fixed: exporting to Word/HTML/EPUB/etc. could silently produce broken output for documents missing certain internal markers",
                "Fixed: autocomplete could get misaligned on lines containing emoji or certain rare symbols before the cursor",
                "Fixed: quick-fixes for compile errors added an extra blank line, and could convert Windows-style line endings to Unix-style",
                "Fixed: Settings could silently discard saved snippets when opened and saved",
                "Fixed: \"Add to Project\"/\"New Document\" dialogs could treat Cancel or Escape as confirming instead of cancelling",
                "Fixed: clicking Cancel mid-compile could clear real compile errors and show a false \"Compiled successfully\" toast",
                "Fixed: a malicious project's .zerkalo/config.toml could point the compiler at arbitrary files outside the project",
                "Fixed: Replace All could silently corrupt file contents when the replacement text contained a $ (e.g. Typst math)",
                "Fixed: restoring a document from Trash could mark it \"restored\" even though the file was never actually moved back",
                "Fixed: Settings' spell-check language list could lose its remove buttons or go blank after removing languages",
                "Fixed: a failed Settings save is now shown as an error instead of silently reported as successful",
                "Fixed: GitHub repo creation in Setup & Onboarding no longer freezes the window, and confirms before replacing an existing remote",
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
