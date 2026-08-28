# Zerkalo v0.28.3 "Clear Reflection"

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

**Clicking the preview now actually jumps to the matching source line.** It used to silently require holding Ctrl while clicking, with no hint anywhere in the app that this was needed — a plain click, the obvious gesture, had no visible effect. Ctrl is no longer needed; a single click jumps to the nearby paragraph, and double-click still jumps to the exact word.

**The preview no longer auto-scrolls to follow the cursor as you type.** That behavior, added in 0.27.0, approximated scroll position from the cursor's line number alone — with no way to account for how unevenly source lines translate into rendered space (a heading, an image, or a dense paragraph all take up very different amounts of page height per line), it routinely landed the preview somewhere visually unrelated to what was actually being edited. Click-to-jump — preview to editor — is the direction that's actually reliable, so that's what's left.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
