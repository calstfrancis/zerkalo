# Zerkalo v0.33.0 "Polished Glass"

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

A follow-up polish release to 0.32.0's document-library and export changes.

**Move stray documents in.** If a document is saved outside your Zerkalo folder — from before name-only New Document existed, or dragged in from elsewhere — the Library's document menu now has "Move into Zerkalo Folder…". It moves the file and its comment/template sidecars in one step, picking a free name if there's a collision. It won't move a document that's currently open, so it can't leave an editor tab pointing at a file that no longer exists.

**Ctrl+Shift+E follows your Export settings.** The quick-export shortcut used to always write a PDF next to the source file with no further action. It now writes to wherever the Export dialog remembers, and opens the result if "Open when finished" is on — including in Pereplyot.

**Smaller things:** New Document now suggests a name that's actually free ("Untitled 2" when "Untitled" is taken). The Library and Template buttons in the header now match, both with an icon. The Compile Errors panel starts collapsed the first time it's showing only warnings, rather than opening full height. Cited-references BibTeX export is properly indented. The Reference Manager's own cited-only export now also supports `.yaml` and Kartoteka vaults, not just `.bib`.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
