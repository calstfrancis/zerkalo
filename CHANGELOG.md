# Changelog

All notable changes to Zerkalo are recorded here.  
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [0.12.12] — 2026-06-08

### Added
- File tree: ★ indicator on the current compilation root row
- File tree: right-click context menu now has "Set as Compilation Root" above "Delete"; selecting it writes `root_file` to `.zerkalo/config.toml`, updates the preview, and triggers a recompile

---

## [0.12.11] — 2026-06-08

### Changed
- Flatpak: strip debug symbols from zerkalo and tinymist binaries — reduces flatpak size by ~50 MB

---

## [0.12.10] — 2026-06-07

### Changed
- "Backup Remotes" menu item renamed to "Git Remotes"
- Git Remotes dialog now includes a "Primary Remote" section at the top for viewing and editing the origin (GitHub) URL — no longer need the Setup Wizard to change which repo the project syncs with

---

## [0.12.9] — 2026-06-07

### Changed
- Syntax scheme preference order: `solarized-dark` / `tango` first, Adwaita as fallback
- Comment block highlight changed from neutral grey wash to a faint blue tint for visual distinction
- Current-line highlight now uses accent colour (`alpha(@accent_color, 0.06)`) for consistency with the cursor
- Style dropdown shows only the style name, not the document filename

---

## [0.12.8] — 2026-06-07

### Fixed
- Plan panel toggle button now uses `view-list-symbolic` instead of the missing `text-editor-symbolic` icon

---

## [0.12.7] — 2026-06-06

### Added

- **Section Notes panel**: the right sidebar now has two tabs — "Plan" (existing scratchpad) and "Notes" (new). The Notes tab mirrors the document outline; clicking a heading loads that section's planning note in a text area below. Notes are saved as `<filename>.notes.json` alongside the `.typ` file. Keys are preserved by heading text across edits; headings that disappear are garbage-collected from the sidecar. The list and notes update live as you type.

---

## [0.12.6] — 2026-06-06

### Changed

- **Preview toolbar**: removed Copy Text, Jump to Editor, and Watch Mode buttons. Ctrl+Click on the preview for jump-to-source still works.
- **Find bar**: hidden by default; the "search" button in the status bar now turns blue when the bar is open (Ctrl+F or Esc to toggle).

### Fixed

- **Settings hang**: spell recheck after changing languages/enabling spell check now runs hunspell off the GTK main thread, so the UI stays responsive.

---

## [0.12.5] — 2026-06-06

### Fixed

- **tinymist bundled in deb/rpm now detected correctly**: startup availability check now probes `/usr/lib/zerkalo/tinymist` first (matching the LSP launcher logic), so deb/rpm installs no longer show a spurious "Optional: tinymist" alert.
- **RPM spec `%files` section**: removed the broken `%if 0%{?with_tinymist}` conditional — tinymist is always bundled in release packages and must be listed unconditionally to avoid an "installed but unpackaged files" build error.

---

## [0.12.4] — 2026-06-06

### Fixed

- **GitHub token dialog now actually works**: previously the token was saved to disk but the in-memory config was not updated, so the next sync attempt still used no token. Fixed — the dialog now updates the live config immediately.
- **Auto-retry after login**: the dialog button now reads "Save & Sync" and automatically retries the push after saving the token, so the user doesn't need to click the sync button a second time.
- `do_sync` and `show_sync_result` now share the live `current_config` so future auth-failure retries read the correct token.

---

## [0.12.3] — 2026-06-06

### Fixed

- Settings dialog now preserves `active_profile`, `word_count_goal`, `last_export_format`, `recent_searches`, and `auto_save_idle_ms` when saving — previously these were reset to defaults on every Settings save.
- Removed stale `cos_for_watch` variable in file-watcher callback (unused since compile-on-save logic was refactored into the pill).
- Removed dead `default_auto_save_idle_ms_pub` export from `config.rs`.

---

## [0.12.2] — 2026-06-06

### Added

