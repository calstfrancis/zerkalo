# Zerkalo Killer-App Plan — from the 2026-08-17 deep review

**Created:** 2026-08-17 · **Baseline:** current `main` (post v0.23.0 line, all of
`HEALTH-PLAN.md`/`UX-AUDIT-PLAN.md`/`REFACTOR-PLAN.md` closed) · **Status:** not started

This file tracks implementation of the gaps identified in the 2026-08-17 deep
review (killer-app / best-Typst-editor / best-word-processor / academic-default
assessment). Same role as `HEALTH-PLAN.md`: **read it before starting any
phase**, update status boxes as phases land, run `/clear` between phases if the
session is getting long — this file is the memory, not the conversation.

---

## Why this exists

The review found Zerkalo already unusually complete for a single-author Typst
editor (embedded compiler, LSP completions, SQLite library, git sync, CSL
styles, print imposition, Kartoteka vault integration). What's missing splits
into: things that make it feel like the best *Typst* editor (package
discovery, math/table authoring), things that make it a real *word processor*
replacement (review workflows, reference-manager import), and things that are
just infrastructure debt blocking the rest (dependency pinning, UI test
coverage, file size, i18n).

**Distribution (Flathub, Windows/macOS ports) is explicitly out of scope for
this plan** — Cal's call, tracked separately if ever revisited.

---

## Verification gate — run after EVERY phase

Same non-negotiable gate as `HEALTH-PLAN.md`/`REFACTOR-PLAN.md`:

```sh
cargo test                    # must not regress the current passing count
cargo clippy --all-targets -- -D warnings   # must stay clean
cargo build --release         # must succeed
./check-versions.sh           # version-consistency guard
```

Manual smoke test after any phase touching UI: app opens, a document loads,
live preview compiles, tab switching works, Ctrl+K palette opens. Phases that
add a new panel/dialog need their own manual click-through too (see Phase 2 —
this is exactly the gap that phase exists to close).

**Commit at the end of each phase, separately.** Never let two phases share a
commit. Phases with lettered sub-plans (this file has several) commit each
sub-phase separately, same convention as `HEALTH-PLAN.md` Phase 9.

---

## Phase 1 — Revisit `fond-bib`/`fond-vault` dependency pinning

**Status:** ☑ DONE (2026-08-17) — Kartoteka has release tags now; pinned.
**Risk:** low · **Effort:** trivial-to-small · **Depends on:** nothing

Kartoteka's git history now has real release tags (`v0.1.0` through `v0.5.0`,
plus interim `-devN` tags), unlike at `HEALTH-PLAN.md` Phase 4's time when
only `skrizhal-core` had one. Pinned `fond-bib`/`fond-vault` to `tag =
"v0.5.0"` (both crates still exist at `crates/fond-bib`, `crates/fond-vault`
at that tag), matching the `skrizhal-core` `tag`-based pattern already known
to survive the offline flatpak build (unlike the `rev`-based pin Phase 4
tried and reverted). `cargo update -p fond-bib -p fond-vault` re-resolved
cleanly against the new tag; `cargo build --release` succeeded against
v0.5.0's API with no call-site changes needed;
`packaging/cargo-sources.json` regenerated via
`~/Projects/kartoteka/flatpak-cargo-generator.py` (small diff, as expected).
Full verification gate green: 486 tests passed (up from 484 as of
`HEALTH-PLAN.md`'s 2026-08-12 baseline — the 2 extra are Zerkalo's own tests
added by other work since then, not from this phase; external crate tests
aren't pulled into `cargo test`'s count), clippy clean, version guard clean.

