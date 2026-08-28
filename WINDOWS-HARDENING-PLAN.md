# Zerkalo Windows-Port Hardening Plan — 2026-08-25

**Created:** 2026-08-25 · **Baseline:** `main` at v0.26.4 "Clear Glyph" · **Status:** not started

Cal asked for a deep look at dependency issues, code duplication, and general
hardening ahead of the planned Windows port. Investigated via three parallel
background audits (Windows-portability, dependency health, code duplication),
each told to report concrete file:line findings, not generic advice — same
discipline as `HEALTH-PLAN.md` and `STABILITY-REVIEW-2026-08-18.md`, both of
which found that raw-grep-sourced claims about this codebase often don't
survive a close read. All three audits here did their own direct-code
verification, not just grep counts.

Read this file before starting any phase. Same verification gate as the other
plans:

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
./check-versions.sh
```

Manual smoke test after any phase touching UI: app opens, a document loads,
live preview compiles, tab switching works.

**Commit at the end of each phase, separately.** Never let two phases share a
commit.

---

## Phase 1 — Two live bugs found along the way (fix regardless of the Windows port)

**Status:** 1a/1b ☑ DONE (2026-08-25); 1c ☐ not started · **Risk:** low ·
**Effort:** small

1a, 1b, and 1c all done 2026-08-25, uncommitted (Cal's call per the repo's
normal flow). Verification: `cargo build`, `cargo clippy --all-targets -D
warnings`, `cargo test` (578 passed), `cargo build --release`, and
`./check-versions.sh` all green throughout.

**What landed for 1c, plus one more instance found while fixing it:**
wrapped `table_dialog.rs` and `snapshot_dialog.rs`'s headers in
`adw::ToolbarView`/`RaisedBorder`, matching every other secondary window.
While auditing which of the 22 `fond-chrome` files actually had the
matching `ToolbarView` (for the Phase 6a work below), found a **third**
instance of the identical bug: `github_signin.rs` also appended its header
straight into a plain `GtkBox` with no shadow separator. Fixed the same
way. All other `fond-chrome` sites checked and confirmed already correct
(`command_palette.rs`, `font_manager.rs`, `print_sheet.rs`,
`welcome_window.rs`, `ref_manager.rs`, `settings_dialog.rs`,
`docs_browser.rs`, `help_window.rs`, `tools_window.rs`,
`export_dialog.rs`); `setup_wizard.rs`'s `Flat` style (not `RaisedBorder`)
is deliberate, not a fourth instance.

**What landed for 1a:** added `entry.set_activates_default(true)` to the 4
dialogs that were missing it (confirmed by reading each of the 7 dialogs
directly, not just the earlier grep-sourced list).

**What landed for 1b:** `LibraryWindow` now holds a `Rc<RefCell<Config>>`
(`config.rs`'s `Config`, plumbed in via `LibraryWindow::new`'s new 4th
parameter, passed from `panels.rs`'s existing `current_config`), and
`export_doc_dialog` resolves `cv_elements_path`/`bib_path` from it — reading
the CV YAML into a `skrizhal-cv-data` sys-input and passing `bib_path` as
the compile's `extra_root`, mirroring exactly what
`PreviewPane::compile_inputs()` already does for the header's Ctrl+Shift+E
export. Confirmed via reading `menus.rs:212-229` that `bib_path`/
`cv_elements_path` are project-wide `Config` fields (not per-document), so
this is correct for every document in the library, not just the one
currently open — no further per-document resolution needed.

Found by the duplication audit while looking for copy-pasted dialog
boilerplate — both are genuine, user-visible bugs on Linux today, not
Windows-specific. Worth fixing first since they're small and unrelated to
everything else in this plan.

### 1a. Enter-to-submit silently doesn't work in 4 of 7 text-prompt dialogs

`library_window.rs` has ~7 near-identical `adw::MessageDialog` + `Entry`-as-
`extra_child` prompts. `entry.set_activates_default(true)` is present in 3 and
missing in 4:

- `rename_project_dialog` (`library_window.rs:1527`) — missing
- `rename_category_dialog` (`:1643`) — has it
- `add_subcategory_dialog` (`:1673`) — has it
- `rename_doc_dialog` (`:1876`) — missing
- `create_project_then_add` (`:2213`) — missing
- `create_project_dialog` (`:2292`) — missing
- `create_category_dialog` (`:2242`, has a color picker) — has it

So "Rename Project," "Rename Document," "New Project," and "New Project (from
doc)" all require reaching for the mouse to confirm, while their siblings
accept Enter. Fix: extract a shared `prompt_for_text(parent, title, ok_label,
initial_or_placeholder, on_confirm)` helper (matching the pattern
`confirm.rs` already uses for destructive/notice dialogs) and route all 7
call sites through it — fixes the 4 broken ones and prevents the 5th
copy-paste from missing it too.

### 1b. Exporting a CV/citation document from the Library window produces a broken PDF

Two independent "compile → PDF bytes → write" paths exist:

- `library_window.rs::export_doc_dialog` (`:2742`, background block at
  `:2783`) calls `compiler::compile_to_pdf_bytes(&src, &HashMap::new(),
  &HashMap::new(), None)` — empty overrides, empty sys_inputs, **no
  bib_path**.
- The header's Ctrl+Shift+E export (`app_window/mod.rs:2807`, background
  block at `:2824`) goes through `preview.compile_inputs()`, which correctly
  resolves CV-mode overrides/sys_inputs and the bib path.

`library_window.rs:2775-2781` already has a comment self-documenting the gap.
Exporting any CV-mode or citation-heavy document via the Library window
(rather than the header button, while that document is open) produces a PDF
missing `#cv-entry`/`#cv-section` resolution and citations. Fix: route the
Library export through the same resolution `compile_inputs()` uses, or
extract one shared `export_pdf_async(root, overrides, sys_inputs, bib_path,
dest, on_done)` both call.

