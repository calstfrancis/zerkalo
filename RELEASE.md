# Zerkalo v0.26.2 "Clean Break"

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

**Paragraph breaks now actually look like paragraph breaks.** `#set par`'s `spacing` (the gap between paragraphs) had been set equal to `leading` (the gap between lines within one), so a new paragraph was marked only by its first-line indent — no visible gap at all, the same as an ordinary wrapped line. Every generated template, and the header bar's legacy "Style" dropdown's built-in styles, now give paragraphs a break a reader actually sees, while keeping that gap an exact multiple of the line pitch so it doesn't disturb the even baseline grid academic citation styles (MLA, APA, Chicago, Turabian) rely on.

**The header bar's "Style" dropdown could silently fall out of sync with Update Template Settings.** Picking a style there rewrites a document's headings, title page, and style marker directly, but never updated the settings file Update Template Settings reads from — so reopening that dialog after using the dropdown could show the *previous* style, and pressing Apply would silently regenerate the document back onto it, discarding the change made through the dropdown. Fixed at the source and defensively: the dialog now also re-derives the citation style from the document itself when it opens, the same way it already does for font, size, paper, and margin.

**Template Settings no longer lets "LaTeX Look" quietly override a font choice with no explanation.** That style has always used its own fixed typography (New Computer Modern, tight leading), but the Body Font and Line Spacing rows stayed fully editable with nothing indicating a choice there wouldn't take effect. They're now greyed out with a tooltip while LaTeX Look is selected.

**The embedded Typst compiler moved from 0.14 to 0.15.1**, keeping pace with upstream and unblocking the newer `typst-kit` package/font APIs. Internal — no formatting or citation-output change intended, and the full test suite (including tests that compile real documents in every citation style and CV layout) confirms none occurred.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
