# Zerkalo v0.26.4 "Clear Glyph"

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

**System-installed fonts work again.** Since 0.26.2's compiler upgrade, documents referencing an installed font like Atkinson Hyperlegible or Goudy Initialen failed to compile with "unknown font family," even though the font was right there on the system. The upgrade had switched font loading over to a new API and only carried over Zerkalo's bundled fonts, not the system font scan. System fonts are found again now, the same way they always were.

**Typewriter scroll actually scrolls now.** It re-centered the viewport based on the buffer's logical line number, but with word wrap on (the default), a single long paragraph spans many wrapped display rows while staying one logical line — so the re-center only fired at paragraph breaks, and text could run off screen the whole time you were typing inside one. It's now tracked by the cursor's actual vertical position, so it re-centers on every wrapped display line.

**Right-clicking a misspelling no longer flashes the suggestions popover shut before you can read it.** It was opening and then closing again almost instantly — a timing quirk in how GTK's popover autohide interacts with the mouse-button release from the right-click that opened it. It now waits a beat for that release to finish before showing.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
