use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation, ScrolledWindow, Separator};
use libadwaita as adw;
use adw::prelude::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const RELEASE_NAME: &str = "New Ground";

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
                "Turn Simple Mode off with the SIMPLE button in the header, beside Library",
            ] {
                body.append(&bullet_row(item));
            }
        } else {
            body.append(&section_label(&format!("What's New in {VERSION}")));
            for item in [
                "New: press F1 and every panel and button on screen gets a bubble saying what it does, drawn over the running window rather than replacing it — you can still see the thing being explained. Escape, F1 again, or a click anywhere puts them away",
                "New: setting up is three screens with one decision each — what it's for, sign in, confirm a name. It used to be one long page of five sections with seven separate Apply buttons, in an order nothing announced, starting by asking for a git name and email. Creating the repository, linking it, the first save and the first upload all happen behind the last button, each step ticked off as it finishes",
                "New: signing in with GitHub supplies your name and email, so you are never asked for them — and uses the address GitHub guarantees will attribute your work to you, rather than the public email field, which is empty for anyone with email privacy on and silently credits every version to nobody",
                "New: git is bundled. The runtime Zerkalo is built on has none, so the flatpak used to run the host's git — making \"install git in a terminal\" a prerequisite for saving your work. There is nothing left to install",
                "New: you don't need a GitHub account. The same screen offers backing up to a folder or drive — a synced Nextcloud or pCloud folder, a USB stick — or pasting the address of a repository you already have. Declining outright is a plain option, and you aren't asked again",
                "New: the repository is named after your work rather than the program — the work folder's name with -docs after it. Folder names GitHub would reject are converted instead of being sent and refused",
                "New: a Tools window (≡ → Tools) lists what's bundled and what's optional, replacing the last step of setup",
                "Changed: Zerkalo no longer opens with an alert listing sudo commands — the first thing a new user saw, about tools that are now bundled anyway",
                "Changed: document fonts moved to Settings, out of setup, where they were standing between a first-time user and getting started",
                "New: save the template dialog's settings as your own template — press the save button beside \"Your Templates\", name it, and it joins the gallery beside the built-in presets. Your name, affiliation and CV contact rows are kept; the title, date, abstract and keywords are not, so one document's front matter can't be stamped onto the next",
                "Fixed: the first upload to a brand-new repository goes through. Sync pulls before it pushes, and a repository with no commits yet has no branch to pull from — that failed pull was read as an interrupted rebase, and the sync stopped with a warning about a mid-rebase repository without ever pushing",
                "Fixed: a new repository starts on main. Setup left the branch to git's own default, often master, so the first push created a second, unrelated branch beside the main GitHub had made",
                "Fixed: \"Double\" line spacing is now actually double, and \"1.5 Lines\" is actually 1.5 — Typst's leading is the gap between lines, not a multiplier, and the old values rendered at about 1.4x and 1.2x. APA, MLA, Chicago and Turabian all require true double spacing for submission. Documents written with the old values still open on the right setting",
                "Fixed: paragraphs are marked once, not twice — generated documents set a first-line indent and a fixed gap between paragraphs, where academic manuscript style uses the indent alone, and the extra gap also broke the line grid on double-spaced documents",
                "Fixed: MLA documents keep their paragraph indents, APA 7th no longer prints \"Running head:\", Executive paper size compiles (Typst calls it us-executive, the template wrote executive), and an abstract fits on small paper instead of losing most of the column to a fixed inset",
                "Fixed: changing the document font or size only edits that one line, instead of regenerating the whole preamble — which on a document with no settings file, or one Zerkalo didn't create, could silently reset paper size, margins and metadata, or replace the file outright with no confirmation and no backup. Every Apply now takes a snapshot first, and documents are written atomically so a crash mid-save can't leave a .typ empty",
                "New: Word, OpenDocument and Markdown files are converted by Zerkalo itself, with nothing to install — three formats that used to need pandoc, which in the flatpak means a tool installed outside the sandbox that most people won't have. Headings, bold and italic, nested lists, tables, links, quotes, code blocks and embedded images all come across, and images travel with the document",
                "New: \"Paste as Document\" reads what you pasted as Markdown the same way, so it too needs nothing installed — and a large paste no longer freezes the window while it converts",
                "New: anything a conversion couldn't carry across is said out loud rather than quietly dropped — raw HTML in Markdown, footnotes reduced to plain markers, and Word citations from a reference manager that can't be read at all",
                "Fixed: importing no longer writes into the folder your source file lives in. The .typ and its extracted images used to appear beside the original before the preview asked whether you wanted them; conversion now happens in a private working folder and nothing lands anywhere until you press Import. Importing from a read-only or shared location works, cancelling leaves nothing behind, and closing the preview window cleans up",
                "Fixed: extracted images resolve — Typst reads a /-rooted path as relative to the project rather than the filesystem, so pandoc's absolute image paths never loaded",
                "Fixed: a large or noisy import could hang forever with an \"Importing…\" toast and no way out, because both output streams were captured and left unread until the conversion finished",
                "Fixed: missing pandoc is detected before the conversion starts and says how to fix it — the old check tested flatpak-spawn rather than pandoc, so in the app's main distribution a missing pandoc produced a raw shell error. Too old a pandoc is now reported up front by version, rather than surfacing mid-conversion as \"unknown writer\"",
                "Fixed: a destination that couldn't be written to looked exactly like a successful import, and in a batch was counted as one",
                "Fixed: import failures are explained in plain language — the wrong format for the file, permission problems, and pandoc's own words only when there is nothing better to say",
                "Fixed: working folders left behind by a crash are cleared out at startup",
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
        toolbar_view.set_top_bar_style(adw::ToolbarStyle::RaisedBorder);
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
