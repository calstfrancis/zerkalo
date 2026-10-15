<!-- zerkalo-release: 0.51.0 -->
### What's new

Count in-text citations.

**Choose whether citations count toward your word total.** In a document that uses an author-date style (APA, Chicago author-date, Harvard, MLA), a new "citations in count" switch sits beside the word count in the status bar. Off, as before, citations are left out. On, each one counts as the words it prints: "(Smith, 2020)" is two, "(Smith & Jones, 2019)" is four, and a page locator like `@smith2020[p. 5]` adds its own words. The text comes from the same citation style the finished document uses, so the number matches the printed result. The choice is saved in the document and applies to the status bar, goal ring, Document Statistics, outline and project total.

**Smaller fixes.** A page locator after a citation no longer counts as prose, punctuation after a citation no longer counts as a word, and the section count beside the breadcrumb now ignores comments and markup like the other counts.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
