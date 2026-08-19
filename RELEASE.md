# Zerkalo v0.24.5 "Clear Glass"

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

No user-visible changes in this release. It updates the `rusqlite` and `hayagriva` dependencies to their current releases — both were blocked on an upstream Kartoteka pin, and Kartoteka just released a version with matching updates (which also fixed an intermittent search-index rebuild race and closed two dependency security advisories on its own side).

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
