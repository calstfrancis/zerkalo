use gtk4::prelude::*;
use gtk4::{
    Notebook, ScrolledWindow, TextBuffer, TextIter, TextView, WrapMode,
};
use libadwaita::prelude::*;
use libadwaita as adw;

// ── Rich-text section DSL ─────────────────────────────────────────────────────

pub(crate) enum Block<'a> {
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
        Block::Body("Zerkalo is a contemplative Typst editor with live preview, multi-file support, LSP completions, and git sync. No external Typst binary required — compilation is built in."),
        Block::Gap,
        Block::H2("Getting started"),
        Block::Body("Zerkalo opens your work folder (~/Documents/Zerkalo by default). The header dropdown shows your recent documents. Click the folder icon to browse all documents."),
        Block::Gap,
        Block::Body("Create a new document from the hamburger menu (≡) or use New from Template… for a complete preamble. The left sidebar shows the document outline and a symbol insert panel."),
        Block::Gap,
        Block::H2("Layout"),
        Block::Code("Left sidebar   Document outline, symbols, files, refs, history\nEditor         Tabbed, syntax-highlighted Typst editor\nFind bar       Persistent search/replace at editor bottom\nPreview        Live rendered output — use +/− to zoom\nError panel    Compile errors and LSP diagnostics"),
        Block::Gap,
        Block::H2("Git sync"),
        Block::Body("Click the sync button (⟳) or press Ctrl+Shift+G to commit all changes and push. On first sync, Zerkalo will ask for a remote URL."),
        Block::Gap,
        Block::H2("Preview & Cheatsheet"),
        Block::Body("The toggle button (?) in the preview toolbar switches the right panel between the live preview and a two-tab reference view (Cheatsheet + Help). Compilation continues in the background regardless."),
    ]
}

fn cheatsheet_blocks() -> Vec<Block<'static>> {
    vec![
        Block::H1("Typst Cheatsheet — Academic Writing"),
        Block::Gap,
        Block::H2("Document Structure"),
        Block::Code("= Heading 1\n== Heading 2\n=== Heading 3\n==== Heading 4\n\nText paragraph. Blank lines start new paragraphs."),
        Block::Gap,
        Block::H2("Text Formatting"),
        Block::Code("*bold*            _italic_          `inline code`\n\"smart quotes\"    #underline[text]  #strike[text]\n#smallcaps[text]  #super[n]         #sub[n]\n#emph[emphasis]   #strong[strong]"),
        Block::Gap,
        Block::H2("Lists"),
        Block::Code("- Bullet item        Unordered list\n+ Numbered item      Ordered list\n/ Term: Definition   Description list"),
        Block::Gap,
        Block::H2("Citations & Bibliography"),
        Block::Code("@authorYear                   In-text citation\n@authorYear[p.~5]             With page locator\n@[see @a, p.~1; @b, ch.~2]   Multiple sources\n\n#bibliography(\"refs.bib\", style: \"chicago-author-date\")\nStyles: \"apa\", \"mla\", \"chicago-author-date\",\n        \"chicago-notes\", \"ieee\", \"harvard-cite-them-right\",\n        \"gost-r-705-2008\""),
        Block::Gap,
        Block::H2("Figures & Cross-references"),
        Block::Code("#figure(\n  image(\"fig.png\", width: 80%),\n  caption: [Caption text.],\n) <fig-label>\n\nAs shown in @fig-label, the results indicate…"),
        Block::Gap,
        Block::H2("Tables"),
        Block::Code("#figure(\n  table(\n    columns: (auto, 1fr, 1fr),\n    table.header([Col A], [Col B], [Col C]),\n    [Row 1A], [Row 1B], [Row 1C],\n    [Row 2A], [Row 2B], [Row 2C],\n  ),\n  caption: [Table caption.],\n) <tbl-label>"),
        Block::Gap,
        Block::H2("Math"),
        Block::Code("Inline:  $E = m c^2$   $x_(i j)^2$   $arrow(v)$\nDisplay: $ integral_0^1 f(x) dif x $\nMatrix:  $ mat(a, b; c, d) $\nVector:  $bold(v) = vec(1, 2, 3)$"),
        Block::Gap,
        Block::H2("Footnotes"),
        Block::Code("Word.#footnote[Footnote text here.]\n\n// Remove indent on footnote entries:\n#set footnote.entry(indent: 0em)"),
        Block::Gap,
        Block::H2("Special Elements"),
        Block::Code("#outline()             Table of contents\n#outline(target: figure.where(kind: table))\n                       List of tables\n#pagebreak()           Page break\n#colbreak()            Column break\n#h(1em)                Horizontal space\n#v(1em)                Vertical space\n#box(width: 100%, line())  Horizontal rule"),
        Block::Gap,
        Block::H2("Links"),
        Block::Code("#link(\"https://example.com\")[Link text]\n#link(\"https://example.com\")  (URL as anchor text)"),
        Block::Gap,
        Block::H2("Blocks & Layout"),
        Block::Code("#block(fill: luma(240), inset: 8pt, radius: 4pt)[\n  Shaded box — useful for quotations or notes.\n]\n#columns(2)[Two-column content]\n#align(center)[Centred text]\n#align(right + bottom)[Corner text]"),
        Block::Gap,
        Block::H2("Includes & Imports"),
        Block::Code("#include \"chapter1.typ\"\n#import \"macros.typ\": my-macro\n#import \"@preview/cetz:0.2.2\": canvas"),
        Block::Gap,
        Block::H2("Common Set Rules (Preamble)"),
        Block::Code("#set text(font: \"Times New Roman\", size: 12pt, lang: \"en\")\n#set par(justify: true, first-line-indent: 0.5in,\n         leading: 1em)\n#set page(paper: \"us-letter\", margin: 1in,\n          numbering: \"1\", number-align: top + right)\n#set heading(numbering: \"1.1\")\n\n// Double-spacing:\n#set par(leading: 24pt)"),
        Block::Gap,
        Block::H2("Git Sync"),
        Block::Code("Ctrl+Shift+S   Commit & push all changes"),
    ]
}

