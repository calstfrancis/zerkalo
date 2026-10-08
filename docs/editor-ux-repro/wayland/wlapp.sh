#!/usr/bin/env bash
# usage: wlapp.sh <steps.py>  — headless GNOME Shell (Wayland) + Zerkalo, driven by wl_drive.py
set -u
S="$(cd "$(dirname "$0")" && pwd)"
STEPS_FILE="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
cd "$S/../../.."
H=$(mktemp -d /tmp/zk-wl.XXXXXX)
export HOME=$H XDG_CONFIG_HOME=$H/.config XDG_DATA_HOME=$H/.local/share XDG_CACHE_HOME=$H/.cache XDG_STATE_HOME=$H/.local/state
mkdir -p $XDG_CONFIG_HOME $XDG_DATA_HOME/zerkalo $XDG_CACHE_HOME
printf '[user]\n\tname = T\n\temail = t@example.com\n' > $H/.gitconfig
W=$H/Documents/Zerkalo; mkdir -p $W
python3 $S/mkdoc.py $W/main.typ
git -C $W init -q; git -C $W add -A; git -C $W -c user.name=T -c user.email=t@e.com commit -q -m i; git -C $W remote add origin https://example.com/x.git
echo -n "${ZVER:-$(grep "^version" Cargo.toml | head -1 | sed "s/version = \"\(.*\)\"/\1/")}" > $XDG_DATA_HOME/zerkalo/.welcome_version
unset DISPLAY
export WAYLAND_DISPLAY=zk-test-$$
gnome-shell --headless --wayland --no-x11 --wayland-display=$WAYLAND_DISPLAY --virtual-monitor 1400x900 > $S/shell.log 2>&1 & SP=$!
sleep 6
if [ -n "${RUNTIME422:-}" ]; then
  B=${ZBINDIR:-$PWD/target/release}
  flatpak run --filesystem=$H --filesystem=$B:ro --socket=wayland --nosocket=x11 --socket=session-bus --share=network \
    --env=HOME=$H --env=XDG_CONFIG_HOME=$XDG_CONFIG_HOME --env=XDG_DATA_HOME=$XDG_DATA_HOME \
    --env=XDG_CACHE_HOME=$XDG_CACHE_HOME --env=XDG_STATE_HOME=$XDG_STATE_HOME \
    --env=GDK_BACKEND=wayland --env=ZERKALO_TRACE_SCROLL=1 \
    --command=$B/zerkalo org.gnome.Platform//50 > $S/app.log 2>&1 & AP=$!
else
  GDK_BACKEND=wayland ZERKALO_TRACE_SCROLL=1 ${ZBIN:-./target/release/zerkalo} > $S/app.log 2>&1 & AP=$!
fi
sleep 30
W=$W python3 $S/wl_drive.py "$STEPS_FILE"
kill $AP 2>/dev/null; pkill -P $AP 2>/dev/null; sleep 1
kill $SP 2>/dev/null; sleep 2
rm -rf $H
