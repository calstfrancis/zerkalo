# Changelog

All notable changes to Zerkalo are recorded here.  
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [0.13.5-rc3] — 2026-06-09

### Fixed

- **Completion popup arrow-key navigation** now skips hidden (filtered-out) rows correctly.
- **Escape in completion popup** now deletes the typed `#word` back to before the `#`, matching standard editor behaviour.
- **Completion popup appears immediately** when `#` is typed using built-in snippets; LSP results are merged in when they arrive (~150 ms later).
- **Completion popup no longer steals focus or blocks typing** (`autohide: false`; focus stays in the editor).
- **Completion popup no longer covers the cursor** (above/below logic, same as citation popup).

## [0.13.5-rc2] — 2026-06-09

### Added

- **Completion popup client-side filter**: popup now shows all completions when `#` is typed; as you type further letters the list filters and refocuses to the first match — no more replacing the whole list on each keystroke.
- **Numbering format selector**: Sections tab gains a "Numbering Format" ComboRow (Decimal 1.1.1., IEEE Roman I.A.1., Alpha a.a.a.) that appears when "Numbered Headings" is on.
- **Preview Code button**: header bar of the template dialog now has "Preview Code…" — shows the generated Typst preamble in a read-only window before applying.

### Changed

- **IEEE/GOST/Vancouver numbering now user-controlled**: the `#set heading(numbering:)` directive is no longer hardcoded inside the heading style strings; the "Numbered Headings" toggle (which defaults to ON for IEEE, GOST, Vancouver) drives it, so users can now disable IEEE's Roman-numeral numbering via the toggle.
- **Heading numbers now actually render**: custom `#show heading` rules now include `#if it.numbering != none [#context counter(heading).display(it.numbering)#h(0.3em)]` before the body, so turning on numbering actually shows numbers. Fixes GOST and Vancouver too.
- **Outline panel icons**: "Outline" and "Symbols" segmented buttons now use 20 px symbolic icons (`view-list-symbolic` / `input-keyboard-symbolic`) instead of text labels.

## [0.13.5-rc1] — 2026-06-09

### Added

- **Vancouver citation style**: new style option with numbered headings and Vancouver bib output.
- **Font size selector**: template dialog now has a 10/11/12/14 pt selector in the Typography section.
- **Numbered headings toggle**: Sections tab now has a "Numbered Headings" switch (1. 1.1 …).
- **#lorem() word count**: `#lorem(N)` is now counted as N words in section WC (breadcrumb) and outline panel word counts.
- **App screenshot**: added screenshot for GNOME Software / Discover.

### Changed

- **GOST style renamed**: "GOST 7.32" → "GOST R 7.0-5 (numeric)" throughout; bib style correctly uses `gost-r-705-2008-numeric` (was incorrectly falling back to APA).
- **Abstract preservation**: Update Template dialog now reads the abstract the user has typed directly in the `.typ` file; that text wins over the sidecar.
- **Codly package version**: bumped to 1.3.0; showybox to 2.0.4; gentle-clues to 1.2.0; drafting to 0.2.2.
- **Window title priority**: `#let doc-title = "..."` template variable is now checked before the first `= Heading` when setting the window title.

### Fixed

- **Preview no longer scrolls on mouse click**: clicking in the editor no longer jumps the preview; only keyboard navigation (typing) triggers the scroll-to-section.
- **LSP completion popup closes on click-away**: clicking outside the popup now dismisses it (autohide re-enabled).

---

## [0.13.4] — 2026-06-09

### Added

- **Ctrl+B / Ctrl+I**: wrap selection in `*bold*` / `_italic_` Typst markup; pressing again strips the markers.
- **Ctrl+Shift+E**: export PDF directly to the document folder with no dialog. Shows a toast on completion.
- **Section word count**: breadcrumb bar shows `§ N` word count for the heading section under the cursor.
- **Compile progress stripe**: thin pulsing bar at the bottom of the header bar while a compile runs.
- **Citation panel single-click**: clicking a reference in the sidebar inserts `@key` immediately (was double-click).
- **Window title from document**: header shows the Typst `title:` metadata, falling back to first `= Heading`, then filename.

### Improved

