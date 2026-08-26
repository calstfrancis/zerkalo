# Zerkalo v0.27.0 "Honest Glass"

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

**The running header setting no longer resets to "None" every time you reopen the template dialog.** With the title page turned off, the running header (and every other metadata field — Subtitle, Affiliation, Course, Professor, Date) was only ever written into the document from inside the title-page generator. No title page meant none of it was written at all, so the setting had nothing to read back on reopen. All of it now works independent of the title page.

**Right-click spelling suggestions are reliable again.** The popover no longer opens and closes instantly before you can read it, and — new in this release — it no longer hangs forever on "Checking…" once it does stay open. Both were timing races against the same right-click's button-release event; the fix now tracks whether the popover was actually dismissed instead of inferring it from a visibility check that turned out to have its own blind spot. The same instant-close bug was also fixed in the tab context menu and all four Library window right-click menus.

**The Tools window no longer claims pandoc is installed when it isn't.** It was treated the same as genuinely-bundled tools like git and tinymist, which always report "OK" regardless of whether the command is actually found — but pandoc isn't bundled. It's now checked for real, with normal install instructions when it's missing.

**The preview now follows your cursor as you type**, instead of only jumping when you click into it.

Also in this release: a round of interface cleanup — a redundant "Browse Documents" window removed (Library, on Ctrl+L, already does everything it did and more), a dead header Save button removed, inconsistent backup/sync wording standardized to "back up" throughout, Library's empty states now offer a way out instead of a dead end, and a stale "Update Template Settings" label fixed everywhere it had lingered after that menu row was renamed.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
