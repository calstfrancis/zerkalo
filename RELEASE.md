# Zerkalo v0.37.0 "Clear Glass"

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

Mistakes are explained, not shouted.

**A problem no longer takes over the screen.** When something in your document can't be understood, the preview keeps showing your last good version, dimmed, with a small "Preview paused" note, instead of turning into a wall of red text. New problems are shown a moment after you stop typing, so half-typed code isn't flagged while you're still writing it, and warnings become quiet "notes" that never pop up.

**The Problems panel.** Each problem is a simple card: a plain-language headline, the line it happened on with the exact spot underlined, a sentence of advice, and one main button — Fix when Zerkalo can fix it, otherwise Show me. Typst's own wording is still there, behind "Show technical details" in the panel header, for when you want to search the forum. F8 and Shift+F8 step through the problems.

**Common slips are recognised.** A price like $5, an asterisk or underscore with no partner, an email address, a stray # and an unclosed <label each get their own explanation, including how to write the character literally.

**Fixes you can trust.** A fix changes only the characters it needs to, at the spot reported, and undoes in one step. A stray $, *, _, <, @ or # can be shown as ordinary text, brackets are closed where they were opened, and if a fix doesn't help you're offered Undo straight away. Fix used to patch the wrong file when the problem was in a file you didn't have open, and hover "Fix It" never appeared at all; both are fixed.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
