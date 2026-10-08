use gtk4::prelude::*;
use gtk4::{ScrolledWindow, TextBuffer, TextIter, TextView, WrapMode};
use libadwaita as adw;
use libadwaita::prelude::*;

use super::theme;

// ── Rich-text section DSL ─────────────────────────────────────────────────────

pub(crate) enum Block<'a> {
    H1(&'a str),
    H2(&'a str),
    Body(&'a str),
    Code(&'a str),
    /// Same rendering as `Code`, for text built at runtime (e.g. from live
    /// keybindings) rather than a string literal.
    CodeOwned(String),
    Gap,
}

// ── Tab content ───────────────────────────────────────────────────────────────

fn overview_blocks() -> Vec<Block<'static>> {
    vec![
        Block::H1("Zerkalo — Typst Editor"),
        Block::Body("Zerkalo is a quiet place to write. You type, a live preview shows the finished pages beside you, and your work is kept safe — saved as you go, backed up online if you like, and always recoverable from ☰ → Recover…. Nothing else needs installing."),
        Block::Gap,
        Block::H2("Getting started"),
        Block::Body("Zerkalo keeps your documents in your work folder (~/Documents/Zerkalo by default). Open ones sit in the tab bar above the editor, and the Library button beside the sidebar toggle (or Ctrl+L) shows all of them."),
        Block::Gap,
        Block::Body("New here? ☰ → Help & About → Take the Tour walks through the essentials again, and What Things Do (or F1) labels any button or panel on screen with what it does — both run inside the editor and inside New from Template."),
        Block::Gap,
        Block::Body("Create a new document from the hamburger menu (≡) or use New from Template… for a complete preamble. You only choose a name: the document is saved in your work folder and listed in the Library, so its fonts and bibliography always resolve. The left sidebar switches between the document outline, your project's files, and a symbol insert panel."),
        Block::Gap,
        Block::Body("≡ → Export… asks where to save and can open the result for you when it's done — PDFs and EPUBs in Pereplyot if it's installed, anything else in your default app. It can also write a .bib or .yaml file holding only the references your document actually cites."),
        Block::Gap,
        Block::Body("For a designer, export Word for InDesign or Word for Canva. Both use a named style for every paragraph (Heading 1, Body Text, First Paragraph, Block Quote, Bibliography…) set in your document's fonts, so the layout program can keep or remap them in one step. In the InDesign file, citations, footnotes and margin notes are real footnotes, numbered together. Canva has no footnotes, so its file numbers them in the text and lists them under Notes at the end. Page layout (columns, margins, drop caps) is left for the layout program."),
        Block::Gap,
        Block::Body("Once a template's settings are how you want them, press the save button beside \"Your Templates\" in that dialog to keep them under a name. Saved templates sit under the built-in presets and start a document exactly the way the last one started — the title, date, abstract and keywords are left out, since those belong to a single document rather than to a template."),
        Block::Gap,
        Block::H2("Layout"),
        Block::Code("Left sidebar   Document outline, symbols, files, refs, history\nEditor         Tab bar (drag to reorder, right-click for more), syntax-highlighted Typst editor\nFind bar       Persistent search/replace at editor bottom\nPreview        Live rendered output — use +/− to zoom\nThings to look at       What needs attention, in plain language"),
        Block::Gap,
        Block::H2("Backing up & working on more than one computer"),
        Block::Body("Autosave (the \"autosave\" word in the status bar, bold when on) saves your document a few seconds after you stop typing, and whenever you switch to another window or tab, compile, export or quit. Ctrl+S still saves too, and is what adds an entry to Version History."),
        Block::Gap,
        Block::Body("Click the sync button (⟳) or press `Ctrl+Shift+S` to save a version of everything and send it up. If nothing is set up yet, ☰ → Set Up Zerkalo walks you through it: sign in with GitHub and press Finish, and the rest — the repository, who the versions are recorded as, and the first upload — is done for you. Nothing to install, and a folder or drive works instead of an account."),
        Block::Gap,
        Block::Body("Sitting down at another computer? When Zerkalo opens it checks GitHub quietly. A computer that is simply behind catches up by itself; if you have work here that isn't backed up yet, you're offered \"Get it\" instead of anything being changed. You can also use ☰ → Get Latest from GitHub at any time: it brings everything backed up online down to this computer and sends nothing back. Work you left on the other computer only arrives if it was backed up there first — press the sync button before you switch."),
        Block::Gap,
        Block::Body("If the same part of a document was changed in both places, Zerkalo changes nothing and says so. You can then choose GitHub's version (or ☰ → Version History → Replace This Computer's Copy with GitHub's…): everything that was only on this computer is first kept in a saved copy, so nothing is lost."),
        Block::Gap,
        Block::H2("Getting something back"),
        Block::Body("☰ → Recover… (or Ctrl+K → Recover) is the one place for it. It opens by saying how safe your work is — backed up online or not, and when it last was — and asks what you want back: an earlier version of the open document, something from a saved copy, a deleted document, or the latest writing from another computer."),
        Block::Gap,
        Block::Body("Earlier versions of a document appear in one timeline, newest first: the ones saved on this computer each time you save, and the ones backed up online. Click one to see what it would bring back, with the changes highlighted. Type a few words you remember into the search box to find the versions that contain them. \"Bring back as a copy\" saves that text as a new file beside the original and opens it, so nothing is overwritten. \"Replace what's here…\" puts it into the open document instead; it asks first, keeps the current text as a saved version, and Ctrl+Z undoes it."),
        Block::Gap,
        Block::H2("Making Zerkalo easier on the eyes"),
        Block::Body("Ctrl+= makes the writing bigger, Ctrl+- smaller and Ctrl+0 puts it back; Zerkalo remembers. Zerkalo follows your system's own settings too: if you've asked for high contrast or reduced animation in your desktop's Accessibility settings, it does that without anything to switch on here (Settings → Editor → High contrast adds a stronger editor look on top). Every button that is only an icon has a name a screen reader can read, and changes in comparisons are shown with underline and strikethrough as well as colour."),
        Block::Gap,
        Block::H2("Which font is easiest to read?"),
        Block::Body("It's personal, and Zerkalo uses any font installed on your computer. Many people find Atkinson Hyperlegible (made for low vision) or OpenDyslexic comfortable. Install one the way you install any font for your system, then choose it under Settings → Editor → Editor font. Ctrl+= and Ctrl+- change the size."),
        Block::Gap,
        Block::H2("Is my work safe?"),
        Block::Body("The quiet line at the left of the status bar always answers that: \"Saving as you write\", \"Saved on this computer\", or \"Saved · backed up 12 min ago\" once an online backup is set up. If a backup doesn't go through, a strip above the editor says so — your work is still saved on this computer — with a Details button, and it clears itself the next time a backup works."),
        Block::Gap,
        Block::H2("The Library"),
        Block::Body("The Library (Ctrl+L) lists every document. Give a document labels (right-click → Organize…, or drag it onto a label or project in the sidebar); a label like \"Course › Ethics\" nests under \"Course\". Authors you cite appear on their own, as \"Surname, I.\", so you can see everything that cites a given author. Search reads the full text of your documents. Deleted documents wait in the Trash; a document whose file was moved shows \"missing\" until you use Locate File…. Your labels, projects and pins are also saved as small readable files in your folder (one subfolder per computer), so they travel with your backup. Notes are included only if you turn that on in the Library's View menu."),
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
        Block::Code("@authorYear                   In-text citation\n@authorYear[p.~5]             With page locator\n@a @b @c                      Multiple sources (footnote styles:\n                              one footnote each, marked ¹,²,³ —\n                              or one combined note, via\n                              Style → \"As the style specifies\")\n\n#bibliography(\"refs.bib\", style: \"chicago-author-date\")\nStyles: \"apa\", \"mla\", \"chicago-author-date\",\n        \"chicago-notes\", \"ieee\", \"harvard-cite-them-right\",\n        \"gost-r-705-2008\""),
        Block::Gap,
        Block::H2("Figures & Cross-references"),
        Block::Code("#figure(\n  image(\"fig.png\", width: 80%),\n  caption: [Caption text.],\n) <fig-label>\n\nAs shown in @fig-label, the results indicate…\n\nThe image button (or dragging a picture onto the editor)\ncopies it into an assets folder beside your document first,\nso the document keeps working if the original moves."),
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
        Block::Code("#outline()             Table of contents\n#outline(target: figure.where(kind: table))\n                       List of tables\n#pagebreak()           Page break\n#colbreak()            Column break\n#h(1em)                Horizontal space\n#v(1em)                Vertical space\n#line(length: 100%)    Horizontal rule (― on the format bar)"),
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
        Block::H2("Backing up"),
        Block::Code("Ctrl+Shift+S   Save a version and back it up online"),
    ]
}

/// Reads live keybindings so a rebind updates this tab too — it used to be a
/// fully static list sitting right next to the hamburger's "Keyboard
/// Shortcuts" row, which reads `keybindings.toml` live; the two disagreed
/// after any rebind. Only the ~11 actions that are actually rebindable
/// (`crate::keybindings::Keybindings`) are substituted; everything else here
/// is fixed and stays a string literal.
fn shortcuts_blocks() -> Vec<Block<'static>> {
    let kb = crate::keybindings::Keybindings::load();
    let d = crate::keybindings::display_binding;
    let save = d(&kb.save);
    let find = d(&kb.find);
    let next_tab = d(&kb.next_tab);
    let prev_tab = d(&kb.prev_tab);
    let compile = d(&kb.compile);
    let palette = d(&kb.command_palette);
    let git_sync = d(&kb.git_sync);
    let shortcuts_help = d(&kb.shortcuts_help);
    let help_overlay = d(&kb.help_overlay);
    let quit = d(&kb.quit);

    vec![
        Block::H1("Keyboard Shortcuts"),
        Block::Gap,
        Block::H2("Editing"),
        Block::CodeOwned(format!(
            "{save:<20}Save current file\n{find:<20}Find & Replace\n{next_tab:<20}Next tab\n{prev_tab:<20}Previous tab\nCtrl+Left/Right     Word jump (Typst-aware: treats #keyword and @cite as units)\nCtrl+Shift+Up/Down  Jump to previous / next heading in the document\nF8 / Shift+F8       Go to the next / previous problem\nCtrl+D              Duplicate line or selection\nCtrl+/              Toggle line comment\nAlt+Up / Alt+Down   Move the line (or selected lines) up / down\nAlt+Shift+Right     Select more: word, brackets, line, paragraph, section, document\nAlt+Shift+Left      Select less again\nCtrl+Enter          Insert page break\nEnter in a list     Starts the next item (Enter on an empty item ends the list)\nTab / Shift+Tab     Nest / un-nest a list item\nCtrl+V on a selection  A copied web address becomes a link on the selected text\nMiddle-click tab    Close tab\nRight-click tab     Duplicate, close others, delete"
        )),
        Block::Gap,
        Block::H2("Compiling & Preview"),
        Block::CodeOwned(format!(
            "{compile:<20}Compile and refresh preview\nCtrl+Shift+E        Export PDF to the Export dialog's remembered folder (no dialog)\nCtrl+P              Print — printer, page range, layout, copies, all on one sheet\nAuto-compile        Fires automatically after each change\nCtrl+click preview  Jump to that exact spot in the source (so does a double-click)\nCtrl+click text     Show that spot in the preview"
        )),
        Block::Gap,
        Block::H2("Navigation"),
        Block::CodeOwned(format!(
            "{palette:<20}Command palette (commands + headings)\nCtrl+G              Command palette pre-filtered to headings only\nCtrl+= / Ctrl+-     Bigger / smaller text (Ctrl+0 resets)\nCtrl+Shift+F        Find in Files (project-wide search)"
        )),
        Block::Gap,
        Block::H2("Autocomplete"),
        Block::Code("#                   Inline suggestion — a preview of what will be inserted appears\n                    dim after the cursor, with its signature in the status bar\nTab                 Accept the inline suggestion\n#xx                 After two characters, a short list of matches opens too\n                    (matches anywhere in the name: #break finds pagebreak)\n↑ / ↓               Navigate the list — the status bar describes each entry\nTab / Return        Accept the selected entry from the list\n@                   Citation popup (requires a .bib file)\nEsc                 Dismiss for this word — your text is left alone\n                    (clicking elsewhere dismisses too)\n@ / !               Citations and CV entries behave the same way"),
        Block::Gap,
        Block::H2("Import"),
        Block::Code("Ctrl+Shift+I        Open the Import picker (LaTeX/Word/Markdown/ODT/HTML/EPUB/RTF/PDF)\nCtrl+Shift+V        Paste as Document (reads clipboard text as Markdown)\nDrag & drop         Drop a document file onto the editor to import it directly"),
        Block::Gap,
        Block::H2("What things do"),
        Block::CodeOwned(format!(
            "{help_overlay:<20}Label every button and panel on screen, in place\nEsc                 Take the labels away (clicking anywhere does too)"
        )),
        Block::Gap,
        Block::H2("Git & Window"),
        Block::CodeOwned(format!(
            "{git_sync:<20}Save a version and back up\n{shortcuts_help:<20}Show keyboard shortcuts\nCtrl+R              Refresh file tree\nF6                  Jump to the project files\n{quit:<20}Quit\nCtrl+?              Open this help window\nSidebar button      Toggle left sidebar\nInsert button       Toggle insert snippets panel\nPop-out button      Open preview in a separate window"
        )),
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
        Block::Body("Zerkalo has a built-in Typst compiler — no external binary is needed. If the preview looks dimmed with a \"Preview paused\" note, the document has something to look at: click the “things to look at” count at the bottom to open that panel, which shows the file, line number, and a plain-English explanation, with the exact spot underlined in the quoted line. Press F8 (Shift+F8 for back) to step through problems without opening the panel; the ⋯ menu on each problem can copy the details or search the Typst forum. Switch on \"Show technical details\" in that panel to see Typst's exact wording. If the same problem is still there after a couple of minutes, a \"Still stuck?\" section appears: it shows what changed since the document last worked, can put it back the way it was (Ctrl+Z undoes that), and can copy a help request to paste into an email or the Typst forum. If Zerkalo can't save a file (the disk is full, the folder is read-only or has moved), a bar above the editor says which file and why, and stays until it works: your changes are still in the window, and \"Try again\" or \"Save a copy elsewhere…\" gets them safe."),
        Block::Gap,
        Block::H2("Changing the style gives a compile error"),
        Block::Body("If you see 'expected string or function' after changing a style, your document may have a conflicting `#show heading` rule outside the template block. Fix it by opening 'Change Document Style' (sidebar button or ≡ → Document Tools) and re-applying your style. That rewrites the formatting section cleanly."),
        Block::Gap,
        Block::H2("The style dropdown doesn't seem to do anything"),
        Block::Body("For template documents (created with 'New from Template' or imported via File → Import), styles are applied inside the template block. If the heading appearance doesn't change, open the “Things to look at” panel — a problem is likely preventing the preview from updating. The button label always shows just the style name; it no longer includes the filename."),
        Block::Gap,
        Block::H2("Table of Contents / abstract / keywords not appearing"),
        Block::Body("Use 'Change Document Style' (sidebar button or ≡ → Document Tools → Change Document Style…). Switch to the Sections tab and toggle Table of Contents, Abstract, or Keywords on. Click 'Apply to Current' — Zerkalo will insert or remove those sections in the document body."),
        Block::Gap,
        Block::H2("Citation keys show as errors"),
        Block::Body("Citations need a bibliography. Open the Citations panel and use its Sources menu to choose a file, choose a Kartoteka vault, or start a new one — Zerkalo then points the document at it for you. Or add this line to your document yourself:\n   `#bibliography(\"refs.bib\", style: \"chicago-author-date\")`\n(adjusting the filename and style). The document's own line always wins; otherwise the project's setting, then Settings, then a bibliography found in the project folder. To drop the reference list, comment the line out (Ctrl+/): citations and footnotes keep working from the file it named, or your vault if that file is gone."),
        Block::Gap,
        Block::H2("Imported LaTeX / DOCX file has formatting problems"),
        Block::Body("After import, use 'Change Document Style' to set the correct style, paper size, and font for your document. The import process preserves the text content and moves all formatting rules into the template block, which Zerkalo controls."),
        Block::Gap,
        Block::H2("Suggestions while typing aren't appearing"),
        Block::Body("tinymist is bundled at /usr/lib/zerkalo/tinymist when installed via the .deb or .rpm package — no extra step needed. For source builds, install it manually:"),
        Block::Code("cargo install tinymist"),
        Block::Gap,
        Block::H2("How do I change the work folder?"),
        Block::Body("Open Settings from the hamburger menu (≡) and change the Work folder path. The work folder is where Zerkalo looks for your .typ documents (default: ~/Documents/Zerkalo)."),
        Block::Gap,
        Block::H2("Can I use a custom bibliography?"),
        Block::Body("Yes — use the Sources menu in the Citations panel (Choose a file…), or set `bib_path` in Settings or in `.zerkalo/config.toml`. If the file lives outside your project, Zerkalo keeps an up-to-date copy of it inside (marked as Zerkalo's, named references.bib) and points the document at the copy, so the document compiles on any computer your folder syncs to; untick \"Keep a copy in the project\" in the Sources menu to keep it linked instead. Zerkalo never overwrites a file of yours."),
        Block::Gap,
        Block::H2("Can I point Zerkalo at a Kartoteka vault instead of a .bib file?"),
        Block::Body("Yes — in the Citations panel's Sources menu choose \"Choose a Kartoteka vault…\" (or use the folder-browse button in Settings → Bibliography). Entries load live: add or edit something in Kartoteka and it appears in the citation popup, sidebar, and reference manager within a second or two, with no restart. The reference manager's rename/add-entry/export actions stay .bib-only, since the vault is edited in Kartoteka, not Zerkalo."),
        Block::Gap,
        Block::H2("How does auto-compile work?"),
        Block::Body("After each keystroke, Zerkalo starts a debounce timer (default 800 ms). When it fires without further changes, it saves all modified files and compiles using the embedded Typst engine. The delay is configurable in Settings."),
        Block::Gap,
        Block::H2("Where are log files?"),
        Block::Code("~/.local/share/zerkalo/zerkalo.log"),
        Block::Gap,
        Block::H2("How do I set up git sync?"),
        Block::Body("Open ☰ → Set Up Zerkalo and press 'Set this up', then 'Sign in with GitHub'. Approve the short code shown at github.com/login/device, confirm the repository name, and press Finish — Zerkalo creates the repository, makes the work folder a git repository, records your name and address from the GitHub account (so you never type them), and pushes the first version. If you'd rather not use GitHub, the same screen offers backing up to a folder or drive instead, or pasting the address of a repository you already have. Nothing needs installing: git ships inside Zerkalo. After that, `Ctrl+Shift+S` saves a version and pushes it. On a second computer, run Set Up Zerkalo and choose \"I already have an online copy\": your writing is brought down first."),
        Block::Gap,
        Block::H2("How do I cite from Zotero?"),
        Block::Body("In the Citations panel's Sources menu, choose \"Connect Zotero…\". It explains installing the Better BibTeX add-on and exporting your library as \"Better BibLaTeX\" with \"Keep updated\", ideally into your Zerkalo folder; choose that file and new sources appear in Zerkalo within seconds. With Zotero open, \"Pick from Zotero…\" opens Zotero's own picker and inserts the @keys you choose. Zerkalo only talks to Zotero on this computer."),
        Block::Gap,
        Block::H2("What is \"Freeze for submission\"?"),
        Block::Body("Sources menu → Freeze for submission saves a small <document>-references.bib holding only the sources the document cites, and points the document at it — one complete, tidy bibliography to hand in with the essay. Your main library isn't changed, and Ctrl+Z puts the document back."),
        Block::Gap,
        Block::H2("Can I edit the title, author, or date directly in the document?"),
        Block::Body("Yes — template documents store metadata as plain Typst variables near the top of the file:\n  #let doc-title = \"My Paper\"\n  #let doc-author = \"Jane Smith\"\n  #let doc-date = \"5 June 2026\"\nEdit these directly in the editor. When you open 'Change Document Style' afterwards, Zerkalo reads the values from the document so the dialog will show your edits, not the old saved values."),
        Block::Gap,
        Block::H2("I built from source but changes aren't appearing"),
        Block::Body("Run `cargo build --release` first, then `bash install.sh`. The install script detects a local build and installs it directly. For end users without Rust, the recommended path is to download the .deb or .rpm from the GitHub releases page."),
        Block::Gap,
        Block::H2("How do compilation profiles work?"),
        Block::Body("The header-bar dropdown next to 'Preview' switches between Final (full 144 dpi) and Draft (72 dpi, fast) profiles. In Draft mode Zerkalo passes sys.inputs.at(\"draft\") = \"true\" so documents can skip slow elements:\n  #if sys.inputs.at(\"draft\", default: \"false\") == \"true\" {\n    // skip heavy rendering in draft\n  }"),
        Block::Gap,
        Block::H2("I lost something — how do I get it back?"),
        Block::Body("Open ☰ → Recover… and choose what you lost. An earlier version of a document, a file from a saved copy made before \"Replace This Computer's Copy with GitHub's…\", a deleted document (in the Trash), or the latest from another computer are all there. Bringing something back makes a copy beside the original unless you choose to replace the text, so it is safe to try."),
        Block::Gap,
        Block::H2("How do snapshots work?"),
        Block::Body("Every `Ctrl+S` saves a timestamped copy of the current file to `~/.local/share/zerkalo/snapshots/<project>/<file>/` on this computer. The last 100 snapshots per file are kept. They are not backed up online and don't travel to other computers — the sync button's versions do. Open ☰ → Recover… to see them in one timeline together with the versions backed up online, search them, compare with the current text, and bring any back as a copy (☰ → Version History → Saved Versions… still opens the older list)."),
        Block::Gap,
        Block::H2("How do I use the project dictionary?"),
        Block::Body("Right-click a misspelled word and choose 'Add to Project Dictionary' to save it in `<work_dir>/.zerkalo/dictionary.dic`. This dictionary is project-specific and can be committed to git. 'Add to Dictionary' saves to the global user dictionary at `~/.config/zerkalo/user.dic`."),
        Block::Gap,
        Block::H2("What is the inline error assistant?"),
        Block::Body("Hover over red-underlined text in the editor to see the error message. For known patterns (missing brace, unknown variable, etc.) a 'Fix It' button applies a small, exact correction — for example putting a backslash in front of a stray $ or @ so it's shown as ordinary text. Fixes undo with Ctrl+Z, and if one doesn't help you're offered Undo. Problem wording and fixes live in `src/diagnostic_catalog.rs` and `locales/en/diagnostics.ftl`."),
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
        window.set_default_width(760);
        window.set_default_height(640);
        window.set_transient_for(Some(parent));
        window.set_modal(false);

        let header = adw::HeaderBar::new();
        header.add_css_class("fond-chrome");

        // An AdwViewStack driven by a header AdwViewSwitcher rather than a raw
        // GtkNotebook — same pill-tab pattern as Settings, so Help matches the
        // rest of the app instead of looking like a GTK3 leftover.
        let view_stack = adw::ViewStack::new();
        view_stack.set_vexpand(true);

        let cheatsheet_fn: fn() -> Vec<Block<'static>> = if cv_mode {
            cv_cheatsheet_blocks
        } else {
            cheatsheet_blocks
        };

        // Overview leads so Ctrl+? gives a first-time user plain-language
        // orientation before the raw-syntax Cheatsheet — ViewStack shows
        // whichever tab was added first.
        let tabs: &[(&str, &str, &str, fn() -> Vec<Block<'static>>)] = &[
            (
                "overview",
                "Overview",
                "dialog-information-symbolic",
                overview_blocks,
            ),
            (
                "cheatsheet",
                "Cheatsheet",
                "text-x-generic-symbolic",
                cheatsheet_fn,
            ),
            ("projects", "Projects", "folder-symbolic", projects_blocks),
            (
                "shortcuts",
                "Shortcuts",
                "input-keyboard-symbolic",
                shortcuts_blocks,
            ),
            ("faq", "FAQ", "dialog-question-symbolic", faq_blocks),
            ("about", "About", "help-about-symbolic", about_blocks),
        ];

        for (tag, title, icon, blocks_fn) in tabs {
            let scroll = make_rich_tab(blocks_fn());
            let sp = view_stack.add_titled(&scroll, Some(tag), title);
            sp.set_icon_name(Some(icon));
        }

        let switcher = adw::ViewSwitcher::new();
        switcher.set_stack(Some(&view_stack));
        switcher.set_policy(adw::ViewSwitcherPolicy::Wide);
        header.set_title_widget(Some(&switcher));

        let toolbar = adw::ToolbarView::new();
        toolbar.set_top_bar_style(adw::ToolbarStyle::RaisedBorder);
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&view_stack));
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
    view.set_left_margin(28);
    view.set_right_margin(28);
    view.set_top_margin(22);
    view.set_bottom_margin(22);
    view.set_pixels_above_lines(1);
    view.set_monospace(false);

    let colors = theme::ref_colors(&view);

    // Hierarchy comes from weight, scale, and whitespace only — no color, no
    // background band. An accent-tinted heading and a filled-grey code block
    // were tried and read as clunky/technical rather than polished.
    buf.create_tag(
        Some("h1"),
        &[
            ("weight", &700i32),
            ("scale", &1.5f64),
            ("pixels-above-lines", &2i32),
            ("pixels-below-lines", &12i32),
        ],
    );
    buf.create_tag(
        Some("h2"),
        &[
            ("weight", &700i32),
            ("scale", &1.15f64),
            ("pixels-above-lines", &22i32),
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
            ("scale", &0.92f64),
            ("pixels-above-lines", &10i32),
            ("pixels-below-lines", &10i32),
            ("left-margin", &44i32),
            ("right-margin", &28i32),
        ],
    );
    buf.create_tag(
        Some("inline-code"),
        &[
            ("family", &"Monospace"),
            ("scale", &0.92f64),
            ("background", &colors.inline_bg),
        ],
    );

    let mut iter = buf.end_iter();
    for block in blocks {
        match block {
            Block::H1(text) => insert_inline(&buf, &mut iter, text, "h1"),
            Block::H2(text) => insert_inline(&buf, &mut iter, text, "h2"),
            Block::Body(text) => insert_inline(&buf, &mut iter, text, "body"),
            Block::Code(text) => insert_with_tag(&buf, &mut iter, &format!("{text}\n"), "code"),
            Block::CodeOwned(text) => {
                insert_with_tag(&buf, &mut iter, &format!("{text}\n"), "code")
            }
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
