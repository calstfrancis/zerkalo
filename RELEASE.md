# Zerkalo v0.24.2 "True Source"

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

**Choosing a bibliography file, vault, or creating a new one from the citation panel now actually updates the document.** Previously these dialogs only updated the app-wide setting that drives the citation panel's own autocomplete — the document's `#bibliography(...)` line was left pointing at the old (or no) source, which meant the document wouldn't compile until that line was fixed by hand. All three dialogs now rewrite the active document's bibliography call directly, preserving its style and title.

**Collapsing the Packages or Comments sidebar section now reclaims the space it used.** Previously the content hid but the divider stayed put, leaving a blank gap where the section used to be instead of giving that room to whichever section is actually open.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
