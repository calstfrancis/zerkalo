<!-- zerkalo-release: 0.61.2 -->
### What's new

Apostrophes that point the right way.

**Apostrophes after brackets.** In writing like "(USA)'s", "[sic]'s", "the U.S.'s" or a citation followed by 's, the apostrophe used to print as an opening quote, which looks like a backwards comma and stands out in some fonts. After a number, as in "1990's", it printed as a prime mark instead. The cause is in Typst, which picks a quote's shape from the character just before it. Zerkalo now sets these as a proper apostrophe in the preview, the PDF and the Word, HTML and EPUB exports. Real quotation marks, code and maths are left alone, and so is any document that sets its own `smartquote` rules.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
