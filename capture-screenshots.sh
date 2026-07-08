#!/usr/bin/env bash
# capture-screenshots.sh — capture a fresh screenshot of Zerkalo against demo data
#
# Runs the existing target/release/zerkalo binary (build one first with
# `cargo build --release` if it doesn't exist or is stale) under a fully
# isolated home — HOME, XDG_CONFIG_HOME, XDG_DATA_HOME, XDG_CACHE_HOME, and
# XDG_STATE_HOME are all redirected to a throwaway directory, so it never
# touches Cal's real config/documents. Overriding just $HOME is NOT enough for
# this app: it's GLib-based and glib::user_data_dir()/user_config_dir() prefer
# XDG_DATA_HOME/XDG_CONFIG_HOME over $HOME when those are set — which they are
# on this machine. (Rubric/Gost/Kopilka are pure-Python and only consult
# Path.home(), so they don't need this; Zerkalo does.)
#
# Also forces GDK_BACKEND=x11 and unsets WAYLAND_DISPLAY: GTK4 otherwise
# prefers the real Wayland session and renders on the actual desktop instead
# of the isolated Xvfb display.
#
# The demo document is screenshots/example_template_one.typ (Cal's real
# template, reused as-is), with its title page (which shows a real name)
# stripped out of the throwaway demo copy only — see TITLE_PAGE_START/END
# below. This avoids needing to navigate the preview to page 2: an earlier
# version of this script tried simulating a click on the preview's "next
# page" button, which proved unreliable (no window manager in the isolated
# Xvfb means the app window never gets real X input focus, and getting a
# synthetic click to register consistently wasn't worth the fragility).
# Deleting the title page is simpler and doesn't depend on UI layout at all.
#
# Requires: Xvfb, ImageMagick (magick), a built and current
# target/release/zerkalo binary, and network access to fetch two Typst
# packages (@preview/droplet, @preview/marginalia) into the isolated cache.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

BINARY="target/release/zerkalo"
if [[ ! -x "$BINARY" ]]; then
  echo "ERROR: $BINARY not found. Run 'cargo build --release' first." >&2
  exit 1
fi

DEMO_HOME=$(mktemp -d /tmp/zerkalo-demo-home.XXXXXX)
OUT="screenshots/zerkalo-main.png"
WINDOW_W=1600
WINDOW_H=1000
# The title-page block in example_template_one.typ: from the
# `#page(header: none, ...)[` line through the `#pagebreak()` right after
# `#counter(page).update(1)`. Recheck these line numbers with
# `grep -n "page(header\|counter(page)\|pagebreak()" screenshots/example_template_one.typ`
# if the source template ever changes.
TITLE_PAGE_START=47
TITLE_PAGE_END=62

cleanup() {
  [[ -n "${APP_PID:-}" ]] && kill "$APP_PID" 2>/dev/null || true
  [[ -n "${XVFB_PID:-}" ]] && kill "$XVFB_PID" 2>/dev/null || true
  rm -rf "$DEMO_HOME"
}
trap cleanup EXIT

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

export HOME="$DEMO_HOME"
export XDG_CONFIG_HOME="$DEMO_HOME/.config"
export XDG_DATA_HOME="$DEMO_HOME/.local/share"
export XDG_CACHE_HOME="$DEMO_HOME/.cache"
export XDG_STATE_HOME="$DEMO_HOME/.local/state"

echo "==> Seeding demo home in $DEMO_HOME"
mkdir -p "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_CACHE_HOME"

# Git identity + a git-initialized work dir with a remote, so the first-run
# setup wizard (which checks both) doesn't block startup.
cat > "$DEMO_HOME/.gitconfig" <<GITCONFIG
[user]
	name = Demo User
	email = demo@example.com
GITCONFIG

WORK_DIR="$DEMO_HOME/Documents/Zerkalo"
mkdir -p "$WORK_DIR"
cp screenshots/example_template_one.typ "$WORK_DIR/main.typ"
cp citations.bib "$WORK_DIR/citations.bib"

# This only patches the throwaway demo copy — the real
# screenshots/example_template_one.typ and citations.bib are untouched.
sed -i "${TITLE_PAGE_START},${TITLE_PAGE_END}d" "$WORK_DIR/main.typ"
# One citation key in this file (sennProtestantSpiritualTraditions1986) isn't
# actually present in citations.bib, which is a hard compile error that blocks
# the whole preview.
sed -i '/@sennProtestantSpiritualTraditions1986/d' "$WORK_DIR/main.typ"

git -C "$WORK_DIR" init -q
git -C "$WORK_DIR" add -A
git -C "$WORK_DIR" -c user.name="Demo User" -c user.email="demo@example.com" commit -q -m "Initial commit"
git -C "$WORK_DIR" remote add origin https://example.com/demo.git

# Welcome/What's New dialog checks this marker against the current version.
mkdir -p "$XDG_DATA_HOME/zerkalo"
echo -n "$VERSION" > "$XDG_DATA_HOME/zerkalo/.welcome_version"

# Pre-fetch the two Typst packages this document needs — a fresh $XDG_CACHE_HOME
# won't have them, and this app doesn't auto-fetch missing packages at compile
# time (it just errors).
echo "==> Fetching required Typst packages"
for pkg in droplet:0.3.1 marginalia:0.3.1; do
  name="${pkg%%:*}"; ver="${pkg##*:}"
  dest="$XDG_CACHE_HOME/typst/packages/preview/$name/$ver"
  mkdir -p "$dest"
  curl -sL "https://packages.typst.org/preview/${name}-${ver}.tar.gz" | tar -xz -C "$dest"
done

# Isolated Xvfb display, well clear of any real display number in use.
DISPLAY_NUM=226
while [[ -e "/tmp/.X${DISPLAY_NUM}-lock" ]]; do
  DISPLAY_NUM=$((DISPLAY_NUM + 1))
done

echo "==> Starting isolated Xvfb on :$DISPLAY_NUM"
Xvfb ":$DISPLAY_NUM" -screen 0 "${WINDOW_W}x${WINDOW_H}x24" &
XVFB_PID=$!
sleep 2

echo "==> Launching Zerkalo against demo data inside the isolated display"
env -u WAYLAND_DISPLAY GDK_BACKEND=x11 DISPLAY=":$DISPLAY_NUM" "./$BINARY" &
APP_PID=$!

echo "==> Waiting for window to render and document to compile"
sleep 20

echo "==> Capturing screenshot (window is maximized to the display size)"
DISPLAY=":$DISPLAY_NUM" magick x:root -crop "${WINDOW_W}x${WINDOW_H}+0+0" +repage "$OUT"

echo "Done. Wrote $OUT"
