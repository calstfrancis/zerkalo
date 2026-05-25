use gtk4::prelude::*;
use gtk4::{
    Notebook, ScrolledWindow, TextBuffer, TextIter, TextView, WrapMode,
};
use libadwaita::prelude::*;
use libadwaita as adw;

// ── Rich-text section DSL ─────────────────────────────────────────────────────

enum Block<'a> {
    H1(&'a str),
    H2(&'a str),
    Body(&'a str),
    Code(&'a str),
    Gap,
}

// ── Tab content ───────────────────────────────────────────────────────────────

fn overview_blocks() -> Vec<Block<'static>> {
    vec![
        Block::H1("Zerkalo — Typst Editor"),
        Block::Body("Zerkalo is a contemplative Typst editor with live preview, multi-file support, LSP completions, and git sync."),
        Block::Gap,
        Block::H2("Getting started"),
        Block::Body("Zerkalo opens your work folder (~/Documents/Zerkalo by default). The header dropdown shows your recent documents. Click the folder icon to browse all documents."),
        Block::Gap,
        Block::Body("Create a new document from the hamburger menu or double-click a .typ file in your file manager. The left sidebar shows the document outline and a symbol insert panel."),
        Block::Gap,
        Block::H2("Layout"),
        Block::Code("Left sidebar   Document outline and symbol insert (toggle with ⊞ button)\nEditor         Tabbed, syntax-highlighted Typst editor\nFind bar       Persistent search/replace at editor bottom\nPreview        Live rendered output — use +/− to zoom\nError panel    Compile errors and LSP diagnostics"),
        Block::Gap,
        Block::H2("Git sync"),
        Block::Body("Click the sync button (⟳) to commit all changes and push. On first sync, Zerkalo will ask for a remote URL."),
    ]
}

fn shortcuts_blocks() -> Vec<Block<'static>> {
    vec![
        Block::H1("Keyboard Shortcuts"),
        Block::Gap,
        Block::H2("Editing"),
        Block::Code("Ctrl+S              Save current file\nCtrl+F              Find & Replace\nCtrl+Tab            Next tab\nCtrl+Shift+Tab      Previous tab"),
        Block::Gap,
        Block::H2("Compiling"),
        Block::Code("Ctrl+Shift+P        Compile and refresh preview\nAuto-compile        Fires automatically after each change"),
        Block::Gap,
        Block::H2("Autocomplete"),
        Block::Code("@                   Citation popup (requires a .bib file)\n#                   LSP function/keyword popup (requires tinymist)\nTab / Return        Accept selected item\nEsc                 Dismiss popup\n↑ / ↓               Navigate completion list"),
        Block::Gap,
        Block::H2("Window"),
        Block::Code("Ctrl+R              Refresh file tree\nCtrl+Q              Quit\nSidebar button      Toggle left sidebar\nInsert button       Toggle insert snippets panel\nPop-out button      Open preview in a separate window"),
    ]
}

fn faq_blocks() -> Vec<Block<'static>> {
    vec![
        Block::H1("Frequently Asked Questions"),
        Block::Gap,
        Block::H2("Why is the preview blank?"),
        Block::Body("Zerkalo needs typst and pdftoppm installed and in your PATH."),
        Block::Code("zypper install typst poppler-tools    # openSUSE\napt  install  typst poppler-utils     # Debian/Ubuntu\nbrew install  typst poppler           # macOS"),
        Block::Gap,
        Block::H2("LSP autocomplete is not working"),
        Block::Body("Install tinymist, the Typst language server:"),
        Block::Code("cargo add tinymist"),
        Block::Gap,
        Block::H2("How do I change the work folder?"),
        Block::Body("Open Settings from the hamburger menu (≡) and change the Work folder path. The work folder is where Zerkalo looks for your .typ documents (default: ~/Documents/Zerkalo)."),
        Block::Gap,
        Block::H2("Can I use a custom bibliography?"),
        Block::Body("Yes — set bib_path in Settings or in .zerkalo/config.toml inside the project to point at your .bib file."),
        Block::Gap,
        Block::H2("How does auto-compile work?"),
        Block::Body("After each keystroke, Zerkalo starts a debounce timer. When it fires without further changes, it saves all modified files and runs typst compile. The delay is configurable in Settings (default 500 ms)."),
        Block::Gap,
        Block::H2("Where are log files?"),
        Block::Code("~/.local/share/zerkalo/zerkalo.log"),
    ]
}

