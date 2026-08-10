use gtk4::prelude::*;
use gtk4::{
    Notebook, ScrolledWindow, TextBuffer, TextIter, TextView, WrapMode,
};
use libadwaita::prelude::*;
use libadwaita as adw;

use super::theme;

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
        Block::Body("Once a template's settings are how you want them, press the save button beside \"Your Templates\" in that dialog to keep them under a name. Saved templates sit under the built-in presets and start a document exactly the way the last one started — the title, date, abstract and keywords are left out, since those belong to a single document rather than to a template."),
        Block::Gap,
        Block::H2("Layout"),
        Block::Code("Left sidebar   Document outline, symbols, files, refs, history\nEditor         Tabbed, syntax-highlighted Typst editor\nFind bar       Persistent search/replace at editor bottom\nPreview        Live rendered output — use +/− to zoom\nError panel    Compile errors and LSP diagnostics"),
        Block::Gap,
        Block::H2("Git sync"),
        Block::Body("Click the sync button (⟳) or press `Ctrl+Shift+G` to save a version of everything and send it up. If nothing is set up yet, ☰ → Set Up Zerkalo walks you through it: sign in with GitHub and press Finish, and the rest — the repository, who the versions are recorded as, and the first upload — is done for you. Nothing to install, and a folder or drive works instead of an account."),
        Block::Gap,
        Block::H2("Multi-file projects"),
        Block::Body("For longer works — journals, theses, books — use ≡ → New Project… to create a folder with a starter template. One file is the compilation root (marked ★ in the file tree, and shown beside the document title when the \"project\" toggle is on). Right-click any file to set it as root, or to insert an `#include` / `#import` directive at the cursor. See the Projects tab for a full walkthrough."),
        Block::Gap,
        Block::H2("Preview & Cheatsheet"),
        Block::Body("The toggle button (?) in the preview toolbar switches the right panel between the live preview and a two-tab reference view (Cheatsheet + Help). Compilation continues in the background regardless."),
    ]
}

fn projects_blocks() -> Vec<Block<'static>> {
    vec![
        Block::H1("Multi-file Projects"),
        Block::Body("A project is a folder that holds several .typ files compiled together. One file — the compilation root — is the entry point. It `#include`-s the others. Zerkalo tracks which file is the root, shows it in the file tree and beside the document title, and always compiles from it."),
        Block::Gap,
        Block::H2("Creating a project"),
        Block::Body("Open the hamburger menu (≡) → New Project… The wizard asks for a project name and a template:"),
        Block::Code("Blank              Empty main.typ — start from scratch\nEssay              main.typ + bibliography.bib\nJournal / Thesis   main.typ, title.typ, ch01-introduction.typ, bibliography.bib\nTheological Journal  main.typ, front-matter.typ, article-01.typ, bibliography.bib"),
        Block::Body("Zerkalo creates a subfolder inside your work folder, writes the starter files, records the compilation root in `.zerkalo/config.toml`, and opens the project."),
        Block::Gap,
        Block::H2("The compilation root"),
        Block::Body("The root is the .typ file you pass to the Typst compiler — typically `main.typ`. All `#include` and `#import` paths are resolved relative to its directory."),
        Block::Gap,
        Block::Body("Zerkalo shows the root in two places:"),
        Block::Code("File tree   ★ icon on the root file row\nHeader      beside the document title, while the \"project\" toggle is on"),
        Block::Gap,
        Block::Body("To change the root, right-click any file in the file tree and choose Set as Compilation Root. Or turn on the \"project\" toggle beside the document title and use Set… there. Writing a single-file document? The ✕ next to those controls puts them away for that project — the toggle stays, so one click brings them back."),
        Block::Gap,
        Block::Body("Zerkalo auto-detects the root on project open by scanning the import graph. The override is saved to `.zerkalo/config.toml` so it persists."),
        Block::Gap,
        Block::H2("File tree"),
        Block::Body("The file tree shows all .typ files in the project folder. Subdirectories are shown as collapsible headers — click the arrow to expand or collapse."),
        Block::Gap,
        Block::Code("+ button         New file (enter name, press Enter)\nFolder button    New folder\nDrag handle      Reorder files within a directory"),
        Block::Gap,
        Block::Body("Right-clicking a file shows:"),
        Block::Code("Set as Compilation Root   Make this file the entry point\nInsert #include           Paste #include \"path\" at the cursor\nInsert #import            Paste #import \"path\": stem at the cursor\nDelete                    Remove the file (with confirmation)"),
        Block::Gap,
        Block::H2("Including files in your document"),
        Block::Body("Typst uses two directives for multi-file documents:"),
        Block::Code("#include \"chapter1.typ\"        Include the file's content inline\n#import \"macros.typ\": my-fn   Import a specific function or variable"),
        Block::Body("The quickest way to insert these: right-click the file in the file tree → Insert `#include` or Insert `#import`. The path is automatically relative to the compilation root's directory."),
        Block::Gap,
        Block::H2("Project config (.zerkalo/config.toml)"),
        Block::Body("Each project can have a `.zerkalo/config.toml` that overrides global settings for that folder:"),
        Block::Code("[project]\nroot_file   = \"main.typ\"     # compilation root\nbib_path    = \"refs.bib\"     # bibliography override\nfile_order  = [              # file tree display order\n  \"main.typ\",\n  \"ch01-introduction.typ\",\n  \"bibliography.bib\",\n]"),
        Block::Body("Zerkalo writes `root_file` and `file_order` automatically. You can edit `bib_path` or other fields by hand."),
        Block::Gap,
        Block::H2("Workflow example — theological journal"),
        Block::Code("my-journal/\n  main.typ             ← compilation root (★)\n  front-matter.typ     ← #include \"front-matter.typ\"\n  article-01.typ       ← #include \"article-01.typ\"\n  bibliography.bib\n  .zerkalo/\n    config.toml"),
        Block::Body("Open the project folder in Zerkalo. The ★ appears on `main.typ`. Edit article-01.typ directly — every save re-compiles from main.typ so the preview always shows the full document."),
    ]
}

