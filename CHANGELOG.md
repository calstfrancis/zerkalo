# Changelog

All notable changes to Zerkalo are recorded here.

---

## [Unreleased]

### Added
- **Preview zoom** — `+` / `−` buttons in the preview toolbar; zoom range 25 %–400 %; re-renders PDF at the corresponding DPI; zoom is persisted in config
- **Pop-out preview** — detach the preview pane into its own floating window; refreshes automatically after each compile
- **Sidebar toggle** — button in the header bar to show/hide the left file-tree + outline sidebar
- **Insert panel** — collapsible column of one-click Typst snippet buttons (headings, bold, italic, math, figure, table, columns, page break, list, link, label, dropcap, bibliography, …); toggled by the list-add button in the header
- **Help window** — accessible via ☰ → Help; tabs for Overview, Keyboard Shortcuts, FAQ, and About; rich-text display with styled headings, body text, and code blocks
- **First-run guide** — welcome dialog shown once on the first launch
- **Hamburger menu** — Settings, Help, and About consolidated into a `☰` popover
- **Header file selector** — dropdown listing all project `.typ` files, replaces the static window title
- **LSP completions** — `#` trigger fires `textDocument/completion` via tinymist; popup shows kind badge, label, and detail; Tab/Return to accept, Esc to dismiss
- **Find & Replace** — `Ctrl+F` revealer with forward/backward search and replace-all
- **Document outline** — heading tree in the left sidebar (level 1 bold, clickable rows jump to the heading)
- **Word count** — live count and reading-time estimate in the editor status bar
- **Cursor position** — line and column indicator at the left of the editor status bar
- **Syntax highlighting** — custom Typst grammar for GtkSourceView: comments, strings, headings, keywords, math, citations, labels, function calls
- **Citation autocomplete** — `@` trigger with BibTeX popup; Tab/Return to accept
- **Auto-compile** — debounced compile-on-change with configurable delay
- **Git sync** — one-click commit + push; remote setup dialog on first sync
- **Multi-file tabs** — open multiple `.typ` files; modified indicator dot; close button
- **Desktop integration** — `install.sh` / `uninstall.sh` install the binary, `.desktop` file, and icon to `~/.local/` for launcher registration

### Changed
- App name is now **Zerkalo** throughout (Latin script only, no Cyrillic)
- Settings dialog reorganised into four groups: Appearance, Editor, Compilation, Bibliography
- **Editor font family** is now configurable (Settings → Editor → Font family); changes apply live
- **Editor font size, tab width, word wrap, show whitespace** added to Settings
- **Preview zoom** stored in `config.toml` and restored on next launch
- Settings button replaced by the hamburger `MenuButton`
- Help window display upgraded from plain text to a rich `TextBuffer` with styled headings and code blocks

---

## [0.1.0] — 2026-05-24

### Added
- Initial scaffold: GTK4 + libadwaita window, GtkSourceView editor, live preview via `typst compile` + `pdftoppm`
- Project config (`~/.config/zerkalo/config.toml`) and per-project config (`.zerkalo.toml`)
- File tree sidebar with create/delete support
- Error panel with jump-to-error
- Settings dialog (font size, debounce, theme, bib path)
- Git sync with `git2`
- tracing-based logging to `~/.local/share/zerkalo/zerkalo.log`
