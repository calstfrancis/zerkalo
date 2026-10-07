#!/bin/bash
# Runs one repro inside the GNOME 50 runtime (the GTK the flatpak ships) on a
# throwaway Xvfb display; the host side performs the xdotool/import command the
# script writes to ./go, so pointer and key events interleave with GTK's loop.
#   ./run.sh repro.py buggy "201 385"
D=$(cd "$(dirname "$0")" && pwd)
if [ -z "$INNER" ]; then exec xvfb-run -a -s "-screen 0 800x600x24" env INNER=1 "$0" "$@"; fi
rm -f "$D/go"
flatpak run --filesystem="$D" --socket=x11 --nosocket=wayland --env=GDK_BACKEND=x11 \
  --command=python3 org.gnome.Platform//50 "$D/$1" "${@:2}" 2>&1 | grep -v -i warn &
P=$!
for _ in $(seq 1 100); do [ -f "$D/go" ] && break; sleep 0.05; done
cmd=$(cat "$D/go" 2>/dev/null); [ -n "$cmd" ] && sh -c "$cmd"
wait $P; rm -f "$D/go"
