# Zerkalo v0.38.0 "Kind Glass"

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

When something goes wrong, Zerkalo now tells you what happened and what to do next.

**A failed save never slips by.** If a document can't be saved — the disk is full, the folder is read-only or gone, a network drive dropped out — a bar above the editor names the file and the reason in plain words, and reassures you that your changes are still in the window. It stays until the save goes through, with Try again and Save a copy elsewhere. Autosave, Save, Export, Print and Sync all report to the same bar. Save As used to fail silently; now it tells you.

**Stuck? Zerkalo helps you back out.** If the same problems keep coming back, or one lasts a couple of minutes, the Problems panel offers a "Still stuck?" section: a short before-and-after of what changed since the document last worked, a button that puts your files back to that version as one undoable step, and a button that copies a tidy help request — version, problems, and the lines involved, never the rest of your document — ready to paste into an email or the Typst forum.

**More mistakes in plain words.** A stray closing bracket, an unclosed quotation mark, a missing comma, mixing text with numbers, asking for something that isn't there, going past the end of a list and more now get a short headline and a sentence of advice instead of the engine's phrasing.

**Everything else says what happened, too.** Exporting, importing a Word document's images, adding an image, changing a font, moving to the trash and reading your settings now give a plain reason such as "the disk is full" or "Zerkalo isn't allowed to write there", rather than raw error text.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
