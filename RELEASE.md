# Zerkalo v0.36.0 "Tempered Glass"

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

Your work is saved as you go, and PDF import works again.

**Autosave.** Zerkalo now saves your document about three seconds after you stop typing, and whenever you move away from it — switching to another window or tab, compiling, opening Export, or quitting. What's on disk is never more than a few seconds behind, and your backups pick up your writing without you having to save first. It's on by default; the "autosave" word in the bottom status bar turns it off and on (bold means on). Autosave doesn't add entries to Version History — pressing Ctrl+S still does that — and with it off, Zerkalo behaves exactly as before.

**Importing a PDF no longer gives a blank document.** Ordinary text is full of characters that mean something to Typst — the @ in an email address, a $ price, asterisks, underscores, the // in a web address — and one of them could stop the imported document from compiling, leaving the preview empty. They now come through as plain text. Two-column PDFs come out in reading order instead of with their columns interleaved, headings are guessed more carefully, bulleted lines become real lists, and a scanned PDF (pictures of pages, with no text in it) gets an explanation instead of an empty document.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
