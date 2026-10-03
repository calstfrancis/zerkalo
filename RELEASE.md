<!-- zerkalo-release: 0.44.0 -->
### What's new

Your vault stays put.

**Zerkalo no longer forgets your Kartoteka vault.** Settings used to refuse to save while the bibliography field held a vault folder, because it was checking for a file and a vault is a folder. The only way to save any other setting was to clear the field, which quietly forgot the vault. Settings now accepts a vault folder, and a document whose `#bibliography` line is missing or commented out goes back to using it.

**Exporting no longer changes your bibliography.** The cited-references file Export can write (`<document>-references.bib` or `.yaml`) and the copies made by "Freeze for submission…" are never picked up as the project's bibliography, even when they land in the project folder.

**One Print window.** Printing now happens entirely in Zerkalo's own Print window. It has a Printer row that remembers your last choice, and Print sends the job straight to that printer with your copies, two-sided and colour settings — no second system dialog that forgot your options. The system dialog is only used if no printers can be found.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
