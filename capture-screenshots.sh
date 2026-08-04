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
# dbus-run-session is not optional: GApplication's single-instance check runs
# over the *session* D-Bus bus, which none of the DISPLAY/HOME/XDG isolation
# above touches. With Zerkalo already open for real, this launch registers,
# finds a primary instance, hands off to it and exits — so the isolated display
# stays empty, every capture is blank, and the run dies after five attempts
# claiming the app never rendered. Worse, the hand-off opens a window in the
# real instance. Giving the child its own bus makes it primary in its own
# session. (Rubric's capture script had exactly this bug.)
#
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
OUT_DARK="screenshots/zerkalo-main-dark.png"
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

# Capture the app once per colour scheme. libadwaita normally resolves
# light/dark from the desktop's settings portal, which on this machine is
# answered by a backend that ignores our isolated config and always reports
# light. ADW_DISABLE_PORTAL=1 makes libadwaita read the GSettings color-scheme
# key instead, and GSETTINGS_BACKEND=keyfile feeds it a value we write into
# the throwaway config (XDG_CONFIG_HOME is already redirected there above) —
# forcing either scheme deterministically without touching the real desktop.
# Zerkalo's own config defaults to Theme::System, which follows this.
capture_scheme() {
  local scheme="$1" out="$2"
  mkdir -p "$XDG_CONFIG_HOME/glib-2.0/settings"
  cat > "$XDG_CONFIG_HOME/glib-2.0/settings/keyfile" <<KEYFILE
[org/gnome/desktop/interface]
color-scheme='$scheme'
KEYFILE

  echo "==> Launching Zerkalo ($scheme) against demo data inside the isolated display"
  env -u WAYLAND_DISPLAY GDK_BACKEND=x11 ADW_DISABLE_PORTAL=1 GSETTINGS_BACKEND=keyfile \
    DISPLAY=":$DISPLAY_NUM" dbus-run-session -- "./$BINARY" &
  APP_PID=$!

  echo "==> Waiting for window to render and document to compile"
  sleep 20

  # Capture, then check the image isn't blank before accepting it. A fixed wait
  # isn't enough on a cold run — the first capture of a release fetches Typst
  # packages over the network first — and a solid-black PNG published to the
  # website is worse than a slow release. Standard deviation of a real
  # screenshot is in the thousands; a single-colour image is 0.
  local attempt
  for attempt in 1 2 3 4 5; do
    echo "==> Capturing screenshot -> $out (attempt $attempt)"
    DISPLAY=":$DISPLAY_NUM" magick x:root -crop "${WINDOW_W}x${WINDOW_H}+0+0" +repage "$out"
    local sd
    sd=$(magick "$out" -format "%[fx:standard_deviation]" info: 2>/dev/null || echo 0)
    if awk -v v="$sd" 'BEGIN { exit !(v > 0.01) }'; then
      break
    fi
    if [[ $attempt -eq 5 ]]; then
      echo "ERROR: $out is blank after 5 attempts — the app never rendered." >&2
      exit 1
    fi
    echo "    blank capture (sd=$sd) — waiting for the window to render"
    sleep 10
  done

  kill "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
  APP_PID=
}

capture_scheme default     "$OUT"
capture_scheme prefer-dark "$OUT_DARK"

echo "Done. Wrote $OUT and $OUT_DARK"

# Publish web-ready copies into the personal website repo, one PNG + WebP per
# scheme, named as the site expects (<slug>.png/.webp + <slug>-dark.png/.webp).
# The capture crop already matches the site's image dimensions, so this is a
# straight convert+copy — no resize. Override the destination with
# WEBSITE_DIR=/path ./capture-screenshots.sh; if it doesn't exist the export is
# skipped with a note rather than failing. The website is a separate repo —
# commit and push it there yourself after reviewing the refreshed images.
SLUG="zerkalo"
WEBSITE_DIR="${WEBSITE_DIR:-$(dirname "$SCRIPT_DIR")/calstfrancis.github.io}"
if [[ -d "$WEBSITE_DIR" ]]; then
  echo "==> Publishing web images to $WEBSITE_DIR"
  cp "$OUT"      "$WEBSITE_DIR/$SLUG.png"
  cp "$OUT_DARK" "$WEBSITE_DIR/$SLUG-dark.png"
  magick "$OUT"      -quality 80 "$WEBSITE_DIR/$SLUG.webp"
  magick "$OUT_DARK" -quality 80 "$WEBSITE_DIR/$SLUG-dark.webp"
  echo "    wrote $SLUG.{png,webp} and $SLUG-dark.{png,webp}"
else
  echo "NOTE: website dir not found ($WEBSITE_DIR) — skipping web export."
fi
