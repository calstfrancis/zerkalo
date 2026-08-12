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

**Status:** ☐ not started
**Risk:** low (read-heavy audit) · **Effort:** small–medium · **Depends on:** nothing

24 non-test `.unwrap()` calls in the DB/CRUD layer. A panic here (corrupt or
locked SQLite file) crashes the whole app on startup or file access instead of
surfacing through the error panel, violating `CLAUDE.md`'s own "no unwrap/expect
in UI code paths" rule (this file backs UI-visible operations even if it isn't
itself a UI file).

**Fix:** go through each of the 24 non-test unwraps, convert to `thiserror`
`Result` propagation per `src/error.rs`'s existing pattern, surface via the error
panel. Prioritize any on the startup/open-library path — those are the ones that
turn a bad file into an unlaunchable app.

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

**Status:** ☐ not started
**Risk:** medium · **Effort:** medium · **Depends on:** nothing

CHANGELOG records the "Update Template Settings" crash on CV documents
(`unknown variable: section`) being fixed twice — once by restoring template
kind from sidecar/marker, again because an already-corrupted sidecar
perpetuated itself, requiring a second fix that cross-checks body content.
Template kind is tracked in more than one place that can desync.

**Fix:** before touching code, map out every place template kind is stored
(sidecar file, in-document marker, any in-memory `TemplateDialog` state) and
pick one source of truth; the others become derived/cached, not independently
writable. This is the kind of judgment-call refactor `REFACTOR-PLAN.md`
explicitly says to stop and reconsider rather than push through mechanically —
treat this phase as design-first, not a blind extraction.

---

## Phase 6 — Accessibility pass on dialogs

**Status:** ☐ not started
**Risk:** low · **Effort:** medium (breadth, not depth) · **Depends on:** nothing

Only `error_panel.rs` and `editor_pane.rs` use `AccessibleRole`/
`AccessibleTristate`/`set_accessible` out of ~80 `src/ui/*.rs` files. None of
`template_dialog.rs`, `settings_dialog.rs`, `setup_wizard.rs`,
`library_window.rs`, `export_dialog.rs` do, despite the root `Projects/CLAUDE.md`
holding up Zerkalo's own status-bar toggle pattern as the house reference.

**Fix:** go dialog by dialog, starting with the ones most used
(`settings_dialog.rs`, `template_dialog.rs`), adding accessible roles/labels to
interactive controls and `AccessibleTristate` to any toggle-like widgets. Land as
several small commits (one dialog per commit) rather than one large sweep, so a
regression is easy to bisect.

---

## Phase 7 — Systemic fix for the viewport/scroll-position bug class

**Status:** ☐ not started
**Risk:** medium-high (touches hot, high-churn code) · **Effort:** medium · **Depends on:** nothing, but do NOT combine with Phase 9/10 work in the same session

At least 5 separate changelog fixes in `editor_pane.rs` for viewport/scroll
issues (right-click jump-to-top, paste-triggered scroll animation, copy/cut
moving the viewport, click-snap-to-left-edge, GtkSourceView's internal
hadjustment fighting the app's own), each patched as a one-off.

**Fix:** before patching another instance, identify whether all 5 share a root
cause (GtkSourceView adjustment vs. app-tracked scroll position both mutating
without coordination) and centralize adjustment ownership behind one function/
guard, rather than adding a 6th independent patch next time this class of bug
resurfaces. This phase is explicitly "investigate first, only then fix" — if the
investigation finds the 5 fixes are actually unrelated, downgrade this phase and
just note that in this file rather than forcing a unification that isn't there.

---

## Phase 8 — Move `pdftotext` (and, if Phase 2 wires it in, `git log --follow`) off the main thread

**Status:** ☐ not started
**Risk:** low · **Effort:** small (pattern already exists) · **Depends on:** Phase 2 decision (for the git log half)

`REFACTOR-PLAN.md`'s Phase 6 (deferred) already identified `preview_pane.rs:985,1019`
(`pdftotext`) and `history_panel.rs:210,237` (`git log --follow`) as synchronous
subprocess calls on the main thread, deferred as "only if it's felt in use."
`pdftotext` sits in the live preview path, so it's a real freeze risk on large
PDFs regardless of the HistoryPanel decision.

**Fix:** wrap both in the `do_sync` async pattern already used at
`app_window.rs:4969` — no new pattern to invent, just apply the existing one.

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