fn about_blocks() -> Vec<Block<'static>> {
    vec![
        Block::H1("About Zerkalo"),
        Block::Body("Version 0.1.0"),
        Block::Gap,
        Block::Body("A contemplative Typst editor built with Rust, GTK4, and libadwaita. The name means \"mirror\" in Russian."),
        Block::Gap,
        Block::H2("Components"),
        Block::Code("Rust             Systems language — fast, safe, no GC\nGTK4             Cross-platform widget toolkit\nlibadwaita       GNOME Human Interface Guidelines\nsourceview5      Syntax-highlighted source editor\ntinymist         Typst Language Server (optional)\ngit2             Git integration via libgit2"),
        Block::Gap,
        Block::H2("Source"),
        Block::Code("https://github.com/calstfrancis/zerkalo"),
        Block::Gap,
        Block::H2("License"),
        Block::Body("MIT"),
        Block::Gap,
        Block::H2("Typst"),
        Block::Body("Typst is a new markup-based typesetting system. Learn more at https://typst.app"),
    ]
}

// ── Public widget ─────────────────────────────────────────────────────────────

pub struct HelpWindow {
    window: adw::Window,
}

impl HelpWindow {
    pub fn new(parent: &impl IsA<gtk4::Window>) -> Self {
        let window = adw::Window::new();
        window.set_title(Some("Help — Zerkalo"));
        window.set_default_width(700);
        window.set_default_height(580);
        window.set_transient_for(Some(parent));
        window.set_modal(false);

        let header = adw::HeaderBar::new();
        let notebook = Notebook::new();
        notebook.set_scrollable(true);

        let tabs: &[(&str, fn() -> Vec<Block<'static>>)] = &[
            ("Overview",  overview_blocks),
            ("Shortcuts", shortcuts_blocks),
            ("FAQ",       faq_blocks),
            ("About",     about_blocks),
        ];
        for (title, blocks_fn) in tabs {
            let lbl = gtk4::Label::new(Some(title));
            let scroll = make_rich_tab(blocks_fn());
            notebook.append_page(&scroll, Some(&lbl));
        }

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&notebook));
        window.set_content(Some(&toolbar));

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }
}

// ── Rich tab renderer ─────────────────────────────────────────────────────────

fn make_rich_tab(blocks: Vec<Block<'_>>) -> ScrolledWindow {
    let buf = TextBuffer::new(None);
    let mut iter = buf.end_iter();

    // Define tags
    let tag_h1 = buf.create_tag(
        Some("h1"),
        &[("weight", &700i32), ("scale", &1.3f64), ("pixels-below-lines", &6i32)],
    );
    let tag_h2 = buf.create_tag(
        Some("h2"),
        &[("weight", &700i32), ("pixels-above-lines", &8i32), ("pixels-below-lines", &2i32)],
    );
    let tag_body = buf.create_tag(
        Some("body"),
        &[("pixels-below-lines", &4i32)],
    );
    let tag_code = buf.create_tag(
        Some("code"),
        &[
            ("family", &"Monospace"),
            ("pixels-above-lines", &2i32),
            ("pixels-below-lines", &2i32),
            ("left-margin", &16i32),
        ],
    );

    // Suppress unused warnings for tags that are only used via create_tag
    let _ = (&tag_h1, &tag_h2, &tag_body, &tag_code);

    for block in blocks {
        match block {
            Block::H1(text) => insert_with_tag(&buf, &mut iter, &format!("{text}\n"), "h1"),
            Block::H2(text) => insert_with_tag(&buf, &mut iter, &format!("{text}\n"), "h2"),
            Block::Body(text) => insert_with_tag(&buf, &mut iter, &format!("{text}\n"), "body"),
            Block::Code(text) => insert_with_tag(&buf, &mut iter, &format!("{text}\n"), "code"),
            Block::Gap => buf.insert(&mut iter, "\n"),
        }
    }

    let view = TextView::with_buffer(&buf);
    view.set_editable(false);
    view.set_cursor_visible(false);
    view.set_wrap_mode(WrapMode::Word);
    view.set_left_margin(20);
    view.set_right_margin(20);
    view.set_top_margin(16);
    view.set_bottom_margin(16);
    view.set_pixels_above_lines(2);
    view.set_monospace(false);

    let scroll = ScrolledWindow::new();
    scroll.set_hexpand(true);
    scroll.set_vexpand(true);
    scroll.set_child(Some(&view));
    scroll
}

fn insert_with_tag(buf: &TextBuffer, iter: &mut TextIter, text: &str, tag_name: &str) {
    let start_offset = iter.offset();
    buf.insert(iter, text);
    let start = buf.iter_at_offset(start_offset);
    if let Some(tag) = buf.tag_table().lookup(tag_name) {
        buf.apply_tag(&tag, &start, iter);
    }
}
