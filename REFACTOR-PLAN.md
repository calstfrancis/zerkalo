# Zerkalo Refactor Plan — file splitting + library.rs tests

**Created:** 2026-08-03 · **Baseline:** v0.20.0-dev2 · **Status:** Phase 1 done

This file tracks progress. **Read it before starting any phase**, and update the
status boxes as phases land. Same role `PRINT-PLAN.md` plays for the print work.

---

## Why this exists

Assessment of the codebase at v0.20.0-dev2 found the code quality itself is
sound — 269 tests passing in 0.39s, clippy clean, only ~23 `unwrap`/`expect`
outside test modules across 46.5k lines, SQL fully parameterised. The problem is
**size concentration**, plus one coverage hole:

| Problem | Where | Size |
|---|---|---|
| `AppWindow::new` is one function | `ui/app_window.rs:81–4380` | **4,299 lines** |
| `EditorPane::open_file` is one function | `ui/editor_pane.rs:2802–5532` | **2,730 lines** |
| `EditorPane::new` | `ui/editor_pane.rs:303–1741` | 1,438 lines |
| `TemplateDialog::new` | `ui/template_dialog.rs` | 1,247 lines |
| `library.rs` has zero tests | `src/library.rs` | 1,083 lines |
| `spellcheck.rs` has zero tests | `src/spellcheck.rs` | 516 lines |

Four files hold 55% of the codebase. `AppWindow::new` is navigable only because
of ~90 `── section ──` comment banners — a table of contents compensating for a
function that should be forty functions. `open_file` is worse: the name lies, so
nobody looking for tab-creation logic has a reason to open it.

**Guiding principle: the banners are already the seams.** This is mechanical
extraction, not redesign. No behaviour changes, no logic rewrites, no API
redesign. If a phase starts requiring judgement calls about behaviour, stop and
reconsider the split rather than pushing through.

---

## Ordering rationale

Tests first (Phase 1), then splitting. Two reasons:

1. `library.rs` is untested *and* untouched by the splitting work, so it's an
   independent win that can't be invalidated by later phases.
2. It builds the habit of "extract a seam, then test through it" on a small
   file before applying the same thinking to a 4,000-line function.

The splitting phases go **easiest → hardest** so the pattern is established on
low-risk code first. `open_file` is last because it's the one with genuinely
tangled state.

---

## Verification gate — run after EVERY phase

Non-negotiable. A phase is not done until all four pass:

```sh
cargo test              # must stay at 269+ passing, 0 failed
cargo clippy --all-targets   # must stay clean (0 warnings)
cargo build --release   # must succeed
./check-versions.sh     # version-consistency guard
```

Then a **manual smoke test** — the compiler can't catch a closure wired to the
wrong widget. Launch and confirm:
- App opens, a document loads, live preview compiles
- Open a second file (tab switching works), edit it, autosave indicator moves
- Ctrl+K palette, sidebar toggle, focus mode, hamburger menu items
- Whichever subsystem the phase touched, exercised directly

**Commit at the end of each phase**, separately. Never let two phases share a
commit — a bisect needs to land on one extraction at a time.

---

## Phase 1 — `library.rs` tests

**Status:** ☑ **DONE** (2026-08-03) — 71 tests added, 269 → 340 total, all gates green
**Risk:** low · **Depends on:** nothing

### Outcome

- **1a done.** `Library` gained a `trash_dir: PathBuf` field + `default_trash_dir()`.
  `open()` and `open_in_memory()` both use the real path (unchanged behaviour —
  `open_in_memory` is a *production* fallback when `open()` fails, not just a test
  hook, so it must keep the real trash dir). Tests use the private
  `in_memory_with_trash_dir()`.
- **1b done.** 46 tests in `library.rs`: trash lifecycle, all nine filters,
  search, sort, upsert/timestamps, tags, categories, projects, import.
- **1c done.** 25 tests in `spellcheck.rs`. `extract_words` turned out to be far
  richer than the plan assumed — a Typst-aware markup skipper (comments, raw
  blocks, math, citations, labels, `#fn(...)`/`{...}` args, heading markers), not
  just a word splitter. Tests cover all of it plus code-point offset correctness.
