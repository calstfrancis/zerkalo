<!-- zerkalo-release: 0.62.0 -->
### What's new

A find bar that keeps its place and finds what you mean.

**Ctrl+F, then Ctrl+F again.** The first press opens search, with your selected text already in it; each press after that goes to the next match (Shift+Enter goes back). Escape closes it and keeps your search for next time. Ctrl+H opens it with Replace, which now waits folded under the search until you want it.

**Smooth in long documents.** Searching as you type now starts at three letters, after a short pause, and refines the match from where you started instead of jumping onward with each letter. The match you're on is in your accent colour and centred in the editor, every other match is highlighted, small marks beside the scrollbar show where they all are, and the preview scrolls to follow. The count stays right while you keep writing.

**Finds what you mean.** Accents are ignored, so "zizek" finds "Žižek" (the á button matches them exactly). Typing a capital letter makes the search case-sensitive. A space also matches a line break, so a phrase that wraps onto the next line in the source is still found.

**Across the project.** "All files" in the find bar hands your search to Find in files (Ctrl+Shift+F), which now searches what's on screen in open documents, unsaved typing included, and matches the same way.

**Safer replacing.** Replace All no longer changes text that simple mode hides, and stays one undo step. The search toggle in the status bar has moved to the left, beside "writing history", and screen readers now hear each result.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
