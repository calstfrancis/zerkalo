# Zerkalo v0.26.0 "Open Folio"

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

**Two new toggles in Template Settings' Sections tab: Title Page and Bibliography, both on by default.** Turn off Title Page for a document that shouldn't have a generated cover page — letters keep their letterhead regardless, since that's a separate section. Turn off Bibliography to suppress the `#bibliography(...)` call even when a `.bib` file is attached via the Citations panel; it leaves a commented-out example line in its place, so it's easy to switch back on later. Both settings round-trip through a document's sidecar the same way the existing Table of Contents, Abstract, and Keywords toggles do, and documents saved before this release keep both on, matching the behavior they always had.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
