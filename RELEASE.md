# Zerkalo v0.29.4 "Steady Hand"

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

A simplification and a data-safety fix.

**The compile trigger is now a plain Auto / Manual choice.** The old three-way "Auto / On Save / Manual" pill in Settings, and the matching status-bar toggle, are down to two options — "On Save" behaved exactly like Manual in practice, since Save already recompiled either way, so it was one option too many. The status-bar button now reads "auto compile" or "manual compile" depending on which is active, and **Manual is the new default** for anyone starting fresh.

**Applying a template to a document that was never templated no longer throws away what you wrote.** If you started typing before ever running "New from Template" or "Change Document Style…", applying one used to replace the whole file with the fresh template's placeholder text — your own writing survived only in a `.typ.bak` backup. Now it's kept: your existing text becomes the new document's body, under the template's regenerated preamble, the same way an already-templated document's body is preserved when you change its settings.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
