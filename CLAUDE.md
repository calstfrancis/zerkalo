# Zerkalo — Claude Instructions

## Version Management

### Release candidate numbering
- Builds get the **next** release version number with an `-rcN` suffix: `0.12.33-rc1`, `0.12.33-rc2`, …
- When Cal says "release", strip the `-rcN` suffix → `0.12.33`; push, tag, and publish flatpak
- After a release, the next build starts at `<next>-rc1` again (e.g. `0.12.34-rc1`)
- Never bump to a plain release version during a build — only on explicit release instruction

### On every build
1. Update `Cargo.toml` version to the next rc number
2. Update `CHANGELOG.md` — add entry at top for the new rc version
3. Update `packaging/io.github.calstfrancis.Zerkalo.metainfo.xml` — add release entry
4. Update What's New in `src/ui/welcome_window.rs` to reflect current features
5. Push to GitHub (needed so the source is current for collaborators and CI)
6. Run `flatpak-builder --force-clean --user --install build-flatpak packaging/io.github.calstfrancis.Zerkalo.yml`

### Flatpak build
- The flatpak manifest sources from the **local directory** (`type: dir`) — no push needed before building
- Still push to GitHub as part of the build flow so the repo stays current
- On release: switch manifest source back to `type: git` for the published build, then switch back to `type: dir` after

### Commit message
- Builds: `v0.12.33-rc1 — short description`
- Releases: `v0.12.33 — short description`

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

### Phase 4 — v0.12.0 (CLEANUP & POLISH)
11. **Keyboard Shortcut Remap**: Command Palette Ctrl+K; Git sync Ctrl+Shift+S; add Ctrl+Shift+H "Keyboard Shortcuts Help" that reads keybindings.toml dynamically.
12. **Compilation Time Display**: show "Compiled in Xs" in status bar; yellow warning for >3s with optimization tips; track stats in `~/.cache/zerkalo/compile_stats.json`.
13. **Auto-backup on Idle**: `auto_save_idle_ms = 30000` in config; auto-save on idle; skip files with compilation errors.
14. **Command Palette enhancements**: add "Find in Files…", "Project Outline", "Toggle Profile", "Browse Snapshots" commands.