- Preview scroll position preserved across recompiles — no more jumping to the top on every compile.
- Preview page shadows: soft drop shadow behind each page on the gray canvas.
- Status bar separators: thin vertical dividers between control groups.
- Modified-dot indicator on tabs now renders in the accent color.
- Citation popup: no longer steals focus while typing a key; positions above/below cursor correctly; suppresses compile errors while the popup is open; shows full bibliography list.
- GOST bibliography style corrected to `gost-r-705-2008-numeric` (the actual bundled identifier).
- Breadcrumb separator changed from `›` to `/`.
- Style button no longer shows a dropdown arrow.
- "Developer mode" renamed to "Experimental mode" in settings.
- Auto-compile no longer halts after typing `@` when no bib matches are found.

---

## [0.13.4-rc8] — 2026-06-09

### Improved

- Section word count (`§ N`) moved from status bar to breadcrumb bar, appearing between the heading path and the word-wrap button.

---

## [0.13.4-rc7] — 2026-06-09

### Added

- **Ctrl+B / Ctrl+I**: wrap selection in `*bold*` / `_italic_` Typst markup; pressing again on an already-wrapped selection strips the markers.
- **Section word count**: status bar shows `§ N` word count for the heading section under the cursor; updates only when the cursor crosses a line boundary.
- **Compile progress stripe**: thin pulsing bar appears at the bottom of the header bar when a compile is running; disappears on completion.

### Improved

- **Preview page shadows**: each page now has a soft three-layer drop shadow against the gray canvas background.
- **Status bar separators**: thin vertical separators group the left controls from the right word-count block.
- **Modified-dot color**: the tab modified indicator (`●`) now renders in the accent color instead of the default foreground.

---

## [0.13.4-rc6] — 2026-06-09

### Added

- **Ctrl+Shift+E**: exports PDF directly to the document's own directory, no dialog. Shows "Exporting PDF…" toast while compiling, then success/error toast.
- **Window title from document**: header title now shows the Typst `title:` metadata field, falling back to the first `= Heading`, then the filename.
- **Citation panel single-click insert**: clicking once in the Citations sidebar now inserts `@key` at the cursor (was double-click).

---

## [0.13.4-rc5] — 2026-06-09

### Fixed