- Fixed a misattached doc comment: `extract_typst_title`'s docs were sitting on
  `restore_collision_path` (`library.rs:1008`).

### Two findings — behaviour pinned, NOT fixed (Phase 1 is no-behaviour-change)

1. ~~**Uncolored categories all render the same blue.**~~ **FIXED 2026-08-03**
   (commit after Phase 1). `color_hex` was `NOT NULL DEFAULT '#3584e4'`, so
   `get_category_color` could never return `None` and the
   `stable_palette_color(&name)` fallbacks were dead code. The fix had three
   parts, because the bug had three: the column is now nullable (one-time
   rebuild migration treating the old auto-applied default as unset);
   `Category.color_hex` is `Option<String>` with the palette fallback applied at
   the three sidebar call sites; and the Set Category dialog only persists a
   colour when a swatch is actually clicked, instead of always writing back
   whatever it happened to be displaying. Covered by 9 tests including a
   file-backed migration test with WAL + foreign keys on.
2. **`move_to_trash` can mark a row deleted without the file having moved.** If
   both `rename` and `copy` fail, the DB still sets `deleted=1` and a
   `trash_path` pointing at a file that was never created. Not currently pinned
   by a test (hard to provoke portably); noted for whoever touches this next.

### Original plan follows (for reference)

`library.rs` already has `open_in_memory()` (line 102) — a function that exists
purely for testing, currently unused by any test. That's the hook.

### 1a. Add the filesystem seam (prerequisite)

`move_to_trash` (line 557), `restore_from_trash` (585), and
`permanently_delete` (616) all resolve the trash directory via
`glib::user_data_dir().join("zerkalo").join("trash")` — a hardcoded global. **They
cannot be tested without a seam**, and testing them against the real path would
write into Cal's actual data dir.

Fix: give `Library` a `trash_dir: PathBuf` field.
- `open()` sets it to `glib::user_data_dir().join("zerkalo").join("trash")` — unchanged behaviour
- `open_in_memory()` gains a variant (or a `with_trash_dir` setter) taking a `tempfile::TempDir` path
- Replace the three inline `glib::user_data_dir()` calls with `self.trash_dir`

`tempfile` is already a dev-dependency. This is the only production-code change
in Phase 1; everything else is pure test addition.

### 1b. Tests to write

Priority order — destructive operations first, since those are the ones that
lose user data when wrong:

**Trash lifecycle** (highest value — real file moves + DB state)
- `move_to_trash` sets `deleted=1` and populates `trash_path`
- `move_to_trash` actually moves the file off the original path
- two same-named files trashed in the same second don't collide
  (the `ts-{doc_id}-{name}` scheme at line 566 exists specifically for this —
  pin it)
- `restore_from_trash` puts the file back and clears `deleted`/`trash_path`
- **restore when a new file now occupies the original path** uses
  `restore_collision_path` instead of clobbering it (line 597 — this is the
  data-loss case)
- `restore_from_trash` on a doc with no `trash_path` is a no-op, not an error
- `permanently_delete` removes both the DB row and the trash file

**Query/filter layer** (`documents()`, line 277)
- each `LibraryFilter` variant returns the right set: All excludes
  archived+deleted; Project respects `pd.position` ordering; Tag/Category filter
  correctly; Trash shows only deleted
- search substring matching, including when `search` is empty (the `"%"` path)
- each `SortOrder` variant orders correctly, and `pinned DESC` wins over the
  sort in every filter
- `doc_count()` agrees with `documents().len()` for the same filter

**Tags & categories**
- `set_doc_tags` replaces (not appends); `add_doc_tags` appends without duplicating
- `delete_tag` removes the tag from all docs that had it
- `rename_tag` preserves associations
- `ensure_category` / `create_category` are idempotent
- category colours round-trip

**Upsert & timestamps**
- `upsert_document` on the same path twice returns the same id, doesn't duplicate
- `touch_opened` / `touch_saved` move only their own column

Target: **~30 tests**. Suite should stay well under 1s.

### 1c. Also in scope — `spellcheck.rs`