fn cv_cheatsheet_blocks() -> Vec<Block<'static>> {
    vec![
        Block::H1("CV / Résumé Helper Reference"),
        Block::Body("Quick start: type `!` anywhere in the editor to search your CV entries and insert one."),
        Block::Gap,
        Block::H2("Skrizhal CV Elements (recommended)"),
        Block::Body("Point Settings → Extras → CV Elements at a Skrizhal YAML file (or click the \"Skrizhal\" button in the citation panel to open the companion app and create one). Then type `!` in the editor for fuzzy autocomplete over your jobs, education, awards, and more — selecting an entry inserts `#cv-entry(\"key\")` at the cursor."),
        Block::Code(
            "#cv-section(category: \"Education\", style: CV_STYLE)\n\
             #cv-section(category: (\"Employment\", \"Ministry Position\"), style: CV_STYLE)\n\
             #cv-section(category: \"Language Skill\", style: CV_STYLE, mode: \"tags\")\n\
             \n\
             #cv-entry(\"hope-united-2025\")   ← renders one entry by its Skrizhal key"
        ),
        Block::Body("Category matching is case-insensitive, so a hand-typed \"education\" matches the same section as \"Education\"."),
        Block::Gap,
        Block::H2("CV Profiles"),
        Block::Body("A profile is a whole CV saved by name in Skrizhal — an ordered list of sections, each with its own heading, filters, and explicit keep/drop lists. Build one in Skrizhal's \"CV Profiles\" dialog, then render the entire thing with a single call. Use this instead of a hand-assembled run of #cv-section calls when you keep more than one version of your CV, since a profile also stores the section order and the one-off exceptions a filter can't express."),
        Block::Code(
            "#cv-profile(\"academic-2026\", style: CV_STYLE)\n\
             \n\
             #cv-profile(\"ministry\", style: CV_STYLE, level: 2)   \u{2190} heading level for section titles"
        ),
        Block::Gap,
        Block::H2("Manual CV Helper Functions (older documents)"),
        Block::Body("Documents created before Skrizhal integration existed may still call these directly instead of `#cv-section` — both keep working."),
        Block::Code(
            "#job(\"Job Title\", \"Company\", \"2022–present\",\n\
             \x20 [Description of role and accomplishments.])\n\
             \n\
             #edu(\"Degree\", \"Institution\", \"2016–2020\")\n\
             #edu(\"Degree\", \"Institution\", \"2016–2020\",\n\
             \x20 note: [Thesis: ...  ·  GPA: 3.9])\n\
             \n\
             #skill(\"Languages\", (\"Rust\", \"Python\", \"Kotlin\"))\n\
             \n\
             #award(\"Award Name\", \"Organisation\", \"2023\")\n\
             #award(\"Award Name\", \"Organisation\", \"2023\",\n\
             \x20 desc: [Brief description of the award.])\n\
             \n\
             #section(\"Section Title\")[\n\
             \x20 Content goes here.\n\
             ]"
        ),
        Block::Gap,
        Block::H2("Switching Style"),
        Block::Body("Use the CV Style button in the format bar to switch between Modern, Academic, Classic, and Two-Column. This rewrites the `#let CV_STYLE` line in the document."),
        Block::Code("// @zerkalo-cv-style: modern   ← marker read by Zerkalo\n#let CV_STYLE = \"modern\"       ← change to \"academic\", \"classic\", or \"sidebar\" (Two-Column)"),
        Block::Gap,
        Block::H2("Adding Sections"),
        Block::Body("Use `#section` to create any custom section. The heading style adapts to `CV_STYLE` automatically."),
        Block::Code("#section(\"Publications\")[\n  ...\n]\n#section(\"Volunteer Work\")[\n  ...\n]"),
        Block::Gap,
        Block::H2("Personal Details"),
        Block::Code("#let cv-name     = \"Your Name\"\n#let cv-email    = \"your@email.com\"\n#let cv-phone    = \"+1 555 000 0000\"\n#let cv-location = \"City, Country\"\n#let cv-links    = \"github.com/handle\""),
        Block::Gap,
        Block::H2("Common Typst Inline Formatting"),
        Block::Code("*bold*    _italic_    #link(\"https://...\")[text]\n#text(fill: luma(80))[dim text]\n#text(weight: \"bold\")[bold text]"),
        Block::Gap,
        Block::H2("Lists"),
        Block::Code("- Bullet item\n+ Numbered item\n/ Term: Definition"),
        Block::Gap,
        Block::H2("Spacing & Layout"),
        Block::Code("#v(0.5em)          Vertical gap\n#h(0.5em)          Horizontal gap\n#pagebreak()       Force new page\n#colbreak()        Column break (two-column CVs)"),
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
        Block::Code("Ctrl+Shift+P        Compile and refresh preview\nCtrl+Shift+E        Export PDF to document folder (no dialog)\nCtrl+P              Print — page range, layout, then the system print dialog\nAuto-compile        Fires automatically after each change\nCtrl+Click preview  Jump to the nearby paragraph in the source\nDouble-click preview Jump to the exact word in the source"),
        Block::Gap,
        Block::H2("Navigation"),
        Block::Code("Ctrl+K              Command palette (commands + headings)\nCtrl+G              Command palette pre-filtered to headings only\nCtrl+Shift+F        Find in Files (project-wide search)"),
        Block::Gap,
        Block::H2("Autocomplete"),
        Block::Code("#                   Inline suggestion — a preview of what will be inserted appears\n                    dim after the cursor, with its signature in the status bar\nTab                 Accept the inline suggestion\n#xx                 After two characters, a short list of matches opens too\n                    (matches anywhere in the name: #break finds pagebreak)\n↑ / ↓               Navigate the list — the status bar describes each entry\nTab / Return        Accept the selected entry from the list\n@                   Citation popup (requires a .bib file)\nEsc                 Dismiss for this word — your text is left alone\n                    (clicking elsewhere dismisses too)\n@ / !               Citations and CV entries behave the same way"),
        Block::Gap,
        Block::H2("Import"),
        Block::Code("Ctrl+Shift+I        Open the Import picker (LaTeX/Word/Markdown/ODT/HTML/EPUB/RTF/PDF)\nCtrl+Shift+V        Paste as Document (reads clipboard text as Markdown)\nDrag & drop         Drop a document file onto the editor to import it directly"),
        Block::Gap,
        Block::H2("What things do"),
        Block::Code("F1                  Label every button and panel on screen, in place\nEsc                 Take the labels away (clicking anywhere does too)"),
        Block::Gap,
        Block::H2("Git & Window"),
        Block::Code("Ctrl+Shift+S        Commit & push (git sync)\nCtrl+Shift+H        Show keyboard shortcuts (dynamic)\nCtrl+R              Refresh file tree\nCtrl+Q              Quit\nCtrl+?              Open this help window\nSidebar button      Toggle left sidebar\nInsert button       Toggle insert snippets panel\nPop-out button      Open preview in a separate window"),
    ]
}

