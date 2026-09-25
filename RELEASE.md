# Zerkalo v0.31.0 "Linked Glass"

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

The editor and the preview now follow each other exactly, and a handful of scrolling annoyances are gone.

**Exact preview ↔ editor sync.** Click anything in the preview and the editor jumps to that exact spot in the source — the right character, not just the nearby paragraph. Ctrl+click in your text (or run "Show Cursor in Preview" from the Ctrl+K palette) to go the other way: the preview scrolls there and briefly highlights the line. It works on code too, so Ctrl+clicking a `#lorem(50)` or a function call finds the text it produced. Both directions use the positions Typst itself records while compiling, instead of guessing by matching text from the PDF.

**No more jumping to the top.** Right-clicking in the editor, or coming back to it after scrolling while something else had focus, could throw the view back to an old position — often the top of the file. The editor now always knows where you actually are.

**Jump to error works every time.** Jumping to an error used to select the right line but sometimes leave it off screen. It now always scrolls there, and an error inside the hidden template setup shows the template first instead of landing on invisible text.

**Typewriter scrolling works again**, including at the end of the document, where the new text usually is.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
