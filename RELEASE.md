<!-- zerkalo-release: 0.61.4 -->
### What's new

Bibliographies that keep up with Kartoteka and Zotero.

**Changes in Kartoteka reach your document.** Adding a date or fixing a title in Kartoteka showed up in Zerkalo's Citations panel but not in the preview or PDF, even after compiling again. The panel was reading the vault's sources live, while the compile read a summary file (`library.yml`) that Kartoteka didn't rewrite after every edit. Zerkalo now compiles from the sources themselves, so the panel and the document always agree. Kartoteka 0.26.1 also keeps that summary file current, for anything else that reads it.

**The project's copy of your bibliography stays current.** When your bibliography lives outside the project, Zerkalo keeps a copy inside it (`references.bib` or `references.yaml`) so the document compiles on any computer. That copy was refreshed only for the bibliography named in Settings, so a document using your Zotero library while Settings named your Kartoteka vault never got updates. Each copy now follows its own original, and on the computer that has the original, the preview reads it directly.

**No keystroke needed.** A Zotero auto-export or a Kartoteka edit used to sit unseen until you next typed something. With Auto compile on, the preview now updates by itself within about a second.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
