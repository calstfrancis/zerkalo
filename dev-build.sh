#!/usr/bin/env bash
# dev-build.sh — build and install Zerkalo locally for testing
#
# Run this after Claude has prepped the dev build (bumped to the next rc version,
# updated CHANGELOG, committed, and tagged). No arguments needed — the version
# is read from Cargo.toml.
#
# Pushes to GitHub first (flatpak-builder pulls source from branch: main),
# then builds and installs locally. Does NOT publish to the flatpak repo.

set -euo pipefail

MANIFEST="packaging/io.github.calstfrancis.Zerkalo.yml"

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
echo "==> Building Zerkalo $VERSION (local dev install)"

echo "==> Pushing to GitHub (flatpak-builder needs this)..."
git push origin main
git push origin "v$VERSION" 2>/dev/null || true

flatpak-builder --force-clean --user --install build-flatpak "$MANIFEST"

echo ""
echo "Done! Zerkalo $VERSION is installed locally."
echo "Run it with: flatpak run io.github.calstfrancis.Zerkalo"
