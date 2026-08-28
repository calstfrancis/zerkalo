# Zerkalo v0.28.2 "Sound Footing"

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

**No visible change on Linux.** This release is internal groundwork ahead of a planned Windows build. Config, session, and live-preview paths now resolve through one consistent, platform-aware helper instead of a mix of hardcoded `~/.config`/`~/Documents`/`/tmp` strings that only worked by assuming a Unix-style `$HOME`; absolute-path detection (used for external bibliography files and git remote targets) now also recognizes Windows-style absolute paths; and the document library no longer risks creating duplicate entries for the same file reached via two differently-cased paths on a case-insensitive filesystem.

The one thing you might actually notice: the live-preview temp directory moves from a hardcoded `/tmp/zerkalo_preview` to the standard cache directory.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
