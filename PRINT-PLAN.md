# Print experience overhaul — plan

Working document for the print system rewrite, agreed 2026-08-02. Scope: **one dev
cycle, all five phases**, minor bump (0.19.1-dev2 → 0.20.0-devN).

Status key: `[ ]` todo · `[x]` done · `[~]` in progress

---

## Where printing stood before this work

`src/ui/print.rs` was rewritten in v0.19.1-dev2 and its bones are sound:

- `print_document()` compiles on a worker thread, polls back to the GTK loop, then
  tries the desktop print portal (`ashpd`) with the compiled PDF — vector output at
  the printer's own resolution.
- Falls back to `gtk4::PrintOperation` with pages raster-rendered at 300 dpi when no
  portal answers.
- PDF staging is careful: temp file written, seeked to 0, unlinked immediately,
  passed by fd. Three tests cover it.
- `print_from_preview()` (`app_window.rs`) flushes unsaved tabs, reuses
  `preview.compile_inputs()` so CV documents work, routes failures to the error panel.

The plumbing was right. What was missing: **Zerkalo contributed nothing to the job** —
it handed the portal a PDF plus `Default::default()` for both settings and page setup,
and offered no options of its own.

---

## Phase 1 — Fidelity

Self-contained, mostly inside `print.rs`. Each item was a real defect.

- [x] **1.1 Document page size is never communicated.** `print_via_portal` passed
  `PageSetup::default()`, so the dialog opened on the desktop default paper
  (A4/Letter) whatever the document actually was — an A5 booklet or custom size got
  scaled or clipped silently. Derive from `doc.pages[0].frame.size()`, feed
  `PageSetup::set_width/set_height/set_orientation`. Typst bakes margins into the
  page, so margins go to zero.
- [x] **1.2 Same bug in the GTK fallback.** `run_gtk_print_dialog` set `n_pages` and
  `embed_page_setup` but never a default `gtk4::PageSetup`, so raster pages landed on
  desktop-default paper. Use `PaperSize::new_custom` from the same derived spec.
- [x] **1.3 Mixed page sizes silently wrong.** Typst allows `#set page()` mid-document;
  both paths assumed uniformity. Detect non-uniform sizes and warn once.
- [x] **1.4 `PRINT_DPI` hardcoded at 300** in the fallback. `PrintContext` exposes the
  printer's real `dpi_x`/`dpi_y` — 600 dpi printers were downsampled, large-format
  pages were a memory spike. Use the context's DPI, clamped to a sane range.
- [x] **1.5 "Current page" greyed out** in the portal dialog because
  `PreparePrintOptions::set_has_current_page` was never called, though the preview
  pane knows the on-screen page.
- [x] **1.6 GTK fallback cannot work in the shipped flatpak.**
  `packaging/io.github.calstfrancis.Zerkalo.yml` has
  `--talk-name=org.freedesktop.portal.Desktop` but no `--socket=cups`, so inside the
  sandbox the fallback enumerates zero printers. Either add the socket or make the
  fallback's failure message honest rather than showing an empty dialog.

## Phase 2 — Flow

- [x] **2.1 Every print recompiled from scratch.** The preview pane already holds a
  laid-out document for the current buffer state; when nothing changed since the last
  compile, printing should skip compiling entirely. Biggest felt improvement — a
  multi-second wait becomes an instant dialog.
- [x] **2.2 No cancel, no progress.** A long compile showed one "Preparing to print…"
  toast and nothing else.
- [x] **2.3 A second Ctrl+P got scolded** ("Already preparing a document to print.")
  instead of being coalesced, or the action simply disabled while in flight.
- [x] **2.4 Poor discoverability.** Print lived in the hamburger popover and the popout
  preview header only — not the main headerbar, not the Ctrl+K command palette.

## Phase 3 — The pre-print sheet

- [x] A small `adw::Window` on Ctrl+P, shaped like `ExportDialog`: what will print
  (root file, page count, page size), Zerkalo-only options (range in the document's own
  numbering, current page, preset), a thumbnail of the first sheet, then one **Print…**
  button handing off to the *system* dialog with those settings pre-applied.
  Printer selection stays where it belongs; Zerkalo adds only what the system dialog
  cannot know.
- [x] **Page ranges in the document's own numbering.** `Page::number` is the logical
  number (roman front matter, `counter(page)` resets); the portal's range is physical
  index. Translating between them is something only Zerkalo can do.

## Phase 4 — Presets, persisted

- [x] Portal settings evaporate between runs. `ashpd` `Settings` accepts printer,
  copies, duplex, colour, quality, n-up — all pre-settable from a preset remembered in
  `config.rs`. Cheap once Phase 3 exists.

## Phase 5 — Booklet / N-up imposition

- [x] 2-up saddle-stitch is what a liturgy or sermon handout actually needs. CUPS
  `number-up` gives 2-up but never booklet page ordering, so this is a real PDF
  imposition pass (reorder + place pages onto sheets) before the file reaches the
  portal. Largest single piece.

---

## Deviations from the plan as written

- **2.1 was implemented as a print-side cache, not by reusing the preview's
  document.** The preview compiles to RGBA pages via `compile_to_rgba_pages` and
  never holds a `PagedDocument` or a PDF, and it compiles in draft mode, which
  printing deliberately excludes. Reusing it would have meant changing the
  preview's compile path and its draft handling to serve printing. Instead
  `print.rs` caches its own last preparation, keyed by a content hash of the
  root, buffer overrides and sys inputs. Same felt result — reopening the sheet,
  changing a setting, or printing again is instant — without entangling the two
  paths. Cancelling keeps the cached result, so an abandoned compile is not
  wasted.
- **1.5 landed as the opposite of what was planned.** `has_current_page` was
  going to be advertised to the portal, but the print sheet resolves ranges
  itself, in the document's own numbering, before the portal dialog opens. The
  dialog's own "current page" would contradict what was already chosen, so both
  it and "selection" are explicitly turned off.
- **Presets are three fixed starting points, not user-managed named presets.**
  They set the controls and step out of the way. A preset-management UI is more
  surface than the recurring jobs justify; persisted last-used settings carry the
  actual benefit.
- **Imposition is unavailable on the GTK fallback path.** It draws pages one at a
  time through Cairo and cannot compose several onto a sheet. That path is only
  reached when no print portal answers.
- **Imposed sheets drop link annotations.** Pages are re-placed as Form XObjects,
  which carry content but not the page's `/Annots`. Unimposed printing is
  untouched.

## Testing

Pure functions to cover: page-size → `PaperSpec` derivation, logical → physical range
translation, booklet sheet ordering, uniform-size detection. The existing temp-file
staging tests stay.

## Release chores for this cycle

Per `CLAUDE.md`: bump `version` in `Cargo.toml`, update `CHANGELOG.md` (one entry per
version, edited in place across dev builds), update the What's New text in
`src/ui/welcome_window.rs` (**not** `RELEASE_NAME` — that waits for an actual release),
never add a `metainfo.xml` `<release>` entry for a dev build. Regenerate
`packaging/cargo-sources.json` if `Cargo.lock` changes. Commit + tag, then stop.
