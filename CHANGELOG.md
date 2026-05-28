# Changelog

All notable changes to Zerkalo are recorded here.  
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [0.4.0] — 2026-05-28

### Added
- **Preview ↔ Reference toggle** — `?` button in the preview toolbar switches the right column between live preview and a built-in reference panel with three tabs: Cheatsheet (full academic Typst syntax), Help (overview + getting started), and FAQ
- **Typst Cheatsheet** — comprehensive in-app reference covering headings, text formatting, citations, figures, tables, math, footnotes, footnote entry settings, special elements (outline, pagebreak, spacers, horizontal rules), links, blocks, multi-column layout, set rules (text, paragraph, page, heading), includes/imports, and git sync shortcut
- **Git sync keyboard shortcut** (`Ctrl+Shift+G`) — triggers commit & push; configurable in `~/.config/zerkalo/keybindings.toml` as `git_sync`
- **DOCX import** — ☰ → Import… → Word (.docx); converts via `pandoc -f docx -t typst --standalone`; applies same post-processing as LaTeX import (pagebreaks, bibliography stub)
- **PDF import** — ☰ → Import… → PDF (.pdf); extracts text via `pdftotext -layout`; wraps in a minimal Typst preamble
- **Unified Import… dialog** — single picker in ☰ → Import… presents LaTeX, DOCX, and PDF options
- **Preview page navigation** — prev/next buttons and "N / M" counter in the preview toolbar; scroll-to-page with midpoint detection
- **Minimap toggle** — `⊞` button in header bar shows/hides a thin GtkSourceView source map alongside the editor
- **Template gallery** — five built-in presets in New from Template: Generic Academic, Research Article APA, GOST 7.32, IEEE, Academic Letter; gallery tab with preview rendering
- **Per-file compile state** — file tree shows `dialog-error-symbolic` icon on files with compile errors; clears on success
- **Inline compile-error banner** — first error line shown in a scrollable banner below the preview; clears on successful compile
- **Drag-and-drop image insertion** — drag an image onto the editor to copy it to the work folder and insert `#figure(image("…"), caption: [])` markup
- **Autosave indicator** — title bar subtitle shows "Modified" while unsaved; "Saved" (auto-clears after 2 s) on save
- **Recent documents grouped by date** — open dropdown groups files as Today / This week / Older
- **Comment highlighting** — `//` and `/* */` comment blocks receive a theme-aware paragraph background fill; adjacent `//` lines merge into one span
- **Style dropdown label** — header style dropdown label updates to the name of the currently applied style
- **Ctrl+? shortcut** — opens the Help & Shortcuts window

### Changed
- **Title bar** — active filename shown without `.typ` extension
- **Header bar layout** — Style dropdown beside the title; Todo button right of Preview; hamburger menu rightmost; preview toolbar moved to bottom of preview area
- **Minimap width** — reduced to 72 px (thin, non-intrusive)
- **Git icon** — changed to `vcs-commit-symbolic`
- **About dialog** — updated to 0.4.0; lists embedded Typst compiler
- **Startup tool check** — removed `typst` from the check; only `git` is needed (compiler is now embedded)

### Fixed
- **GOST template language** — GOST 7.32 template now generates `lang: "en"` (was `"ru"`) to avoid font and hyphenation issues

---

## [Unreleased]

