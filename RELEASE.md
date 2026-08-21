# Zerkalo v0.25.0 "Steady Panes"

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

**The sidebar's collapse buttons actually finish the job now.** Citations, Packages, and Comments can each be minimized to just their header row — Citations gained this button for the first time, and Packages' moved to the far right of its bar to match the other two. Collapsing a section used to only reclaim space from its immediate neighbor, so collapsing both Packages and Comments left a dead gap that only a manual drag could close, and the order you collapsed vs. dragged in made the sizes visibly fight each other. Every section above a newly-collapsed one now grows to fill the freed space, all the way up to Outline if everything below it is collapsed.

**Dragging one sidebar divider no longer resizes sections you didn't touch.** Adjusting Citations, Packages, or Comments used to redistribute space proportionally across neighboring sections too — even from an unrelated divider drag, or just resizing the window — so a carefully-set split could shift on its own. Only Outline now flexes with the window; everything below it holds the exact height you gave it.

**The Sync button shows whether anything's waiting to be backed up.** A colour badge over its icon lights up amber the moment there are unsynced changes, and turns red if the last backup attempt failed — updated immediately after every save, not just periodically. The redundant plain Save icon is gone from the header now that Sync (which saves everything, then backs it up) sits right there doing the fuller job; Ctrl+S and the ≡ menu's Save row still do a fast, local-only save exactly as before.

**Smaller fixes:** the hamburger menu is simpler, with rarely-used items grouped into a few flyouts instead of crowding the top level; "Compile on Save" mode could show a stale preview after saving; moving a document to Trash (or permanently deleting one) could leave the Library disagreeing with what's actually on disk if the underlying file operation failed; and a long error message in the Packages panel could force the whole sidebar wide and lock it there.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
