# Zerkalo v0.32.0 "Homeward Glass"

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

Your documents now have one home, and exports go wherever you want them.

**New documents just ask for a name.** Creating a document — blank, from a template, from the Library, or with Save As — no longer opens a file-save dialog. You type a name and Zerkalo does the rest: it adds the `.typ`, saves the file in your Zerkalo folder, and lists it in the Library. Documents saved in other folders couldn't see the project's fonts and bibliography, which broke citations and fonts in confusing ways; that can't happen by accident any more.

**Export where you choose.** The Export dialog now asks where to save (and remembers), and can open the file when it's done — PDFs and EPUBs in Pereplyot if you have it installed, everything else in your default app.

**Export just the references you cite.** Export can also write a `.bib` or `.yaml` file containing only the bibliography entries your document actually cites — handy for sending a paper to a co-author or journal without your whole library attached. It works from a `.bib` file, a `.yaml` file, or a Kartoteka vault.

**The Library is easier to find.** Its button is back at the top left of the window, next to the sidebar toggle, and the document title's dropdown has a *Show All in Library* row.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
