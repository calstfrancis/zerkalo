# Zerkalo

A contemplative [Typst](https://typst.app) editor built with Rust, GTK4, and libadwaita.  
Live preview · Academic styles · LSP completions · Git sync · Adwaita design.  
**v0.12.1** — Native .deb/.rpm packages · Bundled tinymist · pandoc & hunspell dependencies auto-installed

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
| **Inline diagnostics** | Compile errors and LSP warnings shown as red/amber underlines in the editor |
| **Find & Replace** | `Ctrl+F`; forward/backward; animated slide-in bar; replace one or all |
| **Spell check** | Blue wavy underlines on misspelled prose words; right-click for suggestions, Ignore All; language selector in Settings; optional autocorrect on word boundary |
| **Breadcrumb bar** | Heading path shown above the editor (e.g. "Introduction › Methods") updated as the cursor moves |
| **Auto-pair brackets** | Typing `(`, `[`, `{`, or `"` inserts the closing character and positions the cursor between them |
| **Typewriter scrolling** | Optional (Settings → Editor); cursor stays fixed at ~45 % from the top of the viewport |
| **Line spacing** | Settings → Editor → Compact / Normal / Spacious |
| **High contrast mode** | Settings → Editor → High contrast; forces white-on-black in the editor |
| **Word count** | Live count and reading-time estimate in the status bar; selection shows "N words, M sentences selected" |
| **Word-count goal** | Add `// @goal: 3000` in your file; a progress bar tracks progress in the status bar |
| **Session delta** | Status bar shows `↑ N` words added since the file was opened |
| **Cursor position** | Line and column in the editor status bar |
| **What things do (F1)** | Labels every panel and control on screen with a bubble explaining it, drawn over the running window so the program stays visible underneath; Escape or a click dismisses |
| **Command palette** | `Ctrl+K`; fuzzy search over app commands and document headings; `Ctrl+G` for headings only |
| **Session restore** | Open files, active tab, and cursor positions are restored on next launch |
| **Save-before-close** | Closing with unsaved files shows a dialog listing modified files with Save All / Discard / Cancel |
| **Configurable keybindings** | Edit `~/.config/zerkalo/keybindings.toml` to remap any shortcut |

### Sidebar
| Feature | Detail |
|---|---|
| **Document outline** | Heading tree with cursor-tracking highlight; click to centre and select the heading in the editor |
| **Symbol insert** | One-click insertion of Cyrillic, Greek, Hebrew, and Sanskrit characters |
| **File tree** | Project `.typ` files; collapsible subdirectory headers; click to open; `+` / folder buttons to create files or folders; drag to reorder; right-click for Set as Root, Insert `#include`/`#import`, Delete |
| **Citation panel** | Searchable list of all BibTeX entries; double-click or Enter inserts the `@key` at the cursor |
| **Plan panel** | Freeform scratchpad saved as a `.plan` sidecar alongside the open `.typ` file; falls back to `project.plan` in the work folder when no file is open |

### Multi-file projects

| Feature | Detail |
|---|---|
| **New Project wizard** | ≡ → New Project… — names, slugifies, and creates a project folder with starter files; templates: Blank, Essay, Journal / Thesis, Theological Journal |
| **Compilation root** | One file is the Typst entry point; Zerkalo auto-detects it from the import graph or reads `.zerkalo/config.toml`; marked with ★ in the file tree |
| **Root indicator** | ★ icon on the root file's row in the file tree; root controls beside the document title while the "project" toggle is on (dismissable per project) |
| **Root switcher** | Turn on "project" beside the document title → Set…; or right-click any file → Set as Compilation Root; writes `root_file` to `.zerkalo/config.toml` and recompiles |
| **#include / #import helper** | Right-click a file in the tree → Insert `#include` or Insert `#import`; path is automatically relative to the root's directory |
| **Project config** | `.zerkalo/config.toml` inside the project folder — overrides `root_file`, `bib_path`, `file_order` for that project |

### Document workflow
| Feature | Detail |
|---|---|
| **Live preview** | Auto-compiles on every edit (300 ms debounce); all pages rendered; embedded Typst engine — no binary required |
| **Cheatsheet & Help panel** | Toggle (`?` button) in preview toolbar shows a three-tab reference panel (Cheatsheet, Help, FAQ) in place of the preview |
| **Style switcher** | Header-bar dropdown applies a citation style to the open document; button label shows the detected style name ("GOST 7.32") |
| **New from Template** | Dialog with five tabs — Document, Layout, Sections, Languages, Packages — generates a complete `.typ` preamble |
| **Update Template Settings** | ☰ → Update Template Settings — re-applies preamble settings from a per-document `.zerkalo.toml` sidecar; splices at the `// ── Document body` marker so body content is never touched; font and spacing propagate to manual config sections; metadata fields (`#let doc-title`, `#let doc-author`, etc.) are always read fresh from the document so in-source edits are picked up automatically |
| **Document import** | ☰ → Import… — converts to Typst, with a preview before anything is written. Word (.docx), OpenDocument (.odt) and Markdown are read by Zerkalo itself, so they need nothing installed; LaTeX, HTML, EPUB and RTF use pandoc, and PDF uses pdftotext. All imported files receive a Zerkalo template section and are immediately responsive to "Update Template Settings" |
| **Export** | PDF (typst), HTML (typst), DOCX, ODT, LaTeX (all via pandoc where needed) |
| **Print** | `Ctrl+P` opens the print sheet — page ranges in the document's own numbering, one/two/four pages a sheet or a fold-and-staple booklet, with a preview of the first sheet; hands off to the system print dialog with the paper size, copies, two-sided and colour already set. Text prints as vector at the printer's own resolution |
| **Font management** | ☰ → Font Management — searchable list of system fonts; enable/disable; persisted to `~/.config/zerkalo/font-preferences.toml` |
| **GOST Type B font** | Bundled and installed automatically on first launch |
| **Git sync** | Commit and push in one click or `Ctrl+Shift+S`; pushes to all configured remotes |
| **Setup** | ☰ → Set Up Zerkalo — three screens: sign in with GitHub, confirm a repository name, done. The git identity comes from the account (never typed), the repository is created and linked, and the first version is pushed, all behind one button. A folder or drive works instead of an account; git is bundled, so nothing needs installing |

---

## Requirements

| Tool | Purpose | Install |
|---|---|---|
| `pandoc` | DOCX, ODT, LaTeX export; LaTeX, HTML, EPUB and RTF import (Word, OpenDocument and Markdown import need it no longer) | declared as a package dependency — installed automatically with .deb/.rpm |
| `hunspell` | Spell checking | declared as a package dependency — installed automatically with .deb/.rpm |
| `hunspell-en` | English dictionaries (example) | `apt install hunspell-en-us` · `dnf install hunspell-en` · `zypper install hunspell-en` |
| `git` | Version history and sync | **bundled in the flatpak** — the GNOME runtime ships none; system package otherwise |
| `tinymist` | LSP completions (optional) | bundled at `/usr/lib/zerkalo/tinymist` in .deb/.rpm; `cargo install tinymist` for source builds |

> **Note:** `typst` and `pdftoppm` are no longer required. Compilation and preview rendering are handled in-process by the embedded Typst engine.

---

## Installation

### Ubuntu / Debian / Mint

Download `zerkalo_*.deb` from the [latest release](https://github.com/calstfrancis/zerkalo/releases/latest) and install it:

```bash
sudo apt install ./zerkalo_0.12.1_amd64.deb
```

`pandoc`, `hunspell`, and a bundled `tinymist` LSP are included automatically.

### Fedora / openSUSE / RHEL

Download `zerkalo-*.rpm` from the [latest release](https://github.com/calstfrancis/zerkalo/releases/latest) and install it:

```bash
sudo dnf install ./zerkalo-0.12.1-1.x86_64.rpm   # Fedora
sudo zypper install ./zerkalo-0.12.1-1.x86_64.rpm  # openSUSE
```

### install.sh (auto-detect)

The install script detects your package manager and downloads the right package:

```bash
curl -fsSL https://raw.githubusercontent.com/calstfrancis/zerkalo/main/install.sh | bash
```

Or if you've cloned the repo:

```bash
./install.sh
```

If no native package is found it falls back to building from source (requires Rust and system GTK dev headers — see *Building manually* below).

To remove:

```bash
./uninstall.sh
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

---

## Configuration

Global config at `~/.config/zerkalo/config.toml`:

```toml
work_dir               = "/path/to/your/work/folder"
bib_path               = "/path/to/references.bib"   # optional
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
| `Ctrl+F` | Find & Replace |
| `Ctrl+Tab` | Next tab |
| `Ctrl+Shift+Tab` | Previous tab |
| `Ctrl+Shift+R` | Add reference (citation autocomplete) |
| `F1` | Label every panel and button on screen; Esc or a click closes |
| `Ctrl+K` | Command palette (commands + headings) |
| `Ctrl+G` | Command palette — headings only |
| `Ctrl+Shift+S` | Git sync (commit & push) |
| `Ctrl+Shift+H` | Keyboard shortcuts help (dynamic, reads keybindings.toml) |
| `Ctrl+Q` | Quit |
| `@` | Citation popup (requires a `.bib` file) |
| `#` | LSP completion popup (tinymist — bundled in .deb/.rpm) |

---

## License

MIT
