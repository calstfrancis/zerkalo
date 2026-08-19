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
| **Multi-file tabs** | Open multiple `.typ` files; modified-indicator dot; red error dot on compile failure; close button |
| **Syntax highlighting** | Full Typst grammar via GtkSourceView 5 |
| **Inline completions** | `#` shows the best match dim after the cursor, previewing what will be inserted; Tab accepts, and a compact ranked list joins in after two characters. Backed by [tinymist](https://github.com/Myriad-Dreamin/tinymist) where available |
| **Built-in snippets** | Academic snippets (figure, table, footnote, bibliography, …) prepended to the LSP popup |
| **Citation autocomplete** | `@` (BibTeX keys) and `!` (Skrizhal CV entries) behave the same way — inline suggestion, description in the status bar |
| **Bibliography sources** | A `.bib` file — including a library exported from Zotero, Mendeley, or any other reference manager as BibTeX — a Hayagriva `.yaml` file, or a [Kartoteka](https://github.com/calstfrancis/kartoteka) vault folder — vault entries refresh live via `fond-vault`'s filesystem watch, no restart needed |
| **First-run bibliography** | The Citations panel can start a brand-new `.bib` file with one click — no need to already have one before adding your first source |
| **Inline diagnostics** | Compile errors and LSP warnings shown as red/amber underlines in the editor; the error panel translates Typst's message into plain language, with the exact wording kept under "Technical detail" |
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
| **Simple Mode** | On by default — hides the document's technical setup lines above the body so you can focus on writing prose; change them from the Template button instead |
| **Focus Mode** | Hides the sidebar and secondary panels for distraction-free writing |
| **What things do (F1)** | Labels every panel and control on screen with a bubble explaining it, drawn over the running window so the program stays visible underneath; Escape or a click dismisses. Covers the main editor window and the Library window |
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

### Document Library (`Ctrl+L`)
| Feature | Detail |
|---|---|
| **SQLite-backed library** | Every `.typ` document Zerkalo knows about, with search, sort, and filter |
| **Organisation** | Projects, coloured categories, and frequency-heat-coloured tags; sidebar filters for All Documents, Projects, Categories, Tags, Trash, and Archive |
| **Views** | Card view with prose word count; compact single-line view for dense lists |
| **Document management** | Pin, archive, trash (soft delete with restore), and remove from the library (delists without touching the file on disk) — each confirmed before it happens |
| **Bulk operations** | Multi-select for archive, tag, add-to-project, and remove |
| **Import** | New Document and Import… are both reachable directly from the Library header |
| **Auto-registration** | Any `.typ` file opened in the editor is added automatically |

### Document workflow
| Feature | Detail |
|---|---|
| **Live preview** | Auto-compiles on every edit (debounced, configurable delay); all pages rendered; embedded Typst engine — no binary required |
| **Cheatsheet & Help panel** | Toggle (`?` button) in preview toolbar shows a reference panel (Overview, Cheatsheet, Projects, Shortcuts, FAQ, About) in place of the preview |
| **Style switcher** | Header-bar dropdown applies a citation style to the open document; button label shows the detected style name ("GOST 7.32") |
| **New from Template** | Dialog with tabs for Document, Layout, Sections, Languages, and Packages — generates a complete `.typ` preamble; package descriptions lead with plain language, with the underlying Typst syntax in a tooltip |
| **Saved templates** | The template dialog's gallery keeps your own templates under the built-in presets — set the form up, name it, and start future documents the same way. Stored one file per template in `~/.local/share/zerkalo/templates/` |
| **Change Document Style** | ☰ → Document Tools → Change Document Style — re-applies preamble settings from a per-document `.zerkalo.toml` sidecar; splices at the `// ── Document body` marker so body content is never touched |
| **Insert Table** | ☰ → Document Tools → Insert Table — set row/column count, per-cell text, per-column alignment, an optional header row, and per-cell colspan/rowspan, then generate a `#table(...)` block at the cursor. A form-then-generate dialog, not a live in-place editor — re-run it to build another table rather than editing an inserted one in place |
| **Citations & Bibliography** | ☰ → Document Tools → Citations & Bibliography — a fuller view of the loaded bibliography than the sidebar Citations panel, including project-wide citation key rename |
| **Project File Map** | ☰ → Document Tools → Project File Map — visualises which files `#include`/`#import` which, opened as its own window |
| **Document import** | Ctrl+Shift+I, or Import… in the Library window — converts to Typst, with a preview before anything is written. Word (`.docx`), OpenDocument (`.odt`) and Markdown are read by Zerkalo itself, so they need nothing installed; LaTeX, HTML, EPUB and RTF use `pandoc`, and PDF uses `pdftotext` |
| **Export** | PDF (in-process), HTML, DOCX, ODT, LaTeX (via `pandoc` where needed) — the export dialog checks upfront whether `pandoc` is available and disables the formats that need it if not, instead of only failing after you've clicked Export |
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
| `pandoc` | DOCX, ODT, LaTeX export; LaTeX, HTML, EPUB and RTF import (Word, OpenDocument and Markdown import need it no longer) | system package — the Export dialog detects whether it's available and tells you if it's missing |
| `hunspell` | Spell checking | system package |
| `hunspell-en` | English dictionaries (example) | `apt install hunspell-en-us` · `dnf install hunspell-en` · `zypper install hunspell-en` |
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
auto_compile           = true
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
| `Ctrl+Shift+E` | Export PDF to document folder (no dialog) |
| `Ctrl+P` | Print |
| `Ctrl+F` | Find & Replace |
| `Ctrl+Shift+F` | Find in Files (project-wide) |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / previous tab |
| `Ctrl+Left/Right` | Word jump (Typst-aware: `#keyword` and `@cite` count as one unit) |
| `Ctrl+Shift+Up/Down` | Jump to previous / next heading |
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
