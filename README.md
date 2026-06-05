# Zerkalo

A contemplative [Typst](https://typst.app) editor built with Rust, GTK4, and libadwaita.  
Live preview · Academic styles · LSP completions · Git sync · Adwaita design.  
**v0.8.6** — Template sidecar · Plan panel · Session delta · Tab error indicator · Citation panel fixes

---

## Features

### Editor
| Feature | Detail |
|---|---|
| **Multi-file tabs** | Open multiple `.typ` files; modified-indicator dot; red error dot on compile failure; close button |
| **Syntax highlighting** | Full Typst grammar via GtkSourceView 5 |
| **LSP completions** | `#` triggers a popup via [tinymist](https://github.com/Myriad-Dreamin/tinymist) |
| **Built-in snippets** | Academic snippets (figure, table, footnote, bibliography, …) prepended to the LSP popup |
| **Citation autocomplete** | `@` triggers a BibTeX key popup with fuzzy filtering |
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
| **Command palette** | `Ctrl+P`; fuzzy search over app commands and document headings; `Ctrl+G` for headings only |
| **Session restore** | Open files, active tab, and cursor positions are restored on next launch |
| **Save-before-close** | Closing with unsaved files shows a dialog listing modified files with Save All / Discard / Cancel |
| **Configurable keybindings** | Edit `~/.config/zerkalo/keybindings.toml` to remap any shortcut |

### Sidebar
| Feature | Detail |
|---|---|
| **Document outline** | Heading tree with cursor-tracking highlight; click to centre and select the heading in the editor |
| **Symbol insert** | One-click insertion of Cyrillic, Greek, Hebrew, and Sanskrit characters |
| **File tree** | Project `.typ` files; click to open, buttons to create/delete |
| **Citation panel** | Searchable list of all BibTeX entries; double-click or Enter inserts the `@key` at the cursor |
| **Plan panel** | Freeform scratchpad saved as a `.plan` sidecar alongside the open `.typ` file; falls back to `project.plan` in the work folder when no file is open |

### Document workflow
| Feature | Detail |
|---|---|
| **Live preview** | Auto-compiles on every edit (300 ms debounce); all pages rendered; embedded Typst engine — no binary required |
| **Cheatsheet & Help panel** | Toggle (`?` button) in preview toolbar shows a three-tab reference panel (Cheatsheet, Help, FAQ) in place of the preview |
| **Style switcher** | Header-bar dropdown applies a citation style to the open document; button label shows detected style and filename ("GOST 7.32 · main") |
| **New from Template** | Dialog with five tabs — Document, Layout, Sections, Languages, Packages — generates a complete `.typ` preamble |
| **Update Template Settings** | ☰ → Update Template Settings — re-applies preamble settings from a per-document `.zerkalo.toml` sidecar; splices at the `// ── Document body` marker so body content is never touched; font and spacing propagate to manual config sections |
| **LaTeX / DOCX / PDF import** | ☰ → Import… — converts to Typst via pandoc or pdftotext; all imported files receive a Zerkalo template section and are immediately responsive to "Update Template Settings" |
| **Export** | PDF (typst), HTML (typst), DOCX, ODT, LaTeX (all via pandoc where needed) |
| **Font management** | ☰ → Font Management — searchable list of system fonts; enable/disable; persisted to `~/.config/zerkalo/font-preferences.toml` |
| **GOST Type B font** | Bundled and installed automatically on first launch |
| **Git sync** | Commit and push in one click or `Ctrl+Shift+G`; pushes to all configured remotes |

---

## Requirements

| Tool | Purpose | Install |
|---|---|---|
| `pandoc` | DOCX, ODT, LaTeX export; LaTeX import | `zypper install pandoc` · `apt install pandoc` · `brew install pandoc` |
| `hunspell` | Spell checking | `zypper install hunspell` · `apt install hunspell` · `brew install hunspell` |
| `hunspell-en` | English dictionaries (example) | `zypper install hunspell-en` · `apt install hunspell-en-us` |
| `git` | Sync | system package |
| `tinymist` | LSP completions (optional) | `cargo install tinymist` |

> **Note:** `typst` and `pdftoppm` are no longer required. Compilation and preview rendering are handled in-process by the embedded Typst engine.

---

## Installation

```bash
./install.sh
```

Builds a release binary and installs it to `~/.local/bin/`, the SVG icon to `~/.local/share/icons/`, and the `.desktop` file to `~/.local/share/applications/`. Most GNOME-based desktops will show Zerkalo in the app launcher immediately.

If `~/.local/bin` is not in your `PATH`:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

To remove:

```bash
./uninstall.sh
```

---

## Building manually

Runtime dependencies: GTK4 ≥ 4.10, libadwaita ≥ 1.4, GtkSourceView 5.

```bash
# openSUSE
zypper install gtk4-devel libadwaita-devel gtksourceview5-devel

# Debian / Ubuntu
apt install libgtk-4-dev libadwaita-1-dev libgtksourceview-5-dev
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
| `Ctrl+P` | Command palette (commands + headings) |
| `Ctrl+G` | Command palette — headings only |
| `Ctrl+Shift+G` | Git sync (commit & push) |
| `Ctrl+Q` | Quit |
| `@` | Citation popup (requires a `.bib` file) |
| `#` | LSP completion popup (requires tinymist) |

---

## License

MIT
