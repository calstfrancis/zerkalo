# Zerkalo — Roadmap

Current release: **0.2.0** (2026-05-26)

---

## Shipped

### 0.1.0
- Multi-file tabbed editor (GtkSourceView 5)
- Live preview (typst + pdftoppm, debounced)
- Document outline, symbol insert, file tree
- Citation autocomplete (`@`-trigger, BibTeX)
- LSP completions (`#`-trigger, tinymist)
- Find & Replace
- Export: PDF, HTML, DOCX
- Git sync (commit + push)
- Settings dialog, help window, welcome dialog
- `install.sh` desktop integration

### 0.2.0
- Style switcher: 8 citation styles (SBL, Chicago Notes-Bib, Chicago Author-Date, MLA, APA 7th, ASA, Turabian, Harvard) applied to the active document; bibliography call updated in-place
- New from Template dialog (5 tabs: Document, Layout, Sections, Languages, Packages)
- Todo panel (global + per-file, checkbox rows, completed section)
- Session restore (open files, active tab, cursor positions)
- Configurable keybindings (`~/.config/zerkalo/keybindings.toml`)
- LaTeX import (pandoc)
- Export: ODT, LaTeX added
- Inline LSP diagnostics (squiggles in editor)
- Built-in academic snippets in LSP popup
- Font management dialog (fc-list, enable/disable, persist)
- GOST Type B font bundled
- Cursor-tracking outline highlight
- Whole-word find toggle
- Auto-compile on tab switch
- System accent colours in sidebar

---

## Near-term (0.3.0)

### Spell check (shipped in unreleased)
- **Spell check with Hunspell**: blue wavy underlines on misspelled prose words; right-click suggestions; Ignore All; language selection; optional autocorrect

### Template improvements
- **Re-apply template**: parse the existing preamble and update only the `#set`/`#show` rules, leaving document body intact; requires typst-syntax crate for safe AST-level rewriting
- **Template preview overlay**: render a preview with different settings without committing the change

### Editor improvements
- **Split view**: side-by-side editing of two files (useful for source + include)
- **Breadcrumb bar**: show current heading path above the editor
- **Minimap**: optional GtkSourceView map widget

### Export
- **EPUB export** via pandoc
- **Custom export profiles**: save frequently-used pandoc flag combinations

---

## Medium-term (0.4.0)

### Embedded compiler
- **typst as a crate** (not shelling out): compile in-process for sub-100 ms feedback; eliminates the `typst` binary requirement

### Template gallery
- Built-in templates selectable from New from Template: GOST 7.32, IEEE conference, letter, article
- Shared template-spec schema with Gost toolchain

### Collaboration
- Basic conflict-free file watching: detect external edits and offer to reload

---

## Packaging

- Flatpak manifest (for Flathub submission)
- openSUSE OBS package
- AUR (Arch) PKGBUILD
- GitHub Actions CI: build on push, publish binaries on tag