Two pure functions, no seam needed, trivially testable:
- `extract_words()` (line 208) — offsets are correct; hyphens, apostrophes,
  non-ASCII/unicode, empty input, punctuation-only input
- `levenshtein()` (line 495) — identity is 0, symmetry, empty-string cases,
  single edits of each kind (insert/delete/substitute)

Target: ~10 tests.

---

## Smoke test — 2026-08-03 (covers Phase 1, the category-colour fix, and Phase 2)

Run headless against the real `target/release/zerkalo`, in a throwaway `$HOME`
with all four XDG dirs redirected. Verified afterwards that nothing under
`~/.config/zerkalo` or `~/.local/share/zerkalo` was modified.

**Harness notes for next time** (the script lives in the session scratchpad, not
the repo — rebuild from these notes if it's wanted again):

- **A window manager (`kwin_x11`) breaks window mapping entirely** — blank root,
  no window ever appears. `capture-screenshots.sh` runs without one; match it.
  (Corrected 2026-08-03 during phase 3c: an earlier note here blamed
  `dbus-run-session` as well. That was wrong — it was `kwin` alone. See the
  portal note below; `dbus-run-session` is in fact *required* for some dialogs.)
- **Anything using `gtk4::FileDialog` needs `dbus-run-session`.** Without it the
  harness shares Cal's real session bus, GTK routes the chooser to the real
  desktop portal, and no window appears on the isolated display at all (the row
  fires, the popover blanks, nothing opens). `GTK_USE_PORTAL=0` does not prevent
  it. Under `dbus-run-session` the in-process chooser opens normally. This
  affects Open File, New Blank Document and Save As.
- With no WM there is no X input focus, so `xdotool key` alone goes nowhere.
  **Pointer clicks (XTEST) land regardless of focus**, and once a click has
  landed in a window, typing into it works.
- GTK4 popovers and secondary windows are **separate X surfaces that do not
  appear in a root-window screengrab**. Capture them with
  `import -window <id>` after finding them via `xdotool search --name`.
- The startup "Some tools are missing" alert (pandoc/tinymist absent in a
  throwaway home) covers the main window until dismissed.

**Results — all pass:**

| Area | Result |
|---|---|
| Main window | Editor, syntax highlighting, live preview compiled (`✓ 1 page`), outline populated, status bar intact |
| Category colours (the fix) | Five categories render **five distinct palette colours**, none the old `#3584e4`; the deliberately-set `#e01b24` survived. Two names hash to the same slot, which is inherent to an 8-colour palette, not a regression |
| Schema migration | Ran through the real binary over a seeded **legacy-schema** DB: `color_hex` nullable, auto-default → `NULL`, explicit colour preserved, **0 foreign-key violations** |
| Template dialog | Opens with all six tabs; Document/Layout/Packages tabs correct incl. the Droplet expander |
| Preset gallery | Preset list renders with a live-compiled preview pane |
| CV Mode toggle | All four behaviours: gallery filtered to the 4 CV presets, Sections + Packages tabs hidden, Skrizhal group revealed, Style row swapped |
| `FormWidgets::collect()` | Preview Code generated correct Typst from live form state — typed Title and Author both round-tripped, combos (paper/font/size/style) all correct |

**Checked, not a regression:** the dialog opens on the Document tab rather than
Template. The `append_page` × 5 + `prepend_page` sequence is byte-identical
before and after Phase 2 and neither version calls `set_current_page`, so this
is pre-existing GTK behaviour.

**Not exercised:** Create Document and Apply to Current (both write files or need
a save dialog), the pin buttons, and trash/restore through the UI. Create and
Apply share `collect()` with Preview Code, which is verified, but their
file-writing tails are not.

---

## Phase 2 — `TemplateDialog::new` (1,247 lines)

**Status:** ☑ **DONE** (2026-08-03) — `new()` 1,247 → 229 lines, all gates green
**Risk:** low · **Depends on:** nothing (do after Phase 1 for pattern-setting)

### Outcome

Extracted, in order: five tab builders (`build_document_tab`, `build_layout_tab`,
`build_sections_tab`, `build_languages_tab`, `build_packages_tab`), each
returning a small struct of the widgets the dialog needs later;
`build_cv_elements_group`; `build_templates_gallery`; `wire_cv_mode_toggle`;
`wire_pin_buttons`; `wire_preview_code_button`; `wire_action_buttons`.

**The unplanned win: `FormWidgets`.** The Create, Apply and Preview Code paths
each cloned the same 35 widgets and repeated the same ~70-line `TemplateSettings`
literal — verified semantically identical (differences were whitespace and one
closure parameter name) before merging. They now share one `FormWidgets` value
and a single `collect()`. That removed ~200 lines of triplication and is what
made the tail of `new()` tractable; passing `&form` also cut the parameter counts
of the gallery and toggle helpers dramatically.

**No `#[allow(clippy::too_many_arguments)]` was added.** Where a helper wanted
more than seven parameters, the arguments were grouped into meaning-carrying
structs instead — `ActionButtons`, `StyleRowModels`, `FontDefaults`,
`CvModeTargets`. This is the plan's own "bundle rather than grow the signature"
rule, and it is the pattern Phase 4's `TabContext` should follow.

**Still not tested.** `template_dialog.rs`'s 38 tests cover the pure generation
functions, not the dialog construction — they passed throughout, which is
reassuring but not the same as verifying the wiring. Manual check needed: preset
gallery click-through, CV Mode toggle, pins, Preview Code, Create and Apply.

Deliberately first among the splits: `template_dialog.rs` already has the most
tests in the repo (38), so the extraction is well-covered by the existing suite —
the safest place to establish the pattern.

Split `new()` along its internal sections into private helpers. Leave the pure
functions (`generate_typst_template`, `generate_preset_preview`) where they are;
they're already separate and already tested.

**Deliverable:** `new()` under ~200 lines, remainder in named `fn build_*`
helpers. File stays one file — 5,920 lines is large but the problem here is the
function, not the file.

---

## Phase 3 — `AppWindow::new` (4,299 lines) — the main event

**Status:** ◐ **IN PROGRESS** — 3a ☑ 3b ☑ 3c ☑ 3d ☑ done (2026-08-03); 3e remains
**Risk:** medium · **Depends on:** Phase 2 (pattern established)

### Progress

- **3a ☑** `app_window.rs` (8,302 lines) is now a module directory: `mod.rs`
  (5,159), `import.rs` (2,152), `sync.rs` (502), `dialogs.rs` (249),
  plus 3b's `header.rs` (334) and `panels.rs` (210). Free functions only.
  The import machinery was one contiguous block and **all 23 of the file's
  tests target it**, so they moved with it and covered the move immediately.
  Only 7 of its 32 items were reachable from `impl AppWindow`.
  Also removed a stale doc comment describing the pre-print-overhaul
  "compile to `~/.cache/zerkalo` and xdg-open" behaviour, which had drifted
  onto `restore_snapshot_with_confirm`; `print_from_preview`'s own current doc
  comment already documents that behaviour as removed.
- **3b ☑** `build_header()` → `HeaderWidgets` (43 fields) and `build_panels()`
  → `Panels` (13 fields). `AppWindow::new` 4,299 → **4,006**.
  Note the modest reduction: the destructures cost ~55 lines at the call
  sites. Construction extraction has a floor; the wiring is where the bulk is.

### 3c ☑ DONE (2026-08-03) — `AppWindow::new` 4,006 → 3,210

`menus.rs` (875 lines) holds `MenuCtx` (20 fields of shared state) plus
`wire_app_menus` and `wire_document_menus`, each taking `(&MenuCtx, &Menus)` —
two parameters where positional extraction would have needed 30 and 21.
`HeaderWidgets` gained the nested `menus: Menus` field as planned, so the buttons
travel as one value.

Two preparatory fixes, both behaviour-preserving:
- `toast_overlay` was created *inside* the Print menu section; hoisted above the
  run, where several later sections already needed it.
- `toast_for_sync_btn` was deleted outright. It was `toast_overlay.clone()` —
  the same GObject under a misleading name, and it was what actually got
  assigned to the struct's `toast_overlay` field. All uses now say
  `toast_overlay`.

**Smoke-verified row by row** (see harness notes above; one app launch per row,
because the dialogs are modal and one open dialog blocks input to the parent no
matter whether it is hidden, moved or sent `WM_DELETE_WINDOW`):

- run A, 10/10: Browse Documents → "My Documents", Export → "Export", Print →
  "Print", Font Management, Settings, Setup & Onboarding, Git Remotes, Writing
  Stats, Keyboard Shortcuts & Help → "Help — Zerkalo", About → "Zerkalo 0.20.0-dev2"
- run B, 8/8: New from Template, New Blank Document → "New Document", Open File,
  Update Template Settings, Repair Template Markers → "Marker repaired",
  Save As, Browse Snapshots, Export for Web

Not covered: **Save** and **GOST Type B font** act on the document and open no
window; the **Import** rows are hidden in the throwaway home because pandoc is
absent, so they need a machine with pandoc installed to exercise.

### 3d ☑ DONE (2026-08-03) — `AppWindow::new` 3,210 → 2,486

Three more runs extracted, each with its own context struct:

- `citations.rs` (342) — `wire_citations(&CitationCtx) -> Rc<RefCell<Option<PathBuf>>>`.
  Bibliography and CV-entry loading with their file watches, the citation
  panel's insert/choose actions, and the reference manager's insert, jump and
  project-wide citation-key rename. Returns the auto-detected `.bib` slot, which
  later sections read.
- `file_tree_wiring.rs` (410) — `wire_file_tree(&FileTreeCtx) -> FileTree`.
  The sidebar tree, the root-file context menu, and project mode with its
  inline controls.
- `startup.rs` (123) — `wire_pane_persistence` and `wire_file_watcher`.

**Boundary correction worth remembering:** the "Persist pane positions" banner is
followed directly by the final layout assembly (`main_content`, the toolbar view,
`window.set_content`). Cutting at the next banner swallows it. The layout stays
in `new()`; only the two `connect_position_notify` blocks belong in the helper.

**Smoke-verified** against the release binary, on a work dir with a nested
`chapters/` directory and a `refs.bib` beside the document:

- Citations panel header shows **`refs.bib`** — the auto-detect path found it —
  and both entries render with author and year.
- Project mode: toggle activates, the inline root controls (`no root`, `Set…`)
  appear, and the **"main.typ detected — set it as root?"** banner fires.
- Pane persistence: `sidebar_width` and `preview_split` are written to
  `config.toml`, and `preview_split` differed between two runs with different
  layouts, so it is persisting real positions rather than a constant.
- Watcher: editing `main.typ` externally produced `inotify` MODIFY/CLOSE_WRITE
  events in the log.

### 3c — original analysis (kept for reference)

**Do not start 3c by extracting menu sections one at a time.** Measured:

- The menu sections are **not contiguous** — the "Menu: Import (picker dialog)"
  and "Citation panel: Skrizhal" blocks are interleaved between them. Two
  contiguous runs exist: **983–1273** (Browse Documents → Font Management, 291
  lines) and **1431–1944** (Import PDF → Export for Web, 514 lines).
- Run A needs **30** distinct captures, run B needs **21**. Extracting them with
  positional parameters would produce exactly the 12-argument functions this
  plan forbids.

**The shape it wants** (same move as Phase 2's `FormWidgets`, and the rehearsal
for Phase 4's `TabContext`): one `MenuCtx` struct holding the ~22 shared items
(`window`, `editor_pane`, `preview_pane`, `error_panel`, `toast_overlay`,
`current_config`, `project_root`, `writing_log`, the compile-mode cluster), plus
a `Menus` struct of the 22 `menu_*` buttons — which **`HeaderWidgets` already
holds**, so the cheapest route is to give `HeaderWidgets` a nested `menus: Menus`
field rather than building a second copy. Then each run becomes
`wire_app_menus(&ctx, &menus)` — two parameters.

**Two bindings escape their run and must be hoisted above it first:**
`toast_overlay` (created inside run A) and `toast_for_sync_btn` (run B).

**Verification warning:** nothing in the test suite covers `app_window` wiring —
all 347 tests pass regardless of whether a menu item is connected to the wrong
handler. 3c needs the headless smoke harness (see the Smoke test section above)
driving the hamburger menu item by item.

### The mechanic

Each `── banner ──` becomes a method. The existing banners already name them:

```
── Header bar ──                    → fn build_header_bar(...)
── Hamburger menu items ──          → fn build_hamburger_menu(...)
── Menu: Export ──                  → fn wire_export_menu(...)
── Bibliography loading & watch ──  → fn wire_bibliography(...)
── File tree ──                     → fn wire_file_tree(...)
── LSP: poll for diagnostics ──     → fn spawn_lsp_poller(...)
── Persist pane positions ──        → fn wire_pane_persistence(...)
...
```

~90 banners → roughly 40 helpers after merging trivially small adjacent ones.

### Sub-phases — do NOT do this in one commit

Split across several commits, each independently verified against the gate:

- **3a — Leaf dialogs & helpers.** Free functions at the file's end
  (`show_backup_remote_dialog`, `show_github_token_dialog`,
  `show_dynamic_shortcuts_window`, `post_process_latex_import`,
  `handle_preview_click_jump`, …) are already outside `new()`. Move them to a
  new `ui/app_window/dialogs.rs`. Pure file-move, near-zero risk, and it
  immediately drops the main file by ~1,000 lines.
- **3b — Widget construction.** The `── Header bar ──`, `── Popover layout ──`,
  `── Panels ──`, `── Layout ──` sections. These build widgets and return them;
  the least entangled.
- **3c — Menu wiring.** All ~20 `── Menu: X ──` sections. Highly uniform
  (connect a click handler to a menu row), so they extract almost identically.
- **3d — Subsystem wiring.** Bibliography, CV entries, citations, file tree,
  LSP, sync, watcher, pane persistence.
- **3e — Startup & lifecycle.** Welcome window, setup wizard chain, missing-tool
  checks, auto-backup.

### The hard part: closure captures

`app_window.rs` has **878 `.clone()` calls**, nearly all closure captures. When a
section moves into a method, its captures must become parameters.

- Prefer passing `&` references and cloning inside the helper, so the call site
  stays readable
- Where a helper needs 8+ captures, that's a signal the section is doing too
  much — either it splits further, or those widgets genuinely belong in a
  struct. **Bundle into a small `struct` of related widgets rather than growing
  a 12-argument function.**
- Resist the urge to redesign the ownership model mid-refactor. If a section
  fights the extraction, leave it inline, note it here, and move on. A 600-line
  `new()` with three stubborn inline sections is a huge win over 4,299.

### On splitting into modules

Convert `ui/app_window.rs` → `ui/app_window/` with `mod.rs`, `dialogs.rs`,
`build.rs`, `menus.rs`, `wiring.rs`. Do this in 3a while the file is still
whole — moving files later means re-resolving imports repeatedly.

**Note:** the 23 existing tests in `app_window.rs` must move with the code they
test. Check them after each sub-phase.

---

## Phase 4 — `EditorPane::open_file` (2,730 lines)

**Status:** ☐ not started
**Risk:** high · **Depends on:** Phase 3 (hardest last)

### Why it's hardest

Every section closes over the same freshly-created `buffer`/`view`/`tab`, plus
much of the ~70-field `EditorPane` struct. Unlike `AppWindow::new` — where
sections wire mostly-independent subsystems — these are genuinely interleaved
around one tab's state.

### The approach

Introduce a **`TabContext` struct** holding what the per-tab closures need
(`buffer`, `view`, `scroll_window`, `path`, plus the relevant `Rc` handles from
`EditorPane`). Build it once at the top of `open_file`, then pass `&TabContext`
to each extracted `fn wire_*`. This is the key move — without it, every helper
takes 15 arguments and the refactor makes things worse, not better.

Then extract along the 26 existing banners:

```
2891 ── Image / document drag-and-drop ──    → wire_drag_and_drop
2961 ── Tab label ──                         → build_tab_label
3123 ── Modified flag + word count ──        → wire_modified_and_word_count
3249 ── Cursor position tracking ──          → wire_cursor_tracking
3538 ── @-citation / !-cv-entry autocomplete → wire_citation_autocomplete
3695 ── #-function LSP autocomplete ──       → wire_lsp_autocomplete
3954 ── Key controller ──                    → wire_key_controller  (~550 lines,
                                                splits further into the
                                                Ctrl+B/I, Ctrl+D, Ctrl+/,
                                                Ctrl+Enter, undo/redo and
                                                word-nav sub-banners)
4502 ── Alt+Enter spell suggestions ──       → wire_spell_suggestions
4665 ── Spell check: debounced ──            → wire_spellcheck
4753 ── Spell check: autocorrect ──          → wire_autocorrect
4846 ── Right-click context menu ──          → wire_context_menu
5191 ── Inline error assistant ──            → wire_error_assistant
5486 ── Insert into notebook ──              → (stays inline; it's the tail)
```

Also rename: what remains should be honest about doing tab construction, or the
tab-building body should live in `fn create_tab(...)` with `open_file` reduced
to the existing-tab check plus a call to it.

**Also split the file.** `editor_pane.rs` at 7,081 lines → `ui/editor_pane/`
with `mod.rs`, `tab.rs`, `keys.rs`, `completion.rs`, `spell.rs`.

### Manual testing is critical here

`editor_pane.rs` has **zero tests**. The compiler will catch nothing beyond type
errors. Every phase-4 commit needs hands-on verification of: typing, autocorrect,
every keyboard shortcut listed above, both autocomplete paths, spell right-click,
error hover, drag-and-drop, tab switching, word count, cursor/breadcrumb display.

Consider writing a few `EditorPane` tests **before** this phase for anything
extractable as a pure function (word navigation offsets, comment toggling,
auto-pair logic) — but don't let that block the phase if the logic is too
GTK-entangled to isolate.

---

## Phase 5 — `Library::documents()` deduplication

**Status:** ☐ not started
**Risk:** low · **Depends on:** Phase 1 (needs the tests as a safety net)

`library.rs:277–405` is nine near-identical match arms. Each prepares a query,
calls `query_map`, and loops `for r in rows { docs.push(r?); }`. The only
variation is the JOIN, the WHERE prefix, and the parameter index.

Collapse to one query path that composes those three fragments. **Only attempt
this after Phase 1's filter/sort tests exist** — they're exactly the safety net
that makes it a safe change, and they'll then cover one code path instead of nine.

---

## Phase 6 — optional: unblock the UI thread

**Status:** ☐ not started
**Risk:** low · **Depends on:** nothing · **Priority:** only if it's felt in use

Two synchronous subprocess calls on the GTK main thread:
- `ui/history_panel.rs:210,237` — `git log --follow` (slow on long histories)
- `ui/preview_pane.rs:985,1019` — `pdftotext` (slow on large PDFs)

Both are bounded and fine in the common case. If either ever feels sticky, port
them to the pattern `do_sync` already uses correctly (`app_window.rs:4969`):
spawn a thread, poll a `std::sync::mpsc::sync_channel` from
`glib::timeout_add_local`. **Don't do this speculatively** — it adds async
complexity for no benefit if nobody has noticed a freeze.

---

## Explicitly out of scope

Keeping the blast radius small is the point. Do **not**, as part of this work:

- Change any user-visible behaviour
- Redesign the `Rc<RefCell<Option<Box<dyn Fn>>>>` callback pattern — it's
  consistent, deliberate, and documented at `main.rs:1`
- Remove the `clippy::type_complexity` allow
- Add abstractions "while we're in here"
- Touch `library_window.rs` (3,057 lines) or `setup_wizard.rs` (1,223) — large
  files, but no single monster function; they can wait
- Bump versions or release. This is refactor work; a version bump happens when
  Cal asks for a dev build, per `CLAUDE.md`

## Definition of done

- [ ] No function in `src/` exceeds ~400 lines
- [ ] `AppWindow::new` and `EditorPane::open_file` each under ~300 lines
- [ ] `library.rs` and `spellcheck.rs` have meaningful test coverage
- [ ] Test count materially up from 269; all passing
- [ ] Clippy still clean, release build still works
- [ ] `CHANGELOG.md` updated (internal/refactor entry — per `CLAUDE.md`'s
      documentation policy, meaningful changes get an entry even without a release)
