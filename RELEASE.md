<!-- zerkalo-release: 0.50.6 -->
### What's new

Crashes leave a record.

**If Zerkalo crashes, it now says why.** It writes the error, the line of code it happened on and a backtrace to `crash.log` beside its log file. For the flatpak that's `~/.var/app/io.github.calstfrancis.Zerkalo/data/zerkalo/crash.log`. Until now that detail went only to a terminal, which isn't there when Zerkalo is started from the app menu, so a crash left nothing to fix it from. Nothing else in the app changes.

**The log is readable again.** Background libraries were writing a line for every network check and git lock file, which buried everything else. They now log only warnings and important events.

---

### Full changelog

See [CHANGELOG.md](CHANGELOG.md).
