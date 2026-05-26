# Zerkalo

A contemplative [Typst](https://typst.app) editor built with Rust, GTK4, and libadwaita.  
Live preview · Academic styles · LSP completions · Git sync · Adwaita design.

---

## Features

### Editor
| Feature | Detail |
|---|---|
| **Multi-file tabs** | Open multiple `.typ` files; modified-indicator dot; close button |
| **Syntax highlighting** | Full Typst grammar via GtkSourceView 5 |
| **LSP completions** | `#` triggers a popup via [tinymist](https://github.com/Myriad-Dreamin/tinymist) |
| **Built-in snippets** | Academic snippets (figure, table, footnote, bibliography, …) prepended to the LSP popup |
| **Citation autocomplete** | `@` triggers a BibTeX key popup with fuzzy filtering |
| **Inline diagnostics** | Compile errors and LSP warnings shown as red/amber underlines in the editor |
| **Find & Replace** | `Ctrl+F`; forward/backward; whole-word toggle (`W`); replace one or all |
| **Word count** | Live count and reading-time estimate in the status bar |
| **Cursor position** | Line and column in the editor status bar |
| **Session restore** | Open files, active tab, and cursor positions are restored on next launch |
| **Configurable keybindings** | Edit `~/.config/zerkalo/keybindings.toml` to remap any shortcut |

### Sidebar
| Feature | Detail |
|---|---|
| **Document outline** | Heading tree with cursor-tracking highlight; click to centre and select the heading in the editor |
| **Symbol insert** | One-click insertion of Cyrillic, Greek, Hebrew, and Sanskrit characters |
| **File tree** | Project `.typ` files; click to open, buttons to create/delete |
| **Todo panel** | Global and per-file checkbox lists; Enter adds an item; checked items move to a Completed section with strikethrough |

### Document workflow
| Feature | Detail |
|---|---|
| **Live preview** | Auto-compiles on every edit (300 ms debounce); all pages rendered |
| **Style switcher** | Header-bar dropdown applies a citation style to the open document: SBL, Chicago (Notes-Bib), Chicago (Author-Date), MLA, APA 7th, ASA, Turabian, Harvard |
| **New from Template** | Dialog with five tabs — Document, Layout, Sections, Languages, Packages — generates a complete `.typ` preamble |
| **LaTeX import** | ☰ → Import LaTeX File — converts `.tex` to Typst via pandoc |
| **Export** | PDF (typst), HTML (typst), DOCX, ODT, LaTeX (all via pandoc where needed) |
| **Font management** | ☰ → Font Management — searchable list of system fonts; enable/disable; persisted to `~/.config/zerkalo/font-preferences.toml` |
| **GOST Type B font** | Bundled and installed automatically on first launch |
| **Git sync** | Commit and push in one click |

---

## Requirements

| Tool | Purpose | Install |
|---|---|---|
| `typst` | Compilation and HTML export | `zypper install typst` · `apt install typst` · `brew install typst` |
| `pdftoppm` | PDF → PNG rendering for the preview pane | `zypper install poppler-tools` · `apt install poppler-utils` · `brew install poppler` |
| `pandoc` | DOCX, ODT, LaTeX export; LaTeX import | `zypper install pandoc` · `apt install pandoc` · `brew install pandoc` |
| `git` | Sync | system package |
| `tinymist` | LSP completions (optional) | `cargo install tinymist` |

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
debounce_ms            = 300
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
| `Ctrl+Q` | Quit |
| `@` | Citation popup (requires a `.bib` file) |
| `#` | LSP completion popup (requires tinymist) |

---

## License

MIT
