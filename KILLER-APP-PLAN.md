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

**Status:** ☑ DONE (2026-08-18) — shipped the palette scope this phase's
own text called for; the full structured equation builder (stretch goal)
deliberately not attempted.

**Scope decision, per this phase's own recommendation:** ship the
symbol/operator palette first, treat a full structured builder (visual
fraction/matrix/superscript editing) as a stretch goal only if the palette
alone doesn't close the gap. Went with the palette.

**Found the natural home instead of building new UI:** the existing Symbol
Insert panel (`outline_panel.rs`'s `symbol_tabs()` — Cyrillic, Greek,
Hebrew, Sanskrit tabs, generic click-to-insert-at-cursor mechanism already
shared by all of them) was already exactly the right shape for a math
palette — same interaction model (click a character, it's inserted), same
tooltip/codepoint display, same notebook-tab structure. Added a fifth
"Math" tab rather than building a separate popup or panel, which the
phase's own fallback design (`bib_popup.rs`-style inline popup) would have
been a real second UI surface to build and maintain for no benefit over
reusing the one that already exists.

**Content:** ~50 curated symbols across Basic operators, Relations,
Calculus, Set theory, Logic, Arrows, Number sets (blackboard bold
ℝ/ℕ/ℤ/ℚ/ℂ), and Misc (∥, ⊥, ⊗, ħ, etc.) — each a plain Unicode character
(matching the existing tabs' model), plus one plain-text entry (`lim`,
Typst's literal math-mode limit keyword) as the one deliberate exception to
"single Unicode character," since it's a genuinely common, single-click-
worthy insert with no Unicode symbol of its own.

**Scope trim made explicit, not silently dropped:** the phase's own text
asked for "live rendered preview per entry (reuse the embedded Typst
compiler...)". Skipped deliberately — these are standard BMP Unicode math
symbols that every font already renders correctly as plain text (unlike a
structural equation builder, where seeing the compiled layout is the whole
point), so spinning up a Typst compile per palette entry would add real
complexity and cost for symbols that already display correctly without
one. Matches the existing Cyrillic/Greek/Hebrew/Sanskrit tabs' own
precedent, which don't render-preview either.

**Verified two different things, not just "it builds":**
1. **Insertion mechanism** — headless: switched to the Symbols tab, opened
   the new Math tab (visually confirmed all 5 tabs present: Cyr / Greek /
   Heb / Sans / Math), clicked "±", confirmed it landed at the cursor in
   the editor text (title bar showed "Modified", document text showed the
   inserted `±` exactly where expected).
2. **Typst actually renders these Unicode symbols correctly in math
   mode** — the one thing that genuinely needed checking, since inserting
   a character is easy but Typst's math-mode symbol table might not
   recognize all of them. Wrote a test document using `∑`, `∫`, `≤`, `ℝ`
   inside `$...$` blocks spanning four different categories (calculus,
   calculus, relations, number sets) and confirmed the live preview
   compiled with **zero errors**, rendering a real summation with correct
   sub/superscripts, a real integral, the inequality, and blackboard-bold
   ℝ — screenshot-confirmed.

Full verification gate green: 496 tests, clippy clean, version guard
clean. No new tests added — this phase is a static data table plus reuse
of an already-tested generic mechanism, not new logic; the render
verification above was the check that actually mattered and isn't the
kind of thing a unit test would catch anyway (needs the real Typst
compiler and a real font).

README/CHANGELOG/`welcome_window.rs` updated in the same session.
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

**Status:** ☑ DONE (2026-08-18) — built the scope this phase's own design
pass called for (dialog/generator model), and it directly caught a real
bug that pure code review had missed.

**Went with this phase's own recommendation** without re-litigating it:
a form dialog that generates a `#table(...)` block once and hands control
back to the text editor, not a live preview-overlay editor. New file
`src/ui/table_dialog.rs`.

**Scope, matching this phase's own bullet list exactly:** row/column count
(spinners, 1–20), per-cell content (`Entry` grid), per-column alignment
(left/center/right dropdowns), header-row toggle (bolds the first row's
content, wraps it in `table.header(...)`), and per-cell colspan/rowspan
(spin buttons, 1–20). **One documented scope trim**: colspan/rowspan don't
dynamically grey out the cells they end up covering as you type — that
would need reactive re-layout tracked across a grid that's also growing/
shrinking independently, real added complexity for a first version.
Instead, coverage is computed once at generation time and covered cells'
content is silently skipped — documented via a tooltip on every span
control, not left as a silent surprise.

**A real bug found by code review, before any runtime check ran:**
`gtk_grid_remove` (what shrinking a row/column calls) only operates on a
grid's *direct* children. The per-cell layout wraps content/colspan/
rowspan in a `Box` that's what's actually attached to the grid — so the
first version's shrink logic, which called `grid.remove()` on the leaf
widgets themselves, would have been calling it on grandchildren the grid
doesn't directly own. Checked gtk4-rs's actual binding source rather than
assuming, confirmed the mismatch, and fixed it by tracking the wrapper
`Box` itself in `CellWidgets` and removing that instead.

**Verification, and why it went further than most phases this session:**
finding one real bug via review — in code that had already compiled
clean — raised the bar for what "done" means here; unlike reusing an
already-proven pattern (`show_dep_graph_window`, etc.), this grid grow/
shrink logic was genuinely new and unproven. Since `TableDialog` is (like
Reference Manager and Dependency Graph before it) only reachable through
the hamburger `Popover` this session already found doesn't respond to
synthetic clicks, and this time the added risk justified not accepting
that gap: temporarily wired `TableDialog` to open directly from
`main.rs`'s `connect_activate` (bypassing the whole `AppWindow`/`Popover`
chain), drove it with real pointer clicks, and reverted the instrumentation
completely before committing (`git diff --stat src/main.rs` confirmed
empty afterward). This is what actually caught the click landing on the
wrong window at first — the Welcome dialog was stacked on top and silently
absorbing clicks meant for the table dialog underneath, a test-harness
issue rather than an app bug, resolved by dismissing Welcome first.