- **GitHub login dialog**: if a push fails with an authentication error, Zerkalo shows a "GitHub Login" dialog prompting for a Personal Access Token (PAT). The token is stored in the local config and injected into HTTPS remote URLs on future syncs — no terminal needed.
- **GitHub token in Settings**: the "General" settings page now has a "GitHub Sync" section where the PAT can be set or updated at any time.
- **Pull before push**: `sync()` now runs `git pull --rebase` before each push so that multi-machine workflows don't produce non-fast-forward rejections.

### Fixed

- `Config::default()` missing `github_token` field (would have caused a compile error on new installs).

---

## [0.12.1] — 2026-06-06

### Changed

- **Native packaging**: release workflow now produces `.deb` (Ubuntu/Debian/Mint) and `.rpm` (Fedora/openSUSE) packages instead of an AppImage. `pandoc` and `hunspell` are declared as package dependencies so they are installed automatically.
- **Bundled tinymist**: the LSP binary is bundled at `/usr/lib/zerkalo/tinymist` inside the deb/rpm packages — no separate download step needed after install. The source-build path still prompts to install tinymist separately.
- **install.sh**: now detects dpkg/rpm and downloads the appropriate native package; falls back to cargo build only as a last resort.

---

## [0.12.0] — 2026-06-05

### Added

- **Keyboard Shortcut Remap**: Command Palette moved to **Ctrl+K** (was Ctrl+P); Git Sync moved to **Ctrl+Shift+S** (was Ctrl+Shift+G). Both are configurable via `~/.config/zerkalo/keybindings.toml` using the new `command_palette` and `shortcuts_help` keys.
- **Ctrl+Shift+H — Dynamic Keyboard Shortcuts Help**: opens a dialog showing the *current* effective keybindings read from `keybindings.toml` at runtime rather than a static list.
- **Compilation Time Display**: status bar now shows "Compiled in Xs". Times over 3 s turn **yellow** and show a tooltip with three optimization tips (Draft profile, image placement, file splitting). Stats are appended to `~/.cache/zerkalo/compile_stats.json` on every compile.
- **Auto-backup on Idle**: the autosave backup ticker is now idle-triggered — it fires `auto_save_idle_ms` milliseconds (default 30 000) after the last keystroke, not on a fixed wall-clock interval. Backups are skipped when the document has active compile errors. `auto_save_idle_ms` is a new field in `config.toml`.
- **Command Palette Enhancements**: four new commands — **Find in Files…** (opens the project search panel), **Toggle Profile** (switches Final ↔ Draft compile profile), **Browse Snapshots…** (opens the snapshot timeline for the current file), and **Project Outline** placeholder (use Ctrl+G for full heading navigation).

---

## [0.11.0] — 2026-06-05

### Added

- **Configurable Compilation Profiles**: header-bar dropdown switches between **Final** (full 144 dpi render) and **Draft** (72 dpi, fast preview) profiles. Draft mode passes `sys.inputs.at("draft", default: "false") == "true"` so documents can detect the mode and skip slow elements. Profile persists to `config.toml`.
- **Session Snapshots & Version Recovery**: every Ctrl+S (and ☰ → Save) writes a timestamped `.typ` snapshot to `~/.local/share/zerkalo/snapshots/<project>/<file>/`. The last 50 snapshots per file are retained automatically. ☰ → **Browse Snapshots…** opens a timeline dialog showing each snapshot with a simple diff against the current text; **Restore** replaces the editor content.
- **Enhanced Spell Check**: project-specific dictionary at `<work_dir>/.zerkalo/dictionary.dic` (hunspell `.dic` format). Global user dictionary moved to `~/.config/zerkalo/user.dic`. Right-click on a misspelled word now shows **Add to Project Dictionary** when a project dictionary is available, in addition to the existing **Add to Dictionary** (global).
- **Inline Typst Error Assistant** (`src/error_patterns.rs`): hovering over a red-underlined error line shows a popover with the error description. For known patterns (missing brace/bracket/paren, unknown variable) a **Fix It** button applies the automated correction inline. The fix table lives in `src/error_patterns.rs` and is easy to extend.

