# Zerkalo v0.25.1 "Quiet Anchor"

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

**Right-clicking in the editor no longer throws the view to the top of the document.** This hit hardest on highlighted spelling errors, but a plain right-click anywhere could trigger it too — and so could dismissing the menu by clicking away from it. The scroll-position tracker was recording the viewport position on a fixed timer, but a context menu (native, or Zerkalo's own spell-suggestion popover) can stay open longer than that timer allows; any auto-scroll GTK did after the timer expired got recorded as "real" and restored later, throwing the view to wherever the cursor happened to be. It's now gated on actual keyboard focus instead, so nothing gets recorded for as long as a menu holds it.

**The Find bar searches as you type.** Previously it only jumped to a match on Enter or the next/previous buttons — typing a query did nothing until you triggered one of those. Now the view and cursor jump to the first match live, as expected from Ctrl+F in most editors.

**The print dialog's Long Edge / Short Edge duplex options were swapped.** Picking "long edge" was actually sending the system print portal a short-edge flip, and vice versa. Fixed.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
