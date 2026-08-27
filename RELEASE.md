# Zerkalo v0.28.1 "Steady Preamble"

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

**"Update Template Settings" no longer regenerates an unwanted title page on a document that had it off.** If a document had no sidecar file, the Title Page switch was the one setting in that dialog with no way to read its actual state back from the document — every other setting (margin, header, spacing, and more) did. It silently stayed on the dialog's default of "on," so applying any change at all — even just picking a running header — could resurrect a full cover page with placeholder "Untitled" text on a document that never had one.

**Document metadata (title, author, and the rest) is no longer silently discarded just because no running header was chosen.** These fields are only ever written into the file inside the header/title-page code, so a document with both off had nowhere for its own metadata to survive without a sidecar — turning a header on later showed blank or placeholder text instead of what you'd actually typed. Now preserved whenever there's real content to keep.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
