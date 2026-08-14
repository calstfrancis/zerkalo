# Zerkalo v0.23.0 "Open Shelf"

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

**Point the bibliography at a Kartoteka vault folder, not just a `.bib`/`.yaml` file.** Settings → Bibliography — or a new folder-icon button right in the citation sidebar — now lets you choose a Kartoteka vault directory directly. Entries load through the same library code Kartoteka itself is built on, and refresh live the moment something changes in the vault: add or edit an entry in Kartoteka and it shows up in the `@`-autocomplete popup, the citation sidebar, and the reference manager within about a second, no restart needed. A new **"K" button** beside it launches Kartoteka directly, or focuses it if it's already running. Plain `.bib`/`.yaml` files keep working exactly as before.

**File History…**, in the hamburger menu (next to Browse Snapshots) and the Ctrl+K palette, shows a document's git commit history and diffs without leaving the app.

**The hamburger menu is down to 20 rows in five clusters** (from 26 in seven). Document Fonts, Set Up Zerkalo, Backup Locations, and Tools moved into Settings, where that kind of configuration is easier to find. Settings itself got a plain-language pass — "Debounce" is now "Compile delay," and a few other rows dropped jargon or a raw file path shown as description text.

**Screen readers couldn't tell many icon-only buttons apart.** Folder/file browse buttons, the CV-mode switch, per-tag edit and delete buttons, and a few others had no accessible name — they're labelled now.

**Internal:** clicking to jump from the preview to a line or word no longer blocks the interface while `pdftotext` runs; the largest file in the codebase (`template_dialog.rs`, 7,585 lines) is now a five-file module with no behaviour change.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
