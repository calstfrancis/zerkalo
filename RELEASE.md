# Zerkalo v0.29.0 "Open Ledger"

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

**Documents can belong to more than one category.** Categories used to be a single flat field on a document, so it could only ever hold exactly one — not even two sibling categories under the same parent. Categories now work the same many-to-many way tags already did. The single-document category dialog is now **Edit Categories…**, a checkbox list grouped under each category's parent with inline "New category…" creation, replacing the old single free-text field. A new bulk **Categorize…** action in the selection bar assigns categories to several selected documents at once, dragging a document onto a sidebar category now adds to its categories instead of replacing them, and document rows show a colored chip per category, clickable to filter, the same way tags already do. A category's right-click menu also gained a **Recolor…** item.

**The bulk "Tag Documents…" dialog can create tags inline.** It was missing the inline tag-creation row the single-document "Edit Tags…" dialog already had — both now work the same way.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
