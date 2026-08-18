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

### Follow-up (2026-08-17, same day) — the pin broke `./dev-build.sh` too, fixed

Cal ran `./dev-build.sh` for `v0.24.0-dev4` and hit an offline-build failure:
`can't checkout from 'https://github.com/calstfrancis/kartoteka': you are in
the offline mode (--offline)`. Root cause: `packaging/io.github.calstfrancis.Zerkalo.yml`
**hand-duplicates the cargo source-replacement config inline**, in two
`printf ... > cargo/config` build-commands (`zerkalo-deps` and `zerkalo`
modules) — this is a **second, independent copy** of the same information
`packaging/cargo-sources.json` carries, and nothing keeps them in sync
automatically. Regenerating `cargo-sources.json` earlier in this phase (via
`flatpak-cargo-generator.py`, as documented) updated the right file, but the
manifest's own hardcoded `printf` strings still had kartoteka's `[source...]`
block with no `tag = "v0.5.0"` line — cargo therefore tried to check out
kartoteka's default branch during the build's offline step, which fails by
definition.

Fixed by adding the missing `tag = "v0.5.0"\n` to both `printf` occurrences,
matching the pattern the `skrizhal` block already had correct (proving this
duplication is old and was correctly updated for skrizhal's own pin at some
point — just missed for kartoteka this time, and easy to miss again for any
future pin change to either dependency).

**Not fixed, worth a future cleanup:** the duplication itself. The manifest
could read `cargo/config`'s contents from `cargo-sources.json`'s own inline
entry instead of re-typing it by hand in two places — would make this whole
bug class structurally impossible instead of just fixed this once. Left
alone here to keep this fix minimal and unblock Cal's build; worth revisiting
if this bites a third time.

No code/test changes — YAML-only fix, so the standard `cargo test`/clippy
gate doesn't apply. Cal re-running `./dev-build.sh v0.24.0-dev4` is the only
real verification, per this project's own "fix and rerun, don't retag" rule
for a build that fails partway.

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

**Status:** ☑ DONE (2026-08-17) — implemented, and two real pre-existing bugs
found and fixed along the way (see below).

**Index source, decided by investigation:** `https://packages.typst.org/preview/index.json`
is a real, public, unauthenticated endpoint (confirmed live: `curl` returned
2.1MB of JSON, one entry per package *version*) — this is what
`packages.typst.org` itself serves and is the closest thing to an official
index. New module `src/typst_universe.rs`: fetches it, folds down to the
latest version per package name (proper numeric version comparison, not
lexicographic — `0.9.0 < 0.10.0`), caches the raw response to
`$XDG_CACHE_HOME/zerkalo/typst-universe-index.json`. Cache is used for
instant first paint (`load_cached_only`); a background refresh runs whenever
the cache is missing or older than 24h (`cache_is_fresh`), plus a manual
refresh button. 5 unit tests (latest-version folding, alphabetical sort,
numeric version ordering, malformed-JSON rejection, missing-description
handling).

**Install action:** `compiler::install_package()` (new `pub fn`) reuses the
exact same `PackageStorage`/`prepare_package` the compiler itself resolves
`#import`s through — parses `"@preview/name:version"` via
`typst::syntax::package::PackageSpec`'s existing `FromStr`, so an install
here is immediately visible to the next compile, no separate download path
to maintain.

**UI (`package_browser.rs`, substantially rewritten):** merges locally-
installed packages with the Universe index into one filtered list, keyed by
`(namespace, name)`. Each row shows installed/available version(s) and the
Universe description; an Install button (spinner while in flight, via the
same thread+`mpsc::sync_channel`+`glib::timeout_add_local` pattern
`export_dialog.rs` already uses) for anything not yet installed, an Insert
button (unchanged behavior) for anything that is. Search filters across
name, namespace, and description.

**Two real pre-existing bugs found and fixed, not just my new code:**

