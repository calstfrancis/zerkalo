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

**Status:** ☑ DONE (2026-08-12) — pinned via `rev` (Kartoteka has no release
tags yet; only `v0.1.0-devN` tags, none matching the locked commit). Required
bootstrapping `flatpak-cargo-generator.py` (fetched from the public
flatpak/flatpak-builder-tools repo — the usual `~/Projects/kartoteka/` copy
wasn't present on this machine) plus `pip`/deps via `get-pip.py
--break-system-packages` (no venv/pip preinstalled). Regenerating
`cargo-sources.json` was required even though the pinned commit is unchanged,
because `Cargo.lock`'s source URL gained a `?rev=` query param.
**Risk:** none · **Effort:** trivial · **Depends on:** nothing

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

**Status:** ☐ not started
**Risk:** medium-high · **Effort:** large · **Depends on:** Phase 5 (fix the state-desync issue before restructuring the file around it)

Now the largest file in the codebase (7,585 lines, 250 fns), despite the
CHANGELOG recording its constructor already being cut from 1,247→229 lines once.
It also carries the highest non-test unwrap/expect density in the codebase (32 +
12). Treat this the same way `REFACTOR-PLAN.md` treats `AppWindow::new`/
`open_file`: mechanical extraction along existing seams, not a redesign. If a
seam requires a judgment call about behavior, stop and reconsider rather than
pushing through.

Suggested approach: follow the same phase structure `REFACTOR-PLAN.md` used for
`editor_pane.rs`/`app_window.rs` — write it up as its own numbered sub-plan
inside this phase once started, using this file's verification gate.

---

## Phase 10 — `editor_pane.rs` / `app_window.rs` churn investigation

**Status:** ☐ not started
**Risk:** unknown until investigated · **Effort:** investigation first, fix scope TBD · **Depends on:** Phase 7 (the scroll-bug fix may remove a chunk of this churn on its own)

125 and 121 touches respectively across the last 300 commits — both already went
through `REFACTOR-PLAN.md`'s Phase 3a–5 splitting work, yet remain the two
hottest files by a wide margin. This is the "fragile core" signal and the
biggest unknown in this plan.

**Fix:** before writing any code, spend a session categorizing the last ~40
commits touching each file (bug fix vs. new feature vs. refactor-churn) to find
out whether the touches are concentrated in a few functions (→ targeted fix,
possibly folds into Phase 7 or the `Rc<RefCell<>>` pattern) or spread evenly
(→ likely just "this is where all editor features land," not a code smell, and
this phase should be closed as "investigated, no action" rather than forced into
a rewrite). Do not commit to a restructure until that categorization is done.

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

## How to resume this plan after a context reset

1. Read this file top to bottom.
2. Find the first `☐ not started` phase with satisfied dependencies.
3. Read the phase's own section fully before touching code.
4. Update the status box to `☐ in progress` before starting, `☑ DONE (date)` when
   the verification gate passes and the commit lands.
