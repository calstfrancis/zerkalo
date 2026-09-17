# Zerkalo v0.30.0 "Clear Reflection"

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

A guided tour for new users, and a more legible help overlay.

**A guided first-run tour** walks through the essentials — the editor, the preview, template setup, the library, compile mode, and backup — in seven short steps, shown automatically the first time Zerkalo runs and replayable anytime from ☰ → Help & About → Take the Tour.

**The F1 "What Things Do" overlay is now reachable from the menu**, not just the F1 shortcut, and now also works inside New from Template / Change Document Style, labelling that dialog's own controls the same way it already covered the main window. That dialog also opens with a short banner explaining what it's about to do.

**The F1 overlay's bubbles no longer crowd or overlap each other in busy areas like the header.** Two bugs caused it: bubbles could land edge-to-edge with no visible gap, reading as one unreadable block; and a last-resort fallback in the placement search skipped its own overlap check, which a crowded header reliably hit and pinned two bubbles to the same spot. Both are fixed.

**Change Document Style no longer shows a dead "Create Document" button** alongside "Apply to Current" — clicking it used to do nothing.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
