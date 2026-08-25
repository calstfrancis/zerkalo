# Zerkalo v0.26.5 "Tidy Ledger"

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

**Enter now confirms every name prompt in the Library window.** Rename Project, Rename Document, New Project, and New Project from an existing document all needed a mouse click to confirm — typing a name and pressing Enter did nothing, unlike their siblings (Rename Category, Add Subcategory, New Category), which already worked that way.

**Exporting a CV or citation document from the Library window no longer comes out broken.** Using the Library window's own Export action (rather than the header's Export button on an already-open document) skipped the step that resolves a document's CV data source and bibliography path, so the exported PDF was missing `#cv-entry`/`#cv-section` content and citations. Both now resolve the same way for either export path.

**A few secondary windows (Insert Table, Saved Versions, GitHub sign-in) now have the same header divider every other window in the app already has** — they were quietly missing the line that separates the header from the content below.

Also in this release: a few dependency updates and some internal cleanup (duplicated code consolidated behind shared helpers, and leftover `.deb`/RPM packaging configuration removed now that distribution is flatpak-only) — nothing user-visible on their own, but worth a mention for anyone building from source.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
