# UX Audit Plan — Word-migrant usability pass

Source: 4-way parallel audit (2026-08-17) against a non-technical
Word-migrant user, cross-checked for LaTeX/Typst power-user reward. Full
writeup: https://claude.ai/code/artifact/6c398e24-18f1-4a89-9037-f961abb248b2

32 issues found, 13 things confirmed as strengths (not touched here).

## Phase A — Critical (do first)

- [x] Import ungated from "Experimental mode" — `settings_dialog.rs`
- [x] Snapshot restore confirmation dialog — `snapshot_dialog.rs`
- [x] Git jargon reworded in History panel + command palette — `history_panel.rs`, `command_palette.rs`
- [x] History diff view uses Snapshot's clean +/- rendering — `history_panel.rs`, `snapshot_dialog.rs` (extracted to new shared `diff_render.rs`)
- [x] Package browser empty state no longer points at a terminal — `package_browser.rs`
- [x] File tree root/include/import tooltips + root-file marker — `file_tree.rs`
- [x] Citation panel offers "Start a new bibliography" — `ref_manager.rs`, `citation_panel.rs`, `app_window/citations.rs`
- [x] Sync (no remote configured) opens Setup Wizard, not the bare Sync dialog — `app_window/menus.rs` (removed now-dead `sync_dialog.rs` + `git_sync::add_remote`)
- [x] Welcome window: jargon split out of first-run list; "Get Started" opens template dialog — `welcome_window.rs`, `app_window/lifecycle.rs`
- [x] Help opens on Overview, not Cheatsheet; F1 overlay covers Library — `help_window.rs`, `help_overlay.rs`, `library_window.rs`

## Phase B — Moderate

- [ ] Welcome window layout diagram gets a reassuring line — `welcome_window.rs`
- [ ] GitHub sign-in errors translated to plain language — `github_signin.rs`
- [ ] Template package descriptions lead with plain English, syntax moves to tooltip — `template_dialog/mod.rs`
- [ ] First-open orientation banner (source vs. preview) — `editor_pane.rs`
- [ ] Export dialog checks pandoc availability before offering DOCX/HTML/ODT/EPUB — `export_dialog.rs`
- [ ] Simple Mode inline banner (not tooltip-only) — `editor_pane.rs`
- [ ] BibTeX entry-type dropdown uses plain labels — `ref_manager.rs`
- [ ] Cite key field gets explanatory placeholder — `ref_manager.rs`
- [ ] Command palette differentiates History vs. Snapshots subtitles — `command_palette.rs`
- [ ] "Remove from Library" gets a confirm dialog — `library_window.rs`
- [ ] "What's New" reads as plain-language release notes, not a changelog dump — `welcome_window.rs`

## Phase C — Minor / polish

- [ ] Outline empty state references toolbar buttons, not `=` syntax — `outline_panel.rs`
- [ ] Print dialog "1 page on 1 sheets" grammar fix — `print_sheet.rs`
- [ ] Error panel advice lines gloss `#let`/`#show`/`.bib` on first use — `error_panel.rs`
- [ ] Package browser import-insert tooltip explains `@namespace/name:version` — `package_browser.rs`
- [ ] Dep graph empty state avoids "imports" — `dep_graph.rs`
- [ ] Font manager stale "Setup & Onboarding → Default Fonts" references fixed — `font_manager.rs`
- [ ] Compile delay subtitle reworded — `settings_dialog.rs`
- [ ] Skrizhal CV Elements gets one clause of context — `template_dialog/mod.rs`
- [ ] install.sh tinymist prompt reworded — `install.sh`

## Future project (not in this pass)

- Merge History panel and Snapshot dialog into a single "Versions" system.
  Both currently mean "an old version of this file" via two different
  backends (git-sync history vs. local per-save snapshots) with two
  different UIs. Deferred because it's a real architectural decision
  (single storage model? keep both backends but one UI?) that deserves its
  own design pass, not a UX-polish fix.
