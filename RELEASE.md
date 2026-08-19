# Zerkalo v0.24.6 "Steady Glance"

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

**Fixes a crash that hit reliably whenever a Typst compile error's inline popup closed.** Moving the mouse off an error-underlined line, clicking anywhere in the editor, or the editor losing focus while the popup was open all triggered it — every time, with no warning, the window would just exit. The popup's dismiss handling and its own "closed" signal both tried to update the same tracked state simultaneously, which Rust correctly refuses; as a panic inside a GTK callback, the process can't recover and exits immediately rather than showing an error.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
