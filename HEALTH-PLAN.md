# Zerkalo Health Plan — top 10 from the 2026-08-12 codebase review

**Created:** 2026-08-12 · **Baseline:** current `main` (post v0.21.0 line) · **Status:** not started

This file tracks progress on the findings from the 2026-08-12 deep review. **Read
it before starting any phase**, and update the status boxes as phases land. Same
role `REFACTOR-PLAN.md` plays for the file-splitting work and `zerk-polish.md`
plays for the polish work — this plan is a peer of those, not a replacement.

Run `/clear` between phases if the session is getting long — this file is the
memory, not the conversation.

---

## Why this exists

A deep review (git history, file sizes, TODO/grep sweep, CI config, CHANGELOG
pattern-matching) turned up 20 candidate findings. The 10 below were selected for
impact. Full raw findings are not reproduced here — see git history of this
session's summary if the missing 10 (i18n, `cargo fmt --check`, packaging-manifest
drift, `check-fond-style.sh` weakness, etc.) become relevant later.

---

## Verification gate — run after EVERY phase

Non-negotiable, same gate as `REFACTOR-PLAN.md`:

```sh
cargo test                    # must not regress the current passing count
cargo clippy --all-targets -- -D warnings   # must stay clean
cargo build --release         # must succeed
./check-versions.sh           # version-consistency guard
```

Manual smoke test after any phase touching UI: app opens, a document loads, live
preview compiles, tab switching works, Ctrl+K palette opens.

**Commit at the end of each phase, separately.** Never let two phases share a
commit.

---

## Phase 1 — Fix `CLAUDE.md`'s stale Phased Improvement Plan

**Status:** ☑ DONE (2026-08-12)
**Risk:** none · **Effort:** trivial · **Depends on:** nothing

`CLAUDE.md` lines 78–87 hold a June-2026 "Phase 4" plan whose four items
(Keyboard Shortcut Remap, Compilation Time Display, Auto-backup on Idle, Command
Palette enhancements) are all already shipped — confirmed via `config.rs`,
`command_palette.rs`, `compile_stats.rs`. Left in place, it risks a future
session redoing shipped work or confusing its "Phase 4" with this plan's phases.

**Fix:** delete the stale section from `CLAUDE.md`, replace with a one-line
pointer to this file (`HEALTH-PLAN.md`) and to `REFACTOR-PLAN.md`, so `CLAUDE.md`
stays the index rather than accumulating dead plans in place.

Doing this first so the next 9 phases don't add to the same clutter.

---

## Phase 2 — Wire in or delete `HistoryPanel`

