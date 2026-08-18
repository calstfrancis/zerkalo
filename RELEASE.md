# Zerkalo v0.24.1 "Clean Copy"

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

**A malformed date in one bibliography entry no longer breaks every citation in the document.** Zotero/BetterBibTeX exports occasionally produce a non-numeric date (`year = {Winter/Spring 2001}`) that Typst's bibliography loader rejects — and rejects the *whole file* over, not just that one entry, so every citation failed as "label does not exist" with nothing pointing at the bibliography as the actual problem. Zerkalo now reads the file the same lenient way its own citation panel already does, correcting just the unparseable date field rather than giving up on the whole file. When a bibliography genuinely can't be read, the error now says so plainly instead of a wall of misleading per-citation errors.

**A bibliography path outside the project — most commonly a Kartoteka vault — now works regardless of how it got into the document.** The previous release's fix only widened the sandbox for a path set in Settings; a `#bibliography(...)` line typed or pasted directly into a document is now caught too.

**The Packages and Comments sidebar sections can be collapsed** to just their header row, via a chevron button next to each — useful once a project's manuscript is stable and neither is needed for a while.

**Two editor interaction fixes.** Clicking a format bar button (Bold, a heading, Insert Table…) no longer moves the keyboard focus off the document — apply it and keep typing, instead of needing to click back into the text first. And clicking on error-underlined text now places the cursor there directly, instead of just dismissing the error's inline popup and needing a second click.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