fn shortcuts_blocks() -> Vec<Block<'static>> {
    vec![
        Block::H1("Keyboard Shortcuts"),
        Block::Gap,
        Block::H2("Editing"),
        Block::Code("Ctrl+S              Save current file\nCtrl+F              Find & Replace\nCtrl+Tab            Next tab\nCtrl+Shift+Tab      Previous tab\nCtrl+Left/Right     Word jump (Typst-aware: treats #keyword and @cite as units)\nCtrl+Shift+Up/Down  Jump to previous / next heading in the document\nCtrl+D              Duplicate line or selection\nCtrl+/              Toggle line comment\nCtrl+Enter          Insert page break\nMiddle-click tab    Close tab"),
        Block::Gap,
        Block::H2("Compiling & Preview"),
        Block::Code("Ctrl+Shift+P        Compile and refresh preview\nAuto-compile        Fires automatically after each change"),
        Block::Gap,
        Block::H2("Navigation"),
        Block::Code("Ctrl+K              Command palette (commands + headings)\nCtrl+G              Command palette pre-filtered to headings only\nCtrl+Shift+F        Find in Files (project-wide search)"),
        Block::Gap,
        Block::H2("Autocomplete"),
        Block::Code("@                   Citation popup (requires a .bib file)\n#                   LSP function/keyword popup (requires tinymist)\nTab / Return        Accept selected item\nEsc                 Dismiss popup\n↑ / ↓               Navigate completion list"),
        Block::Gap,
        Block::H2("Git & Window"),
        Block::Code("Ctrl+Shift+S        Commit & push (git sync)\nCtrl+Shift+H        Show keyboard shortcuts (dynamic)\nCtrl+R              Refresh file tree\nCtrl+Q              Quit\nCtrl+?              Open this help window\nSidebar button      Toggle left sidebar\nInsert button       Toggle insert snippets panel\nPop-out button      Open preview in a separate window"),
    ]
}

