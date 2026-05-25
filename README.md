# Zerkalo

A contemplative [Typst](https://typst.app) editor built with Rust, GTK4, and libadwaita.  
Live preview · Multi-file tabs · LSP completions · Git sync · Adwaita design.

---

## Features

| Feature | Detail |
|---|---|
| **Live preview** | Auto-compiles on every keystroke (configurable delay), renders all pages |
| **Adjustable zoom** | `+` / `−` zoom buttons in the preview toolbar; re-renders at the correct DPI |
| **Pop-out preview** | Detach the preview into a separate floating window |
| **Syntax highlighting** | Full Typst grammar via GtkSourceView 5 |
| **LSP completions** | `#` triggers function/keyword popup via [tinymist](https://github.com/Myriad-Dreamin/tinymist) |
| **Citation autocomplete** | `@` triggers a BibTeX popup |
| **Find & Replace** | `Ctrl+F`, case-insensitive, wrap-around |
| **Document outline** | Heading tree in the left sidebar, click to jump |
| **Word count** | Live word count and reading time in the status bar |
| **Cursor position** | Line and column shown in the editor status bar |
| **Insert panel** | One-click insertion of common Typst snippets (headings, figures, tables, …) |
| **Git sync** | Commit and push in one click |
| **First-run guide** | Welcome dialog shown on first launch |
| **Help window** | Built-in documentation with Overview, Shortcuts, FAQ, and About tabs |

---

## Requirements

| Tool | Purpose | Install |
|---|---|---|
| `typst` | Compilation | `zypper install typst` · `apt install typst` · `brew install typst` |
| `pdftoppm` | PDF → PNG rendering | `zypper install poppler-tools` · `apt install poppler-utils` · `brew install poppler` |
| `git` | Sync | system package |
| `tinymist` | LSP completions (optional) | `cargo install tinymist` |

---

## Installation

The `install.sh` script builds a release binary and integrates Zerkalo into your desktop launcher:

```bash
./install.sh
```

This places the binary in `~/.local/bin/`, the icon in `~/.local/share/icons/`, and the `.desktop` file in `~/.local/share/applications/`. Most GNOME-based desktops (openSUSE, Fedora, Ubuntu, Arch with GNOME) will show Zerkalo in the app launcher after running the script.

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

```bash
cargo build --release
```

Runtime dependencies: GTK4 ≥ 4.10, libadwaita ≥ 1.4, GtkSourceView 5.

```bash
# openSUSE
zypper install gtk4-devel libadwaita-devel gtksourceview5-devel

# Debian / Ubuntu
apt install libgtk-4-dev libadwaita-1-dev libgtksourceview-5-dev
```

---

## Configuration

Global config at `~/.config/zerkalo/config.toml`:

```toml
project_path        = "/path/to/your/project"
bib_path            = "/path/to/references.bib"   # optional
debounce_ms         = 500
auto_compile        = true
theme               = "system"    # "system" | "light" | "dark"
editor_font_family  = "Monospace"
editor_font_size    = 13
editor_tab_width    = 2
editor_word_wrap    = false
editor_show_whitespace = false
preview_zoom        = 1.0
```

Per-project config at `<project>/.zerkalo.toml`:

```toml
bib_path      = "refs.bib"
output_dir    = "/tmp/myproject_preview"
compiler_args = ["--font-path", "./fonts"]
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
| `Ctrl+R` | Refresh file tree |
| `Ctrl+Q` | Quit |
| `@` | Citation popup (requires a `.bib` file) |
| `#` | LSP completion popup (requires tinymist) |

---

## License

MIT
