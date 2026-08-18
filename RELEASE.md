# Zerkalo v0.24.3 "Steady Hand"

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

**Fixes a crash after editing the bibliography source.** A large `.bib` file (Zotero exports commonly run to hundreds of KB) was being fully re-read, re-parsed, and re-serialized on every single compile — and a compile fires on every debounced keystroke. Sustained typing while a large bibliography was configured could pile up that work faster than it was freed, matching an out-of-memory crash exactly: the window would close or disappear entirely with no warning. Fixed with a cache keyed by the file's path and modification time, so a stable bibliography is now read and sanitized once instead of on every compile — confirmed empirically: a cold compile of a 623KB bibliography took ~340ms, every warm compile after that ~1ms.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