With real clicks landing correctly: watched Columns shrink from 2→1 (the
exact path the bug fix touches) and correctly remove both the alignment
dropdown and the cell widgets with no crash and no leftover stale
widgets, watched it grow back 1→3 correctly, and clicked Insert Table to
confirm the full read-grid → generate-code → callback path fires and the
dialog closes cleanly. Couldn't type real cell content (the same keyboard-
synthesis gap from Phase 2), so the live run exercised empty-content
cells — which surfaced a second, smaller finding: an empty header cell
generates `[**]` (adjacent `*` markers, empty bold). Rather than just
trust that this looks right, added a permanent regression test that
compiles it through the real Typst engine and confirmed it's valid.

10 unit tests total, 3 of which compile generated output through the real
embedded Typst compiler (a representative table with header/colspan/
rowspan/escaped-content/all-three-alignments, the empty-header-cell edge
case, and the original template-dialog-style "does this actually compile"
check) rather than only asserting the generated string's shape — matching
`template_dialog.rs`'s own established standard for this kind of
generator code. Full gate green: 510 tests, clippy clean, version guard
clean.

**Wired into the hamburger menu** ("Insert Table…", Current Document
cluster, next to Reference Manager/Dependency Graph) — not the Ctrl+K
palette, matching this session's established "menu is sufficient, palette
parity is a low-cost follow-up" call from the Reference Manager/
Dependency Graph phase.

**Deliberately not attempted, matching this phase's own scope
boundary:** live-editing an already-inserted table's structure. A natural
follow-up once this generator model is in real use, not part of this
phase.

README/CHANGELOG/`welcome_window.rs` updated in the same session —
including filling in README rows for Reference Manager/Dependency Graph's
reachability, which should have been added when that fix landed earlier
this session and was missed until now.

---

**Original phase text preserved below:**

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

**Status:** ☑ DONE (2026-08-18) — infrastructure built and proven on a
complete real file (`settings_dialog.rs`, ~90 strings), matching this
phase's own scope: infrastructure and English as the reference locale
only, no translation content.