### Added
- **Spell check** — prose words in `.typ` documents are checked against the system Hunspell dictionary; misspelled words receive a blue wavy underline; right-click on any underlined word shows up to 6 suggestions (click to replace) and an "Ignore All" option that clears the underlines for that word for the session; Typst markup (`#`, `@`, `$`, `//`, `/* */`, raw blocks) is excluded from checking
- **Spell language selection** — Settings → Spell Check → Dictionary language; lists all `.dic` files found under `/usr/share/hunspell` and `/usr/share/myspell`
- **Autocorrect** — optional (off by default); Settings → Spell Check → Autocorrect; when enabled, replaces a word automatically when a word-boundary character (space, period, etc.) is typed and the top Hunspell suggestion has Levenshtein distance ≤ 1 from the original; proper nouns (words starting with a capital letter) are never autocorrected; the replacement is a separate undo action
- **Breadcrumb bar** — a bar above the editor shows the full heading path at the cursor position (e.g. "Chapter One › The Problem Stated"); updated on every cursor move via `connect_mark_set`
- **Update Template Settings** — ☰ → Update Template Settings re-applies preamble settings (citation style, paper size, margins, fonts, spacing, ToC/Abstract/Keywords) to an existing document; the body content is never touched; `ZERKALO-TEMPLATE-BEGIN` / `ZERKALO-TEMPLATE-END` markers delimit the preamble zone in generated files; the current style is pre-selected by reading the `// @zerkalo-style:` metadata line
- **Embedded Typst compiler** — preview compilation and rendering are now fully in-process via the `typst`, `typst-render`, and `typst-kit` crates; `typst` binary and `pdftoppm` are no longer required at runtime; Typst packages are resolved from the local cache at `~/.cache/typst/packages/` (populated by previous `typst` CLI use or by running `typst update`); render resolution fixed at 2.0 px/pt (≈ 144 dpi)

### Changed
- **Heading styles corrected and unified** — all styles now use `block(width: 100%)` + `#set par(first-line-indent: 0pt)` to fix centering when a first-line indent is set:
  - SBL: five levels (H1 centered ALL CAPS, H2 centered bold, H3 centered plain, H4 flush-left bold italic, H5 flush-left plain)
  - Turabian: H2 is now centered plain (not italic, per Kate Turabian §A.2)
  - ASA: H1 is now flush-left ALL CAPS (not centered); run-in H3 with period separator
  - Chicago Notes-Bib: separated from Turabian (H1 centered bold, H2 centered italic, H3 flush-left italic)

### Fixed
- **GTK "Unknown tag" warnings** — `remove_tag_by_name("zerkalo-diag-error"/"zerkalo-diag-warning")` was called before the tags were registered in each buffer's tag table, producing console warnings on every compile; `ensure_diag_tags()` is now called first in both `mark_diagnostics()` and `clear_diagnostic_marks()`
- **Preview pixbuf race condition** — opening a file triggered two concurrent `typst compile` + `pdftoppm` runs; the second run deleted PNGs while glycin was reading them, causing "unexpected end of file" errors; fixed with a generation counter (stale results discarded), unique per-run filename prefix, PNG bytes read into memory in the worker thread, and `gio::MemoryInputStream` used to load pixbufs without touching the filesystem
- **Launcher not launching** — `DBusActivatable=true` in the desktop file caused GNOME/KDE to attempt D-Bus activation, which fails silently with "The name is not activatable" when no `.service` file is registered in the session bus search path; removed `DBusActivatable=true`; Nautilus file routing continues to work via GApplication's own session-bus registration with the `HANDLES_OPEN` flag

---

## [0.2.0] — 2026-05-26

