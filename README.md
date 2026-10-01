# Zerkalo

A contemplative [Typst](https://typst.app) editor built with Rust, GTK4, and libadwaita.
Live preview · Document library · Academic styles · LSP completions · Git-backed sync & history.

Zerkalo embeds the Typst compiler directly — there's no external `typst` binary to install, and no
raw markup dead-end: a live preview pane always shows the real formatting next to what you type.

---

## Features

### Editor
| Feature | Detail |
|---|---|
| **Multi-file tabs** | A tab bar of equal-width tabs above the editor: drag to reorder, close button on each, right-click for Duplicate / Close / Close Others / Close to the Right / Delete; unsaved dot; error icon on compile failure; project file tree as a sidebar page |
| **Syntax highlighting** | Full Typst grammar via GtkSourceView 5 |
| **Inline completions** | `#` shows the best match dim after the cursor, previewing what will be inserted; Tab accepts, and a compact ranked list joins in after two characters. Backed by [tinymist](https://github.com/Myriad-Dreamin/tinymist) where available |
| **Built-in snippets** | Academic snippets (figure, table, footnote, bibliography, …) prepended to the LSP popup |
| **Citation autocomplete** | `@` (BibTeX keys) and `!` (Skrizhal CV entries) behave the same way — inline suggestion, description in the status bar |
| **Bibliography sources** | A `.bib` file — including a library exported from Zotero, Mendeley, or any other reference manager as BibTeX — a Hayagriva `.yaml` file, or a [Kartoteka](https://github.com/calstfrancis/kartoteka) vault folder — vault entries refresh live via `fond-vault`'s filesystem watch, no restart needed |
| **First-run bibliography** | The Citations panel can start a brand-new `.bib` file with one click — no need to already have one before adding your first source |
| **Inline diagnostics** | Problems shown as underlines in the editor, a quiet count at the bottom and a dimmed "Preview paused" note over the last good preview; warnings are just "notes". The Problems panel translates Typst's message into plain language, quotes the line with the exact characters at fault underlined, and offers one main button (Fix, or Show me) plus a ⋯ menu (copy details, search the Typst forum); F8 / Shift+F8 step through problems. A "Show technical details" switch adds the exact wording. If you stay stuck, a "Still stuck?" section shows what changed since it last worked, can go back to that version, and can copy a help request. A failed save (full disk, read-only folder…) stays as a bar above the editor with Try again / Save a copy elsewhere, and other failures give a plain-language reason |
| **Find & Replace** | `Ctrl+F`; forward/backward; animated slide-in bar; replace one or all |
| **Find in Files** | `Ctrl+Shift+F`; project-wide search |
| **Spell check** | Blue wavy underlines on misspelled prose words; right-click for suggestions, Ignore All; language selector in Settings; optional autocorrect on word boundary |
| **Breadcrumb bar** | Heading path shown above the editor (e.g. "Introduction › Methods") updated as the cursor moves |
| **Auto-pair brackets** | Typing `(`, `[`, `{`, or `"` inserts the closing character and positions the cursor between them |
| **Typewriter scrolling** | Optional (Settings → Editor); cursor stays fixed at ~45 % from the top of the viewport |
| **Line spacing** | Settings → Editor → Compact / Normal / Spacious |
| **High contrast mode** | Settings → Editor → High contrast; forces white-on-black in the editor |
| **Theme** | System / Light / Dark, set from the hamburger menu — follows libadwaita's colour scheme by default |
| **Word count** | Live count and reading-time estimate in the status bar; selection shows "N words, M sentences selected" |
| **Word-count goal** | Add `// @goal: 3000` in your file; a progress ring tracks it in the status bar |
| **Session delta** | Status bar shows `↑ N` words added since the file was opened |
| **Cursor position** | Line and column in the editor status bar |
| **Template toggle** | Off by default — the "show template"/"hide template" status-bar button shows or hides the document's technical setup lines above the body, so you can focus on writing prose; change them from the Template button instead |
| **Focus Mode** | Hides the sidebar and secondary panels for distraction-free writing |
| **What things do (F1)** | Labels every panel and control on screen with a bubble explaining it, drawn over the running window so the program stays visible underneath; Escape or a click dismisses. Covers the main editor window, the Library window, and the New from Template / Change Document Style dialog. Also reachable from ☰ → Help & About → What Things Do, not just the F1 shortcut |
| **Guided tour** | A short, step-by-step walkthrough of the essentials — editor, preview, template, library, sync, compile mode — shown automatically the first time Zerkalo runs, and replayable anytime from ☰ → Help & About → Take the Tour |
| **Command palette** | `Ctrl+K`; fuzzy search over app commands and document headings; `Ctrl+G` for headings only |
| **Session restore** | Open files, active tab, and cursor positions are restored on next launch |
| **Save-before-close** | Closing with unsaved files shows a dialog listing modified files with Save All / Discard / Cancel |
| **Configurable keybindings** | Edit `~/.config/zerkalo/keybindings.toml` to remap any shortcut |

### Sidebar
| Feature | Detail |
|---|---|
| **Document outline** | Heading tree with cursor-tracking highlight; click to centre and select the heading in the editor; a folder toggle switches to a manuscript-wide view — headings and word counts rolled up across every file reachable from the project root via `#include`/`#import`, not just the open one |
| **Symbol insert** | One-click insertion of Cyrillic, Greek, Hebrew, Sanskrit, and common math symbols/operators (∑, ∫, ≤, ∈, →, ℝ, and more) — the math ones insert as plain Unicode, which Typst renders correctly inside `$...$` |
| **File tree** | Project `.typ` files; collapsible subdirectory headers; click to open; `+` / folder buttons to create files or folders; drag to reorder; the compilation root is marked in bold; right-click for Set as Root, Insert `#include`/`#import` (each with a tooltip explaining what it does), Delete |
| **Citation panel** | Searchable list of all bibliography entries; double-click or Enter inserts the `@key` at the cursor; a + button starts a new bibliography if none is set yet |
| **Comments** | Threaded, resolvable comments anchored to a line — not edited into the Typst source, so a comment can never break compilation or leak into an export. `+` leaves a note at the cursor's current line; click a comment to jump to it. Anchors survive edits elsewhere in the document (re-located by matching the commented line's text, not just its line number) |
| **Suggested edits from Word** | Importing a `.docx` with track changes turns its `<w:ins>`/`<w:del>` runs into pending suggestions in the Comments panel — both the proposed addition and the proposed removal are inlined into the document so you review them in context, with Accept/Reject buttons per suggestion. Accepting a deletion (or rejecting an insertion) removes that exact text from the document; the reverse choice just marks it resolved and leaves the text as-is |
| **Autosave** | On by default — saves the document a few seconds after you stop typing and whenever you switch away, compile, export or quit; toggle it with "autosave" in the status bar. Separate from Snapshots, which Ctrl+S still takes |
| **Snapshots** | Local, automatic version history on every save, with a clean diff view and a confirmed restore |
| **File History** | Git-backed history of a synced document's earlier versions and what changed, shown without leaving the app |
| **Dependency graph** | Visualises which files `#include`/`#import` which — an opt-in view for multi-file projects |
| **Package browser** | Lists Typst packages already downloaded to the local cache, with one-click `#import` insertion |

### Multi-file projects
| Feature | Detail |
|---|---|
| **New Project wizard** | ≡ → New Project… — names, slugifies, and creates a project folder with starter files; templates: Blank, Essay, Journal / Thesis, Theological Journal |
| **Compilation root** | One file is the Typst entry point; Zerkalo auto-detects it from the import graph or reads `.zerkalo/config.toml`; marked in bold in the file tree |
| **Root switcher** | Turn on "project" beside the document title → Set…; or right-click any file → Set as Root File; writes `root_file` to `.zerkalo/config.toml` and recompiles |
| **#include / #import helper** | Right-click a file in the tree → Insert `#include` or Insert `#import`; path is automatically relative to the root's directory |
| **Project config** | `.zerkalo/config.toml` inside the project folder — overrides `root_file`, `bib_path`, `file_order` for that project |

### Document Library (`Ctrl+L`, or the Library button at the top left)
| Feature | Detail |
|---|---|
| **SQLite-backed library** | Every `.typ` document Zerkalo knows about, with search, sort, and filter |
| **Search** | Looks through every document by default — titles, the text inside them, notes, file names, labels and the authors they cite — as you type, with accents ignored (`zizek` finds Žižek) and half-typed words matched. Each result shows a line of the document around what was found, with the match in bold. Searching from inside a project or label says so and offers one button to hold the search to that view; Escape drops the search. `Ctrl+F` or `/` jumps to the search box |
| **Kept in your folder** | Labels, projects, pins, renamed titles and notes are also written, a few seconds after you change them, to small text files under `.zerkalo/library/` in your Zerkalo folder, so they back up and travel with it (a document's `essay.typ.toml`, a project's file, `labels.toml`, and a README). Notes go in too, so keep that in mind if the folder is pushed somewhere public. The Library's View menu switches it off. It never touches a document and never deletes, writes atomically, and updates a file only if it still holds exactly what it last wrote — a file that arrived from another machine, was edited, or was already there is left alone |
| **…and brought back from it** | On a new machine, after moving the folder, or when another machine's changes arrive in it, Zerkalo merges what the folder holds into the Library — once at startup and when the files change. It only adds: labels and project members are added, never removed; a pin, archive or renamed title follows a change made elsewhere only if you haven't changed it here; notes are never lost (if both sides differ, both are kept); nothing is deleted anywhere. A copy of the whole library is saved first (the last five are kept) and a notice says what was brought in. A file Zerkalo can't read, or whose document isn't on this machine yet, is left alone and tried again later. So removing a label on one machine doesn't remove it on another — that isn't synced, by design |
| **Missing files** | A document whose file has been moved or deleted is dimmed and marked "missing"; its ⋯ menu offers **Locate File…** to point the Library at the new place |
| **Organisation** | Two ideas: **Projects** (documents that belong together, in an order, with a root file) and **Labels** (your own words, any number per document, each with a colour — automatic unless you choose one). A nested name like `Liturgy › Advent` does the job of a sub-category. Sidebar views: All Documents, Recent, Needs a label, each project, each label, Authors, Archive and Trash. A label or project's ⋯ menu (or right-click) renames, recolours and deletes it |
| **Organize…** | One popover for a document or a whole selection, from the row's ⋯ menu or the bar that appears when you select: type to find or create a label, tick labels and projects (a dash means only some of the selected documents have it), and pin — each change applies at once. Drag a document onto a label, a project or the Trash in the sidebar to do the same in one move |
| **Authors** | A section of its own in the sidebar, separate from labels and made automatically: every author a document cites, as `Surname, I.` (first initial), read from the document's citations — including those in files it `#include`s — against the bibliography it names or the one in Settings. Click an author to see everything that cites them; search finds them too, and a document's tooltip lists who it cites. Kept up to date as documents are opened, saved and rescanned, so there is nothing to maintain |
| **Views** | One line per document with its prose word count; a compact list for dense libraries (View menu ⋯ at the top). Sort, compact and window size are remembered |
| **Selecting** | Hover a row for a checkbox at its left and a ⋯ menu at its right; Shift+click selects a range, Ctrl+click toggles, Ctrl+A selects all, Escape lets go. Once anything is selected a plain click toggles instead of opening |
| **Sidebar** | Library views, then Projects, Labels and Authors, each (bar Authors) with a + to make a new one; Archive and Trash sit at the bottom. Empty sections and empty views say what they're for and offer the next step |
| **Document management** | Pin, archive, delete (to the Trash, with restore), and remove from the list (delists without touching the file on disk). Archive and Delete show an Undo button for a few seconds; removing from the list asks first |
| **Bulk operations** | Multi-select for archive, Organize (labels, projects, pin) and remove |
| **Import** | New Document and Import… are both reachable directly from the Library header |
| **Auto-registration** | Any `.typ` file opened in the editor is added automatically, and the Zerkalo folder is scanned at startup (its top-level `Templates` folder is skipped — those are starting points, not documents). Word counts and the search index are kept in the library, and only files that have changed are read again |
| **Move into Zerkalo Folder…** | For a document saved outside your Zerkalo folder (dragged in from elsewhere, or from before name-only New Document existed), moves the `.typ` file and its comment/template sidecars in, picking a free name on a collision. Only offered when the document isn't currently open |

### Document workflow
| Feature | Detail |
|---|---|
| **Live preview** | Auto or manual compile (status-bar toggle; manual is the default) — Auto recompiles on every edit, debounced with a configurable delay; all pages rendered; embedded Typst engine — no binary required; click anything in the preview to jump to its exact spot in the source, or Ctrl+click in the text to show that spot in the preview |
| **Cheatsheet & Help panel** | Toggle (`?` button) in preview toolbar shows a reference panel (Overview, Cheatsheet, Projects, Shortcuts, FAQ, About) in place of the preview |
| **Style switcher** | Header-bar dropdown applies a citation style to the open document; button label shows the detected style name ("GOST 7.32") |
| **New documents** | New Document, New from Template, the Library's New Document, and Save As only ask for a name — Zerkalo adds `.typ` and saves the file in your Zerkalo folder, where the Library lists it, so fonts and bibliographies always resolve. No save-anywhere file dialog |
| **New from Template** | Dialog with tabs for Document, Layout, Sections, Languages, and Packages — generates a complete `.typ` preamble; package descriptions lead with plain language, with the underlying Typst syntax in a tooltip |
| **Saved templates** | The template dialog's gallery keeps your own templates under the built-in presets — set the form up, name it, and start future documents the same way. Stored one file per template in `~/.local/share/zerkalo/templates/` |
| **Change Document Style** | ☰ → Document Tools → Change Document Style — re-applies preamble settings from a per-document `.zerkalo.toml` sidecar; splices at the `// ── Document body` marker so body content is never touched. Applying a template to a document that never had one adopts its existing text as the body instead of discarding it |
| **Insert Table** | ☰ → Document Tools → Insert Table — set row/column count, per-cell text, per-column alignment, an optional header row, and per-cell colspan/rowspan, then generate a `#table(...)` block at the cursor. A form-then-generate dialog, not a live in-place editor — re-run it to build another table rather than editing an inserted one in place |
| **Citations & Bibliography** | ☰ → Document Tools → Citations & Bibliography — a fuller view of the loaded bibliography than the sidebar Citations panel, including project-wide citation key rename |
| **Project File Map** | ☰ → Document Tools → Project File Map — visualises which files `#include`/`#import` which, opened as its own window |
| **Document import** | Ctrl+Shift+I, or Import… in the Library window — converts to Typst, with a preview before anything is written. Word (`.docx`), OpenDocument (`.odt`) and Markdown are read by Zerkalo itself, so they need nothing installed; LaTeX, HTML, EPUB and RTF use `pandoc`, and PDF uses `pdftotext` |
| **Export** | PDF, HTML and Word compile in-process (all via the embedded Typst compiler — no `pandoc`); LaTeX and EPUB go through `pandoc` — the export dialog checks upfront whether it's available and disables those two if not, instead of only failing after you've clicked Export. Word comes in two versions, both built to drop straight into a layout program: **Word for InDesign** gives every paragraph a named style (Heading 1–6, Body Text, First Paragraph, Block Quote, Bibliography, footnote text…) carrying your document's fonts and sizes, with citations, footnotes and margin notes as real footnotes numbered together; **Word for Canva** is the same, but since Canva has no footnotes, notes are numbered in the text and listed at the end. Choose the destination folder (remembered), and optionally open the result when done — PDFs and EPUBs in [Pereplyot](https://github.com/calstfrancis/pereplyot) when it's installed, anything else in your default app |
| **Cited references export** | The Export dialog (and the Citations & Bibliography window) can write a `.bib` or Hayagriva `.yaml` holding only the entries a document — including everything it `#include`s — actually cites. Reads from a `.bib`, a `.yaml`, or a Kartoteka vault |
| **Print** | `Ctrl+P` opens the print sheet — page ranges in the document's own numbering, one/two/four pages a sheet or a fold-and-staple booklet, with a preview of the first sheet; hands off to the system print dialog with the paper size, copies, two-sided and colour already set. Text prints as vector at the printer's own resolution |
| **Font management** | Settings → Editor → Document Fonts → Manage available fonts… — searchable list of system fonts; enable/disable; set default sans/serif fonts used for new documents and template previews |
| **GOST Type B font** | Bundled and installed automatically on first launch |

### Setup & sync
| Feature | Detail |
|---|---|
| **Setup Wizard** | Three screens: sign in with GitHub, confirm a repository name, done. The git identity comes from the account (never typed), the repository is created and linked, and the first version is pushed, all behind one button. A folder or drive works instead of an account; git is bundled, so nothing needs installing |
| **Save & Back Up** | `Ctrl+Shift+S` — commits and pushes to all configured remotes in one click |
| **Automatic backups** | Once a backup location is set up, Zerkalo saves and sends a version on its own while you write, and once more on the way out if anything's still unsent — quiet by design |
| **Plain language throughout** | Setup, sync, and history surfaces describe git in terms of what it does ("save a version," "online copy," "backup location"), not git's own vocabulary |

---

## Requirements

| Tool | Purpose | Install |
|---|---|---|
| `pandoc` | LaTeX and EPUB export; LaTeX, HTML, EPUB and RTF import (Word, OpenDocument and Markdown import need it no longer) | system package — the Export dialog detects whether it's available and tells you if it's missing |
| `hunspell-en` (dictionary files only — spell checking itself is built in, no `hunspell` command needed) | English dictionaries (example) | `apt install hunspell-en-us` · `dnf install hunspell-en` · `zypper install hunspell-en` |
| `git` | Version history and sync | **bundled in the flatpak** — the GNOME runtime ships none; system package otherwise |
| `tinymist` | LSP completions (optional) | `cargo install tinymist` for source builds |

> **Note:** `typst` and `pdftoppm` are not required. Compilation and preview rendering are handled in-process by the embedded Typst engine.

---

## Installation

Zerkalo is distributed as a Flatpak via a self-hosted repository.

### Add the repository

```bash
flatpak remote-add --user calstfrancis \
  https://calstfrancis.github.io/flatpak/calstfrancis.flatpakrepo
```

### Install

```bash
flatpak install calstfrancis io.github.calstfrancis.Zerkalo
```

### Update

```bash
flatpak update io.github.calstfrancis.Zerkalo
```

### Uninstall

```bash
flatpak uninstall io.github.calstfrancis.Zerkalo
```

---

## Building manually

Runtime dependencies: GTK4 ≥ 4.10, libadwaita ≥ 1.4, GtkSourceView 5, libgit2, OpenSSL, D-Bus.

```bash
# openSUSE
zypper install gtk4-devel libadwaita-devel gtksourceview5-devel libgit2-devel openssl-devel dbus-1-devel pkgconf-pkg-config gcc

# Debian / Ubuntu
apt install libgtk-4-dev libadwaita-1-dev libgtksourceview-5-dev libgit2-dev libssl-dev libdbus-1-dev pkg-config gcc
```

```bash
cargo build --release
```

`./install.sh` does this for you — building from source and installing to `~/.local/bin`, with icons
and a `.desktop` file — for anyone who'd rather not use the flatpak.

---

## Configuration

Global config at `~/.config/zerkalo/config.toml`:

```toml
work_dir               = "/path/to/your/work/folder"
bib_path               = "/path/to/references.bib"   # optional — a .bib/.yaml file, or a Kartoteka vault folder
debounce_ms            = 800
auto_compile           = false   # false = manual compile (default); true = auto
theme                  = "system"    # "system" | "light" | "dark"
editor_font_family     = "Monospace"
editor_font_size       = 13
editor_tab_width       = 2
editor_word_wrap       = false
editor_show_whitespace = false
preview_zoom           = 1.0
```

Keybindings at `~/.config/zerkalo/keybindings.toml` (created with defaults on first launch):

```toml
save        = "ctrl+s"
compile     = "ctrl+shift+p"
find        = "ctrl+f"
quit        = "ctrl+q"
next_tab    = "ctrl+tab"
prev_tab    = "ctrl+shift+tab"
add_reference = "ctrl+shift+r"
```

All settings are also editable via **☰ → Settings** inside the app.

---

## Keyboard Shortcuts

| Key | Action |
|---|---|
| `Ctrl+S` | Save current file |
| `Ctrl+Shift+P` | Compile and refresh preview |
| `Ctrl+Shift+E` | Export PDF to the Export dialog's remembered folder (no dialog) |
| `Ctrl+P` | Print |
| `Ctrl+F` | Find & Replace |
| `Ctrl+Shift+F` | Find in Files (project-wide) |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / previous tab |
| `Ctrl+Left/Right` | Word jump (Typst-aware: `#keyword` and `@cite` count as one unit) |
| `Ctrl+Shift+Up/Down` | Jump to previous / next heading |
| `Ctrl+click` (in the text) | Show that spot in the preview |
| `Ctrl+D` | Duplicate line or selection |
| `Ctrl+/` | Toggle line comment |
| `F1` | Label every panel and button on screen; Esc or a click closes |
| `Ctrl+K` | Command palette (commands + headings) |
| `Ctrl+G` | Command palette — headings only |
| `Ctrl+Shift+S` | Save a version & back it up (git sync) |
| `Ctrl+Shift+I` | Open the Import picker |
| `Ctrl+Shift+V` | Paste as Document (reads clipboard text as Markdown) |
| `Ctrl+L` | Open the Library |
| `Ctrl+Shift+H` | Keyboard shortcuts help (dynamic, reads `keybindings.toml`) |
| `Ctrl+?` | Open the Help window |
| `Ctrl+Q` | Quit |
| `@` | Citation popup (requires a bibliography) |
| `!` | CV-entry popup (CV mode, requires a Skrizhal file) |
| `#` | LSP completion popup (tinymist, where available) |

Configurable keys are remapped in `~/.config/zerkalo/keybindings.toml`; the rest are fixed.

---

## Related projects

Zerkalo is part of the **Fond** suite of plain-file, offline-first tools:
[Kartoteka](https://github.com/calstfrancis/kartoteka) (reference manager, usable as a live
bibliography source above) and [Skrizhal](https://github.com/calstfrancis/skrizhal) (CV/résumé
element database, used by Zerkalo's CV mode).

---

## License

MIT