fn faq_blocks() -> Vec<Block<'static>> {
    vec![
        Block::H1("Frequently Asked Questions"),
        Block::Gap,
        Block::H2("How do I create a multi-file project?"),
        Block::Body("Open ≡ → New Project… Enter a name, pick a template (Blank, Essay, Journal / Thesis, or Theological Journal), and click Create. Zerkalo makes a subfolder in your work folder, writes the starter files, and opens the project with the root set automatically."),
        Block::Gap,
        Block::H2("What is the compilation root and why does it matter?"),
        Block::Body("Typst compiles from a single entry-point file. The root is that file — usually `main.typ`. It `#include`-s the other chapters. If the wrong file is the root, you'll either get a blank preview or a single-chapter compile instead of the full document."),
        Block::Gap,
        Block::H2("What does the ★ mean in the file tree?"),
        Block::Body("It marks the current compilation root — the file Zerkalo passes to the Typst compiler. To move it, right-click any other file → Set as Compilation Root."),
        Block::Gap,
        Block::H2("How do I add a new chapter?"),
        Block::Body("1. Click + in the file tree header to create the new .typ file.\n2. Right-click it → Insert `#include` — this pastes #include \"filename.typ\" at the cursor in the active editor.\n3. Move the cursor to the right position in `main.typ` first so the include lands in the right place."),
        Block::Gap,
        Block::H2("The root controls beside the title are missing"),
        Block::Body("They only appear while the \"project\" toggle beside the document title is on — and stay hidden if you dismissed them for this project with the ✕. Click \"project\" to bring them back. For a single-file document in the flat work folder there is no root to choose, which is why they start closed."),
        Block::Gap,
        Block::H2("Why is the preview blank?"),
        Block::Body("Zerkalo has a built-in Typst compiler — no external binary is needed. If the preview is blank, check the error panel at the bottom. The panel shows the file, line number, and a plain-English explanation of the problem."),
        Block::Gap,
        Block::H2("Changing the style gives a compile error"),
        Block::Body("If you see 'expected string or function' after changing a style, your document may have a conflicting `#show heading` rule outside the template block. Fix it by opening 'Update Template Settings' (sidebar button or ≡ menu) and re-applying your style. That rewrites the formatting section cleanly."),
        Block::Gap,
        Block::H2("The style dropdown doesn't seem to do anything"),
        Block::Body("For template documents (created with 'New from Template' or imported via File → Import), styles are applied inside the template block. If the heading appearance doesn't change, open the error panel — a compile error is likely preventing the preview from updating. The button label always shows just the style name; it no longer includes the filename."),
        Block::Gap,
        Block::H2("Table of Contents / abstract / keywords not appearing"),
        Block::Body("Use 'Update Template Settings' (sidebar button or ≡ → Update Template Settings…). Switch to the Sections tab and toggle Table of Contents, Abstract, or Keywords on. Click 'Apply to Current' — Zerkalo will insert or remove those sections in the document body."),
        Block::Gap,
        Block::H2("Citation keys show as errors"),
        Block::Body("Citations require a .bib file. Either:\n1. Add this line to your document:\n   `#bibliography(\"refs.bib\", style: \"chicago-author-date\")`\n   (adjusting the filename and style to match your setup)\n2. Or set the `bib_path` in Settings so Zerkalo can find the file automatically."),
        Block::Gap,
        Block::H2("Imported LaTeX / DOCX file has formatting problems"),
        Block::Body("After import, use 'Update Template Settings' to set the correct style, paper size, and font for your document. The import process preserves the text content and moves all formatting rules into the template block, which Zerkalo controls."),
        Block::Gap,
        Block::H2("LSP autocomplete is not working"),
        Block::Body("tinymist is bundled at /usr/lib/zerkalo/tinymist when installed via the .deb or .rpm package — no extra step needed. For source builds, install it manually:"),
        Block::Code("cargo install tinymist"),
        Block::Gap,
        Block::H2("How do I change the work folder?"),
        Block::Body("Open Settings from the hamburger menu (≡) and change the Work folder path. The work folder is where Zerkalo looks for your .typ documents (default: ~/Documents/Zerkalo)."),
        Block::Gap,
        Block::H2("Can I use a custom bibliography?"),
        Block::Body("Yes — set `bib_path` in Settings or in `.zerkalo/config.toml` inside the project to point at your .bib file. The path should be absolute or relative to the .typ file being compiled."),
        Block::Gap,
        Block::H2("How does auto-compile work?"),
        Block::Body("After each keystroke, Zerkalo starts a debounce timer (default 800 ms). When it fires without further changes, it saves all modified files and compiles using the embedded Typst engine. The delay is configurable in Settings."),
        Block::Gap,
        Block::H2("Where are log files?"),
        Block::Code("~/.local/share/zerkalo/zerkalo.log"),
        Block::Gap,
        Block::H2("How do I set up git sync?"),
        Block::Body("Open ☰ → Set Up Zerkalo and press 'Set this up', then 'Sign in with GitHub'. Approve the short code shown at github.com/login/device, confirm the repository name, and press Finish — Zerkalo creates the repository, makes the work folder a git repository, records your name and address from the GitHub account (so you never type them), and pushes the first version. If you'd rather not use GitHub, the same screen offers backing up to a folder or drive instead, or pasting the address of a repository you already have. Nothing needs installing: git ships inside Zerkalo. After that, `Ctrl+Shift+S` saves a version and pushes it."),
        Block::Gap,
        Block::H2("Can I edit the title, author, or date directly in the document?"),
        Block::Body("Yes — template documents store metadata as plain Typst variables near the top of the file:\n  #let doc-title = \"My Paper\"\n  #let doc-author = \"Jane Smith\"\n  #let doc-date = \"5 June 2026\"\nEdit these directly in the editor. When you open 'Update Template Settings' afterwards, Zerkalo reads the values from the document so the dialog will show your edits, not the old saved values."),
        Block::Gap,
        Block::H2("I built from source but changes aren't appearing"),
        Block::Body("Run `cargo build --release` first, then `bash install.sh`. The install script detects a local build and installs it directly. For end users without Rust, the recommended path is to download the .deb or .rpm from the GitHub releases page."),
        Block::Gap,
        Block::H2("How do compilation profiles work?"),
        Block::Body("The header-bar dropdown next to 'Preview' switches between Final (full 144 dpi) and Draft (72 dpi, fast) profiles. In Draft mode Zerkalo passes sys.inputs.at(\"draft\") = \"true\" so documents can skip slow elements:\n  #if sys.inputs.at(\"draft\", default: \"false\") == \"true\" {\n    // skip heavy rendering in draft\n  }"),
        Block::Gap,
        Block::H2("How do snapshots work?"),
        Block::Body("Every `Ctrl+S` saves a timestamped copy of the current file to `~/.local/share/zerkalo/snapshots/<project>/<file>/`. The last 50 snapshots per file are kept. Open ☰ → Browse Snapshots… to see the timeline, compare with the current text, and restore any version."),
        Block::Gap,
        Block::H2("How do I use the project dictionary?"),
        Block::Body("Right-click a misspelled word and choose 'Add to Project Dictionary' to save it in `<work_dir>/.zerkalo/dictionary.dic`. This dictionary is project-specific and can be committed to git. 'Add to Dictionary' saves to the global user dictionary at `~/.config/zerkalo/user.dic`."),
        Block::Gap,
        Block::H2("What is the inline error assistant?"),
        Block::Body("Hover over red-underlined text in the editor to see the error message. For known patterns (missing brace, unknown variable, etc.) a 'Fix It' button applies the correction automatically. The fix patterns live in `src/error_patterns.rs`."),
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

pub fn cv_cheatsheet_scroll() -> ScrolledWindow {
    make_rich_tab(cv_cheatsheet_blocks())
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
    pub fn new(parent: &impl IsA<gtk4::Window>, cv_mode: bool) -> Self {
        let window = adw::Window::new();
        window.set_title(Some("Help — Zerkalo"));
        window.set_default_width(720);
        window.set_default_height(600);
        window.set_transient_for(Some(parent));
        window.set_modal(false);

        let header = adw::HeaderBar::new();
        header.add_css_class("fond-chrome");
        let notebook = Notebook::new();
        notebook.set_scrollable(true);

        let cheatsheet_fn: fn() -> Vec<Block<'static>> = if cv_mode {
            cv_cheatsheet_blocks
        } else {
            cheatsheet_blocks
        };

        let tabs: &[(&str, fn() -> Vec<Block<'static>>)] = &[
            ("Overview",   overview_blocks),
            ("Projects",   projects_blocks),
            ("Shortcuts",  shortcuts_blocks),
            ("FAQ",        faq_blocks),
            ("About",      about_blocks),
        ];

        // Cheatsheet tab first, then the rest
        {
            let lbl = gtk4::Label::new(Some("Cheatsheet"));
            let scroll = make_rich_tab(cheatsheet_fn());
            notebook.append_page(&scroll, Some(&lbl));
        }
        for (title, blocks_fn) in tabs {
            let lbl = gtk4::Label::new(Some(title));
            let scroll = make_rich_tab(blocks_fn());
            notebook.append_page(&scroll, Some(&lbl));
        }

        let toolbar = adw::ToolbarView::new();
        toolbar.set_top_bar_style(adw::ToolbarStyle::RaisedBorder);
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

    // The view is created before content is inserted so its style context
    // (attached once mapped) can resolve theme colors for the tags below —
    // see theme::ref_colors.
    let view = TextView::with_buffer(&buf);
    view.set_editable(false);
    view.set_cursor_visible(false);
    view.set_wrap_mode(WrapMode::WordChar);
    view.set_left_margin(20);
    view.set_right_margin(20);
    view.set_top_margin(18);
    view.set_bottom_margin(18);
    view.set_pixels_above_lines(1);
    view.set_monospace(false);

    let colors = theme::ref_colors(&view);

    buf.create_tag(
        Some("h1"),
        &[
            ("weight", &700i32),
            ("scale", &1.5f64),
            ("foreground", &colors.accent.as_str()),
            ("pixels-above-lines", &2i32),
            ("pixels-below-lines", &12i32),
        ],
    );
    buf.create_tag(
        Some("h2"),
        &[
            ("weight", &700i32),
            ("scale", &1.15f64),
            ("pixels-above-lines", &16i32),
            ("pixels-below-lines", &6i32),
        ],
    );
    buf.create_tag(
        Some("body"),
        &[("scale", &1.0f64), ("pixels-below-lines", &6i32)],
    );
    buf.create_tag(
        Some("code"),
        &[
            ("family", &"Monospace"),
            ("scale", &0.95f64),
            ("background", &colors.code_bg),
            ("background-full-height", &true),
            ("pixels-above-lines", &6i32),
            ("pixels-below-lines", &6i32),
            ("left-margin", &16i32),
            ("right-margin", &12i32),
        ],
    );
    buf.create_tag(
        Some("inline-code"),
        &[
            ("family", &"Monospace"),
            ("scale", &0.92f64),
            ("background", &colors.inline_bg),
            ("foreground", &colors.inline_fg.as_str()),
            ("weight", &600i32),
        ],
    );

    let mut iter = buf.end_iter();
    for block in blocks {
        match block {
            Block::H1(text) => insert_inline(&buf, &mut iter, text, "h1"),
            Block::H2(text) => insert_inline(&buf, &mut iter, text, "h2"),
            Block::Body(text) => insert_inline(&buf, &mut iter, text, "body"),
            Block::Code(text) => insert_with_tag(&buf, &mut iter, &format!("{text}\n"), "code"),
            Block::Gap => buf.insert(&mut iter, "\n"),
        }
    }

    let scroll = ScrolledWindow::new();
    scroll.set_hexpand(true);
    scroll.set_vexpand(true);
    scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
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

/// Inserts `text` tagged with `base_tag`, additionally applying the
/// `inline-code` tag to any `` `backtick-quoted` `` spans within it — lets
/// prose call out key names, function names, and shortcuts (e.g. the `!`
/// autocomplete trigger) without a whole separate code block.
fn insert_inline(buf: &TextBuffer, iter: &mut TextIter, text: &str, base_tag: &str) {
    let mut in_code = false;
    for segment in text.split('`') {
        if segment.is_empty() {
            in_code = !in_code;
            continue;
        }
        let start_offset = iter.offset();
        buf.insert(iter, segment);
        let start = buf.iter_at_offset(start_offset);
        let tag_table = buf.tag_table();
        if let Some(tag) = tag_table.lookup(base_tag) {
            buf.apply_tag(&tag, &start, iter);
        }
        if in_code {
            if let Some(tag) = tag_table.lookup("inline-code") {
                buf.apply_tag(&tag, &start, iter);
            }
        }
        in_code = !in_code;
    }
    buf.insert(iter, "\n");
    let end = buf.iter_at_offset(iter.offset() - 1);
    if let Some(tag) = buf.tag_table().lookup(base_tag) {
        buf.apply_tag(&tag, &end, iter);
    }
}
