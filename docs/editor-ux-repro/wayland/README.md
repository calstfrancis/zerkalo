# Wayland click/scroll harness

Runs Zerkalo inside a headless GNOME Shell (the same compositor as a real
GNOME session) and drives it with real pointer and keyboard events through
mutter's RemoteDesktop D-Bus API, taking screenshots through its ScreenCast
stream. Every vertical scroll is logged via `ZERKALO_TRACE_SCROLL=1`.

    dbus-run-session -- ./wlapp.sh wl_fuzz.py               # host GTK
    RUNTIME422=1 dbus-run-session -- ./wlapp.sh wl_fuzz.py  # GTK 4.22 (flatpak runtime)

`dbus-run-session` is required: it gives the shell and app their own bus, so
nothing touches the real desktop. Needs `target/release/zerkalo`, gnome-shell,
python3-gi with GStreamer's `pipewiresrc`, and (for RUNTIME422) the
org.gnome.Platform//50 runtime. `ZBIN=`/`ZBINDIR=` point at another build,
`ZVER=` must then match its version (or its What's New window blocks input).

- `wl_fuzz.py` — random clicks/right-clicks/menu dismissals/typing/wheel;
  flags any click or menu action that moved the view. `SEED`, `STEPS`.
- `wl_typed.py` — type, then right/left-click high/mid/low and click away.
- `wl_menu.py` — right-click menus dismissed every way, cursor far away.

Findings (2026-10-08): see EDITOR-UX-PLAN.md Phase 4.
