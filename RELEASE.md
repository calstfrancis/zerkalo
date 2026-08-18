# Zerkalo v0.24.0 "Second Reading"

Install via Flatpak:

```bash
flatpak remote-add --user calstfrancis \
  https://calstfrancis.github.io/flatpak/calstfrancis.flatpakrepo
flatpak install calstfrancis io.github.calstfrancis.Zerkalo
```

Already installed? Update with:

```bash
flatpak update io.github.calstfrancis.Zerkalo
```

---

### What's new

**A Comments panel, with reviewable suggested edits from Word.** Leave threaded, resolvable notes anchored to a line — never written into the Typst source itself, so a comment can't break compilation or leak into an export. And if someone sends back a `.docx` with Word's track changes turned on, importing it now turns those changes into suggestions right in the same panel: both the proposed addition and the proposed removal are shown in context, with Accept and Reject buttons per suggestion, instead of Zerkalo silently accepting or dropping the edits the way plain document conversion always has.

**The Packages panel can search and install from the Typst Universe**, not just list what's already downloaded — browse the full public package index with descriptions, and install with one click.

**The Outline panel has a manuscript-wide view.** A folder toggle rolls up headings and word counts across every file reachable from a project's root, not just the one open in the current tab — useful for theses and other multi-file projects where the total matters, not just one chapter's.

**Math authoring and table authoring got real tools.** The Symbol Insert panel has a Math tab — common operators, relations, calculus notation, set theory, logic, arrows, and number sets, one click to insert. Insert Table builds a complete table block from a form — rows, columns, per-cell text, alignment, a header row, colspan/rowspan — instead of hand-writing the markup.

**Bibliographies now actually work when they live outside the project** — most commonly a Kartoteka vault, which is meant to be shared across projects rather than copied into each one. Two bugs meant this silently failed at compile time even though the citation panel worked fine: the bibliography path pointed at a folder Typst can't read as a data file, and more fundamentally, Typst couldn't reach anything outside the project directory at all. Both are fixed, and a document created before this release just needs Update Template Settings → Apply once to pick it up.

**A 32-item usability pass for anyone coming from Word.** Import is a normal, always-available feature now, not a developer toggle. Restoring a snapshot asks first. The file that actually compiles is marked in the file tree. Help opens on plain language instead of raw Typst syntax. The Reference Manager and Dependency Graph — both built, both working, neither reachable before — are in the hamburger menu now. And a couple of smaller fixes: an inflated per-section word count, and the sidebar's Outline/Citations/Packages/Comments sections can now be resized against each other, not just against the editor.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
