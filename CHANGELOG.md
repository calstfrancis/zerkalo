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

## [0.5.0] — 2026-05-28

### Added
- **Spell check** — prose words in `.typ` documents are checked against the system Hunspell dictionary; misspelled words receive a blue wavy underline; right-click on any underlined word shows up to 6 suggestions (click to replace) and an "Ignore All" option; Typst markup (`#`, `@`, `$`, `//`, `/* */`, raw blocks) is excluded from checking
- **Spell language selection** — Settings → Spell Check → Dictionary language; lists all `.dic` files found under `/usr/share/hunspell` and `/usr/share/myspell`
- **Autocorrect** — optional (off by default); Settings → Spell Check → Autocorrect; replaces a word on word-boundary input when the top Hunspell suggestion has Levenshtein distance ≤ 1; proper nouns are never autocorrected; undo-able as a separate action
- **Breadcrumb bar** — a bar above the editor shows the full heading path at the cursor position (e.g. "Chapter One › The Problem Stated"); updated on every cursor move
- **Update Template Settings** — ☰ → Update Template Settings / sidebar "Update Template…" button re-applies preamble settings (citation style, paper size, margins, fonts, spacing, ToC/Abstract/Keywords) to an existing document without touching the body; the current style is pre-selected by reading the `// @zerkalo-style:` metadata line
- **Embedded Typst compiler** — preview compilation and rendering are fully in-process via the `typst`, `typst-render`, and `typst-kit` crates; no `typst` binary or `pdftoppm` required; render resolution fixed at 2.0 px/pt (≈ 144 dpi)
- **Multi-remote Git push** — `sync()` pushes to every configured remote; per-remote failures reported individually without blocking other remotes
- **Backup remote setup** — Setup wizard and ☰ → Backup Remotes… dialog let users add a second remote (e.g. Codeberg) alongside the primary origin
- **Broken-citation jump** — clicking a broken `@key` citation in the Refs panel jumps to and selects that citation in the editor
- **Animated find bar** — `Ctrl+F` slides the find/replace bar in with a 200 ms `gtk4::Revealer` `SlideDown` animation instead of appearing instantly
- **Dark-mode syntax fallback** — `apply_style_scheme` tries `Adwaita-dark → oblivion → solarized-dark → classic-dark` in order; light mode tries `Adwaita → classic`
- **Sidebar section headers** — dim "Structure" label above the outline panel and "Project" label above the Refs/History/Files notebook
- **Simple-mode explanation** — `?` button beside the Simple mode switch opens a tooltip-style dialog explaining what the mode hides
- **Paned divider hover** — CSS transition highlights the editor↔preview drag handle in the accent colour on hover
- **Style button shows filename** — the Style dropdown label now reads "GOST 7.32 · main" (detected style + active filename); updates on tab switch and file open
- **Minimap in hamburger menu** — minimap toggle moved from the header to ☰ → Toggle Minimap; Browse Documents also moved to the hamburger View section, decluttering the header
- **Abbreviated cursor position** — status bar shows "L12:C5" (was "Ln 12, Col 5") with a "Line 12, Column 5" tooltip

### Changed
- **Settings dialog** — reorganised into three tabs: General (folders + compilation), Editor (color scheme + font/whitespace), Extras (bibliography + spell check); was a single long scrollable page
- **Header bar** — only `sidebar toggle | focus | Style ▾` on the start; end unchanged; docs browser and minimap toggle moved to hamburger
- **Heading styles corrected and unified** — all styles use `block(width: 100%)` + `#set par(first-line-indent: 0pt)`; SBL gets five heading levels; Turabian H2 centred plain; ASA H1 flush-left ALL CAPS; Chicago Notes-Bib separated from Turabian

### Fixed
- **GTK "Unknown tag" warnings** — `ensure_diag_tags()` now called before `remove_tag_by_name` in both `mark_diagnostics()` and `clear_diagnostic_marks()`
- **Preview pixbuf race condition** — generation counter discards stale results; PNG bytes read into memory in the worker thread
- **Launcher not launching** — removed `DBusActivatable=true` from the desktop file
- **Find bar layout** — removed `set_width_chars(12)` reservation on the result label that caused a large empty gap
- **Minimap position** — minimap was added outside the editor pane and covered text; now placed inline beside the `ScrolledWindow` inside the editor pane

---

## [0.6.0] — 2026-05-28

### Added
- **Line spacing control** — Settings → Editor → Line spacing: Compact (0 px), Normal (2 px, default), Spacious (6 px); persisted in config
- **Zen writing mode** — Focus button now dims the sidebar to 30 % opacity via a CSS transition instead of hiding it entirely; editor text gains 40 px left/right padding so the writing area feels centred
- **Typewriter scrolling** — Settings → Editor → Typewriter scrolling; on every cursor move the view scrolls to keep the cursor at ~45 % from the top of the viewport; automatically disabled during mouse-selection drags
- **Per-document word-count goal** — add `// @goal: 3000` anywhere in a `.typ` file to set a word target; a progress bar appears in the status bar showing progress toward the goal; bar is hidden when no goal is set
- **Command palette** — `Ctrl+P` opens a fuzzy command palette listing all standard app commands and every heading in the current document; `↑`/`↓` navigates; `Enter` activates; `Esc` closes
- **Selection word/sentence stats** — while text is selected the status bar replaces the word count with "N words, M sentences selected"; reverts to the document word count when selection is cleared
- **High contrast mode** — Settings → Editor → High contrast mode; adds a `high-contrast` CSS class to the window that forces white-on-black in the editor text view; persisted in config
- **Auto-pair brackets and quotes** — typing `(`, `[`, `{`, or `"` inserts the matching closing character and places the cursor between them; implemented as a single undo-able buffer action
- **Save-before-close dialog** — closing the window with unsaved files now shows a modal listing each modified filename with **Save All**, **Discard**, and **Cancel** responses; "Save All" writes all modified buffers to disk before closing

### Changed
- **Horizontal scroll locked in word-wrap mode** — editor `ScrolledWindow` horizontal policy is set to `Never` when word wrap is active, eliminating the rightward cursor-follow drift; policy is updated when word wrap is toggled and when existing tabs are affected
- **Sidebar scroll fixed** — all sidebar panels (reference manager, file tree, outline, search, todo) now enforce horizontal scroll policy `Never`, preventing unexpected rightward scroll when clicking items

### Removed
- **Git history panel** — the git-history sidebar panel has been removed; it was unreliable and of unclear value. Use `git log` in a terminal or a dedicated Git client for history browsing.

### Fixed
- **Update Template Settings** — the "Update Template Settings" flow no longer opens a file-save dialog; preamble is applied in-memory and written directly to the current file

---

## [Unreleased]

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