- Auto-compile no longer dies after typing `@`: `bib_active` flag is now cleared unconditionally on dismiss, and set only if the popup actually shows (previously stayed `true` when no bib matches were found, permanently suppressing compile).
- GOST 7.32: corrected bibliography style name to `gost-r-705-2008-numeric` (Typst's actual bundled identifier).

---

## [0.13.4-rc4] — 2026-06-09

### Fixed

- Citation popup: when appearing above the cursor, the bottom of the popup now lands at the cursor line's top edge — the cursor line is fully visible.

---

## [0.13.4-rc3] — 2026-06-09

### Fixed

- Citation popup: no longer steals keyboard focus — keystrokes always register in the editor.
- Citation popup: smart above/below placement — anchors below cursor when in the upper half of the view, above when in the lower half, so it never lands on the line being typed.
- Citation popup: compile and LSP diagnostics are suppressed while the popup is open, preventing error spam from partial `@keys`.
- GOST 7.32: bibliography style changed to `gost-r-7-0-5` — numeric citations with GOST-format entries using `//` article separators.

---

## [0.13.4-rc2] — 2026-06-09

### Changed

- Citation popup: positioned to the right of the cursor so it never overlaps the text being typed.
- Citation popup: shows the full bibliography list instead of capping at 15 entries.
- GOST 7.32 citation style: switched from author-date (APA) to footnote-based (Chicago Notes).
- Breadcrumb heading path: separator changed from `›` to `/`.
- Style button: plain button with no dropdown arrow (popover still works on click).

---

## [0.13.4-rc1] — 2026-06-09

### Changed

- Style dropdown: the dropdown triangle arrow is hidden (button still opens the menu).
- Settings: "Developer mode" renamed to "Experimental mode".
- Preview pane: scroll position is no longer reset after each compile — position is fully user-controlled (scroll, page buttons, arrow keys). Only the first compile auto-fits to width.

---

## [0.13.3] — 2026-06-09

### Changed

- LSP status indicator: running dot (●) is now green, error (✗) is red.
- LSP completion popup: items are now sorted alphabetically.
- LSP completion popup: double-click on a row inserts the completion (Tab and Enter already worked).

---

## [0.13.2] — 2026-06-09

### Fixed

- Simple Mode: preamble stays hidden after Update Template or Style dropdown — `buffer.set_text` was clearing all text tags; simple mode tag is now reapplied after every content replacement.
- Simple Mode: hidden preamble text is no longer silently dropped during compilation, saving, style changes, or spell-check — all content-retrieval calls now use `include_hidden_chars = true` so the invisible front-matter is always preserved.

---

## [0.13.1] — 2026-06-08

### Added

- **Simple Mode** toggle in the status bar (SIMPLE — bold when on, plain when off): hides the Typst front-matter above `// ── Document body` so you see only your document content. Line numbers are unchanged. Edit front-matter via the Update Template button. Defaults to on for new installs with a first-run explainer popup.
- Style dropdown moved from the status bar to the toolbar (breadcrumb bar, right side), next to word-wrap and undo/redo.
- Removed the open-tabs pan-down button from the toolbar (the file dropdown in the title bar handles this).

---

## [0.13.0] — 2026-06-08

### Fixed

- Preview pane: scroll now works immediately after the first compile, without needing to resize the pane first (root cause: content dimensions were set asynchronously, leaving the vadjustment with no scrollable range until the next resize)

---

## [0.12.34] — 2026-06-08

### Fixed

- DOCX/LaTeX import: strip ALL pandoc-generated `#set`, `#show`, and `#let` preamble blocks (previously only a subset was stripped, leaving `#let conf(...)` and `#show terms:` in the body and causing compile errors)
- Multi-line `#let` blocks now tracked with full delimiter depth (parens, brackets, braces) so large template functions are consumed correctly

---

## [0.12.34-rc2] — 2026-06-08

### Fixed

- Import LaTeX and Import DOCX now route pandoc through `flatpak-spawn --host` so they work inside the flatpak sandbox
- Error panel rows now show "Line N · filename:col" instead of "filename:line:col" for quicker scanning

---

## [0.12.34-rc1] — 2026-06-08

### Added

- Export for Web — converts the active Typst file to an HTML fragment via pandoc; footnotes become hover tooltips that respond to light/dark mode toggles (`data-theme`, `.dark`/`.light` classes, and `prefers-color-scheme`)

---

## [0.12.33] — 2026-06-08

### Added

- Delete button (trash icon) beside every file in the open dropdown — asks for confirmation, removes from disk, closes the tab if open, and removes the row from the list

---

## [0.12.33-rc1] — 2026-06-08

### Added / Changed

- Changelog window now renders release notes with proper GTK formatting — version headers, category sub-heads, and formatted bullets instead of raw monospace text
- Template dialog: lock (padlock) buttons on Author and Affiliation fields to save them as defaults for new documents
- Template dialog: date field now shows a tooltip noting it defaults to today if left blank
- Flatpak manifest: sourced from local directory instead of GitHub — test builds no longer require a push
- RC versioning scheme introduced: builds are numbered `X.Y.Z-rcN`; the suffix is removed on release

### Fixed

- Setup wizard widened to 640×620 for better readability
- What's New window updated to reflect 0.12.32 features

---

## [0.12.32] — 2026-06-08

### Added / Fixed

- Right-click on any editor tab shows a context menu with "Close tab" and "Delete file…" (with confirmation dialog)
- Fix: app was re-creating old project folder on every launch (removed unconditional `create_dir_all` of work_dir at startup)
- Author/affiliation lock: fixed compile error (missing fields in settings dialog Config initializer)

---

## [0.12.31] — 2026-06-08

### Fixed / Changed
- **Start maximized** — main window now opens maximized
- **Setup wizard: resizable** — removed non-resizable constraint; window is scrollable and resizable again
- **Setup wizard: bundled tools** — tinymist and pandoc shown as bundled (always ✓); only git and hunspell show install instructions

---

## [0.12.30] — 2026-06-08

### Changed
- **Welcome window: What's New** — updated to reflect 0.12.29 features (single-file workspace, status bar layout, page gaps, session restore, LCS diff, popout maximize)

---

## [0.12.29] — 2026-06-08

### Changed / Fixed
- **Status bar layout** — autocorrect and GOST Type B toggles moved to left side of status bar; Style dropdown and Draft/Final toggle moved from header bar to right side of status bar (Draft shown bold)
- **Preview: page gaps** — pages now separated by a visible 20px gray gap so page boundaries are clear
- **Session restore** — app now opens the last-edited file on startup
- **Snapshot diff** — replaced positional line diff with LCS-based diff; only truly changed lines shown as red/green, context lines shown around each change
- **Setup wizard sizing** — capped at 500×560, non-resizable to prevent oversized dialog
- **Setup wizard tool check** — `git`, `pandoc`, `tinymist` now correctly verified inside flatpak sandbox via `flatpak-spawn --host`; bundled tinymist at `/app/lib/zerkalo/tinymist` detected directly
- **Popout preview: maximize button** — added maximize button to the popout window header
- CLAUDE.md: added rule that GitHub pushes and flatpak publishes only happen on explicit release instruction

---

## [0.12.28] — 2026-06-08

### Fixed / Improved
- **Snapshot diff: color coding** — removed lines shown with red background/text, added lines green; hunk headers blue in git history
- **Snapshot diff: history panel click** — `connect_activate` → `connect_row_selected` (same fix as snapshot list; Single-mode ListBox rows weren't firing activate on single click)
- **Document Statistics popup** — replaced monospace text layout with `adw::PreferencesGroup` / `adw::ActionRow`; removed stale "Project total" row
- **Audit** — no other `connect_activate`-on-Single-mode bugs found in remaining panels

---

## [0.12.27] — 2026-06-08

### Fixed
- **Browse Snapshots: clicking a snapshot did nothing** — row handler was `connect_activate` (fires only on Enter/double-click); replaced with `list_box.connect_row_selected` so single clicks update the diff view and enable Restore

---

## [0.12.26] — 2026-06-08

### Fixed
- **Preview pane size resets on open**: pane position-notify now ignores changes during initial GTK layout (flag set on idle after realize), so the saved split is always restored correctly
- **Git sync uses wrong directory**: sync now derives the git repo root from the active file's path (`git rev-parse --show-toplevel`) instead of `config.work_dir`
- **Remove Open Project Folder / Recent Projects menu items**: these were holdovers from project mode and no longer serve any purpose

---

## [0.12.25] — 2026-06-08

### Changed
- **Single-file workspace**: removed project mode entirely. The active tab is always the compilation root — no root chip, no "Set as Compilation Root", no project config `root_file`. Removes `ProjectModel`, `is_project_mode` flag, New Project wizard, Project Settings dialog, and all related UI

---

## [0.12.24] — 2026-06-08

### Fixed
- **Crash on outline click in project**: `jump_to_line` was called synchronously inside `open_file`'s callback chain, causing reentrancy. Now deferred to idle so all page-switch callbacks complete before scrolling
- **Compile ignores changes in project mode**: `is_project_mode` was checking `EditorPane.project_root` (always set to `work_dir`, never None) instead of whether the user created an explicit project. Now uses a proper `is_project_mode` flag (true when `.zerkalo/config.toml` has `root_file`). In single-file mode, tab switch/save/keystroke all update the compilation root correctly
- **Ctrl+S reset project root**: saving any file was unconditionally calling `preview.set_root_file(path)`, clobbering the project root
- **Root chip "Project Settings…" did nothing**: `list_box.parent()` is GTK's internal `PopoverContent` wrapper, not a `Popover`, so the popdown before presenting the dialog never fired. Now stores `root_popover` directly in `EditorPane` and calls `popdown()` on it
- **Weird grey area in root chip popover**: removed separator `ListBoxRow` and replaced with top margin on the settings row
- **No way to leave project mode**: added "Clear root file" row to the root chip popover; clears the compilation root and returns to single-file mode (root follows active tab)

---

## [0.12.23] — 2026-06-08

### Fixed
- **Root chip now shows "Project Settings…"**: clicking the compilation root chip in the status bar opens a popover that lists candidate root files and includes a "Project Settings…" row at the bottom, so users can change or clear the root file without going through the hamburger menu
- **Outline filenames as tooltip**: in multi-file projects the file name is now shown on hover instead of inline, making the section title fully readable

---

## [0.12.22] — 2026-06-08

### Fixed
- **Preview not updating while typing in project mode**: the debounced on_change handler was calling `set_root_file(active_tab)` on every keystroke, so edits to a non-root file were compiled as if that file were the root. Now skips `set_root_file` when a project root is already set (same fix as the Compile button and tab-switch handler)
- Flatpak runtime bumped to GNOME Platform 50

---

## [0.12.21] — 2026-06-08

### Fixed
- **Crash on file switch in project**: `connect_switch_page` held `state.borrow()` while the page-switch callback called `all_tab_texts()` which tried to borrow state again → double-borrow panic. Fixed by extracting page data and releasing the borrow before firing the callback
- **Compile button resets project root**: clicking Compile (or switching tabs) was overwriting the project's compilation root with whatever file was active. Both now skip `set_root_file` when a project root is already set
- **tinymist not found in flatpak**: binary is at `/app/lib/zerkalo/tinymist` in the flatpak, not `/usr/lib/`; both `lsp.rs` and the startup check now probe both paths
- **history panel git calls broken in flatpak**: now uses `flatpak-spawn --host git` via the shared `host_command()` helper
- **pandoc/pdftotext broken in flatpak**: export dialog and PDF text extraction now use `host_command()` so they reach the host binaries

---

## [0.12.20] — 2026-06-08

### Fixed
- Startup git warning: use `flatpak-spawn --host git` inside the flatpak sandbox so the check passes correctly
- Welcome window "What's New" now lists the 0.12 features (multi-file projects, template build system, New Chapter, Project Settings, cross-file outline) instead of 0.11 era items
- GOST Type B font ships in all three flatpaks (Kopilka, Rubric, Zerkalo) at `/app/share/fonts/gosttypeb.ttf` so fontconfig inside the sandbox finds it

---

## [0.12.19] — 2026-06-08

### Added
- **Template build system**: built-in templates are now real `.typ` files in `templates/` (embedded via `include_str!` at compile time); user templates can be added to `~/.config/zerkalo/templates/<name>/manifest.toml` and appear in the New Project dialog automatically
- **New Chapter**: file tree header has a "New Chapter" button — enter a chapter name, creates `<slug>.typ` with a heading stub and appends `#include "<slug>.typ"` to `main.typ` before `#bibliography` (or at end); opens the new file immediately
- **Project Settings dialog**: "Project Settings…" in the menu opens a per-project settings sheet — change compilation root and bibliography path, saved to `.zerkalo/config.toml`
- **Cross-file outline**: document outline now shows headings from all project `.typ` files, not just the active tab; each heading shows which file it belongs to; clicking jumps to the correct file and line

---

## [0.12.18] — 2026-06-08

### Fixed
- New Project: after creation, the spawned process now receives `main.typ` as a CLI argument so session restore is skipped and the new project opens directly instead of restoring the previous project's documents
- Session restore: only files inside the current work_dir are restored; files from a previous project no longer leak in when the work_dir has changed

### Added
- Flatpak: `--socket=ssh-auth` added to finish-args so SSH git remotes work inside the sandbox

---

## [0.12.17] — 2026-06-08

### Added
- Help window: new "Projects" tab covering the full multi-file workflow (wizard, root, file tree, #include helper, project config, worked example)
- Help window: Overview tab now mentions multi-file projects with a pointer to the Projects tab
- Help window: five new FAQ entries (create a project, root concept, ★ indicator, add a chapter, missing root chip)
- README: Multi-file projects feature table; updated file tree row description

---

## [0.12.16] — 2026-06-08

### Added
- File tree: right-click menu now has "Insert #include" and "Insert #import"; inserts at the cursor with a path relative to the compilation root's directory
- `#import` snippet also adds the file stem as the imported identifier (`#import "ch01.typ": ch01`)

---

## [0.12.15] — 2026-06-08

### Added
- File tree: subdirectory rows are now collapsible — click the folder header to toggle; arrow icon shows expand/collapse state
- File tree: "New Folder" button (folder-new-symbolic) in the panel header creates a subfolder in the project root
- DnD idle-rebuild simplified to use `FileTree::clone()` instead of manual field reconstruction

---

## [0.12.14] — 2026-06-08

### Added
- Status bar: "Root: filename.typ" chip button; clicking it opens a popover listing all candidate root files so you can switch the compilation root without touching the file tree
- `ProjectModel::candidate_roots()` — returns files not imported by any other

---

## [0.12.13] — 2026-06-08

### Added
- File tree: ★ indicator on the current compilation root row
- File tree: right-click context menu now has "Set as Compilation Root" above "Delete"; selecting it writes `root_file` to `.zerkalo/config.toml`, updates the preview, and triggers a recompile
- New Project wizard: four templates (Blank, Essay, Journal/Thesis, Theological Journal); creates project folder, generates starter .typ files, opens the new project directly

---

## [0.12.12] — 2026-06-08

### Fixed
- **Flatpak: git sync now works** — all `git` calls delegate to the host system via `flatpak-spawn --host git`; added `--talk-name=org.freedesktop.Flatpak` to finish-args
- **Flatpak: Typst package cache accessible** — added `--filesystem=~/.cache/typst` so packages installed on the host are found inside the sandbox

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
