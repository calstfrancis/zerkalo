<!-- zerkalo-release: 0.54.0 -->
### What's new

Citation styles that match their manuals.

**Headings are the right size and weight.** Typst's own bold, enlarged headings showed through every style, so SBL's plain headings, APA's 12-point headings and ASA's roman and italic levels all came out bold. Each style now sets exactly what its manual says, and headings sit on the line grid instead of crowding the text.

**The right citation style behind each name.** Vancouver lists its authors again (the built-in style had dropped them). ASA is formatted as ASA and Harvard as Cite Them Right, where before they borrowed APA and Chicago. SBL and Turabian include the place of publication their manuals still require. A new document titles its reference list as the style does: References, Works Cited, Reference list.

**Choosing a style sets up the page.** Picking a style in the template dialog now also sets margins, line spacing, font size, page-number position and heading numbering to what the style asks for, and every row stays editable. "1.5 Lines" and "Double" now match Word's spacing, and paragraphs are separated by the half-inch indent alone.

**Per-style details.** MLA gets a centred title, a day-month-year date, "Surname 2" page headers and a centred Works Cited. APA gets the student title page, an abstract on its own page and run-in fourth and fifth levels. SBL gets its all-capitals title page and note layout. IEEE spans the title across both columns and numbers sub-headings A. and 1). GOST drops the trailing dot from section numbers and is set in Russian.

**Page locators follow the style.** The citation panel inserts `@key[12]` for styles that print a bare number and `@key[p. 12]` for APA, Harvard and IEEE.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
