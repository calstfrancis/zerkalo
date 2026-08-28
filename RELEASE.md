# Zerkalo v0.28.4 "True Anchor"

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

**Spell check no longer flags contractions and possessives as misspelled.** "Doesn't" was being checked as "doesn" — never a real word on its own — because the apostrophe split it into two pieces. An apostrophe with a letter on both sides now stays attached to the word it belongs to; a genuine quote mark still isn't absorbed.

**Search results, click-to-jump from the preview, and heading navigation now actually scroll the editor to show what you jumped to.** The cursor and match count were always updating correctly — the view itself just wasn't scrolling to follow. Root-caused rather than patched: the editor's real scroll adjustments were never bound to it, because its direct parent is a plain `Overlay` (used for the "start writing" placeholder text), and `Overlay` doesn't implement the interface `ScrolledWindow` needs to connect to it automatically. Every scroll call was quietly succeeding against a disconnected adjustment nobody was listening to. Fixing the binding once, at the root, fixes every scroll-to-position call site in the editor at once — not just search.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
