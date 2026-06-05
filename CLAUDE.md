# Zerkalo — Claude Instructions

## Version Management
- After any functional change, increment the patch version in `Cargo.toml` (e.g. 0.7.1 → 0.7.2)
- Update `CHANGELOG.md` with a brief entry for each version bump
- Commit message for releases should be just the version: `v0.7.2`

## Documentation
- Keep `README.md` in sync with any new features or changed CLI flags
- Update the help text in `src/ui/help_window.rs` when user-facing behavior changes

## Code Style
- No comments unless the WHY is non-obvious
- No multi-line docstrings or comment blocks
- No trailing summaries at the end of responses — the user can read the diff

## GTK/UI Layer
- New UI panels go in `src/ui/` as their own file, registered in `src/ui/mod.rs`
- UI state should live in the GTK widget/model, not in `config.rs` or `project_model.rs`

## Compiler / Typst
- The embedded Typst compiler (`src/compiler.rs`) runs in a Tokio async context — don't block the GTK main thread; use channels or `glib::spawn_future`
- Preview re-renders should be debounced, not triggered on every keystroke

## Config & Persistence
- User settings belong in `src/config.rs` with serde derive; don't scatter config fields across modules
- New config fields need a sensible default so existing user configs don't break on upgrade

## Git Sync
- Git operations go through `src/git_sync.rs` via `git2` — don't shell out to `git`

## Error Handling
- Use `thiserror` types in `src/error.rs`; don't use `unwrap()` or `expect()` in UI code paths
- Surface errors to the user via the error panel, not just logs

## Installation
- When adding a new binary dependency or system package requirement, update `install.sh`

---

## Phased Improvement Plan

This plan was defined in June 2026. After each phase, run `cargo check` and `cargo test --no-run` to confirm compilation. Commit with the phase version. Run `/clear` between phases — the plan lives here so it survives context resets.

### Phase 1 — v0.9.0 (CRITICAL) ✅
1. **System Check Wizard** (`src/ui/setup_wizard.rs`): detect Linux distro via `/etc/os-release`; per-distro install commands (apt/dnf/pacman/zypper) for pandoc, hunspell, git, tinymist; verify-after-install button per tool.
2. **Template Marker Recovery** (`src/ui/template_dialog.rs`, `src/ui/app_window.rs`): `repair_template_markers(path)` that backs up to `.typ.bak` and re-inserts the `// ── Document body` marker; warning comment above the marker in generated templates; "Repair Template Markers…" menu item in ☰.
3. **Compile-on-Save** (`src/config.rs`, `src/ui/app_window.rs`, `src/ui/settings_dialog.rs`, `src/file_watcher.rs`): add `compile_on_save = true` and `manual_compile_only = false` to Config; `notify` crate watches project dir for external `.typ` changes; when `compile_on_save` is set, on-keystroke debounce skips compilation (outline/LSP still update); compilation fires on Ctrl+S save.

### Phase 2 — v0.10.0 (HIGH)
4. **Find in Files** (Ctrl+Shift+F): project-wide search panel (`src/ui/search_panel.rs` — already exists, extend it); search all .typ files recursively, respecting .gitignore; results show file+line+content with match highlighted; click to jump; replace-in-files mode with preview; store last 10 searches in config.
5. **Interactive Preview Click-to-Jump**: Ctrl+Click on preview extracts text snippet via pdftotext, searches in open files, jumps to line; preview toolbar buttons "Copy Text from Preview" and "Jump to Editor"; graceful degradation if pdftotext missing.
6. **Export Progress Dialog**: replace silent failures with a modal progress dialog; real-time stderr for native typst PDF; pandoc output for DOCX/ODT/LaTeX; show exact errors; "Install Missing Dependencies" link to system check wizard; batch export option.

### Phase 3 — v0.11.0 (MEDIUM)
7. **Configurable Compilation Profiles**: `[profile.draft]` and `[profile.final]` in config.toml; draft = fast, no PDF output; final = full compile; toolbar dropdown to switch; pass `--input draft=true` in draft mode.
8. **Session Snapshots & Version Recovery**: on save, snapshot to `~/.local/share/zerkalo/snapshots/<project>/<timestamp>.typ`; keep last 50 per file; "Browse Snapshots" timeline view with diffs; restore from any snapshot; integrate with Git commits.
9. **Enhanced Spell Check** (`src/spellcheck.rs`): project-specific dictionary at `<work_dir>/.zerkalo/dictionary.dic`; global user dict at `~/.config/zerkalo/user.dic`; right-click → "Add to Project Dictionary"; hunspell .dic format.
10. **Inline Typst Error Assistant**: on hover over red-underlined code, show error message + "Fix It" button; known error patterns in `src/error_patterns.rs`; auto-apply fix for common patterns.

### Phase 4 — v0.12.0 (CLEANUP & POLISH)
11. **Keyboard Shortcut Remap**: Command Palette Ctrl+K; Git sync Ctrl+Shift+S; add Ctrl+Shift+H "Keyboard Shortcuts Help" that reads keybindings.toml dynamically.
12. **Compilation Time Display**: show "Compiled in Xs" in status bar; yellow warning for >3s with optimization tips; track stats in `~/.cache/zerkalo/compile_stats.json`.
13. **Auto-backup on Idle**: `auto_save_idle_ms = 30000` in config; auto-save on idle; skip files with compilation errors.
14. **Command Palette enhancements**: add "Find in Files…", "Project Outline", "Toggle Profile", "Browse Snapshots" commands.