fn faq_blocks() -> Vec<Block<'static>> {
    vec![
        Block::H1("Frequently Asked Questions"),
        Block::Gap,
        Block::H2("Why is the preview blank?"),
        Block::Body("Zerkalo has a built-in Typst compiler — no external binary is needed. If the preview is blank, check the error panel at the bottom. The panel shows the file, line number, and a plain-English explanation of the problem."),
        Block::Gap,
        Block::H2("Changing the style gives a compile error"),
        Block::Body("If you see 'expected string or function' after changing a style, your document may have a conflicting #show heading rule outside the template block. Fix it by opening 'Update Template Settings' (sidebar button or ≡ menu) and re-applying your style. That rewrites the formatting section cleanly."),
        Block::Gap,
        Block::H2("The style dropdown doesn't seem to do anything"),
        Block::Body("For template documents (created with 'New from Template' or imported via File → Import), styles are applied inside the template block. If the heading appearance doesn't change, open the error panel — a compile error is likely preventing the preview from updating."),
        Block::Gap,
        Block::H2("Table of Contents / abstract / keywords not appearing"),
        Block::Body("Use 'Update Template Settings' (sidebar button or ≡ → Update Template Settings…). Switch to the Sections tab and toggle Table of Contents, Abstract, or Keywords on. Click 'Apply to Current' — Zerkalo will insert or remove those sections in the document body."),
        Block::Gap,
        Block::H2("Citation keys show as errors"),
        Block::Body("Citations require a .bib file. Either:\n1. Add this line to your document:\n   #bibliography(\"refs.bib\", style: \"chicago-author-date\")\n   (adjusting the filename and style to match your setup)\n2. Or set the bib_path in Settings so Zerkalo can find the file automatically."),
        Block::Gap,
        Block::H2("Imported LaTeX / DOCX file has formatting problems"),
        Block::Body("After import, use 'Update Template Settings' to set the correct style, paper size, and font for your document. The import process preserves the text content and moves all formatting rules into the template block, which Zerkalo controls."),
        Block::Gap,
        Block::H2("LSP autocomplete is not working"),
        Block::Body("Install tinymist, the Typst language server:"),
        Block::Code("cargo install tinymist"),
        Block::Gap,
        Block::H2("How do I change the work folder?"),
        Block::Body("Open Settings from the hamburger menu (≡) and change the Work folder path. The work folder is where Zerkalo looks for your .typ documents (default: ~/Documents/Zerkalo)."),
        Block::Gap,
        Block::H2("Can I use a custom bibliography?"),
        Block::Body("Yes — set bib_path in Settings or in .zerkalo/config.toml inside the project to point at your .bib file. The path should be absolute or relative to the .typ file being compiled."),
        Block::Gap,
        Block::H2("How does auto-compile work?"),
        Block::Body("After each keystroke, Zerkalo starts a debounce timer (default 800 ms). When it fires without further changes, it saves all modified files and compiles using the embedded Typst engine. The delay is configurable in Settings."),
        Block::Gap,
        Block::H2("Where are log files?"),
        Block::Code("~/.local/share/zerkalo/zerkalo.log"),
        Block::Gap,
        Block::H2("How do I set up git sync?"),
        Block::Body("Git sync works on the work folder as a git repository. Press Ctrl+Shift+S or click the sync button — on first use Zerkalo will ask for a remote URL (e.g. GitHub or Gitea). After that, each sync commits all changes and pushes."),
        Block::Gap,
        Block::H2("Can I edit the title, author, or date directly in the document?"),
        Block::Body("Yes — template documents store metadata as plain Typst variables near the top of the file:\n  #let doc-title = \"My Paper\"\n  #let doc-author = \"Jane Smith\"\n  #let doc-date = \"5 June 2026\"\nEdit these directly in the editor. When you open 'Update Template Settings' afterwards, Zerkalo reads the values from the document so the dialog will show your edits, not the old saved values."),
        Block::Gap,
        Block::H2("I built from source but changes aren't appearing"),
        Block::Body("Run cargo build --release first, then bash install.sh. The install script now detects a local build and installs it directly — it no longer downloads from GitHub when a built binary exists in target/release/."),
        Block::Gap,
        Block::H2("How do compilation profiles work?"),
        Block::Body("The header-bar dropdown next to 'Preview' switches between Final (full 144 dpi) and Draft (72 dpi, fast) profiles. In Draft mode Zerkalo passes sys.inputs.at(\"draft\") = \"true\" so documents can skip slow elements:\n  #if sys.inputs.at(\"draft\", default: \"false\") == \"true\" {\n    // skip heavy rendering in draft\n  }"),
        Block::Gap,
        Block::H2("How do snapshots work?"),
        Block::Body("Every Ctrl+S saves a timestamped copy of the current file to ~/.local/share/zerkalo/snapshots/<project>/<file>/. The last 50 snapshots per file are kept. Open ☰ → Browse Snapshots… to see the timeline, compare with the current text, and restore any version."),
        Block::Gap,
        Block::H2("How do I use the project dictionary?"),
        Block::Body("Right-click a misspelled word and choose 'Add to Project Dictionary' to save it in <work_dir>/.zerkalo/dictionary.dic. This dictionary is project-specific and can be committed to git. 'Add to Dictionary' saves to the global user dictionary at ~/.config/zerkalo/user.dic."),
        Block::Gap,
        Block::H2("What is the inline error assistant?"),
        Block::Body("Hover over red-underlined text in the editor to see the error message. For known patterns (missing brace, unknown variable, etc.) a 'Fix It' button applies the correction automatically. The fix patterns live in src/error_patterns.rs."),
    ]
}

