<!-- zerkalo-release: 0.63.2 -->
### What's new

A security and stability pass.

**Documents stay inside their project.** A document can no longer reach other files on your computer through an absolute path or a symlink. An external bibliography still works, as that one file, if it is a `.bib`, `.bibtex`, `.yml` or `.yaml`.

**Backup is careful about what it runs and where.** Zerkalo no longer runs a repository's own hooks or programs, and says which setting it refused. Your work folder backs up as before; any other git folder asks "Back up … with Zerkalo?" once on first Sync.

**Safer imports.** Word and OpenDocument files with unusual links, images or text no longer break the document, and oversized archives are capped.

**No more freezes.** Keyring and git checks now run in the background, so a slow one can't lock the window at startup, on the timer, or on close.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
