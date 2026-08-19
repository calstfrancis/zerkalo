#!/usr/bin/env bash
# publish-flatpak.sh — push a release; GitHub Actions builds and publishes it
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
#   2. Push main and the version tag to GitHub
#
# Pushing the tag is what triggers .github/workflows/release-flatpak.yml, which does
# everything this script used to do locally: build the flatpak, export it into the public
# repo, GPG-sign it, and push. Watch it at:
#   https://github.com/calstfrancis/zerkalo/actions/workflows/release-flatpak.yml
#
# Needs CI to have already passed for this commit — release-flatpak.yml checks this itself
# and refuses to publish otherwise, so there's no separate manual check to remember here
# anymore. If GitHub Actions is down or you need to debug the build locally, use
# publish-flatpak-local.sh instead (does the full build+publish here, same as this script
# used to).

set -euo pipefail

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

echo "==> Publishing Zerkalo $VERSION"
git push origin main
git push origin "v$VERSION"

echo ""
echo "Done! GitHub Actions is building and publishing $VERSION now:"
echo "  https://github.com/calstfrancis/zerkalo/actions/workflows/release-flatpak.yml"
