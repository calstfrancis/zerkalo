<!-- zerkalo-release: 0.61.0 -->
### What's new

Zerkalo now counts the pages your marker counts.

**Body pages.** The page counter under the preview also says how many pages are body text: `3 / 12 · 9 body`. The title page, table of contents, references and appendices are left out. A page that the body shares with the references counts as body, the way page limits are usually read. Hover over the counter to see the breakdown, such as "12 pages: 1 title page, 9 body, 2 reference pages". Documents Zerkalo generated are understood as they are. In a document you set up yourself, put `<body-start>` on the first heading after the title page, and `<appendices>` on appendices whose headings don't start with "Appendix". The extra number only appears when it differs from the total.

**Page limit.** Set one in Settings → Page limit, or add a `// @zerkalo-page-limit: 10` line near the top of a document to set it for that document alone. The counter then reads `9 of 10 body`, the tooltip says how many pages you have to spare, and the counter turns orange once the body runs over.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
