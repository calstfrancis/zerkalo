# Zerkalo — Claude Instructions

## Build and release rules

- **Never build or release unless Cal explicitly says to.** Code changes alone do not trigger a build.
- Saying "build a dev" / "prep a dev build" triggers a build. Saying "release" triggers a release. Nothing else does.

## Version Management

### Dev build numbering
- Builds get the **next** release version number with a `-devN` suffix: `0.12.33-dev1`, `0.12.33-dev2`, …
- When Cal says "release", strip the `-devN` suffix → `0.12.33`; push, tag, and publish flatpak
- After a release, the next build starts at `<next>-dev1` again (e.g. `0.12.34-dev1`)
- Never bump to a plain release version during a build — only on explicit release instruction

### On every build
1. Update `Cargo.toml` version to the next dev number
2. Update `CHANGELOG.md` — add entry at top for the new rc version
3. Update `packaging/io.github.calstfrancis.Zerkalo.metainfo.xml` — add release entry
4. Update What's New in `src/ui/welcome_window.rs` to reflect current features
5. Push to GitHub (needed so the source is current for collaborators and CI)
6. Run `flatpak-builder --force-clean --user --install build-flatpak packaging/io.github.calstfrancis.Zerkalo.yml`

### Flatpak build
- The flatpak manifest (`packaging/io.github.calstfrancis.Zerkalo.yml`) should normally sit at `type: dir`, `path: ..` for the `zerkalo` module source — this builds from local/pushed source, no pinned tag.
- Still push to GitHub as part of the build flow so the repo stays current.
- **On release, this is a two-part step — do not do the first half without the second:**
  1. Before running `publish-flatpak.sh`: switch the `zerkalo` module's source to `type: git`, `tag: vX.Y.Z` (the release tag just pushed).
  2. **Immediately after the release build/publish finishes, switch it back to `type: dir`, `path: ..` and commit that.** Forgetting this step is silent and easy to miss: the manifest keeps pointing at the old release tag, so every subsequent dev build quietly rebuilds the stale release instead of current source (this happened after the v0.15.0 release — dev builds kept installing 0.15.0 for a while before anyone noticed). Before ending a release session, run `git diff packaging/io.github.calstfrancis.Zerkalo.yml` and confirm it shows `type: dir`, not `type: git`.

### Release names
- Every release gets a name. Choose a two-word name: an adjective + a noun (e.g. "Amber Tide", "Silent Forge", "Iron Coast"). Pick something that evokes the theme of the main changes in the release, or just something that sounds good. Avoid clichés.
- Include the name in the CHANGELOG heading: `## [0.13.8] "Amber Tide" — short description`
- Include the name in the metainfo `<release>` description: `<p>0.13.8 "Amber Tide" — short description</p>`
- Include the name in the commit message: `v0.13.8 "Amber Tide" — short description`
- Update `RELEASE_NAME` constant in `src/ui/welcome_window.rs` so the name appears in the version subtitle and "What's New" heading

### Commit message
- Builds: `v0.12.33-dev1 — short description`
- Releases: `v0.12.33 "Release Name" — short description`

## Documentation
- Keep `README.md` in sync with any new features or changed CLI flags
- Update the help text in `src/ui/help_window.rs` when user-facing behavior changes
- With each release, update `packaging/io.github.calstfrancis.Zerkalo.metainfo.xml` — the `<description>` block must reflect the current feature set accurately (not just the latest release notes). Review the full feature list and fix any that are stale or missing.

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