**Framework: went with this phase's own recommendation, `fluent-templates`
0.15.1** (wraps `fluent-bundle` 0.16, `unic-langid` 0.9), rather than
re-litigating gettext vs. Fluent. `static_loader!` embeds `.ftl` files into
the binary at compile time (matching this project's "nothing to install"
philosophy — no runtime file lookup, no `msgfmt` build step, no flatpak
manifest changes needed since `locales/` just travels with the git
checkout the `zerkalo` module already sources).

**One correction to the framework's default behavior, found by a failing
test, not assumed:** Fluent wraps every interpolated variable in invisible
Unicode bidi-isolation marks (U+2068/U+2069) by default — sound design for
apps mixing RTL/LTR text, but Zerkalo has none of that, and the marks would
otherwise leak into copy-pasted error text and accessible-name properties.
Disabled via the documented `customise: |bundle| bundle.set_use_isolating(false)`
hook. Caught immediately: `tr_args`'s own unit test failed on a byte-for-byte
string comparison the first time, which is exactly the kind of thing this
test was worth writing.

**Convention established** (`src/i18n.rs`): `tr(id) -> String` and
`tr_args(id, &[(name, value)]) -> String`, both falling back to the ID
itself on a lookup miss via `try_lookup`/`try_lookup_with_args` rather than
the panicking `lookup`/`lookup_with_args` the crate also offers — a typo'd
ID during migration (or later, a string genuinely not yet translated)
shows up as visibly-wrong text in the UI, not a crash. 4 unit tests,
including one that resolves a real production message ID
(`settings-window-title`) through the actual compiled `.ftl` file, not a
synthetic test fixture — the strongest verification available for "does
the lookup path actually work end-to-end" without a GUI screenshot.

**Migrated `settings_dialog.rs` completely, not partially** — every
user-facing string (titles, subtitles, tooltips, accessible-name
properties, button labels, dropdown items, file-filter names, and all 8
save-time validation notices, several with runtime interpolation:
paths, error messages, a GitHub username, a field label) now routes
through `tr`/`tr_args`. Left alone, correctly: font family names
(`"Noto Sans"`, `"Monospace"`, etc.) — proper nouns, not UI copy, and
translating them would break font lookup. `locales/en/settings_dialog.ftl`
holds ~90 messages, organized into sections matching the dialog's own
`PreferencesGroup` layout, so a future translator (or a future session
extending this to a second file) has an obvious model to copy.

**Multi-line Fluent value confirmed working**, not just single-line
lookups: `settings-open-file-failed-body`'s indented-continuation-line
syntax reproduces the original `"Edit it by hand at:\n{}"` exactly.

**Verification — two tiers, honestly distinguished:**
1. **Strong**: full gate green (500 tests — 4 new in `i18n.rs`, clippy
   clean, version guard clean), `cargo-sources.json` regenerated for the
   two new dependencies (`fluent-templates`, `unic-langid`) per this
   project's own vendoring requirement. `i18n.rs`'s tests resolve real
   production IDs end-to-end.
