#!/usr/bin/env bash
# publish-flatpak.sh — build and publish Zerkalo to the personal flatpak repo
#
# Usage:
#   ./publish-flatpak.sh 1.2.0
#
# What this script does NOT do (Claude's job, done before running this):
#   - Write the CHANGELOG entry
#   - Update metainfo.xml release notes
#   - Bump the version in Cargo.toml
#   - Commit and tag the release in this repo
#
# What this script DOES do:
#   1. Verify the version you pass matches what's in Cargo.toml (sanity check)
#   2. Push this repo to GitHub (flatpak-builder pulls sources from there)
#   3. Build the flatpak
#   4. Pull/clone the public flatpak repo
#   5. Export the build into it
#   6. Regenerate the OSTree summary
#   7. Commit and push the flatpak repo

set -euo pipefail

GPG_KEY="A2918A9B43B199ADF9879F934AC9D5173DE4BC41"
FLATPAK_REPO="/tmp/flatpak-checkout"
MANIFEST="packaging/io.github.calstfrancis.Zerkalo.yml"
APP_LABEL="Zerkalo"

# ── argument check ────────────────────────────────────────────────────────────
if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <version>   e.g.  $0 1.2.0"
  exit 1
fi
VERSION="$1"

# ── sanity: version must match Cargo.toml ────────────────────────────────────
CARGO_VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
if [[ "$CARGO_VERSION" != "$VERSION" ]]; then
  echo "ERROR: Cargo.toml says version is '$CARGO_VERSION', but you passed '$VERSION'."
  echo "Did you forget to bump the version? (Ask Claude to do the version bump + docs first.)"
  exit 1
fi

echo "==> Publishing $APP_LABEL $VERSION"

# ── 1. push this repo so flatpak-builder can pull it ─────────────────────────
echo "==> Pushing source repo to GitHub..."
git push origin main
git push origin "v$VERSION" 2>/dev/null || true

# ── 2. build the flatpak ──────────────────────────────────────────────────────
echo "==> Building flatpak (this will take a while)..."
flatpak-builder --force-clean --user --install build-flatpak "$MANIFEST"

# ── 3. pull / clone the public flatpak repo ───────────────────────────────────
echo "==> Syncing public flatpak repo..."
if [[ -d "$FLATPAK_REPO/.git" ]]; then
  git -C "$FLATPAK_REPO" pull
else
  git clone https://github.com/calstfrancis/flatpak "$FLATPAK_REPO"
fi

# ── 4. export build into the repo ────────────────────────────────────────────
echo "==> Exporting build..."
flatpak build-export \
  --gpg-sign="$GPG_KEY" \
  "$FLATPAK_REPO" \
  build-flatpak \
  master

# ── 5. regenerate summary ────────────────────────────────────────────────────
echo "==> Regenerating OSTree summary..."
flatpak build-update-repo \
  --gpg-sign="$GPG_KEY" \
  "$FLATPAK_REPO"

# ── 6. commit and push flatpak repo ──────────────────────────────────────────
echo "==> Pushing flatpak repo..."
cd "$FLATPAK_REPO"
git add -A
git commit -m "$APP_LABEL $VERSION"
git push origin main

echo ""
echo "Done! $APP_LABEL $VERSION is live at https://calstfrancis.github.io/flatpak/"
