# Zerkalo v0.28.5 "Steady Current"

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

**Code editor scrolling is fixed.** The previous release (0.28.4) fixed search/click-to-jump/heading-navigation not scrolling the editor into view, but the fix itself introduced a much bigger regression: mouse wheel, keyboard, and the scrollbar all stopped scrolling the editor at all. The cause was the same `Overlay` wrapper the earlier fix touched — binding the editor's scroll position directly to it made GTK's own auto-generated viewport and the editor fight over the same scroll offset, and the fight won. The editor view is now wired up the normal, correct way (a direct child of its scroll container, no `Overlay` in between), so both problems are gone: scrolling works, and jump-to-position still works too. Verified live in a running instance before this release, not just by the test suite.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