**Status:** ☑ DONE (2026-08-12) — wired in, not deleted (Cal's call).

Turned out `history_panel.rs` wasn't just unwired — it was never registered in
`src/ui/mod.rs` at all, so it had never actually been compiled or linted as
part of the binary. Wiring it in surfaced 3 pre-existing `clippy::ptr_arg`
violations (`&PathBuf` params that should've been `&Path`) that had never been
caught because the file was dead code from clippy's perspective too.

What landed:
- `mod history_panel;` registered in `src/ui/mod.rs`.
- "File History…" row added to the hamburger menu (save/version group, right
  after "Browse Snapshots…") and to the Ctrl+K command palette
  (`browse_history`), both opening a small `adw::Window` wrapping
  `HistoryPanel`'s existing widget — mirrors `SnapshotDialog`'s structure
  closely, sharing the click-handler logic via a new
  `app_window::show_file_history_window` helper used by both entry points.
  Sensitivity-gated the same way as Browse Snapshots (insensitive with no
  document open).
- Removed the now-stale `#[allow(dead_code)]` on `theme::DiffColors::hunk_fg`
  (was only unused because `HistoryPanel` wasn't reachable).
- Added 3 unit tests for `git_log_for_file`/`git_diff_for_commit` against a
  real temp git repo — this file had zero tests before. Did **not** add a
  widget-construction test: no other file in the codebase calls `gtk4::init()`
  in tests, and CI has no display, so that would've been a new (and likely
  CI-breaking) precedent rather than following one.

**Verification:** full gate green (484 tests, up from 481; clippy clean;
version guard clean). Additionally ran the actual compiled binary headlessly
(Xvfb + isolated XDG/HOME + `dbus-run-session`, per the root CLAUDE.md's
documented recipe) against a demo project with real git history — app starts,
loads the library DB, opens the document, and runs stably with the new code
paths compiled in. **Could not get scripted UI interaction (Ctrl+K → type →
Enter) to register** in this no-window-manager Xvfb setup — tried
`xdotool key --window` (blocked by GDK's synthetic-XSendEvent guard) and
`XTestFakeKeyEvent` after explicit `windowfocus` (silently didn't land either).
This matches `capture-screenshots.sh`'s own documented conclusion that
synthetic input isn't reliable without a WM in this environment — not a new
problem, the existing screenshot scripts avoid interactive automation for the
same reason. **Cal: worth a 10-second manual click on "File History…" the next
time you're in the app**, since the actual menu-click → dialog-open path is the
one thing untested by the above (the widget-construction code itself is a
close structural copy of the already-shipped, production-used
`SnapshotDialog`, so risk there is low).

Deferred to Phase 8 (already tracked there): `git_log --follow` still runs
synchronously on the main thread. Low urgency now that it's reachable but only
via an explicit user action, not on every keystroke.

---

## Phase 3 — Audit `library.rs` non-test `.unwrap()`s

**Status:** ☑ DONE (2026-08-12) — investigated, no action needed. The original
review's "24 non-test unwraps" finding did not hold up.

Actual count of non-test code (lines 1–1134; everything from `#[cfg(test)]` at
1136 to EOF is two test modules, `tests` and `sql_shape`): **zero**
`.unwrap()`s, **one** `.expect()` — `Connection::open_in_memory().expect("in-memory
DB")` at line 198, inside `in_memory_with_trash_dir`.

Traced its only two callers (`grep -rn "open_in_memory" src/`, both in
`ui/app_window/mod.rs`): the placeholder DB the window opens with immediately
on startup (line 118), and the fallback when the real on-disk `Library::open()`
fails (line 128, already routed through `.unwrap_or_else` with a
`tracing::warn!` and no panic). So the actual risky path — opening the
real on-disk SQLite file, which genuinely can fail from corruption/locking/
permissions — is already handled correctly with a graceful fallback. The one
`.expect()` that exists is on an **in-memory** SQLite connection, which does not
fail under any realistic condition, used only as a placeholder/fallback that's
never itself exposed to a bad file on disk.

`Library::open()` (the real disk path, line 182) returns `SqlResult<Self>` and
uses `?` throughout — no unwraps there either.

Conclusion: the DB/CRUD layer does not have the unwrap-panic problem the
original review described. No code change made — adding error handling here
would be validating a scenario that can't happen, which the project's own
conventions say not to do. Worth noting for calibration: this is the one
finding out of the ten that didn't survive verification: the review claimed a
grep count without checking whether the matches were in test code, which
`library.rs`'s two large test modules apparently threw off.

---

## Phase 4 — Pin `fond-bib`/`fond-vault` git dependencies

**Status:** ☑ REVERTED (2026-08-12) — the `rev`-based pin broke the flatpak
offline build; reverted to unpinned, matching what was already shipping.

Originally pinned via `rev` (Kartoteka has no release tags yet; only
`v0.1.0-devN` tags, none matching the locked commit at the time). That broke
`./dev-build.sh`: flatpak-builder's offline cargo build failed trying to
reach the network for the exact `?rev=...` source URL, even though
`packaging/cargo-sources.json` was regenerated to match (its
`[source."https://github.com/calstfrancis/kartoteka"]` table gained a
matching `rev = "..."` field) — cargo's source-replacement matching did not
treat that as equivalent to the `?rev=`-qualified URL cargo actually resolved
in `Cargo.lock`. `skrizhal-core`'s `tag = "v0.3.0"` pin, by contrast, already
works in this same pipeline — so this project's vendoring setup reliably
matches `tag`-based git sources but not (at least not the way it was tried
here) `rev`-based ones.

**Reverted** `fond-bib`/`fond-vault` back to bare `git = "..."` (no rev),
which re-resolved to Kartoteka's then-current HEAD (a newer commit than the
one originally pinned) and regenerated `cargo-sources.json` to match — this
is exactly the original unpinned/reproducibility-risk state the phase set
out to fix, now knowingly left in place because the fix broke a working
build. **Not re-attempted this session.** If this is worth revisiting: try
pinning via `tag` once Kartoteka cuts a real release tag (matching
`skrizhal-core`'s working pattern), rather than `rev` again.

Required bootstrapping `flatpak-cargo-generator.py` (fetched from the public
flatpak/flatpak-builder-tools repo — the usual `~/Projects/kartoteka/` copy
wasn't present on this machine) plus `pip`/deps via `get-pip.py
--break-system-packages` (no venv/pip preinstalled) — both reused for the
revert.
**Risk:** none (as reverted) · **Effort:** trivial · **Depends on:** nothing

`Cargo.toml` pins `skrizhal-core` to `tag = "v0.3.0"` but leaves `fond-bib`/
`fond-vault` (Kartoteka) as bare `git = "..."` with no tag/rev. `Cargo.lock`
pins today's resolved commit, but any `cargo update` or fresh lockfile silently
pulls Kartoteka's current HEAD — a young, actively-changing project per the root
`Projects/CLAUDE.md`.

**Fix:** pin both to the current Kartoteka commit/tag (check with Cal what the
right tag/rev is — Kartoteka may not have version tags yet, in which case pin to
a `rev = "<sha>"` instead), matching the `skrizhal-core` convention.

---

## Phase 5 — CV template-kind desync (structural fix)

**Status:** ☑ DONE (2026-08-12) — investigated; the structural fix the phase
called for already exists. One stale comment fixed, no behavioral change.

Mapped every place template kind is stored, per the phase's own instruction to
design-first:
1. **In-document marker** — `// @zerkalo-kind:` comment, read by
   `parse_doc_kind`.
2. **Sidecar file** — `SidecarSettings::body_kind: String`
   (`load_sidecar`/`save_sidecar`), a cache of the last "Apply."
3. **In-memory dialog state** — `TemplateDialog`'s `body_kind: Rc<RefCell<BodyKind>>`,
   scoped to one dialog session only.
4. **The document body itself** — whether it actually calls `#cv-section(...)`
   or imports `cv-helpers.typ` (`body_looks_like_cv`).

Reading `open_template_for_active_document` (`app_window/mod.rs`, the single
function both the header's "Template" button and the hamburger's "Update
Template Settings…" call — already consolidated from two drifting ~110-line
copies, per its own doc comment) shows the second CHANGELOG fix already
implemented exactly the "one source of truth" design this phase called for:
sidecar/marker (1–2) are consulted first as a cache, every field the app can
parse back out of the document is then re-derived from the document itself
(3373–3416, "the sidecar is a cache of the last Apply and the document is
what compiles"), and CV-ness specifically has an explicit final override
(3418–3434) that trusts the body over sidecar/marker if they disagree —
exactly closing the drift loop the CHANGELOG bug describes, and self-healing
on every subsequent dialog-open even if something writes a stale sidecar
again later.

The one actual finding: `body_looks_like_cv`'s doc comment in
`template_dialog.rs` still said "see its two call sites in app_window.rs" —
stale since the consolidation above already reduced that to one. Fixed the
comment to point at `open_template_for_active_document` and explain the
consolidation, so a future reader doesn't go looking for a second site that
no longer exists. No code behavior changed.

Second finding worth the record even though it needed no fix: this is now the
**second** phase (after Phase 3) where the original review's premise didn't
survive investigation. Both undersold work already done — worth factoring in
when weighing the remaining phases (7, 9, 10), which are review-sourced but
not yet independently re-verified against current code the way 3 and 5 were.

---

## Phase 6 — Accessibility pass on dialogs

**Status:** ☑ DONE (2026-08-12) — all 5 named dialogs covered, 3 commits.

Went dialog by dialog per the plan, each its own commit:
- `settings_dialog.rs` — 7 icon-only buttons (folder/file browse buttons,
  remove-language) had no accessible name; 2 also lacked tooltips. All 7 fixed.
- `template_dialog.rs` — 5 icon-only buttons (author/affiliation pin, Skrizhal
  browse, save-as-template, delete-template) plus the CV-mode `Switch` (sits
  next to a plain unassociated `Label`, not an `Adw.SwitchRow`, so GTK doesn't
  auto-derive its name). All 6 fixed.
- `library_window.rs` — the multi-select clear button and the per-tag
  edit/delete buttons (which repeat once per tag row — folded the tag name
  into the label so screen readers don't announce identical "Edit"/"Delete"
  for every row). All 3 fixed.
- `setup_wizard.rs`, `export_dialog.rs` — checked, no changes needed. Both
  only use decorative `Image::from_icon_name` inside `ActionRow`s whose title
  already carries the accessible name; no bare interactive icon-only widgets.

Pattern used throughout: `gtk4::accessible::Property::Label` (the same one
already established in `editor_pane.rs`), added alongside a tooltip where one
didn't already exist. `Adw.SwitchRow`/`Adw.ComboRow` instances were left
alone — their title text already serves as the accessible name by
construction, so they were never part of the gap.

Full verification gate green after each of the 3 commits (484 tests, clippy
clean, version guard clean).

---

## Phase 7 — Systemic fix for the viewport/scroll-position bug class

**Status:** ☑ DONE (2026-08-12) — investigated per the phase's own
instruction; found the bug class already resolved and dormant. No code
change made.

Read the full changelog history of scroll/viewport/jump bugs, not just the 5
the original review sampled — there were closer to 15 entries once "snap,"
"drift," "jump," and "hadjustment" were all searched, spanning versions
0.13.10-dev2 through 0.19.0. Grouping by actual root cause rather than
symptom:

- **Cluster A — GTK's native focus-in `scroll_mark_onscreen`/`scroll_to_mark`
  snapping the viewport to the cursor** whenever focus returns to the editor
  (right-click menu dismiss, spell-popover dismiss, context-menu dismiss).
  This *is* one real root cause, and the six iterative fixes between
  0.13.10-dev2 and 0.13.12-dev3 were successive hardening of the same
  `saved_scroll`/`saved_hscroll` save-and-restore mechanism against edge
  cases (mouse-wheel scroll not updating the saved position, a race between
  `focus_ctrl.connect_leave`/`connect_enter` and GTK's own snap, right-click
  when already focused vs. gaining focus) — not six independent bugs. The
  changelog's own 0.13.10-dev8 entry is literally titled "(root cause)."
- **Cluster B — GTK's eased multi-frame scroll-to-top *animation*** after
  paste or a popover dismiss (0.19.0, distinct mechanism from Cluster A's
  instant snap). Fixed once for paste, then explicitly reused — not
  re-diagnosed — for the spelling-suggestion case ("the same GTK
  scroll-to-mark animation behind the paste jump fixed in 0.19.0"): the
  changelog shows the team had already spotted the shared cause in real time.
- **Distinct, unrelated to A/B:** GtkSourceView5's separate internal
  horizontal hadjustment snapping to `left_margin` on cursor movement
  (simple-mode click-snap, typewriter scroll) — a different adjustment
  object entirely, correctly fixed as its own thing.
- **Not in `editor_pane.rs` at all:** the preview-pane scroll drift
  (fraction-based restore vs. actual document height) and the preview scroll
  signal handler leak are a completely separate subsystem (compiled-PDF
  page-rendering scroll, not text-buffer scroll) that happen to share the
  word "scroll" in their changelog entries.

**The dormancy check:** searched every changelog entry from the current
version (0.23.0-dev1) back through 0.20.0 for any recurrence — zero hits.
This bug class has had no incidents across four-plus version cycles since
0.19.0. Combined with the "(root cause)" framing already in the historical
fix and the explicit reuse across Cluster B's two instances, there's no
evidence of an unresolved sixth instance or ongoing fragility to unify
against right now.

Per the phase's own escape hatch ("if the investigation finds the fixes are
actually unrelated [or already resolved], downgrade this phase and just note
that in this file rather than forcing a unification that isn't there") — no
refactor made. Third phase (after 3 and 5) where the original review's
implied urgency didn't survive checking against current code; see Phase 5's
closing note on what that means for 9 and 10.

---

## Phase 8 — Move `pdftotext` (and, if Phase 2 wires it in, `git log --follow`) off the main thread

**Status:** ☑ PARTIALLY DONE (2026-08-12) — `pdftotext` half done; `git log`
half deliberately deferred (see below).

`pdftotext` half: converted `extract_page_text_via_pdftotext` and
`extract_word_at_position` (`preview_pane.rs`, backing click-to-jump and
double-click-word-jump on the preview) from synchronous return-value functions
to `*_async` versions taking an `on_done` callback, following `do_sync`'s
spawn-thread + `mpsc::sync_channel` + `timeout_add_local` shape — this was
more than a drop-in wrap, since the original functions returned `Option<String>`
directly to a `match` in their callers; both call sites in
`app_window/mod.rs` (`handle_preview_click_jump`, `handle_preview_word_jump`)
were restructured to take the same match arms into the completion closure.
The `PreviewPane` fields these read (`root_file`, `buffer_snapshot`) are
`Rc<RefCell<...>>` and not `Send`, so a `gather_pdf_text_inputs` step clones
the needed data on the main thread before the background thread spawns.
`ensure_pdf_path` (which can trigger a full `compile_to_pdf_bytes` if the PDF
isn't cached — the actual slow part on large documents) now always runs off
the main thread. Verification: full gate green (484 tests, clippy clean,
version guard clean); headless launch + document-open smoke test shows no
startup regression. **Not verified interactively** — clicking the preview to
trigger click-to-jump/word-jump can't be scripted reliably in this
environment (same Xvfb-has-no-WM limitation as Phase 2's HistoryPanel
verification), and this path had zero test coverage before or after this
change. **Cal: worth clicking/double-clicking the preview once** after the
next dev build to confirm jump-to-line and jump-to-word still work — this is
the one behavior-changing edit in this phase that static checks can't cover.

`git log`/`git show` half (`history_panel.rs`, backing the newly-wired File
History window from Phase 2): **deliberately left synchronous.** Lower
urgency than originally scoped — Phase 2 wired History behind an explicit
modal-dialog-open action (menu click / palette), not something on the hot
live-preview path or triggered on every keystroke, so a brief block while the
dialog opens is a much smaller cost than the `pdftotext` case. Revisit if it's
ever felt in practice (e.g. on a repo with very long file history).

---

## Phase 9 — `template_dialog.rs` re-shrink

**Status:** ☑ DONE (2026-08-12) — split into a 5-file module directory,
4,706-line `mod.rs` down from a 7,585-line flat file. See "Result" below.
**Risk:** medium-high · **Effort:** large · **Depends on:** Phase 5 (done)

Now the largest file in the codebase (7,585 lines, 250 fns), despite the
CHANGELOG recording its constructor already being cut from 1,247→229 lines
once. The size claim is real and independently confirmed.

**The unwrap/expect-density half of the rationale does not hold** — checked
the same way Phase 3 checked `library.rs`: with the test-module boundary
correctly identified (`#[cfg(test)] mod tests` starts at line 5963; a
single unrelated `#[cfg(test)]`-gated helper at 2922–2925 is the only other
hit), non-test code (lines 1–5962) has **zero** `.unwrap()`/`.expect()` calls,
not 32+12. This is the second file (after `library.rs` in Phase 3) where the
original review's unwrap/expect count didn't survive a boundary-aware
recount — worth treating any remaining raw grep-based unwrap/expect claims
elsewhere in the original review with the same skepticism if they resurface.

Size alone is still a legitimate reason to split this file — `REFACTOR-PLAN.md`
already treats file size as sufficient justification on its own for
`editor_pane.rs`/`app_window.rs`, without needing an unwrap-density argument.

**Not started this session.** Unlike Phases 3/5/7/10, this one doesn't close
with an investigation — the size finding holds, and closing it means actually
doing the split. That's real, large, multi-commit mechanical work in the
single highest-risk file left (structurally identical to
`REFACTOR-PLAN.md`'s `AppWindow::new`/`open_file` splits: no behavior
changes, no redesign, extract along existing seams, stop and reconsider if a
seam needs a judgment call). Given the size of this already, it's the right
place to check in before starting rather than launching into it.

### Sub-plan (started 2026-08-12)

`TemplateDialog::new` is already small (229 lines, per `REFACTOR-PLAN.md`
Phase 2) — this isn't a monster-function problem like the other two files.
It's ~150 free functions and impls with no internal module structure, none
individually huge (largest ~500 lines). So this is a pure file split into a
module directory, using the file's own `// ── banner ──` comments as seams,
same principle as `REFACTOR-PLAN.md` Phase 3a/4. Ordered lowest-risk first
(pure functions with existing dedicated tests) → highest-risk last (the GTK
widget-construction/wiring core, which stays as `mod.rs`):

- **9a ☑ DONE (2026-08-12) — `parsing.rs`** (1,078 lines incl. new header
  comment). `template_dialog.rs` → `template_dialog/mod.rs` (6,528 lines) +
  `template_dialog/parsing.rs`. "Preamble parsers for documents with no
  sidecar" through "Title-page updater" banners, moved verbatim via `sed`
  extraction (no hand-transcription). `parsing.rs` opens with `use super::*;`
  (Rust's privacy rules already let a child module see its parent's private
  items, so this needed no visibility changes on `mod.rs`'s side); `mod.rs`
  gained `mod parsing; pub(crate) use parsing::*;` so external callers
  (`template_dialog::parse_font` etc.) and the `#[cfg(test)] mod tests`
  block's own `use super::*;` keep resolving unchanged. Two fixups the build
  caught: a handful of `parsing.rs` functions turned out to also be called
  from code that stayed in `mod.rs` (the reverse direction *isn't* automatic
  — a parent can't see a child's private items), fixed by bumping all of
  `parsing.rs`'s free functions to `pub(crate)` uniformly rather than
  chasing individual call sites; and all six `include_str!("../../templates/
  cv-helpers.typ")` paths needed an extra `../` since the file moved one
  directory deeper. All 484 tests passed unchanged (pure move, confirmed by
  zero test-count or behavior change).
- **9b ☑ DONE (2026-08-12) — `generate.rs`** (1,133 lines). "Template
  generator" + "CV template generator" + "CV: two-column sidebar layout" +
  `heading_styles`, same sed-extraction approach as 9a. Same two fixup
  categories recurred: several `generate.rs` functions (`header_block`,
  `package_import`, `margin_values`, `resolve_font_size`,
  `default_dropcap_lines`, `extract_heading_numbering`,
  `inject_heading_numbering`) are also called from `parsing.rs` (a sibling
  module) and `mod.rs`, so all of `generate.rs`'s free functions got bumped
  to `pub(crate)` uniformly, same fix as 9a. No `include_str!` paths in this
  block. All 484 tests passed unchanged.
  `mod.rs`: 6,528 → **5,404 lines** (down from the original 7,585 before
  9a/9b combined).
- **9c ☑ DONE (2026-08-12) — `sidecar.rs`** (562 lines). "Sidecar
  persistence" + body-splice/marker logic + legacy CV helpers, same
  sed-extraction approach. Pre-emptively bumped all of `sidecar.rs`'s free
  functions to `pub(crate)` before the first build attempt (learned from
  9a/9b's pattern) — compiled clean on the first try. No `include_str!`
  paths in this block. All 484 tests passed unchanged.
  `mod.rs`: 5,404 → **4,851 lines**.
- **9d ☑ DONE (2026-08-12) — `util.rs`** (155 lines). Font list, Typst
  escaping/sanitizing helpers. New failure mode this sub-phase (not seen in
  9a–9c): `build_font_list` referenced `super::font_manager::FontManager` —
  correct when this code lived directly in `template_dialog.rs` (a child of
  `ui`, so `super` meant `ui`), but wrong once nested one level deeper into
  `template_dialog/util.rs` (`super` there means `template_dialog`, not
  `ui`). Fixed with an absolute `crate::ui::font_manager::FontManager` path.
  Proactively grepped all four extracted submodules for other `super::X`
  references that might have the same problem — none found, this was the
  only one in the whole file. All 484 tests passed unchanged.
  `mod.rs`: 4,851 → **4,706 lines**.
- **Stays in `mod.rs`**: static data tables, `BodyKind`/`TemplateSettings`,
  the `Dialog`/tab-builder/`FormWidgets` GTK construction, `TemplateDialog`
  impl itself, and the `#[cfg(test)] mod tests` block — all re-export via
  `pub use` from the split-out modules so external callers
  (`template_dialog::parse_font` etc. from `app_window/mod.rs`) and the tests
  keep working unchanged. No behavior change, no API change — same rule as
  `REFACTOR-PLAN.md`: if a seam needs a judgment call, stop and leave it
  inline rather than push through.

Each sub-phase gets its own commit and full verification-gate run, per the
non-negotiable rule at the top of this file.

### Result

**All four sub-phases (9a–9d) done 2026-08-12.** `template_dialog.rs`
(7,585 lines, one file) is now `template_dialog/` (5 files):

| File | Lines |
|---|---|
| `mod.rs` | 4,706 |
| `generate.rs` | 1,133 |
| `parsing.rs` | 1,078 |
| `sidecar.rs` | 562 |
| `util.rs` | 155 |

`mod.rs` holds exactly what was always meant to stay: static data tables,
`BodyKind`/`TemplateSettings`, the GTK tab-builder/`FormWidgets` construction,
the `TemplateDialog` impl itself, and the test module. Every extraction was a
`sed`-based verbatim line move (no hand-retyping), and all 484 tests passed
unchanged after every single sub-phase — strong evidence this was truly
mechanical, not a rewrite wearing a refactor's clothes.

**Manual verification, beyond the automated gate:** headless smoke test
(Xvfb, isolated XDG/HOME, `dbus-run-session`) clicking the header's
"Template" button — per `REFACTOR-PLAN.md`'s own notes, pointer clicks
(XTEST) land in this no-window-manager setup even though keyboard input
doesn't, unlike the Ctrl+K approach that failed for Phase 2/8's verification.
The "Update Template Settings" dialog opened as a real, separate X window
(captured via `import -window <id>`, since secondary GTK windows don't
appear in a root screengrab — another `REFACTOR-PLAN.md` gotcha) and
rendered correctly: all six tabs, the Document tab's metadata fields
pre-filled from the demo document, the Style row correctly resolved to
"Chicago (Notes-Bib)," lock icons on Author/Affiliation. This exercises the
exact code that moved — `parsing.rs`'s field readers and `generate.rs`'s
style resolution — at runtime, not just at compile time. (One pre-existing,
unrelated cosmetic bug surfaced in the log: a few preset descriptions with a
literal `&` — e.g. "Skills & Awards" — fail GTK markup parsing and log a
warning; this text lives untouched in `mod.rs`'s static data and predates
this refactor, not a regression from it.)

**Recurring fixup patterns, worth remembering for any future split like
this:** (1) a child module can see its parent's private items via `use
super::*`, but not the reverse, so every extracted module's free functions
needed bumping to `pub(crate)` — cheapest done pre-emptively before the
first build rather than chasing one compiler error at a time; (2)
`include_str!` paths are relative to the file, so moving a file one
directory deeper breaks every relative include in it; (3) a `super::sibling`
reference that was correct when code lived in the flat file breaks once
that code nests one level deeper — worth grepping every extracted module
for stray `super::` once, rather than waiting for each to surface.

---

## Phase 10 — `editor_pane.rs` / `app_window.rs` churn investigation

**Status:** ☑ DONE (2026-08-12) — investigated, no action needed. Churn is
diffuse, not concentrated — matches the plan's own "not a code smell"
alternative outcome.

**`editor_pane.rs`:** categorized its ~100 most recent touching commits
(spanning v0.13.1 through v0.21.1, roughly 80% of its 125 lifetime touches)
by type:
- **~18 commits** are the scroll/viewport bug family — already fully
  investigated and closed in Phase 7 as one hardened, now-dormant root cause
  plus a couple of genuinely distinct, correctly-fixed-once issues.
- **~7 commits** are the `Rc<RefCell<>>` re-entrant-borrow crash family
  ("Fix crash when changing style twice (spell poll timer SourceId)", "Fix
  SIGABRT crash: state borrow held across GTK ops," etc.) — this is the same
  pattern the original review's finding #3 already named and
  `REFACTOR-PLAN.md` explicitly declines to redesign project-wide; not new
  information, and out of scope for this plan per this plan's own "Deliberately
  excluded" section below.
- **~6 commits** are other already-resolved, non-recurring correctness bugs
  (undo reliability, Simple Mode tag reapplication, a full-codebase review
  pass) with no evidence of the same failure resurfacing since.
- **The remaining ~55+ commits — the majority — are new features and polish**
  landing in the app's main editor widget: autocomplete, citation management,
  CV templates, Skrizhal integration, import system, accessibility passes,
  formatting toolbar, visual polish rounds. This is exactly what "this is
  where all editor features land" looks like, not fragility.

**`app_window/mod.rs`:** its entire post-split lifetime (22 commits, since
`REFACTOR-PLAN.md`'s Phase 3a) is the 5-commit split itself plus described,
intentional feature/UX work ("every window and dialog takes the suite's
chrome," "Template moves to the header," "Word/OpenDocument/Markdown convert
without pandoc") — zero "fix regression from a previous fix" cycling. Recent
churn rate (last 100 real repo commits) is 22% for both files, notably lower
than each file's lifetime average, i.e. churn has been slowing, not
accelerating, since the split.

Conclusion: no targeted fix identified, no restructure justified. This is the
fourth phase (after 3, 5, 7) where the original review's implied "ongoing
fragility" framing didn't survive checking against the actual commit history
— see Phase 5's closing note for what that pattern means when weighing
Phase 9 next.

---

## Deliberately excluded from this plan

- **The `Rc<RefCell<...>>` borrow-panic pattern itself** (the two known crashes:
  `spell_poll_timer`, `lsp_client` re-entrant borrow). `REFACTOR-PLAN.md`
  explicitly declines to redesign this pattern project-wide. Individual crash
  fixes (as they're found) stay as normal bug fixes outside this plan, not a
  phase here — a full redesign is out of scope until/unless a third crash makes
  the case for it.
- **`library_window.rs` (2,939 lines) and `setup_wizard.rs` (992 lines)** — both
  explicitly carved out of `REFACTOR-PLAN.md`'s scope already ("no single
  monster function; they can wait"). No new information from this review changes
  that call.

---

## Cross-reference: 2026-08-17 external (ChatGPT) architecture note

Cal shared an unprompted ChatGPT assessment of Zerkalo's architecture on
2026-08-17, after this plan's 10 phases were already all closed out. Recorded
here so a future session doesn't re-litigate it from scratch on seeing a
similar external take:

- File-size claims checked out exactly: `editor_pane.rs` 331,333 bytes,
  `app_window/mod.rs` 173,067 bytes — still the two largest files.
- Its "pin the Kartoteka git deps" recommendation is exactly what **Phase 4**
  above already tried and reverted — a `rev`-based pin broke the flatpak
  offline build. The note's own suggested fix ("wait for a Kartoteka release
  tag, then pin like `skrizhal-core`") is already the documented path forward
  there; nothing new to act on until Kartoteka cuts a tag.
- Its framing of `editor_pane.rs`/`app_window.rs` churn as an "architectural
  inflection point" risk is contradicted by **Phase 10**'s direct commit-
  history investigation: churn there is diffuse (mostly new-feature landing,
  not bug-fix cycling) and *decelerating* as a share of recent commits, not
  accelerating. The note inferred risk from file size alone without checking
  history.
- Its UI-integration-testing gap observation matches what **Phase 2** hit
  directly (scripted Ctrl+K interaction doesn't register under headless Xvfb
  without a window manager) — a known, already-documented limitation, not a
  new finding.
- Its suggested decomposition (`editor_pane/{completion,citations,...}.rs`,
  `AppWindow` → controller objects) isn't contradicted by anything here, but
  isn't called for by this plan either — see `REFACTOR-PLAN.md` for the
  actual file-splitting work, which already carved out similarly-sized files
  (`library_window.rs`, `setup_wizard.rs`) as lower priority for the same
  reason Phase 10 found no fragility signal. `REFACTOR-PLAN.md` Phase 4
  (`EditorPane::open_file`) is partly done; Phase 6 (unblock UI thread) is not
  started — that's the plan to check for follow-up on the editor-size
  concern, not this one.

Net: nothing in the note was factually wrong, but 9 of its 10 points restate
findings this plan already investigated and closed with more context than the
note had access to.

---

## How to resume this plan after a context reset

1. Read this file top to bottom.
2. Find the first `☐ not started` phase with satisfied dependencies.
3. Read the phase's own section fully before touching code.
4. Update the status box to `☐ in progress` before starting, `☑ DONE (date)` when
   the verification gate passes and the commit lands.