`HEALTH-PLAN.md` Phase 4 tried pinning these to a `rev`, which broke the
flatpak's offline cargo build; reverted to unpinned `git = "..."`. The
documented path forward was to wait for Kartoteka to cut a real release tag
(matching `skrizhal-core`'s working `tag = "v0.3.0"` pattern) and pin that way
instead.

**Fix:** check `~/Projects/kartoteka/Cargo.toml` / its git tags for whether a
release tag now exists. If yes: pin `fond-bib`/`fond-vault` via `tag = "..."`,
regenerate `packaging/cargo-sources.json`
(`~/Projects/kartoteka/flatpak-cargo-generator.py`, recipe in the root
`Projects/CLAUDE.md`), verify the offline flatpak dependency fetch still
works via `./dev-build.sh`. If no tag exists yet, leave as-is and don't
re-attempt a `rev` pin (already known to break the build) — just re-check at
the start of the next phase that touches `Cargo.lock`.

---

## Phase 2 — UI integration test infrastructure

**Status:** ☐ IN PROGRESS (2026-08-17) — investigation done, options 1 and a
D-Bus-actions alternative both ruled out; option 2 (AT-SPI) identified as the
viable path but the reusable test helper itself isn't built yet.

**Option 1 (Xvfb + a WM) tested and ruled out.** Installed `fluxbox` (apt),
ran it against an isolated `Xvfb :227`, launched a real headless Zerkalo
instance in it (isolated HOME/XDG/D-Bus per the established recipe), and
confirmed via screenshot that a real window renders. Pointer input (`xdotool
mousemove` + `click`) works exactly as `HEALTH-PLAN.md` Phase 9 found —
clicked "Get Started" on the Welcome dialog and it closed. **Keyboard
synthesis still does not register**, even with a WM present and explicit
`windowactivate`/`windowfocus` calls first: `xdotool key ctrl+k` produced no
visible effect, and a direct `XSetInputFocus` attempt (via
`xdotool windowfocus`) returned a hard X error (`BadMatch (invalid parameter
attributes)`). This confirms the WM-presence hypothesis from this phase's
original write-up does **not** hold — the blocker is elsewhere (almost
certainly GDK's synthetic-XSendEvent guard, as originally suspected, which a
WM being present doesn't route around).

**Alternative considered and ruled out: invoking commands via GApplication's
D-Bus `org.gtk.Actions` interface instead of any input synthesis at all** —
would sidestep the input-synthesis problem entirely if it worked. Ruled out
by inspection: `grep -rn "add_action\|ActionEntry\|SimpleAction" src/` finds
zero hits. Zerkalo's hamburger menu and Ctrl+K palette are hand-built widgets
dispatching to plain Rust closures (per `CLAUDE.md`'s documented "hand-built
popover, not `gio::Menu`" design), not `GAction`s, so nothing is exposed on
the app's D-Bus action-group interface to invoke this way.

**Option 2 (AT-SPI) — identified as the live path, not yet built out.**
The AT-SPI registry is confirmed present and already in use: Zerkalo's own
startup log shows it activating `org.a11y.atspi.Registry` and registering
with it (`SpiRegistry daemon is running with well-known name`) on every
launch, with no extra setup needed — GTK4 apps register with AT-SPI
automatically. `gi.repository.Atspi` (Python, via the already-installed
`gir1.2-atspi-2.0` package) can walk an app's accessible tree and invoke
actions on widgets directly, bypassing X input synthesis entirely — this is
architecturally the right fix, not just a workaround. **Not yet proven
end-to-end**: a first probe connected to the *real* desktop session's AT-SPI
bus instead of the isolated `dbus-run-session` one Zerkalo was launched in
(enumerated `gnome-shell`, `gsd-*`, etc. — the host's own apps, not Zerkalo),
because the probe process didn't share the inner session's
`DBUS_SESSION_BUS_ADDRESS`/AT-SPI bus address with the launched app. Fixing
that is straightforward (capture and export the inner bus address to the
probe process) but is real remaining work, not a two-minute fix — scoping it
as the concrete next step rather than closing this phase now.

**Unrelated finding surfaced during this investigation, flagged for Cal
separately from this plan's scope:** launching Zerkalo fresh (no existing
instance) **with a `.typ` file path as a CLI argument** appears to open no
window at all and exit near-instantly (clean exit code 0, no error, no
panic) — reproduced 3 times. Root cause appears structural: with only
`ApplicationFlags::HANDLES_OPEN` set (no `HANDLES_COMMAND_LINE`), GLib's
default command-line handling emits the `open` signal instead of `activate`
whenever file arguments are present, and `main.rs`'s `connect_open` handler
(`src/main.rs:137-147`) only acts on an *already-existing* window
(`if let Some(w) = borrow.as_ref()`) — there's no fallback to create one.
Launching with **no** file argument works perfectly (confirmed: window
opens, Library DB loads, full startup log). **Not investigated further or
fixed** — outside this plan's scope, and it needs verification against real
desktop launch paths (a `.desktop` file's `Exec=zerkalo %U` and GNOME
Files' double-click-to-open both go through D-Bus `Open()` activation, which
may or may not hit this same code path in practice) before concluding it's a
real user-facing bug rather than a headless-launch-only artifact. Worth its
own investigation session.

**Original phase text preserved below for the remaining Xvfb/WM background
and the option-3 fallback description, still accurate:**

**Risk:** low (investigation) → medium (if a working approach is adopted
project-wide) · **Effort:** medium · **Depends on:** nothing
**Risk:** low (investigation) → medium (if a working approach is adopted
project-wide) · **Effort:** medium · **Depends on:** nothing

Every phase below this one adds new dialogs/panels (package browser search,
math palette, table editor, comments UI, plugin surface). Right now none of
that gets automated click-path coverage — `HEALTH-PLAN.md` Phases 2 and 8 both
hit the same wall: this Xvfb setup has no window manager, so synthetic
keyboard input (`xdotool key`, `XTestFakeKeyEvent`) doesn't register, even
though pointer clicks do (confirmed working in Phase 9's manual verification).
Without fixing this, every UI-heavy phase below inherits the same
"static-checks-only + ask Cal to click it once" pattern, which doesn't scale
to the number of new surfaces this plan adds.

**Investigate, in order of likely payoff:**
1. Run Xvfb with a minimal window manager (`fluxbox` or `openbox`) instead of
   bare Xvfb — the synthetic-XSendEvent guard that blocks `xdotool`/XTEST
   keyboard input may specifically need a WM present to accept focus/input
   correctly. Cheapest thing to try first.
2. If that doesn't resolve it, look at GTK4's own accessibility tree
   (AT-SPI) as a driver instead of X input synthesis — `dogtail` or raw
   `pyatspi` can invoke actions on widgets directly without needing real
   input events to land, sidestepping the X focus problem entirely.
3. If neither works reliably, document that conclusively (so future phases
   stop re-discovering it) and fall back to a lighter-weight pattern: pure
   Rust unit tests for any new panel's non-GTK logic (already the existing
   convention — e.g. `history_panel.rs`'s `git_log_for_file` tests), plus a
   standing manual smoke-test checklist appended to this file that Cal runs
   once per release rather than once per phase.

**Fix (if 1 or 2 succeeds):** wire the working approach into a reusable
test helper so later phases (4, 8, 9, 11, 13) can add real click-path tests
instead of only static checks + manual verification.

---

## Phase 3 — Confirm/complete `REFACTOR-PLAN.md` Phase 6 (compiler off main thread)

**Status:** ☑ DONE (2026-08-17) — investigated, no action needed;
`REFACTOR-PLAN.md`'s own text is already accurate and current.

Read `REFACTOR-PLAN.md` Phase 6 directly rather than trusting
`HEALTH-PLAN.md`'s cross-reference summary. It's already marked "HALF DONE,
other half deliberately deferred (checked 2026-08-17)" — the same day as
this session, so it was independently re-verified very recently, not stale.
State: `pdftotext`/`ensure_pdf_path` (the actually slow, hot-path work) is
async, done under `HEALTH-PLAN.md` Phase 8. `git_log_for_file`/
`git_diff_for_commit` in `history_panel.rs` remain synchronous, by deliberate
choice — History opens behind an explicit modal action (menu/palette), not
on every keystroke or the live-preview path, so a brief block there is much
lower cost than the `pdftotext` case was, and nothing has surfaced a felt
slowdown since. `REFACTOR-PLAN.md` itself says not to async it
speculatively.

**Conclusion for this plan's purposes:** nothing here blocks Phases 8/9
(math palette, table editor) — neither adds interactive load on the same
synchronous `git log` path, so the "benefits from Phase 3" note on those
phases can be read as already satisfied. No code change made.

---

**Original phase text preserved below:**

**Status:** ☐ not started
**Risk:** medium · **Effort:** medium · **Depends on:** nothing, but should
land before Phases 8/9 (math palette, table editor) add more editor-side
interactivity on top of the same thread.

The 2026-08-17 cross-reference note in `HEALTH-PLAN.md` flagged this as
"partly done... not started" — re-verify current status against
`REFACTOR-PLAN.md`'s own text first (don't trust the summary, per this
project's own pattern of stale-summary bugs). `compiler.rs` runs in a Tokio
async context per `CLAUDE.md`'s Compiler/Typst rule, but confirm live-preview
recompiles and any newly-interactive editing surfaces this plan adds don't
block the GTK main thread on large documents.

**Fix:** whatever `REFACTOR-PLAN.md` Phase 6 specifies, scoped to still being
correct against current `editor_pane.rs`/`compiler.rs`. If the plan's own
text is stale, update it in the same commit rather than silently diverging
(matches this project's established habit of fixing stale plan text as part
of picking a phase back up).

---

## Phase 4 — Typst Universe package browser: search + install

**Status:** ☐ not started
**Risk:** low-medium · **Effort:** medium · **Depends on:** nothing

`src/ui/package_browser.rs` (205 lines) currently only lists packages already
in the local Typst cache (`scan_local_packages`) — there's no way to discover
or install a package before first using it in a document (which triggers
auto-download today). This is the single item where Zerkalo trails Typst's
own web app most directly.

**Design questions to resolve first:** where does the package index come
from (Typst Universe has a public package registry — check for an existing
JSON/API endpoint used by `typst-kit`/the `typst` CLI itself, since Zerkalo
already embeds `typst-kit`); does search need to hit the network live or can
it ship a periodically-refreshed local index; UI — extend the existing
`package_browser.rs` list with a search box + "Install" button per result,
reusing `typst-kit`'s existing package-fetch machinery (it already downloads
packages on first use per the README) rather than writing a second HTTP
client.

**Fix:** add search-by-name/description over the Typst Universe index, an
Install action that pre-fetches into the local cache without requiring a
document to reference the package first, and a visible state (installed vs.
not) per list row. Add unit tests for the index-parsing/search-matching logic
regardless of Phase 2's outcome (pure logic, no GTK needed).

---

## Phase 5 — Bundle `tinymist` (remove optional-completions fallback)

**Status:** ☐ not started
**Risk:** low · **Effort:** small-medium (mostly packaging) · **Depends on:**
nothing

Per the README, LSP completions silently degrade to built-in snippets only
when `tinymist` isn't found on the system — for a project that already embeds
its own Typst compiler specifically so nothing external is required, this is
the one remaining "works less well and the user doesn't know why" path. The
`.deb`/RPM packaging metadata in `Cargo.toml` already ships a
`packaging/tinymist` binary asset, so this is partly already solved for
non-flatpak installs — check whether the flatpak manifest
(`packaging/io.github.calstfrancis.Zerkalo.yml`) bundles it the same way.

**Fix:** if the flatpak doesn't already vendor a `tinymist` binary/build
step, add one (matching how the embedded Typst compiler itself is vendored,
not a runtime dependency). If it does already, this phase is just closing the
loop: verify `lsp.rs`'s fallback path is unreachable in the flatpak build and
adjust the README/in-app messaging (`?` help panel, status bar description
per the README's "admits when only built-in snippets are available" note) to
stop describing tinymist as optional there.

---

## Phase 6 — Manuscript-wide outline & cross-file word count rollup

**Status:** ☐ not started
**Risk:** low-medium · **Effort:** medium · **Depends on:** nothing

`src/ui/outline_panel.rs` (604 lines) and `src/ui/dep_graph.rs` (392 lines)
already give a per-file heading tree and a visual `#include`/`#import` graph,
but nothing merges them into a single "whole manuscript" view — a thesis
with 8 chapter files has no rolled-up outline or aggregate word count across
files, only per-file numbers in the library/status bar.

**Fix:** using the compilation-root detection already in `project.rs`/
`project_model.rs` and the include/import graph `dep_graph.rs` already
builds, walk the graph from the root and produce a merged heading tree
(prefixed by source file) plus a summed prose word count. Surface it as a
mode/tab on the existing outline panel rather than a new panel, consistent
with `CLAUDE.md`'s "new UI panels go in `src/ui/`... registered in
`src/ui/mod.rs`" only when something genuinely new is needed. Recompute on
the same debounce as live preview, not on every keystroke (per `CLAUDE.md`'s
Compiler/Typst rule).

---

## Phase 7 — Zotero library import (one-time, not live sync)

**Status:** ☐ not started
**Risk:** medium · **Effort:** medium · **Depends on:** nothing

Zerkalo currently accepts BibTeX, Hayagriva YAML, or a Kartoteka vault as a
bibliography source — no path exists for the far more common case of an
existing Zotero library. `fond-bib`'s "acquire" feature (DOI/ISBN network
lookup) was deliberately excluded from Zerkalo's dependency per the
`Cargo.toml` comment — this phase is different in kind: a one-time local
import, not a live network-backed lookup service, so it doesn't reopen that
decision.

**Design questions to resolve first:** Zotero exposes a local SQLite database
(`zotero.sqlite`) and also supports exporting a `.bib`/CSL-JSON file — decide
whether to read the local SQLite directly (richer: collections, tags,
attachments) or just accept a Zotero-exported `.bib`/`.json` file (much
simpler, reuses `biblatex`/`hayagriva` parsing already in `Cargo.toml`, no
new SQLite schema to reverse-engineer or keep in sync with Zotero's own
schema changes). **Recommend starting with the export-file path** — far lower
maintenance burden, and Zotero's own "Export Library" → BibTeX is a two-click
action for the user.

**Fix:** add an import entry point (Citations panel `+` button, matching the
existing "start a new bibliography" affordance) that accepts a Zotero-
exported `.bib`/CSL-JSON file and folds it into the document's bibliography
source the same way an existing `.bib` file is handled today.

---

## Phase 8 — Visual math/equation palette

**Status:** ☐ not started
**Risk:** medium · **Effort:** medium-large · **Depends on:** benefits from
Phase 3 (main-thread compiler work) landing first, not hard-blocked by it.

No palette, symbol picker, or assisted authoring for Typst math mode exists
today — Symbol Insert (`file_tree`-adjacent sidebar panel per the README)
only covers Cyrillic/Greek/Hebrew/Sanskrit scripts, not math notation.

**Design questions to resolve first:** scope — a symbol/operator palette
(click to insert `\alpha`-style Typst math names) is much smaller than a
visual equation builder (structured fraction/superscript/matrix editing that
emits Typst math syntax). Recommend shipping the palette first as its own
milestone within this phase, treating a full structured builder as a
stretch goal only if the palette alone doesn't close the gap in practice.

**Fix:** new sidebar panel or popup (pattern-match `bib_popup.rs`'s
inline-popup approach, since math symbol insertion is a similar
"trigger-character → searchable list → insert" flow as citation
autocomplete) covering common Typst math symbols/operators with live
rendered preview per entry (reuse the embedded Typst compiler to render small
snippets, same mechanism the main preview pane already uses).

---

## Phase 9 — Visual table editor

**Status:** ☐ not started
**Risk:** medium-high · **Effort:** large · **Depends on:** benefits from
Phase 3, not hard-blocked by it.

Typst tables are pure markup today; no UI exists to insert/resize/merge
cells and have Zerkalo emit the corresponding `#table(...)` call.

**Design questions to resolve first:** this is the largest single UI
addition in this plan — needs its own design pass before implementation
starts (matching `HEALTH-PLAN.md` Phase 9's convention of a design-first
sub-plan for large risky work). Key decisions: is the visual editor a
separate dialog that generates code once and hands control back to the text
editor (simpler, lower risk — closer to the existing Template dialog's
"generate a form, emit code" model already proven in
`template_dialog/generate.rs`), or a live in-place overlay on top of the
rendered preview (much higher risk, likely GTK `Overlay` + hit-testing
against rendered PDF coordinates, no existing precedent in the codebase to
build from). **Recommend the dialog/generator model** — it reuses an
already-proven pattern instead of inventing preview-overlay interaction from
scratch.

**Fix:** once scoped, a new dialog (row/column count, per-cell content,
merge/span controls, header-row toggle, alignment) that generates a
`#table(...)` block and inserts it at the cursor — no live-editing of an
already-inserted table's structure in this phase; that's a natural follow-up
once the generator model is proven.

---

## Phase 10 — i18n infrastructure

**Status:** ☐ not started
**Risk:** low (infra) → high (translation coverage over time) · **Effort:**
large · **Depends on:** nothing technically, but doing this *after* Phases
4–9 land means wrapping more new strings later rather than fewer now — see
note below.

The UI is English-only (spell-check already supports multiple *dictionary*
languages, which is a separate thing from UI string translation). Flagged as
a deferred finding in `HEALTH-PLAN.md`, never picked back up.

**Sequencing note:** this phase is placed after the feature phases
deliberately — retrofitting i18n into a large existing string surface is a
mechanical, one-time cost regardless of when it happens, whereas blocking 5
feature phases on i18n infrastructure landing first would slow down higher
user-facing-value work for a benefit (translated UI) that has no
demonstrated demand yet. Revisit this ordering if Cal decides discoverability
work (which this plan excludes) surfaces non-English-speaking users sooner
than expected.

**Design questions to resolve first:** framework choice — `fluent` (Mozilla's
project, has a mature Rust crate `fluent-rs`, handles pluralization well) vs.
`gettext`-style (`.po` files, more translator tooling exists but weaker Rust
ecosystem support). Recommend `fluent-rs`: better Rust-native ergonomics, no
external `msgfmt` build step needed.

**Fix:** add the chosen crate, establish the string-extraction convention
(likely a macro/helper wrapping every user-facing `&str` literal), migrate
one representative panel first as a proof of pattern (recommend
`settings_dialog.rs` — self-contained, already fully accessibility-audited
in `HEALTH-PLAN.md` Phase 6, good size to validate the approach without
being either trivial or `editor_pane.rs`-sized), then sweep the rest
file-by-file as separate commits. Actual translation *content* (beyond
English) is out of scope for this phase — infrastructure and English as the
first/reference locale only.

---

## Phase 11 — Comments / suggested-edits layer

**Status:** ☐ not started
**Risk:** high · **Effort:** large · **Depends on:** nothing, but Phase 12
depends on this.

The single most-requested word-processor-parity feature missing today.
Snapshots/File History give a *developer's* diff view of change history;
nothing gives an *advisor reading a draft* model — inline comment threads
anchored to a text range, or suggested edits that can be accepted/rejected
without directly editing the document.

**Design questions to resolve first (design-first, like Phase 9):**
- **Storage model** — recommend a sidecar file per document (matching the
  existing `.plan` scratchpad and `.zerkalo.toml` settings sidecar
  conventions already established in `project_model.rs`/`cv_mode.rs`), not
  inline Typst-source markers, so comments never risk corrupting compiled
  output and survive round-trips through export.
- **Anchoring** — comments need to survive the document being edited above/
  below the commented range. Recommend anchoring by a stable range
  (line/column at creation time) plus a fuzzy-match fallback (surrounding
  text snippet) to re-locate the anchor if line numbers shift, rather than
  requiring exact-position tracking through every edit (which would need
  hooking every `TextBuffer` mutation — high complexity, high crash risk
  given the project's own documented `Rc<RefCell<>>` re-entrant-borrow
  fragility pattern that `REFACTOR-PLAN.md` already flags as a known crash
  class to be careful around).
- **Scope for v1** — recommend comments-only (threaded, resolvable) before
  suggested-edits (propose-a-replacement, accept/reject). Suggested-edits is
  strictly harder (needs a diff/patch model, not just an anchored note) and
  comments alone already cover the "advisor reviews a draft" workflow this
  phase exists for.

**Fix:** new sidebar panel (comments list, click to jump) + inline gutter
markers in the editor (pattern-match the existing spell-check wavy-underline
and LSP-diagnostic underline mechanisms in `editor_pane.rs` for the
visual-marker part), sidecar persistence module (pattern-match
`writing_log.rs`/`import_log.rs` for the "simple structured sidecar" shape),
resolve/reopen state per comment.

---

## Phase 12 — DOCX/ODT round-trip preserving track changes

**Status:** ☐ not started
**Risk:** high · **Effort:** large · **Depends on:** Phase 11 (needs an
internal comment/suggestion model to map Word's revision marks onto).

Today's DOCX/ODT export/import goes through `pandoc` one-directionally —
someone who needs to submit a Word-format manuscript, or receives a
track-changes-marked DOCX back from a journal or co-author, has no way to
bring those edits into Zerkalo.

**Design questions to resolve first:** DOCX track-changes are stored as
`<w:ins>`/`<w:del>` runs in the OOXML — `doc_import/docx.rs` (435 lines)
already parses DOCX's ZIP/XML structure directly (not via pandoc, per the
README's "Word... read by Zerkalo itself" note), so this extends existing,
already-proven parsing code rather than starting fresh. Decide whether to
map Word revisions onto Phase 11's comment/suggestion model directly (a
Word insertion becomes a "suggested addition," a Word deletion becomes a
"suggested removal") — this is why the dependency on Phase 11 is hard, not
soft: without that model already existing, there's nowhere to put the
imported revisions that isn't a one-off special case.

**Fix:** extend `doc_import/docx.rs` to parse `<w:ins>`/`<w:del>` into
Phase 11's suggestion model on import; extend the DOCX export path to emit
the same revision-mark XML from unresolved suggestions/comments on export.
ODT's equivalent (`<text:change>` elements) follows the same shape in
`doc_import/odt.rs`. Treat DOCX as the primary target and ODT as a
follow-up once the model is proven, rather than building both at once.

---

## Phase 13 — Plugin/extension API

**Status:** ☐ not started
**Risk:** very high · **Effort:** very large · **Depends on:** nothing
technically, but should come after the panel patterns established in
Phases 4/8/9/11 so the extension surface is designed against real, varied
examples of what a plugin might need to do, not guessed at in the abstract.

No extension points exist today. This is an architectural bet, not a
feature — recommend treating this phase as **design/prototype only** until
Cal explicitly commits to it as a real initiative, same posture
`HEALTH-PLAN.md` Phase 9 took toward its highest-risk work (stop and check in
before the large mechanical push, don't launch straight into it).

**Design questions to resolve first:** what's actually extensible — new
citation styles and templates already work today via plain files
(`~/.local/share/zerkalo/templates/`, CSL styles) without needing a plugin
API at all, so the real gap is *behavioral* extension (custom panels,
commands, import/export formats). Language/runtime choice for plugins (WASM
sandboxing is the safer default for a desktop app users install
system-wide, vs. a scripting language like Lua/Rhai embedded directly) is
the single highest-leverage decision here and should not be made without a
dedicated design doc, not just this plan entry.

**Fix:** not specified at this level of detail deliberately — this phase's
actual first deliverable is a short design doc (new file, e.g.
`docs/PLUGIN-DESIGN.md`) covering the above, reviewed with Cal, before any
code lands. Everything past that point gets planned once the design doc
exists.

---

## Phase 14 — Real-time collaboration

**Status:** ☐ not started
**Risk:** very high · **Effort:** very large · **Depends on:** nothing
technically, but is the largest bet in this plan and should land last.

No live co-editing exists; git sync is version control (commit/push), not
simultaneous multi-user editing. This is the single biggest architectural
undertaking in this plan — likely CRDT-based (e.g. `yrs`, the Rust port of
Yjs) given GTK's `TextBuffer` needs a local-first model that can merge
remote edits without a central lock, plus a sync transport (self-hosted
relay server, or peer-to-peer) that doesn't exist anywhere in Zerkalo's
current architecture (which is entirely local-file + git today).

**Recommend treating this the same way as Phase 13:** first deliverable is a
design doc, not code. Key open question before any implementation: does this
need a always-on relay/server component (a real infrastructure commitment,
unlike everything else in this codebase which is a local desktop app plus
git), or can it work peer-to-peer for the realistic "advisor + one student"
or "two co-authors" case without one. **This phase should not start until
Phases 1–13 are substantially through** — it's the least-validated,
highest-cost item in the plan, and the review's own framing treats it as a
bet worth having on a roadmap, not a near-term commitment.

---

## Deliberately excluded from this plan

- **Flathub submission, Windows/macOS ports** — explicitly out of scope per
  Cal's instruction when this plan was created. The 2026-08-17 review still
  named Flathub as the single highest-leverage discoverability fix; revisit
  as its own initiative whenever Cal decides to take it on.
- **Live DOI/ISBN network lookup for citations** — `Cargo.toml` already
  documents this as a deliberate exclusion (`fond-bib`'s "acquire" feature
  left out). Phase 7's Zotero import is a one-time local-file import, not a
  reopening of that decision.

---

## How to resume this plan after a context reset

1. Read this file top to bottom.
2. Find the first `☐ not started` phase with satisfied dependencies.
3. Read the phase's own section fully before touching code — several phases
   (2, 9, 11, 13, 14) explicitly call for a design/investigation step before
   any implementation.
4. Update the status box to `☐ in progress` before starting, `☑ DONE (date)`
   when the verification gate passes and the commit lands.
