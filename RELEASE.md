# Zerkalo v0.29.1 "True Course"

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

**Dependency hygiene only.** The pinned Kartoteka `fond-bib`/`fond-vault` dependency (used by the "point the bibliography at a Kartoteka vault folder" live-source feature) is bumped from v0.7.0 to v0.9.0, matching Kartoteka's current release. Checked the diff between the two tags directly: the only change touching those two crates is Bookshelf-view cover-image caching, which Zerkalo's citation autocomplete doesn't use. Nothing user-visible changes in this release.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
