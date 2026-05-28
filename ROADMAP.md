# Zerkalo — Roadmap

Current release: **0.6.0** (2026-05-28)

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

## In progress (0.3.0)

### Shipped in this cycle
- **Spell check with Hunspell**: blue wavy underlines on misspelled prose words; right-click suggestions; Ignore All; language selection; optional autocorrect
- **Breadcrumb bar**: heading path shown above the editor, updated on cursor move
- **Update Template Settings**: re-apply preamble to existing document (ZERKALO-TEMPLATE-BEGIN/END markers preserve body)
- **Heading style corrections**: SBL 5-level hierarchy; Turabian H2 plain; ASA flush-left ALL CAPS H1; Chicago Notes-Bib separated from Turabian
- **Fix**: GTK "Unknown tag" warnings on diagnostic squiggles
- **Fix**: Preview pixbuf race condition (generation counter + in-memory PNG bytes)
- **Fix**: Launcher not launching (removed DBusActivatable=true)

- **Embedded typst compiler**: compile in-process via `typst` + `typst-render` + `typst-kit`; ZerkaloWorld impl; eliminates `typst` binary and `pdftoppm`; packages resolved from `~/.cache/typst/packages/`

---

## Shipped (0.4.0)

### Reference panel
- **Preview ↔ Cheatsheet/Help toggle**: button in preview toolbar switches the right column between live preview and a three-tab reference view (Cheatsheet, Help, FAQ); compile continues in background

### Git
- **Ctrl+Shift+G keybinding**: triggers git sync from keyboard; configurable in `keybindings.toml`

### Import
- **DOCX import** (pandoc) and **PDF import** (pdftotext) added under ☰ → Import…
- **Unified Import… dialog**: single picker that presents LaTeX, DOCX, and PDF options

### Preview
- **Page navigation**: prev/next buttons + "N / M" counter in preview toolbar
- **Minimap toggle**: thin GtkSourceView minimap alongside the editor (toggle in header)
- **Template gallery**: five built-in templates in New from Template (Generic, APA Article, GOST 7.32, IEEE, Letter)

### Polish
- Version bump to 0.4.0; About dialog updated
- Removed stale typst-CLI startup check (compiler is now embedded)
- `[profile.dev] debug = 1` in Cargo.toml — debug builds ~40 % smaller

### 0.6.0
- Command palette (`Ctrl+P`) — fuzzy search over app commands and document headings
- Zen writing mode — Focus button dims sidebar via CSS opacity transition; editor padding
- Typewriter scrolling — cursor pinned to 45 % from top; guards against selection-drag conflict
- Per-document word-count goal — `// @goal: N` comment + status bar progress bar
- Selection stats — "N words, M sentences selected" in status bar while text is selected
- Line spacing control — Compact / Normal / Spacious in Settings → Editor
- High contrast mode — white-on-black editor CSS; persisted in config
- Auto-pair brackets and quotes — `(`, `[`, `{`, `"` insert matching closer and position cursor
- Save-before-close dialog — lists unsaved files; Save All / Discard / Cancel
- Horizontal scroll locked in word-wrap mode; all sidebar panels also locked horizontally
- Git history panel removed
- Update Template Settings — applies in-memory without file-save dialog

---

## Near-term (0.7.0)

### Export
- **EPUB metadata**: title, author, cover image fields in Export dialog
- **Custom export profiles**: save frequently-used pandoc flag combinations

### Template improvements
- Per-style title-page formatting in the Style switcher (manual Typst title layout per citation style)
- Shared template-spec schema with Gost toolchain

---

## Medium-term (0.8.0)

### Collaboration
- Basic conflict-free file watching: detect external edits and offer to reload

---

## Packaging

- Flatpak manifest (for Flathub submission)
- openSUSE OBS package
- AUR (Arch) PKGBUILD
- GitHub Actions CI: build on push, publish binaries on tag
