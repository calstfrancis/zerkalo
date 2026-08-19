# Zerkalo v0.24.4 "Sound Footing"

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

**Save, Autosave, and closing the window could all silently fail on a write error** — a full disk, a permissions problem — instead of telling you. The worst case: choosing "Save All" from the "Save before closing?" dialog closed the window even if the save itself failed, discarding the unsaved document with no recovery path. Save and Autosave now show a toast when a write fails, and the window-close dialog now stays open and names the files that didn't save, instead of closing regardless.

**Printing with an imposition layout (booklet, two-up) could briefly freeze the app** on a large document — the page-rearrangement work ran on the main thread before the print job even started sending. It now runs on the same background thread the print job already used.

Also updates the GTK4/libadwaita/GtkSourceView toolkit bindings to their current releases (no user-visible change, but closes several years of upstream bug and soundness fixes) and fixes two dependency security advisories flagged by a newly-added `cargo audit` CI check.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
