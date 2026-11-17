<!-- zerkalo-release: 0.61.1 -->
### What's new

A security fix, and nothing else.

**Updated zlib.** Zerkalo uses libgit2 to set up projects and to read Kartoteka vaults, and libgit2 is built with the zlib compression library. A heap buffer overflow was found in that library (RUSTSEC-2026-0544), and this release updates it to the fixed version. Nothing you see or do in Zerkalo changes.

For what 0.61.0 added (a body page count and a page limit), see the changelog.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
