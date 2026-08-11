# Zerkalo v0.22.0 "Steady Hand"

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

**Backups now happen on their own.** Once a backup location is set up, Zerkalo saves and sends a version automatically every so often while you write, and once more on the way out if anything's still unsent — so backing up no longer depends on remembering the sync button. It's quiet by design: failures show as a small toast rather than a popup, so an offline moment doesn't interrupt you, and it never blocks quitting for more than a few seconds.

**Error messages can be copied now.** Every notice and error dialog has a Copy button next to OK, so the message — a sync failure, an export failure — can be pasted into a bug report instead of retyped by hand from a screenshot.

**Plain language throughout setup, sync, and backup screens.** "Repository," "remote," "commit," and "clone" — terms that meant nothing to a non-technical user — are now described in terms of what they do: "online copy," "backup location," "save a version."

**Help window redesigned for clarity.** Tabs are now a proper libadwaita view switcher instead of old-style notebook tabs, and the panel's typography was cleaned up — no color, just weight, scale, and whitespace doing the work of hierarchy.

**Fixed:** sync could fail outright if the system's git was configured to sign commits — Zerkalo's own commits now skip signing, without touching the signing setup for anything else on the system.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