---

## [0.10.0] — 2026-06-05

### Added

- **Find in Files enhancements** (Ctrl+Shift+F): search results now highlight the matched text in bold (Pango markup); `.gitignore` patterns are respected so build artifacts and output directories are excluded; replace-in-files mode (toggle button in search bar) with a replace entry and "Replace All" button that writes files and reloads any open tabs; last 10 searches stored in `config.toml` and shown in a dropdown next to the search entry.
- **Interactive Preview Click-to-Jump**: Ctrl+Click on the preview jumps to the matching source line by extracting text from the current PDF page via `pdftotext`; if no PDF exists it is compiled on demand. New "Copy Text from Preview" button (clipboard icon) and "Jump to Editor" button (jump icon) in the preview toolbar. Graceful error message if `pdftotext` (poppler-utils) is not installed.
- **Export Progress Dialog**: redesigned with a scrollable log view showing real-time stderr output line-by-line for all export operations; batch export mode with per-format checkboxes so multiple formats can be exported in one click; "Install Dependencies…" button opens the System Check Wizard; full error detail is always visible instead of only the first line.

---

## [0.9.0] — 2026-06-05

### Added

- **System Check Wizard**: dependency rows now detect the Linux distro from `/etc/os-release` and show the exact `apt`/`dnf`/`pacman`/`zypper` install command for each missing tool (pandoc, hunspell, git, tinymist). A "Verify" button re-checks presence after installation.
- **Template Marker Recovery**: ☰ → "Repair Template Markers…" scans the active file for the `// ── Document body` marker; if missing, re-inserts it at the preamble boundary and saves a `.typ.bak` backup. Generated templates now include a "DO NOT DELETE" warning comment above the marker.
- **Compile-on-save mode** (`compile_on_save = true` by default): on-keystroke debounce no longer triggers compilation; compilation fires on Ctrl+S instead. New `manual_compile_only` setting (default `false`) disables all automatic compilation — use Ctrl+Shift+P to compile manually. Both settings exposed in Settings → Compilation.
- **Filesystem watcher** (`notify` crate): watches the project directory for external `.typ` file changes (e.g., sync agents, other editors) and triggers re-compilation automatically.

### Fixed

- Config test: `spell_language` → `spell_languages` (field name mismatch)
- Template dialog test: added missing `sidecar_to_settings` function used by the round-trip test

---

## [0.8.19] — 2026-06-05

### Fixed

- Update Template dialog now reads metadata (title, author, etc.) from the document rather than the sidecar, so in-document edits to `#let doc-*` variables are reflected when the dialog opens
- Chicago Author-Date bibliography section heading corrected from "Reference List" to "References" (CMOS §15.2)

---

## [0.8.18] — 2026-06-05

### Fixed

- Preview auto-reflows when the window is resized: the viewport-width is now watched and `fit_width` re-runs whenever `auto_fit` is active. Zooming in/out disables auto_fit; clicking the fit-width button re-enables it

---

## [0.8.17] — 2026-06-05

### Added