1. **The package browser's widget was never attached to the UI at all.**
   Discovered while trying to manually verify the new search/install feature
   headlessly — the panel simply didn't appear anywhere. Traced it: `
   PackageBrowser::new()` was constructed and wired (`set_on_insert`) in
   `panels.rs`'s `build_panels()`, but never added to the `Panels` struct
   that function returns, so it was silently dropped at the end of the
   function — never reaching `editor_extras.rs`'s sidebar assembly the way
   `outline_panel`/`citation_panel` do. The struct field also had
   `#[allow(dead_code)]` on it already, which should have been a tell.
   Fixed by threading it through properly: added `package_browser` to the
   `Panels` struct (`panels.rs`) and its return, to the destructure in
   `mod.rs`, to `SidebarToolbarCtx` (`editor_extras.rs`), and appended
   `ctx.package_browser.widget()` (new accessor, field's `dead_code` allow
   removed since it's now genuinely used) to the sidebar's `left_box` after
   Citations, matching the existing Outline/Citations pattern exactly.
   **Verified visually** (headless Xvfb screenshot): the "Packages" section
   now renders in the sidebar with live Typst Universe entries.
   **Related finding, not fixed (out of scope for this phase):**
   `dep_graph` and `ref_manager` appear to have the *same* problem —
   `dep_graph` has no `.widget()` method at all (cannot be shown by any
   means as it stands), and `ref_manager.widget()` exists but is never
   called anywhere in `src/`. Both are threaded through `Panels` and get
   `.refresh()`/data calls, but neither's widget reaches the window. This
   contradicts the README's description of the Dependency graph as a real
   "opt-in view" — worth its own investigation session, matching how
   `HEALTH-PLAN.md` Phase 2 found and fixed the identical situation for
   `history_panel.rs`. Not touched here to keep this phase's diff scoped to
   what search/install actually needed.

2. **`scan_local_packages` was scanning the wrong directory**, independent
   of anything above. It read `glib::user_data_dir().join("typst/packages")`
   (XDG *data* dir), but `compiler::package_cache_root()` — where the
   compiler actually downloads `@preview` packages, matching `typst-cli`'s
   own convention, per that function's own doc comment — resolves to
   `$XDG_CACHE_HOME/typst/packages` (XDG *cache* dir). These are different
   directories, so **no package the compiler had ever downloaded, implicitly
   or via this phase's new explicit Install button, was ever detected as
   "installed."** Confirmed concretely: installed `@preview/a2c-nums` via the
   new Install button, watched it land in `.cache/typst/packages/...` on
   disk, and watched the row *not* flip to "installed" until this was fixed.
   Fixed by making `compiler::package_cache_root()` `pub` and having
   `scan_local_packages` call it directly instead of guessing its own path —
   single source of truth, can't drift again. **Re-verified after the fix**:
   same install flow, row correctly flips to "v0.0.1 installed" with the
   Insert button.

**Full verification gate green**: 491 tests (5 new), clippy clean, version
guard clean, plus the headless manual verification above (real network
fetch, real install, real UI render, confirmed via screenshot at each step).

### Follow-up (2026-08-17, same day) — both flagged issues resolved

Cal asked to prep a dev build then handle the two issues this phase flagged
but didn't fix. Dev build `v0.24.0-dev4` prepped first (Cargo.toml, CHANGELOG,
`welcome_window.rs`, signed tag) — see repo history, not duplicated here.

**1. `dep_graph`/`ref_manager` reachability — fixed, same pattern as
`package_browser` above.** Investigation confirmed both had the identical
problem: `#[allow(dead_code)]` on the `widget` field, no way to display
either. Worse than initially framed for `RefManager` specifically: its
per-entry "Rename citation key" button is the *only* UI that can ever
trigger `set_on_rename`'s project-wide rename logic (confirmed by grep —
`citation_panel.rs` has no rename UI of its own) — so the metainfo's
"project-wide citation key rename" feature had literally no way to be
invoked, not just a missing convenience view.

