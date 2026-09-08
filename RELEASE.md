# Zerkalo v0.29.2 "Quiet Guard"

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

A hardening and cleanup release, prompted by a deep codebase review — nothing broken before, but a few things quietly stronger now.

**GitHub sync is more careful with your token.** It's now passed to git via environment variables instead of a command-line argument, so it can't be recovered from the process list by another user on the same machine.

**Sync, PDF export, and Print now stop and tell you if a save fails**, instead of silently proceeding with the old content on disk. Previously, if a document failed to save — a full disk, a permissions problem — those three actions would quietly commit, export, or print the stale version with no visible difference from success.

Also: two dependency security advisories resolved, two mutex-poisoning bugs fixed (a panic in one spot could previously cascade into repeated panics elsewhere), and a full audit of every suppressed dead-code warning in the codebase — some genuinely unused code removed, and a few complete-but-unwired features flagged for a future decision rather than silently deleted.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
