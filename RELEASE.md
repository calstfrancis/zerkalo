# Zerkalo v0.33.1 "Warm Glass"

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

A fix-and-polish release for the F1 "what things do" overlay, plus a handful of small visual touches elsewhere.

**The overlay no longer overlaps itself.** In a busy window — a long document title, a multi-file project, several header buttons — its bubbles could land on top of each other, or on top of a neighbouring control's own highlight. Both are fixed: a bubble that genuinely has no free space is left unlabelled that pass instead of forced into a collision.

**The overlay is legible in dark mode.** Its bubbles used the app's own background, which goes dark in dark mode — but a bubble sits over arbitrary window content, not just the app's own, so it lost contrast there. Now a fixed near-white card with fixed dark text, always.

**A few visual touches:** overlay bubbles lead with a small icon and size themselves to their text, the connector to each bubble's target is a gentle curve instead of a straight line, and bubbles fade in with a brief stagger when the overlay opens. The preview page has a very faint paper-like gradient instead of flat white, and status-bar toggles get a soft highlight on hover and keyboard focus.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
