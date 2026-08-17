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
3. Update What's New in `src/ui/welcome_window.rs` to reflect current features
3a. **If `Cargo.lock` changed** (any dependency added, removed or updated), regenerate `packaging/cargo-sources.json` and commit it with the bump. The flatpak's `zerkalo-deps` module builds with `CARGO_NET_OFFLINE: 'true'` against that vendored manifest, so without it the build fails at the dependency fetch. See the Zerkalo bullet under **Version files by project** in the root `Projects/CLAUDE.md` for the exact commands.
4. Push to GitHub (needed so the source is current for collaborators and CI)
5. Run `flatpak-builder --force-clean --user --install build-flatpak packaging/io.github.calstfrancis.Zerkalo.yml`
- **Do not** add a `metainfo.xml` `<release>` entry for dev builds — only at actual release time (see root `Projects/CLAUDE.md`'s Release workflow), matching Rubric/Gost/Kopilka/Skrizhal. This used to be a Zerkalo-only exception and caused a real bug: AppStream's version comparison treats `0.16.1-dev6` as *higher* than `0.16.1` (it has no concept of pre-release ordering, unlike semver — confirmed via `appstreamcli compare-versions`), so `flatpak info` displayed the wrong "Version" for the app any time a same-base-version dev entry existed alongside the clean release entry. Reordering entries in the file doesn't fix this — the comparison, not document order, decides. Fixed by removing the interim `0.16.1-devN` entries and stopping future ones.

### Flatpak build
- The flatpak manifest (`packaging/io.github.calstfrancis.Zerkalo.yml`) sources the `zerkalo` module with `type: git`, `branch: main` — permanently, for both dev builds and releases. Matches Rubric/Kopilka's manifests exactly; no tag-pinning, no switch-back step. `publish-flatpak.sh` pushes `main` right before running `flatpak-builder`, so the build always picks up the latest commit regardless of dev/release.
- (History: this used to toggle between `type: dir` and a release-tag-pinned `type: git` around the release step, requiring a manual switch-back afterward — that got missed once after v0.15.0, silently rebuilding the stale release for a while. Simplified to the permanent-branch pattern the other apps already use, removing the failure mode entirely instead of just remembering to revert it.)
- `skrizhal-core`'s git dependency in `Cargo.toml` requires the `calstfrancis/skrizhal` GitHub repo to stay **public** — CI (and this flatpak build) can't authenticate to a private repo to fetch it. If it's ever made private again, CI will fail at the dependency-fetch step with "failed to authenticate when downloading repository."

### GitHub Releases — dev tags must not create them
- `.github/workflows/release.yml` triggers only on `v[0-9]+.[0-9]+.[0-9]+` (matching Rubric/Gost), **not** a bare `v*`. A loose `v*` pattern makes every pushed dev tag (`v0.16.1-dev1`, `v0.16.1-dev2`, …) create a public GitHub Release — this actually happened for months before being caught (dozens of dev-build "releases" publicly listed on the repo's Releases page). Keep the restrictive pattern; don't loosen it back to `v*`.

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

## Active plans

Several standing plan docs track multi-session work — read the relevant one
before starting anything that overlaps it, and update its status boxes as you
go so the plan (not the conversation) is the source of truth across `/clear`s.

**As of 2026-08-17, every plan below is fully closed out** — no open phases,
no unstarted items. Still worth reading before new codebase-health/UX/
refactor work starts, both to avoid redoing something already investigated
and closed, and because the next such initiative should probably become a
new dated plan rather than reopening one of these:

- **`HEALTH-PLAN.md`** — 10-phase plan from the 2026-08-12 codebase review
  (fragile-core files, unwrap audits, dependency pinning, a11y, dead code).
  All 10 phases done.
- **`UX-AUDIT-PLAN.md`** — 32-item plan from the 2026-08-17 Word-migrant
  usability audit (onboarding jargon, destructive-action confirmations,
  Typst-complexity leakage, help/discoverability gaps). All 32 items done.
- **`REFACTOR-PLAN.md`** — file-splitting + `library.rs`/`spellcheck.rs` test
  coverage. All phases done or explicitly, deliberately deferred (see the
  plan's own notes on why).
- **`PRINT-PLAN.md`**, **`zerk-polish.md`** — feature-specific plans, both
  fully done; see each file's own header.

Two older plans were archived to `docs/archive/` on 2026-08-17 since their
own merge/completion criteria were already satisfied (not deleted — kept for
history):
- **`docs/archive/zerkalo-todo.md`** — multi-file project feature plan;
  items 1–6 done and merged into `main` back at v0.12.18, items 7–8 were
  explicitly optional "nice-to-have" stretch goals never required by the
  plan's own merge criteria.
- **`docs/archive/ROADMAP.md`** — pre-`CHANGELOG.md` release history,
  superseded by `CHANGELOG.md` and stale by 17 versions at archive time
  (still headed "Current release: 0.7.1" against an actual `main` at
  v0.24.0-dev3).
