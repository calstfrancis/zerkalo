# Zerkalo v0.28.0 "Home Ground"

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

**Selecting text and typing an opening bracket or quote now wraps the selection instead of replacing it.** Select a word and type `"` and it becomes `"word"`, with the original text still selected — the same auto-pairing behavior VS Code, Sublime, and other editors use. Works uniformly across all five pair characters the editor already auto-inserts while typing: `()`, `[]`, `{}`, `""`, and `$$`.

**Spell checking no longer shells out to a `hunspell` subprocess.** It now reads the same system dictionary files directly, in-process, via the same library the Helix editor uses for its own spell checking. No fork/exec per lookup, no dependency on a `hunspell` binary being on the host's PATH. This also closes out a bug from last release: the right-click suggestions popover hanging forever on "Checking…" was tangled up with the subprocess round-trip it no longer has to wait on.

**HTML export no longer needs pandoc.** It compiles through Typst's own HTML exporter now, in-process, the same way PDF export already does. The output is genuinely standalone — images embed directly in the file instead of being written out as loose media files, math renders as real math markup instead of an image, and footnotes/citations come out with proper accessibility roles. DOCX, ODT, LaTeX, and EPUB export are unchanged and still use pandoc.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