- Multi-language spell checking: Settings → Extras → Spell Check now shows a list of active dictionaries with remove buttons, and an "Add language" dropdown to add more. A word is considered correctly spelled if it passes in any of the active dictionaries (so bilingual documents don't flag words from either language)

---

## [0.8.16] — 2026-06-05

### Fixed

- Word wrap now correctly reflowing on window resize: when wrap is on the horizontal scroll policy is `Never` (GTK wraps at the window edge); when wrap is off it switches to `Automatic` so long lines can be scrolled rather than silently clipped

---

## [0.8.15] — 2026-06-05

### Added

- Clicking the word count in the status bar opens a Document Statistics window: words (with session delta), characters, paragraphs, sentences, reading time, and project total if a project root is set

---

## [0.8.14] — 2026-06-05

### Fixed

- "search" status bar button now correctly shows/hides the Find & Replace bar (same as Ctrl+F), not a code-search toggle

---

## [0.8.13] — 2026-06-05

### Added

- Clicking the version number in the status bar opens the changelog in a scrollable window

---

## [0.8.12] — 2026-06-05

### Changed

- **GOST type B** toggle moved from the sidebar to the status bar — same clickable-text format as autocorrect (bold = on, dim = off)
- **search** toggle added at the left end of the status bar — controls whether Find/Replace searches inside `#commands` and `//comments` (bold = searching code too, dim = prose only)
- Removed the old sidebar Switch widget for GOST type B

---

## [0.8.11] — 2026-06-05

### Fixed

- Droplet package import updated from 0.2.0 to 0.3.1

---

## [0.8.10] — 2026-06-05

### Improved

- Completion popup snippets now show a plain-English description of what each snippet does, instead of just the raw key name
- Snippet labels no longer carry the redundant "· snippet" suffix — the kind badge already shows that
- Added a `dropcap` snippet: typing `#dropcap` now offers a ready-to-use example with a note that the Droplet package must be enabled in template settings → Packages

---

## [0.8.9] — 2026-06-05

### Added

- Autocorrect toggle in the status bar: click the word "autocorrect" to turn it on (bold) or off (dim). State is saved to config immediately, so it persists across sessions.

---

## [0.8.8] — 2026-06-05

### Fixed

- LSP/snippet completion popup no longer overlaps the text being typed — it now anchors at the left margin of the editor, below the current line
- Popup is wider (480 px) and taller (380 px max), so function signatures and documentation are readable without truncation
- Detail text now wraps instead of being cut off with an ellipsis
- Added a footer hint showing the keyboard controls (↑↓ navigate · Tab/↵ insert · Esc dismiss)

---

## [0.8.7] — 2026-06-05

### Internal

- Removed 6 dead functions from `template_dialog.rs` (`extract_preamble`, `sidecar_to_settings`, `replace_in_set_blocks`, `reapply_preamble`, `update_body_front_matter`, `update_body_front_matter_headingless`) along with their tests

---

## [0.8.6] — 2026-06-05

### Fixed
- **Citation panel — missing titles** — replaced regex `[^{}]*` field parser with a brace-depth-aware manual parser; titles containing nested braces (e.g. `{On {Church} and {State}}`, `{{All Caps Title}}`) now parse correctly instead of returning empty
- **Citation panel — double-click** — switched from per-row `connect_activate` to a single `list.connect_row_activated` handler (the canonical GTK4 activation path); double-click and Enter now both insert the citation key; `activate_on_single_click` explicitly set to `false` to match expected UX

---

## [0.8.5] — 2026-06-05

### Fixed
- **No-marker confirmation** — "Update Template Settings" now shows a destructive-action confirmation dialog when the document has no `// ── Document body` marker, warning the user that their content will be replaced
- **Corrupt sidecar logging** — `load_sidecar` now emits a `WARN` log entry when the `.zerkalo.toml` exists but fails to parse (previously swallowed the error silently)

---

## [0.8.4] — 2026-06-05

### Changed
- **Template settings sidecar** — each `.typ` document now gets a `<stem>.zerkalo.toml` sidecar file that stores all template settings (style, font, paper, margins, sections, languages, packages, metadata). "Update Template Settings" reads from the sidecar instead of text-parsing the `.typ` file, so pre-fill is always reliable.
- **Apply redesign** — "Apply to Current" now regenerates the preamble/title/front-matter completely from the new settings and splices at the `// ── Document body` marker, preserving user body content. Replaces the fragile four-pass text-surgery approach.
- **`TemplateDialog` extended** — dialog now stores and preselects page-numbers, language switches, and package switches from sidecar (previously could not round-trip these fields).

---

## [0.8.3] — 2026-06-05

### Changed
- **app_window.rs split** — CSS loading extracted to `load_app_css()`; hamburger menu items extracted to `build_hamburger_menu_items()` + `HamburgerItems` struct
- **Plan panel project fallback** — panel accepts `work_dir`; when no file is open it loads `project.plan` from the project root instead of disabling
- **Export dialog** — remembers last-used format across sessions via `last_export_format` in config
- **Style button loop** — replaced `unwrap()` on downcast with safe `if let`

### Added
- **Session delta label** — status bar shows `↑ N` words added since file was opened
- **Tab error indicator** — red ⬤ dot on tab label when the file has compile/LSP errors
- **Ctrl+G** — opens command palette pre-filtered to document headings only

### Fixed
- `Cargo.lock` removed from `.gitignore` (correct for binary applications)

---

## [0.8.2] — 2026-06-05

### Changed
- **TODO panel → Plan panel** — replaced the per-file checklist with a freeform text scratchpad; notes are saved as a `.plan` sidecar file alongside the `.typ` document

---

## [0.8.1] — 2026-06-05

### Fixed
- **Style switch** — switching styles no longer wipes out the abstract, outline, extra pagebreaks, bib file pointer, or bibliography; the title-block replacement now stops at the first front-matter/body marker instead of scanning the whole document for `#pagebreak()`
- **Default font** — template dialog now defaults to Times New Roman instead of GOST type B
- **Sidebar** — sidebar can now be compressed to a much smaller width; search entry, buttons, and labels have `min-width: 0` so the paned divider is no longer blocked; citation key labels ellipsize rather than forcing a minimum width

---

## [0.7.1] — 2026-06-01

### Fixed
- **Simple mode** — cheatsheet/help toggle and pop-out preview button are now visible in simple mode; only watch mode, page navigation, compile-time label, and advanced menu items are hidden

---

## [0.7.0] — 2026-06-01

### Added
- **Import wrapping** — LaTeX, DOCX, and PDF files imported via ☰ → Import… now receive a Zerkalo-managed template section (`ZERKALO-TEMPLATE-BEGIN/END`) automatically; imported documents are immediately responsive to "Update Template Settings" without any manual preamble setup
- **Startup checks for `hunspell` and `tinymist`** — if either is missing, a dialog at startup shows per-distro install instructions (`zypper`, `apt`, `brew`, `dnf`); pandoc and pdftotext error dialogs also now include platform-specific install commands
- **22 new unit tests** — covering `parse_font`, `parse_paper`, `parse_spacing`, `replace_in_set_blocks`, `strip_style_block`, `reapply_preamble` (font and spacing propagation), and `strip_pandoc_preamble`

### Changed
- **Line spacing recalibrated** — spacing options now use Typst `leading:` (inter-line gap) rather than `spacing:` (paragraph gap): Single = 0.65 em, 1.5 Lines = 0.9 em, Double = 1.2 em; templates generate both `leading:` and a fixed `spacing: 1.2em` in `#set par`
- **Font replacement scoped** — "Update Template Settings" font substitution now only touches `#set text(…)` blocks; comments and string literals containing the old font name are left unchanged
- **Spacing propagation** — "Update Template Settings" now propagates `leading:` changes to every `#set par(…)` block in the document (including hand-written config sections after the template marker), matching the existing font-propagation behaviour

### Fixed
- **RefCell re-entrancy crashes (3 classes)** — `set_content`, `set_active_content`, and `close_file` each held an active borrow guard when calling `buffer.set_text()` or `notebook.remove_page()`, which synchronously fired GTK signals that re-entered the same `RefCell` and panicked; all three patched with the borrow-then-clone-then-drop pattern
- **Startup crash: stale `glib::SourceId`** — `SourceId::remove()` was called on a timer ID that had already auto-removed itself on first fire, causing a panic on startup; timer callbacks now clear their own slot immediately so stale IDs are never removed
- **Template style-block override** — a `ZERKALO-STYLE-BEGIN/END` block from the legacy Style button appearing after the template marker would silently override font and spacing; it is now stripped whenever "Update Template Settings" is applied
- **Tab dropdown borrow safety** — the tab-list popover held `state.borrow()` across GTK widget construction and `vbox.append()` calls; the borrow is now released before any GTK calls

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