### Added
- **Style switcher** — header-bar dropdown applies a full citation style to the active document; styles: SBL, Chicago (Notes-Bib), Chicago (Author-Date), MLA, APA 7th, ASA, Turabian, Harvard; updates or appends `#bibliography(...)` at the end of the document with the correct style key and section title ("Works Cited" for MLA, "Reference List" for Chicago Author-Date)
- **New from Template dialog** — five-tab dialog (Document, Layout, Sections, Languages, Packages) generates a complete Typst preamble; citation styles, paper sizes, margin presets, font selection, line spacing, page numbers, ToC/Abstract/Keywords toggles, language support (Russian, Hebrew, Greek, Japanese, Sanskrit, Tibetan, Chinese), extra packages (Droplet, Codly, Showybox, Gentle Clues, Tablex, Drafting)
- **Todo panel** — split pane with Global and per-file todo lists; checkbox rows; Enter adds item; checked items move to a Completed section with strikethrough; persisted as `- [ ] / - [x]` markdown files
- **Session restore** — open files, active tab, and cursor positions saved to `~/.local/share/zerkalo/session.json` and restored on next launch
- **Configurable keybindings** — `~/.config/zerkalo/keybindings.toml` written on first launch with defaults; parsed at runtime so edits take effect on next start
- **LaTeX import** — ☰ → Import LaTeX File; converts `.tex` to Typst via `pandoc -f latex -t typst` and opens the result in a new tab
- **Export ODT and LaTeX** — two new formats added to the Export dialog (pandoc)
- **Inline LSP diagnostics** — compile errors and LSP warnings rendered as red/amber underlines in the editor using GtkSourceView TextTags
- **Built-in academic snippets** — figure, table, footnote, bibliography, pagebreak, outline, lorem, set rule, show rule, block; prepended to the LSP completion popup with `#`-prefix matching
- **Font management** — ☰ → Font Management; searchable checkbox list of all fc-list fonts; Enable All / Disable All; persisted to `~/.config/zerkalo/font-preferences.toml`
- **GOST Type B font** — bundled in `assets/fonts/` and installed to the user font directory on first launch
- **Welcome window** — version-keyed "What's New" dialog shown on first launch of each new version; scrollable; includes Quick Start and keyboard shortcuts
- **Cursor-tracking outline** — outline panel highlights the heading the cursor is currently under as you type
- **Whole-word find** — "W" toggle button in the Find bar; checks word boundaries before and after each match
- **LSP diagnostic deduplication** — when tinymist sends diagnostics, compile-stderr errors are suppressed to avoid duplicates
- **Outline click navigation** — single click on an outline row centres and selects the heading line in the editor
- **Auto-compile on file open** — switching to a tab immediately triggers a compile (no manual Preview click needed)
- **Simple mode sidebar toggle** — switch at the bottom of the sidebar
- **System accent colours** — outline hover and selected rows use `@accent_color` / `@accent_bg_color` from the Adwaita theme

### Changed
- Debounce reduced from 500 ms to 300 ms
- Preview button labelled "Preview" (was an icon-only button)
- Segmented Outline|Symbols control moved inside the outline panel
- Version bumped to 0.2.0

### Removed
- DOI/ISBN import (Zotero does not sync `.bib` additions made by external tools)

### Fixed
- Style switcher crash: `apply_style()` held a `state.borrow()` across `buffer.set_text()`, which fired `connect_changed` → `borrow_mut()` → RefCell panic; fixed by cloning the buffer before releasing the borrow
- Outline click did nothing: `row.connect_activate` only fires on Enter/double-click; replaced with `list_box.connect_row_activated` which fires on single click
- `scroll_to_iter` centering: `use_align` was `false`, causing the `yalign: 0.5` argument to be ignored; corrected to `true`

---

## [0.1.0] — 2026-05-24

### Added
- GTK4 + libadwaita window, GtkSourceView editor, live preview via `typst compile` + `pdftoppm`
- Multi-file tabbed editor with modified-indicator dot and close button
- Project file tree (create, delete, click to open)
- Document outline sidebar (heading tree, click to jump)
- Symbol insert panel (Cyrillic, Greek, Hebrew, Sanskrit)
- Citation autocomplete: `@` trigger with BibTeX popup; Tab/Return to accept
- LSP completions: `#` trigger via tinymist; kind badge, Tab/Return to accept
- Find & Replace (`Ctrl+F`): forward/backward search, replace one/all, wrap-around
- Live word count and reading-time estimate in status bar
- Cursor line/column indicator in status bar
- Export: PDF (typst), HTML (typst), DOCX (pandoc)
- Git sync: one-click commit + push; remote setup dialog on first sync
- Help window (Overview, Shortcuts, FAQ, About tabs)
- Settings dialog (appearance, editor, compilation, bibliography)
- Hamburger menu (☰) consolidating settings, help, file operations
- Recent files list in the open dropdown
- Setzer-style open dropdown: search box + work-folder scan (2 levels deep)
- Save / Save As / New Document via native file dialogs
- Desktop integration: `install.sh` / `uninstall.sh`; SVG icon + PNG sizes 16–256 px generated at install time
- tracing-based logging to `~/.local/share/zerkalo/zerkalo.log`
- Global config at `~/.config/zerkalo/config.toml`
