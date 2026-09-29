# Zerkalo v0.35.0 "Leaded Glass"

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

Word files a designer can drop straight into InDesign or Canva, and notes that count together.

**Export to Word for InDesign or for Canva.** The Export dialog has two new Word formats, built into Zerkalo, so nothing extra needs installing. Every paragraph uses a named style (Heading 1, Body Text, First Paragraph, Block Quote, Bibliography and so on), set in your document's own fonts, sizes and spacing. InDesign can keep those styles or remap them to its own in one step. Citations come out exactly as in your PDF. In the InDesign file, citations, footnotes and margin notes are real footnotes that reflow with the text. Canva has no footnotes, so the Canva file numbers them in the text and lists them as endnotes at the end. Page layout such as columns and drop caps is left for the layout program.

**One numbering for every kind of note.** Footnotes, citations and margin notes now count as one sequence, and any two marks that touch get a small comma between them (¹,²). Margin notes used to keep their own separate count. New documents get this automatically; for an existing one, reopen Template… and apply it once.

**Undo stays available.** The Undo and Redo buttons used to grey out after every compile or error, even though Ctrl+Z still worked. They now stay in step with your edits.

**More room for your outline.** The Packages and Comments panels start collapsed, leaving the sidebar to the outline and citations.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
