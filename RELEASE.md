# Zerkalo v0.34.0 "Gathered Glass"

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

Clearer footnote citations, and images that stay with your document.

**Several citations in a row read clearly.** In footnote styles, back-to-back citations used to run together — marks 11 and 12 printed side by side looked like one long number. There's now a small superscript comma between them.

**Or combine them, if your style calls for it.** Some citation styles want several sources cited at one point to share a single footnote — Chicago notes, for example, lists them all in one note separated by semicolons. The Style menu (the citation-style button above the editor) now has a "Several citations in a row" choice: keep Zerkalo's one note each, or follow what your style specifies. The choice is saved in the document, so exports and printing follow it too.

**Footnote marks are readable in every font.** Some fonts carry broken superscript measurements that shrank footnote marks to tiny specks — GOST type B is one. Zerkalo now notices and sizes them properly, keeps every mark the same size, and a citation in a heading no longer gets a bigger, bolder mark than everywhere else.

**Pictures are kept with your document.** Inserting an image used to assume the picture was already next to your document. Now Zerkalo copies it into an assets folder beside the document first, so it keeps working if the original is moved or deleted, and it's included in your backups.

**A horizontal-rule button** (―) joins the format bar, next to the page break.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
