# Зеркало (Zerkalo) — Development Roadmap for Claude Code

**Current Status**: Minimal compilable scaffold (main.rs, error.rs, ui stubs)

**Next Phase**: Full implementation in Claude Code

---

## Phase 1: Core Editor & UI (High Priority)

### 1.1 Multi-File Editor with GtkSourceView
- [ ] Implement `EditorPane` widget with:
  - GtkSourceView-based text editor with syntax highlighting
  - Tabbed interface (multiple open files)
  - Line numbers, auto-indent, smart backspace
  - Auto-detect language from file extension (Typst, LaTeX, Python, TOML, etc.)
  - Track modified status (dot on tab for unsaved files)
- [ ] File buffer management:
  - Load `.typ` files into memory on open
  - Write buffers to disk on save
  - Track which file is active

### 1.2 Project File Tree
- [ ] Implement `FileTree` widget:
  - Scan project folder for `.typ` files
  - Display in a hierarchical list
  - Detect and show imports (`#include`, `#import`)
  - Click to open file in editor
  - Respect `.gitignore` (don't show build artifacts)

### 1.3 Live Preview Pane
- [ ] Implement `PreviewPane` widget:
  - Call `typst compile input.typ output.pdf`
  - Convert PDF to PNG using `pdftoppm`
  - Display rendered page in scrollable area
  - Show spinner while compiling
  - Update on save or timer-based debounce

### 1.4 Error Panel
- [ ] Implement `ErrorPanel` widget:
  - Parse Typst compilation stderr
  - Extract file, line, column, message
  - Display in collapsible panel at bottom
  - Click error row to jump to line in editor
  - Auto-show when errors occur, auto-hide when cleared

---

## Phase 2: File I/O & Project Management (High Priority)

### 2.1 Project Initialization
- [ ] On first launch:
  - Show dialog: "Pick project folder"
  - Default to `~/Documents/Zerkalo`
  - Create folder structure (`.git`, `.zerkalo/`, template `main.typ`)
  - Initialize git repo and create `.gitignore`
- [ ] Store choice in `~/.config/zerkalo/config.toml`
- [ ] Remember last project on subsequent launches

### 2.2 Project Detection
- [ ] Scan project folder for:
  - Root `.typ` file (largest file, or let user designate)
  - All `.typ` files in directory
  - Parse `#include` and `#import` statements
- [ ] Build dependency tree of imports
- [ ] Always compile from root file (all imports are included)

### 2.3 File I/O
- [ ] Read file contents when opening in editor
- [ ] Write buffer to disk on save (Ctrl+S)
- [ ] Create new blank files in project
- [ ] Delete files from UI
- [ ] Watch file system for external changes (optional, for later)

---

## Phase 3: Event Loop & Keyboard Shortcuts (High Priority)

### 3.1 Main Event Loop
- [ ] Wire up UI interactions:
  - **File tree**: Row click → open file in editor
  - **Editor tabs**: Click to switch, close button to close
  - **Preview button**: Trigger compilation
  - **Sync button**: Git add/commit/push
  - **Settings button**: Open preferences dialog
- [ ] Keyboard shortcuts:
  - `Ctrl+S` → save active file
  - `Ctrl+Shift+P` → compile and preview
  - `Ctrl+Q` → quit
  - `Ctrl+Tab` → next tab
  - `Ctrl+Shift+Tab` → previous tab

### 3.2 Preview Loop
- [ ] Debounced compilation:
  - On text edit in editor, start 500ms idle timer
  - On idle timeout, compile
  - While compiling, show spinner
  - Update preview pane with new PDF
  - If errors, show in error panel (auto-reveal)
- [ ] Manual "Preview Now" button (for immediate compile)

### 3.3 Auto-Save to Disk
- [ ] Periodically save modified buffers to disk
- [ ] Or save on compile trigger
- [ ] Show unsaved indicator on tabs

---

## Phase 4: Bibliography & Autocomplete (Medium Priority)

### 4.1 Bibliography Integration
- [ ] Load `.bib` file from:
  - Global path in `~/.config/zerkalo/config.toml`
  - Or per-project path in `.zerkalo/config.toml`
- [ ] Parse `@key` citation keys from `.bib`
- [ ] Watch `.bib` file for changes (auto-reload)
- [ ] Suggest keys on `@` in editor (popup/dropdown)

### 4.2 Citation Autocomplete
- [ ] On `@` typed in editor:
  - Show dropdown with matching citation keys
  - Fuzzy filter as user types
  - Insert selected key on Enter/Tab
  - Show key's entry details (author, title, etc.)

### 4.3 Typst Syntax Autocomplete (Optional, Lower Priority)
- [ ] Connect to Typst LSP:
  - Start `typst lsp` subprocess on app launch
  - Send `did_open`, `did_change` notifications
  - Request completions on `.`, `#`, `{` triggers
  - Merge with bibliography suggestions
- [ ] For MVP, focus on bibliography autocomplete first

---

## Phase 5: Git Sync (Medium Priority)

### 5.1 Smart Git Commit
- [ ] Track changed files since last sync
- [ ] On "Sync" button click:
  - `git add .`
  - Auto-craft commit message:
    - If 1 file: "Edited foo.typ: [timestamp]"
    - If 2+ files: "Edits to foo.typ, bar.typ\n\n[timestamp]"
    - If no changes: "Auto-save: [timestamp]"
  - `git commit -m "message"`
  - `git push origin main` (or current branch)
  - Show success/error dialog

### 5.2 GitHub Setup
- [ ] On first sync, if no remote:
  - Show dialog: "Link to GitHub?"
  - User can paste GitHub repo URL
  - Run `git remote add origin <url>`
  - Then push

---

## Phase 6: Configuration & Preferences (Low Priority)

### 6.1 Settings Dialog
- [ ] User can configure:
  - Bibliography path (global)
  - Preview debounce delay (ms)
  - Auto-compile on save (toggle)
  - Font/size for editor
  - Theme (light/dark, system default)
- [ ] Save to `~/.config/zerkalo/config.toml`

### 6.2 Project-Level Config
- [ ] `.zerkalo/config.toml` can override globals:
  - Bibliography path (per-project)
  - Typst compiler settings
  - Preview output directory

---

## Phase 7: Polish & Distribution (Low Priority)

### 7.1 Error Handling & Logging
- [ ] User-facing error dialogs for:
  - Missing files
  - Typst not installed
  - Git errors (auth, push failure)
  - Permission issues
- [ ] Detailed logs to `~/.local/share/zerkalo/zerkalo.log`

### 7.2 GitHub Actions CI/CD
- [ ] `.github/workflows/build.yml`:
  - On tag (v0.1.0, etc.), build release
  - `cargo build --release`
  - Create AppImage using `appimagetool`
  - Upload to GitHub Releases
- [ ] Users download `.AppImage`, `chmod +x`, run

### 7.3 AppImage Packaging
- [ ] Build script to create portable AppImage
- [ ] Distribute via GitHub Releases
- [ ] Optional: Submit to openSUSE OBS, AUR later

---

## Implementation Order (Recommended)

1. **Phase 1.1** → Editor pane (GtkSourceView tabs)
2. **Phase 2.1** → Project init dialog
3. **Phase 2.2** → Project detection & file tree
4. **Phase 1.2** → File tree widget integration
5. **Phase 2.3** → File I/O (read/write to disk)
6. **Phase 3.1** → Event loop (button clicks, file selection)
7. **Phase 3.2** → Preview loop (debounced compile)
8. **Phase 1.3** → Preview pane (PDF display)
9. **Phase 1.4** → Error panel
10. **Phase 4.1-4.2** → Bibliography & citation autocomplete
11. **Phase 5** → Git sync
12. **Phase 6** → Settings dialog (optional for MVP)
13. **Phase 7** → Polish, CI/CD, distribution

---

## Key Technical Details

### Dependencies
- **gtk4** 0.7, **libadwaita** 0.5, **sourceview5** 0.7 — UI
- **tokio** — async runtime (all I/O is non-blocking)
- **git2** — git operations
- **regex** — parse imports and .bib keys
- **serde/toml** — config files

### File Structure
```
src/
├── main.rs              # GTK app entry
├── error.rs             # Error types
└── ui/
    ├── mod.rs
    ├── app_window.rs    # Main window, paned layout
    ├── editor_pane.rs   # GtkSourceView tabs
    ├── preview_pane.rs  # PDF display
    ├── error_panel.rs   # Error list
    └── file_tree.rs     # Project files
```

(Add project/, config/, bibliography/, git_sync/, lsp/ modules as needed)

### Async Pattern
- All file I/O, git ops, compilation use `async`/`await`
- GTK signal callbacks spawn `tokio::spawn(async { ... })` tasks
- Error handling: return `Result<T>` and convert to user dialogs

---

## Testing Strategy

- **No GUI tests** (too fragile)
- **Manual testing** on openSUSE Tumbleweed + KDE Plasma
- **Compile checks**: `cargo build --release` after each phase
- **Simple scenarios**:
  - Create project → open .typ file → edit → compile → preview
  - Click file tree → file opens in new tab
  - Modify file → 500ms debounce → auto-compile
  - Sync button → git push

---

## Estimated Effort

- **Phase 1** (Editor UI): 20-30 hours
- **Phase 2** (File I/O): 15-20 hours
- **Phase 3** (Event loop): 20-25 hours
- **Phase 4** (Autocomplete): 10-15 hours
- **Phase 5** (Git sync): 8-10 hours
- **Phase 6** (Settings): 5-10 hours
- **Phase 7** (Polish/CI): 10-15 hours

**Total**: ~100 hours of focused Rust + GTK4 development

---

## Success Criteria (MVP)

✅ Can create/open Typst project
✅ Can edit multi-file projects in tabs
✅ Can save to disk
✅ Can compile and preview PDF
✅ Can see compilation errors
✅ Can sync to GitHub
✅ Can use citation autocomplete

This is **production-ready for personal use** and ready to share with others.
