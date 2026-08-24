# Zerkalo v0.26.3 "Steady Margin"

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

**Typewriter scroll actually scrolls now.** It re-centered the viewport based on the buffer's logical line number, but with word wrap on (the default), a single long paragraph spans many wrapped display rows while staying one logical line — so the re-center only fired at paragraph breaks, and text could run off screen the whole time you were typing inside one. It's now tracked by the cursor's actual vertical position, so it re-centers on every wrapped display line.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
