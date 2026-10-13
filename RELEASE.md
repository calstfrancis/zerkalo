<!-- zerkalo-release: 0.50.4 -->
### What's new

Templates that hold, and a quieter problem list.

**Templates do what they say.** Applying a template to the document you're in now refreshes the preview straight away, even with manual compile on. Before, the preview went blank until you pressed Compile, so a template's line spacing, fonts and title page looked as if they had never been applied. New from Template and Change Document Style now open on the Template tab where Your Templates live, Enter saves a new template, a name with "&" in it shows up, and each saved template's summary says its line spacing.

**Only this document's problems.** The problem list no longer shows leftovers from other files you opened earlier in the session. A Kartoteka `library.yml` outside your project folder no longer gets reported as missing, and the "variable fonts are not supported" warnings are gone: the suggestions engine (tinymist 0.15.8) now uses the same Typst version as the preview.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