2. **Not achieved, honestly**: a live screenshot of the migrated Settings
   dialog. `SettingsDialog::new` has exactly one call site
   (`menus.rs`, wired to the hamburger's `Popover`), and this session
   already independently found — twice now, first for Reference
   Manager/Dependency Graph, now here — that this `Popover` doesn't open
   via synthetic `xdotool` clicks in the current headless setup, with or
   without a window manager, even though plain `Button` clicks work fine
   elsewhere. Not a new limitation, the same one already documented under
   Phase 2. Given the actually-risky part (does Fluent resolution work) is
   already proven by unit tests against the real `.ftl` file, and rendering
   an owned `String` vs. a `&'static str` is not a meaningful GTK-specific
   risk, this is assessed as sufficient — but **Cal: worth opening
   Settings once** to eyeball it, the same ask as the Reference
   Manager/Dependency Graph phase left open.

**Sequencing note for whoever extends this next**: per this phase's own
original text, migrating the rest of the UI file-by-file is deliberately
left for later, separate commits — `settings_dialog.rs` was chosen
specifically as the proof-of-pattern (self-contained, already
accessibility-audited in `HEALTH-PLAN.md` Phase 6, a real but bounded
size). The pattern is now established firmly enough that sweeping another
file is mechanical repetition of what's in this phase, not a design
question.

### Follow-up (2026-08-18) — this phase's new dependencies broke `./dev-build.sh`, fixed by clearing stale cache

Cal ran `./dev-build.sh` for `v0.24.0-dev7` (after this phase and Phase 9
had both landed) and hit a different failure than Phase 1's: the offline
dependency fetch succeeded this time, but the `zerkalo` module (module 2)
failed compiling with `failed to write .../.fingerprint/biblatex-.../
lib-biblatex: Read-only file system`.

Root cause: `.flatpak-builder/build/zerkalo-deps-3` (the cached module-1
dependency precompilation) was dated **2026-08-17 19:11 — before this
phase added `fluent-templates`/`unic-langid` to `Cargo.lock`** on
2026-08-18. Confirmed directly (`find ... -iname "*fluent*"` inside that
cached build dir came back empty). `flatpak-builder`'s own cache-hit logic
for module `zerkalo-deps` didn't invalidate on the `Cargo.lock` change, so
module 2 ended up needing to compile packages whose dependency-graph
resolution had shifted (adding `fluent-templates` can reshuffle shared
transitive dependency versions even for unrelated crates like `biblatex`)
against module 1's stale, read-only, already-finalized output —
`flatpak-builder`'s `rofiles-fuse` overlay allows writing *new* paths from
the current module but not overwriting a path an earlier module already
produced, hence the `EROFS`.

**Not a manifest bug this time** — nothing to fix in
`io.github.calstfrancis.Zerkalo.yml`. Fixed by clearing the stale cache
(confirmed with Cal first, given the ~2.2 GB / next-build-is-slower cost):
`rm -rf .flatpak-builder/build/zerkalo-deps-{1,2,3} .flatpak-builder/build/zerkalo-{1,2,3}`
plus the now-dangling `zerkalo` symlink. `.flatpak-builder/` is pure,
git-ignored, fully regenerable cache per this repo's own conventions — not
something committed or otherwise persisted, so no data was at risk, only
rebuild time.

**Worth remembering for next time this bites:** any dev-build session that
adds/removes/updates a dependency (regenerating `cargo-sources.json`, per
this project's own documented requirement) and *doesn't* get a
`./dev-build.sh` run immediately after is a latent trap — if
`flatpak-builder`'s cache-hit check for `zerkalo-deps` doesn't actually key
off `Cargo.lock` content the way the manifest's own comment claims it
does ("Cache key is based only on Cargo.toml, Cargo.lock, and
cargo-sources.json"), a build days or commits later can silently reuse a
now-stale dependency precompilation and fail in a confusing,
dependency-name-shifted way (here, `biblatex`, nothing to do with the
actual new dependency) rather than a clear "stale cache" message. If this
recurs, clearing `.flatpak-builder/build/zerkalo-deps-*` is the fix, not
debugging the named package in the error.

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

**Status:** ☑ DONE (2026-08-18) — comments-only scope, per this phase's own
design pass; suggested-edits (and Phase 12, which depends on this) remain
open. Caught and fixed a real bug via live testing that the pure-logic
unit tests couldn't have caught.

**Went with every one of this phase's own already-resolved design
questions** without re-litigating them: a sidecar file (`<stem>.comments.toml`,
matching `template_dialog/sidecar.rs`'s `<stem>.zerkalo.toml` — the actual
existing per-document sidecar; see the `.plan` finding below), anchoring by
stable line + a text-snippet fallback rather than exact-position tracking
through every edit, and comments-only for v1 (no suggested-edits/diff
model). New `src/comments.rs` (pure logic + persistence, zero GTK) and
`src/ui/comments_panel.rs` (sidebar panel).

**A finding this phase's own text set out to match against, that turned out
false:** the design recommendation said to match "the existing `.plan`
scratchpad and `.zerkalo.toml` settings sidecar conventions." Checked both
before writing code — `.zerkalo.toml` is real (`template_dialog/sidecar.rs`).
**`.plan` is not.** `grep -rn "\.plan\b" src/` returns nothing; no module,
no string literal, nothing — despite `README.md` describing a "Plan panel"
in detail ("Freeform scratchpad saved as a `.plan` sidecar... falls back to
`project.plan`...") as a shipping feature. This is a different shape of
problem than this session's three earlier "built but unreachable" findings
(`package_browser`, `dep_graph`/`ref_manager`, outline's `update_project`) —
those were real, compiled, dead code with a tell (`#[allow(dead_code)]`).
This is documentation describing a feature with **zero corresponding code
anywhere**, not even dead code. Not investigated further or built (a real
scratchpad feature is its own scoped piece of work, not part of a comments
phase) — the false README row was corrected in this session since it's now
*confirmed* false rather than merely stale, but the feature itself is a
separate future initiative if wanted.

**Scope, matching this phase's "Fix" bullet with one deliberate cut:**
sidebar panel (comment list, click-to-jump) and sidecar persistence, both
as specified. **Cut**: "inline gutter markers in the editor." Editor-side
wavy-underline/diagnostic-marker integration would mean real surgery inside
`editor_pane.rs` — this codebase's largest file and the one file
`REFACTOR-PLAN.md` and `HEALTH-PLAN.md` both explicitly single out for
`Rc<RefCell<>>` re-entrant-borrow fragility. The sidebar list alone already
covers the "advisor reviews a draft" workflow this phase exists for
without touching that file's internals at all — the add/jump/anchor wiring
lives entirely in `app_window/mod.rs`, calling only `editor_pane.rs`'s
existing public methods. Gutter markers are a real follow-up, not a v1
requirement.

**A real bug found only by live testing, not by the unit tests** (13 of
which passed cleanly on the pure `relocate`/`CommentThread` logic before
any GTK code existed): the first wiring used `editor_pane.rs`'s
`set_on_cursor_moved` to track the cursor line for new-comment anchoring.
Reading that callback's own implementation site revealed why this was
wrong *after* the bug had already shown up live — it fires **only on
keyboard-driven movement**, by original, documented design elsewhere in
`editor_pane.rs` ("Only fire on keyboard movement... otherwise a click in
the editor jumps the preview to match the clicked line"), a restriction
that made sense for its original purpose (syncing the *preview* pane) but
is exactly wrong for "where did the user just click before pressing +."
Caught concretely: clicked into the document's line 5, clicked "+", and
the comment saved with `anchor_line = 1` — the cell's stale default,
because the click had never updated it. Fixed by switching to a direct,
on-demand query instead of a cached callback value:
`editor_pane.get_cursor_positions()` (character offset per open tab) plus
counting newlines up to that offset in `get_active_content()` — synchronous,
no dependency on *how* the cursor got there. Re-ran the identical
click-then-add sequence after the fix and confirmed `anchor_line = 5`,
correctly.

**A second, unrelated finding from the same testing session, worth
recording for every future phase**: `xdotool type` (text input) **works**
in this headless setup, even though `xdotool key` (individual key events —
Ctrl+K, Escape, arrows) does not. This wasn't known before this phase; every
earlier "couldn't verify, keyboard synthesis is broken" note this session
(Phase 2's original finding, repeated for Settings/Reference Manager/
Dependency Graph in Phases 9–10) was about `key`, and none of those actually
needed to type prose into a text field the way this phase did — so this is
the first phase that would have surfaced the distinction. Worth
retrying `xdotool type` specifically (not `key`) before writing off any
future verification as blocked by "the keyboard limitation."

**Verified live, beyond the bug fix above**: add-comment → correct anchor
line and snippet captured (`line 5 · = Methods`, confirmed against a
real cursor click, not assumed); the sidecar file matches exactly
(`anchor_line`, `anchor_snippet`, `body`, `created_at`); Resolve → count
and badge update correctly, button flips to "Reopen"; Reply → threaded
reply renders indented under the parent, sidecar shows a nested
`[[comments.replies]]` table (TOML's array-of-tables nesting, nested two
levels deep, round-trips correctly — also unit-tested); click-to-jump →
moved the cursor from line 1 to line 5 and selected the heading, confirmed
via the status bar and a visibly highlighted line, not just "no crash."

23 unit tests total in `src/comments.rs` (add/reply/resolve/delete
isolation, TOML round-trip including nested replies, the sidecar path
convention, missing-sidecar-returns-empty rather than erroring, and 6
dedicated `relocate` tests: unchanged position, lines shifted, nearest-of-
duplicate-lines preferred, anchor-text-deleted returns `None` rather than
guessing, empty document). Full gate green: 523 tests, clippy clean,
version guard clean.

**Deliberately not attempted, matching this phase's own scope
boundary**: suggested edits (propose-a-replacement, accept/reject) — needs
a diff/patch model this phase's own text correctly identified as strictly
harder than anchored comments, and Phase 12 (DOCX track-changes
round-trip) still depends on whichever model gets built here, so this
stays open rather than being retrofitted onto the comments-only shape
built now.

README/CHANGELOG/`welcome_window.rs` updated in the same session,
including correcting the now-confirmed-false `.plan` panel claim in the
README.

---

**Original phase text preserved below:**

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

**Status:** ☑ DONE (2026-08-18) — DOCX *import* of track changes into a new
suggestion model, with in-app accept/reject that mutates the document.
DOCX *export* of unresolved suggestions back to revision-mark XML, and
ODT's equivalent, are both deliberately deferred — see "Scope" below.

**Resolved this phase's own open design question**: extended
`crate::comments::Comment` with an optional `Suggestion { kind: Insertion
| Deletion, text, status: Pending | Accepted | Rejected }`, rather than
building a separate diff/patch model — a suggestion is a comment with a
proposed edit attached, not a different kind of object, which kept it a
small, additive change to Phase 11's existing sidecar/anchoring/relocate
machinery instead of a parallel system.

**How accept/reject actually changes the document** — this is the part
Phase 11's own text flagged as "strictly harder" than anchored comments,
resolved with a deliberately narrow, symmetric rule rather than a general
diff/patch engine: both an insertion's and a deletion's proposed text are
inlined into the document *at import time* (Typst has no track-changes
rendering, so "review in context" means the text is just there to read).
Accepting or rejecting decides whether it *stays*: accepting an insertion
or rejecting a deletion is a no-op (the text was already visible); the
other two combinations call a new `EditorPane::remove_text_at_line(line,
text)` that finds and deletes that exact substring on that line — a
narrow, single-purpose operation built by mirroring the already-shipping
`do_replace_one`/`do_replace_all`'s exact GTK `TextBuffer`/`forward_search`
pattern, not new API surface. Pure logic (`suggestion_removes_text`) is
unit-tested on its own; the GTK-facing removal isn't (no other `EditorPane`
method is either), but was verified live (below).

**DOCX import**: `doc_import/docx.rs`'s existing direct ZIP/XML parser
(not pandoc) now also reads `<w:ins>`/`<w:del>` — a new `Inline::Tracked`
variant wraps their runs (added to the shared `doc_import` model, not
DOCX-only, so ODT can reuse it later), rendering exactly like untracked
text so zero Typst-emission code changed. `<w:delText>` (Word's separate
tag for deleted-run text, used instead of `<w:t>` precisely so naive
`<w:t>`-only readers don't double-count it) is now read alongside `<w:t>`.
A new `doc_import::collect_tracked_changes` flattens every tracked span to
plain text, in document order; the import flow
(`app_window/import.rs::record_tracked_changes_as_suggestions`) then
searches for each one's *escaped* form (matching what the Typst emitter
actually wrote) as a substring of the just-saved `.typ`, advancing the
search position past each match so repeated identical changes anchor to
their own line instead of all collapsing onto the first occurrence. A
change whose escaped text can't be found is silently skipped rather than
mis-anchored — a known, narrow gap (documented in the function's own doc
comment), not a silent majority-case failure.

**A real, live-only bug, found by screenshot, not by any of the 15 new
unit tests**: the first version of the Accept button was `.flat` +
`.suggested-action` — which compiled clean, passed clippy, and *rendered
as a genuinely blank gap* between "Reply" and "Reject" in a live capture.
Root-caused by extracting libadwaita's actual shipped CSS
(`gresource extract .../libadwaita-1.so /org/gnome/Adwaita/styles/base.css`)
rather than guessing: `button.suggested-action { color:
var(--accent-fg-color); }` applies unconditionally (no `:not(.flat)`
guard), and `.flat` drops the accent background that white text is meant
to sit on — white-on-white, invisible. `.destructive-action` (the
"Reject" button) has no equivalent bare color rule and rendered fine.
Checked the rest of the codebase: every other `suggested-action` button
already avoids `.flat` — this was the one place that combined them.
Fixed by dropping `.flat` from Accept only, matching that existing,
proven convention, with the finding recorded as a comment at the call
site so it doesn't get reintroduced.

**Verified live** (screenshot-only, `xdotool click` on a plain `Button` —
consistent with every prior phase's finding that plain buttons work
headlessly here while Popovers and drag gestures don't): pre-seeded a
`.comments.toml` sidecar with a pending Insertion suggestion via the
app's own `CommentThread::save`, opened the matching `.typ`, confirmed
the sidebar rendered "Insert 'added text'" with Accept/Reject — first
capture showed the invisible-button bug above; after the fix, Accept
rendered as a solid blue button and Reject as flat red. Clicked Reject:
the editor text changed from "Before added text after." to "Before
after." live, the title bar showed "Modified," the preview pane
re-rendered to match, the word count updated 4→2, the comment badge
switched to "✗ rejected," and (correctly, matching Phase 11's own
relocate design — the anchor line's text no longer matches) an "⚠ anchor
lost" warning appeared next to it.

**Scope cuts, both deliberate and narrower than the phase's original
text asked for**:
- **DOCX export of unresolved suggestions as `<w:ins>`/`<w:del>` was not
  built.** The phase's own "Fix" bullet asked for this, but Zerkalo's
  DOCX *export* today goes through `pandoc` — there is no custom
  OOXML-writing code anywhere in this codebase to extend, unlike import
  (which extends the already-proven direct-XML reader). Emitting real
  revision-mark XML would mean building a DOCX writer from nothing, a
  substantially larger, separately-scoped effort. Import — "receives a
  track-changes-marked DOCX back from a journal or co-author" — is the
  half of this phase's own motivating paragraph that's actually solved
  now; round-tripping *back out* to Word with marks intact stays open.
- **ODT's `<text:change>` was not built**, per the phase's own explicit
  permission ("treat DOCX as the primary target and ODT as a follow-up
  once the model is proven, rather than building both at once") — the
  shared `Inline::Tracked`/`collect_tracked_changes` plumbing added to
  `doc_import/mod.rs` is format-agnostic, so `doc_import/odt.rs` extending
  it later is a much smaller follow-up than this phase was.
- **No UI to author a suggestion by hand** (as opposed to reviewing one
  imported from DOCX) — matches Phase 11's own comments-only v1 scope
  reasoning: the workflow this exists for is reviewing someone else's
  marked-up manuscript, not proposing edits from inside Zerkalo itself.

15 new unit tests across three files (`comments.rs`: suggestion
add/status/round-trip/removal-symmetry; `doc_import/docx.rs`: insertion
and deletion parsing, `<w:delText>`, empty-case, a document with tracked
changes still compiles; `app_window/import.rs`:
`record_tracked_changes_as_suggestions`'s line-anchoring, duplicate-text
ordering, not-found-is-skipped, and escaped-character matching). Full
gate green: 542 tests, clippy clean, version guard clean.

README/CHANGELOG updated in the same session.

---

**Original phase text preserved below:**

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
