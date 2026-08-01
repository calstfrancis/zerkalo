#!/usr/bin/env bash
#
# check-versions.sh — guards against the two version-drift bugs documented in
# ../CLAUDE.md ("App capability matrix" / dev-build workflow):
#
#   1. A pre-release version (e.g. 0.17.2-dev1) must NEVER appear in a metainfo
#      <release> entry. AppStream's version comparison has no concept of
#      pre-release ordering, so it reads "-dev1" as *higher* than the clean
#      "0.17.2" and tools like `flatpak info` then show the wrong Version.
#   2. On a clean release (version has no "-" suffix), the app version must have
#      a matching metainfo <release> entry — catches "tagged a release but forgot
#      to add the metainfo entry."
#
# Runs on every push/PR via .github/workflows/ci.yml. Also safe to run locally.
# Portable across the Rust (Cargo.toml) and Python (pyproject.toml) apps — the
# same script drops into any of them unchanged.

set -euo pipefail

# --- locate the metainfo file (ignore build artifacts) ---
METAINFO=$(find . -name '*.metainfo.xml' \
  -not -path '*/.flatpak-builder/*' \
  -not -path '*/build-flatpak/*' \
  -not -path '*/build/*' 2>/dev/null | head -1)
[ -n "$METAINFO" ] || { echo "ERROR: no *.metainfo.xml found"; exit 1; }

# --- read the app version (Rust, or Python literal / dynamic attr) ---
read_py_version() {
  # 1) literal: version = "x.y.z" in pyproject.toml
  local v
  v=$(grep -m1 '^version[[:space:]]*=[[:space:]]*"' pyproject.toml \
      | sed -E 's/.*"([^"]+)".*/\1/' || true)
  if [ -n "$v" ]; then echo "$v"; return; fi
  # 2) dynamic: version = { attr = "pkg.__version__" } -> resolve from the module
  local attr; attr=$(grep -oE 'attr[[:space:]]*=[[:space:]]*"[^"]+"' pyproject.toml \
      | head -1 | sed -E 's/.*"([^"]+)".*/\1/' || true)
  if [ -n "$attr" ]; then
    local var=${attr##*.} base=${attr%.*}; base=${base//.//}
    local f
    for f in "$base/__init__.py" "$base.py"; do
      [ -f "$f" ] || continue
      v=$(grep -m1 "^${var}[[:space:]]*=[[:space:]]*[\"']" "$f" \
          | sed -E "s/.*[\"']([^\"']+)[\"'].*/\1/" || true)
      [ -n "$v" ] && { echo "$v"; return; }
    done
  fi
  echo ""
}

if [ -f Cargo.toml ]; then
  APP_VERSION=$(grep -m1 '^version[[:space:]]*=[[:space:]]*"' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/' || true)
elif [ -f pyproject.toml ]; then
  APP_VERSION=$(read_py_version)
else
  echo "ERROR: no Cargo.toml or pyproject.toml to read the app version from"; exit 1
fi
[ -n "$APP_VERSION" ] || { echo "ERROR: could not determine app version"; exit 1; }

echo "App version : $APP_VERSION"
echo "Metainfo    : $METAINFO"
echo

fail=0

# --- check 1: no *stable* pre-release <release> entry ---
# A pre-release version is only safe in metainfo if it carries type="development"
# (AppStream then excludes it from the stable-version calc). A pre-release entry
# WITHOUT that attribute is the dangerous kind that mis-sorts above the real
# release. We flag those; type="development" history is tolerated.
bad=$(grep -oE '<release[^>]*>' "$METAINFO" \
  | grep -E 'version="[^"]*-(dev|rc|alpha|beta|pre)' \
  | grep -v 'type="development"' || true)
if [ -n "$bad" ]; then
  echo "ERROR: metainfo has pre-release <release> entries that are NOT type=\"development\""
  echo "       (AppStream sorts these above the real release — wrong 'Version' in flatpak info):"
  echo "$bad" | sed 's/^/  /'
  fail=1
fi

# --- check 2: a clean release must have a matching metainfo <release> entry ---
case "$APP_VERSION" in
  *-*)
    echo "Dev build ($APP_VERSION) — skipping the metainfo-entry match check."
    ;;
  *)
    if grep -qE "<release[[:space:]]+version=\"$APP_VERSION\"" "$METAINFO"; then
      echo "Clean release $APP_VERSION has a matching <release> entry."
    else
      echo "ERROR: clean release $APP_VERSION has no matching <release> entry in $METAINFO"
      fail=1
    fi
    ;;
esac

echo
if [ "$fail" -eq 0 ]; then
  echo "Version consistency OK."
else
  echo "Version consistency FAILED — see errors above."
  exit 1
fi
