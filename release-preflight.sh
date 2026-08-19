#!/usr/bin/env bash
#
# release-preflight.sh — hard-checks that every source of version/release
# truth actually agrees before Cal runs publish-flatpak.sh, instead of
# relying on the "Release preflight" checklist in ../CLAUDE.md being
# followed by memory. Complements check-versions.sh (which runs on every
# push/PR and only catches AppStream ordering + "forgot the metainfo
# entry"): this script is release-specific and stricter — it refuses to
# pass at all on a dev build, and cross-checks CHANGELOG.md, the
# release-name constant (if the app has one), and the git tag/working tree.
#
# Portable across the Rust (Cargo.toml) and Python (pyproject.toml) flatpak
# apps, same auto-detection approach as check-versions.sh — drops into any
# of them unchanged. Safe to run locally at any time; intended to be run as
# the last step of the release workflow, after commit+tag, before telling
# Cal "Ready — run ./publish-flatpak.sh X.Y.Z".

set -euo pipefail

# --- read the app version (Rust, or Python literal / dynamic attr) ---
read_py_version() {
  local v
  v=$(grep -m1 '^version[[:space:]]*=[[:space:]]*"' pyproject.toml \
      | sed -E 's/.*"([^"]+)".*/\1/' || true)
  if [ -n "$v" ]; then echo "$v"; return; fi
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

METAINFO=$(find . -name '*.metainfo.xml' \
  -not -path '*/.flatpak-builder/*' \
  -not -path '*/build-flatpak/*' \
  -not -path '*/build/*' 2>/dev/null | head -1)
[ -n "$METAINFO" ] || { echo "ERROR: no *.metainfo.xml found"; exit 1; }

echo "App version : $APP_VERSION"
echo "Metainfo    : $METAINFO"
echo

fail=0
note() { echo "  - $1"; }
err() { echo "ERROR: $1"; fail=1; }

# --- 1: must be a clean release version, not a dev/rc snapshot ---
case "$APP_VERSION" in
  *-*)
    err "$APP_VERSION is not a clean release version (has a pre-release suffix) — this checks release readiness, not dev builds."
    ;;
  *)
    note "clean release version"
    ;;
esac

# --- 2: metainfo has a matching <release> entry with a real date ---
release_line=$(grep -oE "<release[^>]*version=\"$APP_VERSION\"[^>]*>" "$METAINFO" || true)
if [ -z "$release_line" ]; then
  err "no <release version=\"$APP_VERSION\"> entry in $METAINFO"
else
  date_val=$(echo "$release_line" | grep -oE 'date="[0-9]{4}-[0-9]{2}-[0-9]{2}"' || true)
  if [ -z "$date_val" ]; then
    err "the <release version=\"$APP_VERSION\"> entry in $METAINFO has no valid date=\"YYYY-MM-DD\""
  else
    note "metainfo entry present with $date_val"
  fi
fi

# --- 3: CHANGELOG.md has a finalised heading for this version ---
if [ ! -f CHANGELOG.md ]; then
  err "no CHANGELOG.md found"
else
  changelog_heading=$(grep -m1 -E "^## \[$APP_VERSION\]" CHANGELOG.md || true)
  if [ -z "$changelog_heading" ]; then
    err "CHANGELOG.md has no '## [$APP_VERSION]' heading"
  elif echo "$changelog_heading" | grep -qE -- '-(dev|rc|alpha|beta|pre)[0-9]*\]'; then
    err "CHANGELOG.md heading for $APP_VERSION still carries a dev/rc suffix: $changelog_heading"
  else
    note "CHANGELOG.md heading: $changelog_heading"
  fi
fi

# --- 4: release-name constant (if this app has one) matches the CHANGELOG name ---
# Convention from ../CLAUDE.md: Rust apps (Zerkalo/Iskra) use a `RELEASE_NAME`
# const in src/ui/welcome_window.rs; Python apps (Kopilka/Gost) use
# `__release_name__` in their package's __init__.py. Apps without either
# (Rubric, Retseptura, Skrizhal, Chered, Kartoteka) simply have neither file
# pattern match, and this check is silently skipped for them.
release_name_val=""
if [ -f src/ui/welcome_window.rs ]; then
  release_name_val=$(grep -m1 -oE 'RELEASE_NAME[[:space:]]*:[[:space:]]*&str[[:space:]]*=[[:space:]]*"[^"]*"' src/ui/welcome_window.rs \
    | sed -E 's/.*"([^"]*)".*/\1/' || true)
  release_name_file="src/ui/welcome_window.rs"
else
  for f in */__init__.py; do
    [ -f "$f" ] || continue
    v=$(grep -m1 -oE '__release_name__[[:space:]]*=[[:space:]]*"[^"]*"' "$f" | sed -E 's/.*"([^"]*)".*/\1/' || true)
    if [ -n "$v" ]; then release_name_val="$v"; release_name_file="$f"; break; fi
  done
fi
if [ -n "$release_name_val" ]; then
  if [ -n "${changelog_heading:-}" ] && echo "$changelog_heading" | grep -qF "$release_name_val"; then
    note "release name \"$release_name_val\" ($release_name_file) matches CHANGELOG heading"
  else
    err "release name \"$release_name_val\" in $release_name_file does not appear in the CHANGELOG.md heading for $APP_VERSION — likely bumped in one place but not the other"
  fi
fi

# --- 5: git tag and working tree, if this is a git checkout ---
if command -v git >/dev/null 2>&1 && git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  if [ -n "$(git status --porcelain)" ]; then
    err "working tree is not clean — commit or stash before releasing"
  else
    note "working tree clean"
  fi
  tag="v$APP_VERSION"
  if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
    if [ "$(git rev-parse "$tag")" = "$(git rev-parse HEAD)" ]; then
      note "tag $tag points at HEAD"
    else
      err "tag $tag exists but does not point at HEAD — HEAD has moved past the release commit"
    fi
  else
    note "tag $tag not created yet"
  fi
else
  note "not a git checkout — skipping tag/working-tree check"
fi

echo
if [ "$fail" -eq 0 ]; then
  echo "Release preflight OK — $APP_VERSION is ready to publish."
else
  echo "Release preflight FAILED — see errors above. Do not tell Cal to run publish-flatpak.sh yet."
  exit 1
fi
