# Zerkalo v0.30.1 "Split Light"

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

One footnote per citation.

**Back-to-back citations in a footnote style now get a footnote each.** Writing `text @key1 @key2 @key3` with Chicago Notes, SBL, or Turabian (or a custom `.csl` footnote style) used to produce a single footnote listing all three sources. It now renders as `text¹²³`, with three separate numbered notes at the bottom of the page, which is much easier to read. It's applied automatically for the preview, PDF, and every export, and nothing is written into your document.

Author-date and numeric styles (APA, MLA, IEEE, …) are unchanged: `(Smith 2020; Jones 2021)` still groups as before.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
