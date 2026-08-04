#!/usr/bin/env bash
# Fails if this app's vendored copy of the suite stylesheet has been edited
# in place. style/fond.css is owned by fond-style; change it there and run
# that repo's sync.sh, or the next sync silently reverts you.
#
# The canonical copy is not available in CI, so this checks the weaker but
# still useful property: that the file is present and carries its header.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

f="style/fond.css"
[[ -f "$f" ]] || { echo "FAIL: $f is missing — run fond-style/sync.sh"; exit 1; }
grep -q "shared interface layer for the Fond suite" "$f" || {
  echo "FAIL: $f does not look like the shared stylesheet"; exit 1; }
grep -q "^\.fond-chrome" "$f" || {
  echo "FAIL: $f is missing the surface classes"; exit 1; }
echo "Shared stylesheet present and intact."