### 1c. Two secondary windows are missing their header shadow

`table_dialog.rs:238` and `snapshot_dialog.rs:111-195` append their
`HeaderBar` directly into a plain `GtkBox` instead of wrapping it in
`adw::ToolbarView` with `set_top_bar_style(RaisedBorder)` — every other
secondary window in the app (Font Manager, Help, Docs Browser, Export,
Package Browser, etc.) has the raised-border separator; these two don't.
Cosmetic, but a visible inconsistency. Fix inline, or fold into the
`chrome_window()` helper described in Phase 6 if that's done at the same
time.

---

## Phase 2 — Dependency de-duplication and staleness

**Status:** ☑ DONE (2026-08-25) · **Risk:** low-medium · **Effort:** small-medium

All three sub-items done, uncommitted (Cal's call). Full gate green throughout
(`cargo build`, `cargo clippy --all-targets -D warnings`, `cargo test` — 578
passed, `cargo build --release`, `./check-versions.sh`, `cargo audit` — same
0 vulnerabilities/9 warnings as before, no regression).

**2a:** `biblatex` bumped `"0.11"` → `"0.12"` and `roxmltree` `"0.20"` →
`"0.21"` in `Cargo.toml` — both built with **zero source changes**.
`tiny-skia` direct dependency removed outright (confirmed zero
`tiny_skia::` references in `src/`; the `.encode_png()` call in
`compiler.rs` runs through the transitively-pulled instance via
typst-render→resvg/krilla, unaffected by removing Zerkalo's own unused
pin). **Correction to the original audit's framing:** `cargo tree
--duplicates` after the bump shows both crates *still* have two resolved
versions — but from different sources than diagnosed. `biblatex 0.11` now
comes from `fond-bib` (kartoteka)'s own direct pin (`biblatex = "0.11"`,
confirmed unchanged even at kartoteka's latest tag v0.7.0) — fixing this
fully means kartoteka bumping its own pin, out of scope for a Zerkalo-only
change. `roxmltree 0.20` now comes from `fontconfig-parser` (pulled via
`typst-kit`/`fontdb`), a transitive requirement with no path back to
Zerkalo's own `Cargo.toml` at all. Both bumps were still correct and worth
doing — Zerkalo's own requirement now matches what the rest of its tree
already needs (`hayagriva`'s `biblatex 0.12`, `typst-library`'s `roxmltree
0.21.1`) rather than adding a third, gratuitous version to the graph — but
full collapse of either duplicate isn't achievable from this repo alone.

**2b:** `tokio`'s features trimmed from `["full"]` to `["rt"]` — matches the
one actual call site (`ui/print.rs`'s `Builder::new_current_thread()`).
Built clean; `reqwest`/`ashpd`/`zbus` pull whatever tokio subfeatures they
need independently via feature unification, unaffected by Zerkalo's own
feature request shrinking.

**2c:** `skrizhal-core` bumped `v0.3.0` → `v0.4.0`; `fond-bib`/`fond-vault`
bumped `v0.5.1` → `v0.7.0`. Checked each upstream's own `CHANGELOG.md`/diff
locally (both are Cal's own repos, checked out at `~/Projects/skrizhal` and
`~/Projects/kartoteka`) before bumping rather than bumping blind:
- `fond-bib`/`fond-vault` v0.5.1→v0.7.0: diffed the two crates' `src/`
  between tags — purely additive public API (new `custom_field` module,
  `ParentRole`, book-part helpers in `fond-bib`; zero changes to
  `fond-vault`'s public surface at all). Built with **zero source changes**.
- `skrizhal-core` v0.3.0→v0.4.0: **one real breaking change**, expected and
  handled — `parse_str`/`load_file` now return `ParseOutcome` (`{ entries,
  failed, raw_failed, profiles }`) instead of a bare `Vec<CvEntry>`, since
  v0.4.0 added native `_profiles` support. Fixed the one call site
  (`cv_mode.rs::load_cv_entries`: `Ok(entries) => entries` →
  `Ok(outcome) => outcome.entries`). This also **retired the reason** for
  `cv_mode.rs`'s `strip_reserved_blocks` workaround — it existed
  specifically because v0.3.0 failed the whole parse on a `_profiles`
  block, and its own doc comment predicted this exact moment ("Newer
  skrizhal-core skips these keys itself, and then this simply has nothing
  left to do"). Confirmed via a test that had the same premise baked in
  (`unfiltered_input_still_breaks_the_parser`, asserted `.is_err()`) — it
  now fails as an assertion because parsing genuinely succeeds, exactly as
  its own comment said it would if the pin ever moved. Updated that test
  (now `unfiltered_input_now_parses_natively_since_the_skrizhal_core_0_4_0_bump`)
  to assert the new success case instead of deleting it, and updated
  `strip_reserved_blocks`'s doc comment to describe it as defense-in-depth
  now rather than load-bearing. Left the function itself in place (still
  correct, still a no-op pass-through on already-clean input) rather than
  removing it — that's a separate, larger cleanup than "bump the pin,"
  deliberately not done here.
- `packaging/cargo-sources.json` regenerated for the new `Cargo.lock`
  (`flatpak-cargo-generator.py`, per the root `Projects/CLAUDE.md` recipe).

`cargo audit` is currently clean (0 vulnerabilities, 9 pre-existing
`unmaintained`/`unsound`/`yanked` warnings, unchanged from
`STABILITY-REVIEW-2026-08-18.md`; the quick-xml DoS pair is suppressed via a
documented `.cargo/audit.toml` ignore, not fixed — still tracked there, no
action here). License check clean, nothing copyleft. This phase is about
compile-time/binary-size bloat and dependency staleness, not vulnerabilities.

### 2a. Collapse duplicate crate versions caused by Zerkalo's own stale pins

- **`biblatex` 0.11 vs 0.12.** `Cargo.toml` pins `"0.11"` (5 call sites in
  `src/`); `hayagriva 0.10` (already at its current version per the earlier
  bump) requires `biblatex 0.12`, so two full copies compile. Bump the pin to
  `"0.12"` and check the 5 call sites for API drift (0.11→0.12 is a 0.x
  major bump).
- **`roxmltree` 0.20 vs 0.21.** Same shape: `Cargo.toml` pins `"0.20"` (9
  uses), `typst-library`→`fontdb` pulls 0.21.1. Bump to `"0.21"`.
- **`tiny-skia` — likely a dead dependency entirely, not just a duplicate.**
  `Cargo.toml` pins `tiny-skia = { version = "0.11", features = ["png"] }`
  but grep found **zero** `tiny_skia::` references in `src/` (only a comment
  at `compiler.rs:534`). The actual `.encode_png()` call
  (`compiler.rs:566`) runs on a `Pixmap` from `typst_render::render()`, which
  resolves through the typst-render→resvg/krilla chain to a *different*,
  transitively-pulled tiny-skia **0.12.0** — Zerkalo's own 0.11 pin isn't
  even the crate instance doing the work. Try commenting out the direct
  dependency and building; if it still compiles, remove it — collapses the
  duplicate for free and removes an unused direct dependency.

### 2b. Trim `tokio`'s `"full"` feature

Only one call site in `src/`: `tokio::runtime::Builder::new_current_thread()`
in `ui/print.rs:274`, driving a synchronous ashpd print-portal call. No
`tokio::fs`/`process`/`net`/`signal`/`io`, no `#[tokio::main]`. `"full"`
pulls in the multi-thread scheduler and signal handling that nothing here
uses; `reqwest` and `ashpd`/`zbus` already pull whatever tokio subfeatures
*they* need independently via feature unification. Trim to `features =
["rt"]` (add `"macros"` only if `Builder` needs it — it currently doesn't)
and build to confirm nothing from reqwest/ashpd's own requirements breaks.

### 2c. Bump the two pinned git dependencies — both upstreams have moved on

Both are correctly `tag`-pinned already (not bare `git = "..."` as an
earlier note in `HEALTH-PLAN.md`/`STABILITY-REVIEW` assumed — confirmed
current `Cargo.toml` state directly), but both tags are now stale:

- `skrizhal-core` pinned at `v0.3.0`; skrizhal's latest tag is `v0.4.0`
  (`v0.3.1` also skipped).
- `fond-bib`/`fond-vault` pinned at `v0.5.1`; kartoteka's latest tag is
  `v0.7.0` (`v0.6.0`, `v0.6.1` also skipped).

Scope each bump the same way the earlier `rusqlite`/`hayagriva` bump was
scoped (per `STABILITY-REVIEW-2026-08-18.md`) — check each project's own
`CHANGELOG.md` for breaking changes first, bump in an isolated worktree,
regenerate `packaging/cargo-sources.json`, run the CV/citation-style compile
tests specifically since both crates feed that path.

---

## Phase 3 — Windows path/config-resolution hardening (do this before any Windows build attempt)

**Status:** ☐ not started · **Risk:** medium · **Effort:** medium

These are silent-corruption/wrong-location bugs, not compile failures — the
app would build and appear to run on Windows with these unfixed, then
scatter data or fail features in ways that are hard to diagnose later. Fix
before Phase 4's platform-gated feature work, since several of Phase 4's
`#[cfg(windows)]` branches will want to call the consolidated path helper
this phase creates.

### 3a. Three inconsistent config/data-path resolution strategies, one with a broken Windows fallback

- `config.rs:237,489` hand-rolls `shellexpand::tilde("~/.config/zerkalo")` /
  `("~/Documents/Zerkalo")`.
- `fonts.rs:6`, `main.rs:58`, `font_manager.rs:250`, and 6 more call sites
  use `glib::user_data_dir()`/`user_config_dir()` directly instead of a
  shared helper (also flagged by the duplication audit as its own,
  lower-priority finding).
- `session.rs:34-37` does neither — reads `$HOME` directly and falls back to
  a **bare `"/tmp"` string**:
  ```rust
  fn session_path() -> PathBuf {
      let base = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
      PathBuf::from(base).join(".local/share/zerkalo/session.json")
  }
  ```
  `HOME` is normally unset on Windows, so this resolves to
  `/tmp/.local/share/zerkalo/session.json`, which Rust treats as
  drive-relative (not absolute — `Path::is_absolute()` is false), landing
  wherever the current drive's root is relative to the process's CWD at
  launch.
- `preview_pane.rs:314` — the default live-preview output directory (used
  for every project without a custom `output_dir` — the common case) is a
  hardcoded `PathBuf::from("/tmp/zerkalo_preview")`. Same drive-relative
  problem, and it's on the core preview-render path, not an edge case.

**Fix:** add one canonical `zerkalo_config_dir()`/`zerkalo_data_dir()`/
`zerkalo_cache_dir()` in `config.rs` built on `glib::user_*_dir()` (already
cross-platform via GLib on Windows), route all of the above through it
including `session.rs` and `preview_pane.rs`'s default output dir, and
delete the `$HOME`/`/tmp` fallback entirely.

### 3b. Absolute-path detection assumes a leading `/` everywhere

Three independent call sites treat `starts_with('/')` as "is this an
absolute filesystem path," which is false for any Windows path
(`C:\...`/`C:/...`):

- `git_sync.rs:194-195` — `is_local_path()`, used by `add_backup_remote`/
  `add_named_remote` (`:164`) to decide whether to auto-`git init --bare` a
  typed-in local folder. A Windows path silently skips the convenience
  auto-init.
- `compiler.rs:231-260` — decides whether a `#bibliography("...")` path is an
  external filesystem path that needs the compiler sandbox's root to widen
  (`:262-265`). Never triggers for a Windows absolute path, so an
  out-of-project `.bib` file may fail to resolve at compile time on Windows
  even though the equivalent Linux path works. (The separate fallback at
  `:252`, `if p.starts_with('/') { .. } else { project_root.join(p) }`,
  happens to still work by luck since `PathBuf::join` replaces the base when
  the argument is itself absolute — the sandbox-widening check doesn't have
  that luck.)

Fix both with `Path::is_absolute()` (which is platform-aware) instead of a
manual `/` check.

### 3c. Case-sensitive document identity in the library DB

`library.rs:343,371` matches a document's SQLite row by exact string
(`WHERE path = ?1` against `path.to_string_lossy()`), no canonicalization.
Windows filesystems are normally case-insensitive, so the same file reached
via two differently-cased paths (a file dialog vs. a recent-files entry vs.
a drive-letter case mismatch) would `upsert` as a second, duplicate library
row instead of being recognized as the same document. Fix: canonicalize (or
at minimum lowercase-normalize on `cfg(windows)`) before the DB lookup/write.
Related, lower-priority: `library.rs:1208`'s `.typ` extension check is also
case-sensitive.

---

## Phase 4 — Platform-gated feature work (the actual "doesn't work on Windows" list)

**Status:** ☐ not started · **Risk:** medium-high · **Effort:** large ·
**Depends on:** Phase 3 (shared path helper)

This is the real port work — each item needs a `#[cfg(target_os = "linux")]`
/ `#[cfg(windows)]` split with a working Windows-side implementation, not
just a guard that disables the feature. Ordered by how core the feature is.

### 4a. Printing — `ashpd` is Linux-portal-only; one line doesn't even compile on Windows

`ui/print.rs:434`, inside `print_via_portal()`: `use std::os::fd::AsFd;` —
Unix/WASI-only, fails to compile on `x86_64-pc-windows-*` outright. The
whole feature is built on `ashpd` (freedesktop D-Bus portals — no Windows
equivalent exists at all), so this isn't a one-line fix: the file already
has a GTK4 `PrintOperation`-based fallback path at the top for the
non-portal case — investigate promoting that to the *primary* path on
Windows (`#[cfg(not(target_os = "linux"))]`) rather than writing a new print
path from scratch.

### 4b. Font enumeration shells out to Linux-only `fontconfig` tools, degrades to empty silently

- `font_manager.rs:231-247` — `list_system_fonts()` shells out to `fc-list`;
  `.ok()` swallows the spawn failure, so the Font Manager UI shows **zero
  fonts** on Windows with no error surfaced. Fix: use Pango's own
  font-family listing (already available via GTK, used elsewhere in the
  app) as the cross-platform path.
- `fonts.rs:12-16` — bundled GOST font registration calls `fc-cache`; same
  `.ok()`-swallowed failure. The font file gets written to disk but never
  picked up by discovery on Windows. Needs a Windows-side registration path
  (or confirm Typst/Pango's own font scanning picks up the bundled file
  without an explicit cache-refresh step on Windows).

### 4c. `tinymist` (LSP) bundled-binary lookup hardcodes flatpak/deb paths

`lsp.rs:44` / `app_window/lifecycle.rs:95-109` — `tinymist_command()` only
checks `/app/lib/zerkalo/tinymist` and `/usr/lib/zerkalo/tinymist`, falling
back to bare `Command::new("tinymist")` on PATH. Add a "next to
`current_exe()`" fallback (the natural Windows bundling location, e.g.
`tinymist.exe` shipped alongside `zerkalo.exe`) so LSP completions don't
silently disable on Windows.

### 4d. `keyring` has no Windows backend enabled

`Cargo.toml` enables only `features = ["sync-secret-service"]` (Linux Secret
Service). `keyring 3.6.3` gates Windows Credential Manager behind a separate
`windows-native` feature (`dep:windows-sys` + `dep:byteorder`) that isn't
turned on anywhere. Without it, GitHub sign-in token save/load/delete
(`secret_store.rs`, used from `ui/github_signin.rs`) has no working backend
on Windows at all. Fix: target-gate the feature —
```toml
[target.'cfg(windows)'.dependencies]
keyring = { version = "3", features = ["windows-native"] }
```
alongside the existing Linux-targeted `sync-secret-service` feature.

### 4e. Bundled `git` has no Windows story

`git_sync.rs:29-31` — `bundled_git()` hardcodes `/app/bin/git` and
`/usr/lib/zerkalo/bin/git` (flatpak/deb bundling), falling through to bare
`git` on PATH otherwise — which works on Windows today (`CreateProcess`
appends `.exe`) *if* Git for Windows is separately installed, but there's
currently no "nothing to install" bundled-git story for Windows the way the
Linux packages have. Lower urgency than 4a-4d (has a working fallback), but
worth deciding during the port whether to bundle `git.exe` or document it as
a prerequisite.

### 4f. Lower severity — external CLI tools with Linux-flavored error text

`spellcheck.rs:431` (hunspell) and `preview_pane.rs:1087,1174` /
`app_window/import.rs:2585` (pdftotext) already degrade gracefully (feature
disables, doesn't crash) when the binary is missing — but the surfaced error
text (`"pdftotext was not found. Install poppler-utils..."`,
`import.rs:2623`) names Linux package managers. Update the message to be
platform-appropriate, or point at a bundled/Windows-installer path once
Phase 4's bundling story is decided.

---

## Phase 5 — Test suite: unguarded Unix-only code (breaks `cargo test` on Windows, not just a runtime gap)

**Status:** ☑ DONE (2026-08-28) · **Risk:** low · **Effort:** trivial ·
**Depends on:** nothing, can land any time, cheap to do early

All four functions below got `#[cfg(unix)]` added, matching the pattern
already used by their gated siblings (`library.rs:1452,1712`). No other
changes. Verification gate green: 584 tests passed (up from 578 at this
plan's baseline — the extra 6 are from other work since 2026-08-25, not
this phase), clippy clean, release build clean, version guard clean.
Windows-side equivalent coverage (permission denial via ACLs,
`std::os::windows::fs::symlink_file`/`symlink_dir`) is still a separate
follow-up, not required to unblock compilation — not done here, matching
the phase's own scope.

Most Unix-only test helpers are correctly `#[cfg(unix)]`-gated already (e.g.
`library.rs:1452,1712`), but four are not, and will fail to compile a
`cargo test` run on a Windows dev machine or Windows CI runner, not just
silently skip:

- `error.rs:97` — `atomic_write_preserves_permissions()`, uses
  `std::os::unix::fs::PermissionsExt`, no `#[cfg(unix)]`.
- `auto_save.rs:253` — `save_reports_failure_instead_of_lying_when_the_write_fails()`,
  same pattern, no `#[cfg(unix)]`.
- `library.rs:1745` — `permanently_delete_keeps_the_row_if_the_file_cannot_be_removed()`,
  uses `PermissionsExt`, unguarded (its two siblings at `:1455`/`:1715` *are*
  gated — this one was missed).
- `project_model.rs:236` — `scan_through_a_symlinked_root_still_detects_the_correct_compile_root()`,
  `std::os::unix::fs::symlink(...)`, no `#[cfg(unix)]` at all.

Fix: add `#[cfg(unix)]` to all four, matching the existing pattern. If test
coverage for the underlying behavior matters on Windows too (permission
denial, symlink roots), that's a separate follow-up using
`std::os::windows::fs::symlink_file`/`symlink_dir` and Windows ACL-based
permission denial — not required to unblock `cargo test`, just to restore
equivalent coverage.

---

## Phase 6 — Duplication cleanup (lower priority, do opportunistically)

**Status:** ☑ DONE (2026-08-25) — scoped and landed the parts with real,
faithfully-preservable value; deliberately left the genuinely-divergent
sites hand-rolled rather than forcing a one-size-fits-all abstraction. See
below for the scoping call on each sub-item. Full gate green throughout.

**Landed — shared `crate::ui::async_poll::poll_result<T>` helper**
(`src/ui/async_poll.rs`, new file): covers the `thread::spawn` +
`mpsc::sync_channel::<Result<T, String>>` + `timeout_add_local` shape
*where the Disconnected case is silently ignored* (no callback, no UI
change) — confirmed that's what every migrated site already did before
touching them. Migrated 4 of the ~15 candidate sites onto it:
`library_window.rs`'s PDF export (also touched by Phase 1b — same
function), `app_window/mod.rs`'s Ctrl+Shift+E export, and both
`package_browser.rs` sites (Universe index refresh, package install).
**Deliberately not migrated:**
- `template_dialog/mod.rs`'s preview job (`:1961`) — its `Disconnected` arm
  actually does something (stops the spinner) unlike every migrated site's
  silent-break, so forcing it through the shared helper would either lose
  that behavior or require widening the helper's signature for one caller.
  Left hand-rolled rather than either.
- `print.rs` (×2), `export_dialog.rs`, `search_panel.rs` (×2),
  `app_window/mod.rs:127` — genuinely different shapes on inspection, not
  copies of the same thing: a custom `PortalOutcome` enum, a multi-message
  `ExportMsg` channel that doesn't break on the first message, a plain
  `Vec<PathBuf>`/`Library` value with no `Result` wrapper at all. Forcing
  these through a `Result<T, String>`-shaped helper would be a worse
  abstraction than what's there, not a better one — left as they were.
- The 3 hunspell-suggestion poll loops in `editor_pane.rs` — see below,
  handled via a different, narrower helper instead since only part of each
  site is actually shared.

**Landed — `EditorPane::spawn_spelling_suggestions`** (new private method,
`editor_pane.rs`): the 3 hunspell-suggestion lookups (`:7150`, `:7893`,
`:8188` after the day's other edits) had a fully-identical spawn+lookup
body (confirmed byte-for-byte identical between the first two; the third
lacked the `already_ignored` short-circuit, now an explicit `false`
argument at that call site) wrapped in 3 different polling loops that
legitimately differ (two are popover-visibility-gated with different UI to
fill, one is autocorrect with no popover at all) — exactly the audit's own
assessment. Extracted only the shared spawn+lookup part (returns the
`Rc`-wrapped receiver), left each site's own poll loop as it was.

**Landed — `config::zerkalo_data_dir`/`zerkalo_config_dir`/`zerkalo_cache_dir`**:
11 call sites across 8 files (`main.rs`, `fonts.rs`, `user_templates.rs`,
`library.rs` ×2, `error_panel.rs`, `welcome_window.rs` ×3, `editor_pane.rs`,
`font_manager.rs`, `typst_universe.rs`) that independently joined
`"zerkalo"` onto `glib::user_data_dir()`/`user_config_dir()`/
`user_cache_dir()` now go through one of three canonical functions in
`config.rs`. (Originally scoped as depending on Phase 3a — turned out not
to: these call sites were all already using the GLib-based path resolution
that Phase 3a's own writeup calls out as the *correct* strategy, so
consolidating them doesn't touch the actually-broken `session.rs`/
`preview_pane.rs`/`shellexpand` paths at all. Did this independently of
Phase 3a rather than waiting.)

**Deliberately not built — a generic `chrome_window()` window-chrome
helper.** Surveyed all 22 `fond-chrome` files (see Phase 1c above): 3 had
the actual bug (now fixed directly) and the other ~13-19 secondary-dialog
sites already correctly implement the same 4-line
`ToolbarView`/`RaisedBorder`/`add_top_bar`/`set_content` pattern by hand.
The bug-prevention value a shared helper would add is already realized by
fixing the 3 actual instances directly; retrofitting ~13 already-correct,
independently-authored files (some with header-only variants, some with
`add_top_bar`, subtly different construction order) onto a new shared
helper is mechanical-refactor risk with no corresponding new value —
skipped as not worth it, consistent with this plan's own "no urgency, pick
up piecemeal" framing for Phase 6 rather than forcing every item through.

---

## How to resume this plan after a context reset

1. Read this file top to bottom.
2. Find the first `☐ not started` phase with satisfied dependencies.
3. Phase order reflects dependency, not just priority — Phase 3 (paths)
   should land before Phase 4 (platform-gated features) since several of
   Phase 4's Windows branches want the Phase 3a path helper. Phases 1, 2,
   and 5 have no dependencies and can be done in any order, including before
   or interleaved with 3/4.
4. Update the status box to `☐ in progress` before starting, `☑ DONE (date)`
   when the verification gate passes and the commit lands.
