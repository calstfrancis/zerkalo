#!/usr/bin/env bash
set -euo pipefail

BINARY_NAME="zerkalo"
APP_ID="io.github.calstfrancis.Zerkalo"
INSTALL_BIN="${HOME}/.local/bin"
ICONS_BASE="${HOME}/.local/share/icons/hicolor"
INSTALL_DESKTOP="${HOME}/.local/share/applications"

echo "Building Zerkalo (release)..."
cargo build --release

echo "Installing binary..."
mkdir -p "${INSTALL_BIN}"
cp "target/release/${BINARY_NAME}" "${INSTALL_BIN}/${BINARY_NAME}"
chmod +x "${INSTALL_BIN}/${BINARY_NAME}"

echo "Installing icons..."
# Scalable (SVG) — always installed
mkdir -p "${ICONS_BASE}/scalable/apps"
cp "packaging/zerkalo.svg" "${ICONS_BASE}/scalable/apps/${BINARY_NAME}.svg"

# PNG icons at standard FreeDesktop sizes — generated from SVG if rsvg-convert is available
if command -v rsvg-convert &>/dev/null; then
    for SIZE in 16 24 32 48 64 96 128 256; do
        ICON_DIR="${ICONS_BASE}/${SIZE}x${SIZE}/apps"
        mkdir -p "${ICON_DIR}"
        rsvg-convert -w "${SIZE}" -h "${SIZE}" "packaging/zerkalo.svg" \
            -o "${ICON_DIR}/${BINARY_NAME}.png" 2>/dev/null || true
    done
    echo "  PNG icons generated (16–256px)"
elif command -v inkscape &>/dev/null; then
    for SIZE in 16 32 48 64 128 256; do
        ICON_DIR="${ICONS_BASE}/${SIZE}x${SIZE}/apps"
        mkdir -p "${ICON_DIR}"
        inkscape --export-width="${SIZE}" --export-height="${SIZE}" \
            --export-filename="${ICON_DIR}/${BINARY_NAME}.png" \
            "packaging/zerkalo.svg" 2>/dev/null || true
    done
    echo "  PNG icons generated via Inkscape (16–256px)"
else
    echo "  Note: install rsvg-convert (librsvg) or Inkscape for PNG icons"
fi

echo "Installing .desktop file..."
mkdir -p "${INSTALL_DESKTOP}"
# Write desktop file with absolute binary path so Nautilus / KDE can find the
# binary even when ~/.local/bin is not in the GUI session's PATH.
sed "s|Exec=zerkalo %U|Exec=${INSTALL_BIN}/${BINARY_NAME} %U|" \
    "packaging/${APP_ID}.desktop" > "${INSTALL_DESKTOP}/${APP_ID}.desktop"

echo "Updating desktop database..."
update-desktop-database "${INSTALL_DESKTOP}" 2>/dev/null || true

echo "Updating icon cache..."
gtk-update-icon-cache -f -t "${ICONS_BASE}" 2>/dev/null || true

# Register Zerkalo as the default application for Typst files.
xdg-mime default "${APP_ID}.desktop" text/x-typst 2>/dev/null || true

# Rebuild KDE/Plasma service cache so the panel shows the icon immediately
kbuildsycoca6 --noincremental 2>/dev/null || kbuildsycoca5 --noincremental 2>/dev/null || true

# Bump the welcome-window version marker so the welcome screen shows on next launch
MARKER="${HOME}/.local/share/zerkalo/.welcome_version"
rm -f "${MARKER}" 2>/dev/null || true

echo ""
echo "Zerkalo installed."
echo "  Binary  : ${INSTALL_BIN}/${BINARY_NAME}"
echo "  Icons   : ${ICONS_BASE}/scalable/apps/${BINARY_NAME}.svg"
echo ""
echo "If ${INSTALL_BIN} is not in your PATH, add this to ~/.bashrc or ~/.zshrc:"
echo "  export PATH=\"\${HOME}/.local/bin:\${PATH}\""
