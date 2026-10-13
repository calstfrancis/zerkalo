<!-- zerkalo-release: 0.50.5 -->
### What's new

Security update.

**A networking library is patched.** `quinn-proto`, which some of Zerkalo's network connections go through, is updated to 0.11.19. It fixes five security problems published on 13 October that could let a remote server use up memory or crash the connection. Nothing else changes; everything in 0.50.4 "Kept Pattern" is still here.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
