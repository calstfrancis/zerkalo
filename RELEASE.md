# Zerkalo v0.39.0 "Framed Glass"

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

Moving between documents is now something you can see.

**A real tab bar.** Open documents sit in a bar above the editor: equal-width tabs that share the space, a close button on each, drag to reorder, and a dot on any tab with unsaved changes. Closing a tab with unsaved changes asks whether to save. Right-click a tab for Duplicate (a copy of what the tab shows, unsaved edits included), Close, Close Others, Close to the Right, or Delete File. The old hidden "Open tabs" button is gone.

**Your project's files, in the sidebar.** The Outline | Symbols switch now has a third segment, Project files, showing every file in the project as a tree. Press F6 to jump to it.

**Two fixes.** Switching to a document that was already open could crash Zerkalo when autosave was on; it no longer does. And a bibliography whose full path is written in an included chapter (say, a Kartoteka library) is now found, instead of failing with "file not found".

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