fn about_blocks() -> Vec<Block<'static>> {
    vec![
        Block::H1("About Zerkalo"),
        Block::Body(concat!("Version ", env!("CARGO_PKG_VERSION"))),
        Block::Gap,
        Block::Body("A contemplative Typst editor built with Rust, GTK4, and libadwaita. The name means \"mirror\" in Russian."),
        Block::Gap,
        Block::H2("Components"),
        Block::Code("Rust             Systems language — fast, safe, no GC\nGTK4             Cross-platform widget toolkit\nlibadwaita       GNOME Human Interface Guidelines\nsourceview5      Syntax-highlighted source editor\ntypst            Embedded Typst compiler (no binary needed)\ntinymist         Typst Language Server (optional)\ngit2             Git integration via libgit2"),
        Block::Gap,
        Block::H2("Source"),
        Block::Code("https://github.com/calstfrancis/zerkalo"),
        Block::Gap,
        Block::H2("License"),
        Block::Body("MIT"),
        Block::Gap,
        Block::H2("Typst"),
        Block::Body("Typst is a modern markup-based typesetting system. Learn more at https://typst.app"),
    ]
}

// ── Public scroll builders (used by the embedded reference panel) ─────────────

pub fn cheatsheet_scroll() -> ScrolledWindow {
    make_rich_tab(cheatsheet_blocks())
}

pub fn overview_scroll() -> ScrolledWindow {
    make_rich_tab(overview_blocks())
}

pub fn faq_scroll() -> ScrolledWindow {
    make_rich_tab(faq_blocks())
}

// ── Public widget ─────────────────────────────────────────────────────────────

pub struct HelpWindow {
    window: adw::Window,
}

impl HelpWindow {
    pub fn new(parent: &impl IsA<gtk4::Window>) -> Self {
        let window = adw::Window::new();
        window.set_title(Some("Help — Zerkalo"));
        window.set_default_width(720);
        window.set_default_height(600);
        window.set_transient_for(Some(parent));
        window.set_modal(false);

        let header = adw::HeaderBar::new();
        let notebook = Notebook::new();
        notebook.set_scrollable(true);

        let tabs: &[(&str, fn() -> Vec<Block<'static>>)] = &[
            ("Overview",   overview_blocks),
            ("Cheatsheet", cheatsheet_blocks),
            ("Shortcuts",  shortcuts_blocks),
            ("FAQ",        faq_blocks),
            ("About",      about_blocks),
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

pub(crate) fn make_rich_tab(blocks: Vec<Block<'_>>) -> ScrolledWindow {
    let buf = TextBuffer::new(None);
    let mut iter = buf.end_iter();

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
