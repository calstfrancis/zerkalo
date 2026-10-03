<!-- zerkalo-release: 0.44.1 -->
### What's new

Comment out the bibliography.

**Commenting out `#bibliography` no longer breaks your document.** If you comment out a document's `#bibliography` line to drop the reference list, the document used to fail with "label does not exist" for every citation, because Typst can't print a citation without a bibliography. Zerkalo now adds a hidden one behind the scenes, in the preview, PDF, print and Word export. Your citations and footnotes still appear, and the reference list doesn't. Zerkalo uses the file the commented-out line names, with that line's citation style, or your Kartoteka vault if that file is gone. Uncomment the line to bring the list back.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