Fixed by adding `.widget()` to `DepGraph` (didn't have one at all;
`RefManager` already did) and wiring both into the hamburger menu —
"Reference Manager…" and "Dependency Graph…" rows in the "Current document"
cluster, each opening a small `adw::Window` via new
`show_ref_manager_window`/`show_dep_graph_window` functions
(`app_window/mod.rs`), following `show_file_history_window`'s established
shape. **One real difference from that precedent, worth flagging for future
reuse of this pattern:** `HistoryPanel` sidesteps any reparenting question by
constructing a *fresh* instance on every open (history is cheap to
re-query). `DepGraph`/`RefManager` can't do that — they're long-lived
singletons kept continuously in sync via callbacks wired once at startup
(`load_bib`, `.refresh()` on every compile); constructing a new instance per
open would show a stale, never-updated empty widget. So these two windows
reuse the *same* instance's widget every open, guarded with
`if widget.parent().is_some() { widget.unparent(); }` before re-adding it to
the new window's content box — required because a GTK widget can only have
one parent, and after the first open-then-close cycle it still has one
(the previous window's now-destroyed content box).

Not added to the "insensitive with no active document" `document_rows`
group — both operate on the project/bibliography, not the open file, so
gating them the same way would be wrong. Not added to the Ctrl+K palette in
this pass (menu-only is sufficient to fix "completely unreachable"; palette
parity is a low-cost follow-up, not required for the fix).

**Verification:** full gate green (491 tests, clippy clean, version guard
clean). **Runtime click-path could not be screenshot-verified** — a newly
discovered headless-testing limitation, distinct from Phase 2's keyboard-
input finding: plain `Button` clicks work fine here (confirmed against the
"Template" button and the Welcome dialog's "Get Started"/"×", both mid-
session), but the hamburger's `Popover`-attached `MenuButton` did not open
via synthetic `xdotool click` in this Xvfb setup, with or without a `fluxbox`
WM present, across multiple coordinate/timing variations. Given the
structural fix compiles clean and follows an already-proven, shipped pattern
(`show_file_history_window`) exactly except for the reparenting guard (which
follows GTK4's documented widget-reparenting contract directly), this is
assessed as low-risk but **genuinely unverified at runtime**. **Cal: worth
clicking ☰ → "Reference Manager…" and ☰ → "Dependency Graph…" once** to
confirm both windows open and show real content (a loaded bibliography /
the project's file graph) — the one thing static checks and the click
limitation above couldn't cover this session.

**2. File-arg launch bug — confirmed as a real bug and fixed**, not just a
headless artifact as Phase 2 left it. Root cause exactly as suspected:
`ApplicationFlags::HANDLES_OPEN` with no `HANDLES_COMMAND_LINE` makes GLib
fire `open` instead of `activate` whenever there are file arguments, and
`main.rs`'s `connect_open` only ever acted on an *already-existing* window.
Fixed by extracting the shared "prune stale state, build `AppWindow`,
present" sequence into a `new_window()` helper, then having `connect_open`
call it when `shared_window` is still `None` (first launch with a file) —
opening the first file via `open_initial_file` (matches `connect_activate`'s
own call, including its create-missing-file behavior) and any additional
files via `open_external`. Already-running-instance behavior (the original,
working half of `connect_open`) is unchanged.

**Verified by direct reproduction and re-test**, not just code review: the
original bug reproduced 4 independent times across this session (headless,
fresh `HOME` each time, `zerkalo somefile.typ` — process exited cleanly with
no window, no error). After the fix, the identical launch command against a
fresh `HOME` opened a real window with `main.typ` loaded and rendering
correctly (confirmed via screenshot — file tree import, heading, and a
citation reference all visible and compiling). This one *is* fully runtime-
verified, unlike finding 1 above — it only needed a plain process launch,
not a click on anything.

**Not yet verified: real desktop launch paths** (double-click a `.typ` file
in a file manager, `xdg-open`) — both should exercise the exact same
`connect_open` code path per GApplication's D-Bus `Open()` semantics, but
that's inference from the API contract, not an observed test on a real
desktop session. Worth a real check next time either is used.

---

**Original phase text preserved below:**

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

**Status:** ☑ DONE (2026-08-18) — investigated, already fully done; no
action needed. Sixth phase across `HEALTH-PLAN.md`/this plan where the
original review's premise didn't survive checking against current code —
worth continued skepticism toward any remaining un-reverified review claims.

Checked all three things the phase's own text called for before touching
anything:
- **Flatpak already vendors a real tinymist binary.**
  `packaging/io.github.calstfrancis.Zerkalo.yml` downloads
  `tinymist-x86_64-unknown-linux-gnu.tar.gz` from tinymist's `v0.14.18`
  GitHub release and installs it to `/app/lib/zerkalo/tinymist` — confirmed
  live (`curl -IL`: 200, 29.7 MB, resolves through GitHub's release-asset
  CDN correctly).
- **Version match confirmed.** `Cargo.toml` embeds `typst = "0.14"`;
  tinymist's own versioning tracks Typst's, and `0.14.18` matches — the
  right pairing, not a stale pin.
- **`src/lsp.rs`'s `tinymist_command()` already checks the flatpak path
  first.** `["/app/lib/zerkalo/tinymist", "/usr/lib/zerkalo/tinymist"]` are
  tried before falling back to bare `tinymist` on `PATH` — so inside the
  actual shipping flatpak, completions never hit the "optional" fallback
  path at all.

**What the original review actually observed** (headless dev-build testing,
via `target/release/zerkalo` run directly, not the flatpak) — "tinymist not
found — LSP completions disabled" — is real but reflects the **source-build
gap**, not a flatpak gap: no flatpak sandbox means no `/app/lib/zerkalo/`,
and this dev machine has no system `tinymist` on `PATH` either. That's
exactly what the README's own wording already scopes correctly:
`cargo install tinymist` **"for source builds"** — it isn't claiming
flatpak needs this. `install.sh` also already offers to install it
interactively for source builds. Nothing here is misleading users about the
flatpak experience, which is the copy that actually matters for the vast
majority of installs (distribution is flatpak-only per the root
`Projects/CLAUDE.md`).

**Not investigated further, out of scope for this phase:** `Cargo.toml`'s
`.deb`/`.rpm` packaging metadata references `packaging/tinymist` as a
binary asset to install, but no such file currently exists in
`packaging/`. Per the root `Projects/CLAUDE.md`, `.deb`/`.rpm` aren't part
of the actual release pipeline (flatpak-only distribution), so this is
either dead packaging config or a gap in an unused secondary path — not
worth chasing down as part of a phase about the *shipping* app's tinymist
experience.

No code change made.

---

**Original phase text preserved below:**

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

**Status:** ☑ DONE (2026-08-17) — built, and it turned out to build on
existing, previously-unwired scaffolding, not from scratch.

**Discovery before writing anything:** `outline_panel.rs` already had a
`pub fn update_project(&self, files: Vec<(PathBuf, String)>)` — multi-file
heading merge, per-section word counts, file-name-as-tooltip in multi-file
mode, all fully implemented — but **`grep -rn "update_project" src/`
returned zero call sites.** Exactly the same "built, wired for data, never
reachable" shape as `package_browser`/`dep_graph`/`ref_manager` earlier in
this session, just one level more finished (here the rendering logic itself
was complete; only the *trigger* was missing). Reading `repopulate()`
confirmed it already does everything this phase's spec asked for at the
per-heading level — the only genuinely new work was: gathering files in the
right order, a UI toggle to trigger it, and a *total* word-count rollup
(the existing code only showed a heading count, not a summed word count).

**What got built:**
- `project.rs`: new `pub(crate) fn parse_typ_imports` (extracted verbatim
  from `dep_graph.rs`'s private `parse_imports`, now shared — `dep_graph.rs`
  calls it too, removing the duplicate regex) and new
  `pub fn manuscript_files(root, project_root) -> Vec<(PathBuf, String)>`,
  a breadth-first walk of the `#include`/`#import` graph returning file
  content in visitation order (root first) — exactly what
  `update_project`'s doc comment always said it wanted. Cycle-safe (visited
  set), tolerant of a broken/missing include (skips, doesn't blank the
  view). 4 new unit tests: order, cycle safety, missing-include tolerance,
  single-file passthrough.
- `outline_panel.rs`: a folder-icon toggle button in the outline header,
  `set_on_project_mode(f: impl Fn(bool))` callback (same narrow-callback
  idiom every other panel in this codebase already uses), and a total-word
  rollup added to the count label — `"· 3 headings · 80 words"` in
  multi-file mode vs. the existing `"· 1 heading"` in single-file mode.
- `app_window/mod.rs`: wired the toggle — on, gather via
  `manuscript_files` using the same root-resolution the compile path
  already uses (`configured_root` if project mode is set, else the active
  file) and call `update_project`; off, revert to `update` for just the
  active file. The debounced-change handler and the tab-switch handler both
  respect an `outline_manuscript_mode` flag — tab-switching *while* in
  manuscript mode intentionally does **not** re-populate (the file set
  didn't change, and `row_positions` already spans every file, so
  cursor-following/jump-to-heading keeps working across tabs without a
  refresh). Needed to move `configured_root`/`proj_mode_active`'s
  declarations earlier in `new()` (pure reordering, nothing behavioral) so
  the debounced-change handler could see them.

**Full verification gate green**: 495 tests (4 new), clippy clean, version
guard clean. **Runtime-verified end-to-end** (unlike some of this session's
other UI fixes — plain `Button`/`ToggleButton` clicks work fine headlessly,
this isn't a `Popover` like the hamburger menu): built a real 3-file test
project (`main.typ` including `ch1.typ`/`ch2.typ`), confirmed single-file
mode shows only the active file's heading, toggled on and confirmed
"3 headings · 80 words" with per-file entries (28+28+24=80, correct),
clicked a heading from a *different* file and confirmed it switched tabs
and jumped to the right line while manuscript mode stayed on (didn't
collapse back to single-file), then toggled off and confirmed it reverted
cleanly to showing just the active file. Screenshots at every step.

README and CHANGELOG updated in the same session per the root
`Projects/CLAUDE.md` documentation policy; `welcome_window.rs`'s What's New
text updated too (not yet in a tagged dev build — this and the
`dep_graph`/`ref_manager` fix both landed after `v0.24.0-dev4` was tagged).
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

**Status:** ☑ DONE (2026-08-18) — turned out to already work end-to-end;
the real gap was discoverability, not missing functionality.

**Investigated the two design questions this phase's own text raised
before writing any new parsing code:**
- **CSL-JSON**: checked `hayagriva` 0.9.1's actual public API
  (`src/io.rs`) — it has `from_yaml_str` and `from_biblatex_str`/
  `from_biblatex`, but **no CSL-JSON reader at all**. Building one would be
  real, non-trivial new work (CSL-JSON's schema has a lot of surface area
  to map correctly) for a format this phase's own text already flagged as
  the higher-effort path.
- **BibTeX**: Zerkalo already accepts an arbitrary `.bib` file today via
  the Citations panel's existing "Choose Bibliography File" button
  (`citations.rs`'s `set_on_choose_bib` → `bibliography::load_bib` →
  the `biblatex` crate) — **already exactly the "one-time file import" this
  phase asked for**, since a Zotero-exported `.bib` is just a `.bib` file
  Zerkalo has always been able to open. No new import path was structurally
  missing.

**What could still have been a real gap: does the parser survive a *real*
Zotero export's quirks**, not just clean textbook BibTeX? Zotero's default
export is known to include non-standard fields (`urldate`), a `date` field
instead of `year`, `file` attachment paths with colons and spaces, LaTeX
escapes in abstracts, and multi-word `keywords`. Added a permanent
regression test (`bibliography.rs`,
`parse_bib_handles_a_realistic_zotero_export`) using a representative
sample with all of the above — **passed on the first run**, confirming the
`biblatex` crate already handles it correctly. This is now a standing
regression test, not just a one-off check — it'll catch a future
`biblatex` crate upgrade that breaks this silently
(`parse_bib` returns an empty `Vec` on any parse error, with nothing
surfaced to the user, so a silent break here would be a real "nothing
happened" dead end for a first-time Zotero migrant).

**The actual fix: discoverability.** Nothing anywhere in the app or docs
mentioned Zotero (or any reference manager) by name, so a user had no way
to know the existing "Choose Bibliography File" flow already covered their
case. Updated, all in the same session:
- `citation_panel.rs`: both the header button's tooltip and its CV-mode
  reset counterpart now say "...including a library exported from Zotero,
  Mendeley, or any other reference manager as BibTeX".
- `settings_dialog.rs`: the Bibliography group's description, same
  addition.
- `README.md`: the Bibliography sources row, same addition.

**Not done, correctly out of scope per this phase's own recommendation:**
CSL-JSON support, live/two-way Zotero sync, and DOI/ISBN lookup (the last
already a deliberate exclusion elsewhere in this plan). If CSL-JSON import
is ever wanted, it needs a real from-scratch parser — worth its own future
phase, not a small addition to this one.

Full verification gate green: 496 tests (1 new), clippy clean, version
guard clean.
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
