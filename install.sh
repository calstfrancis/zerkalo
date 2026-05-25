#!/usr/bin/env bash
set -euo pipefail

BINARY_NAME="zerkalo"
APP_ID="io.github.calstfrancis.Zerkalo"
INSTALL_BIN="${HOME}/.local/bin"
INSTALL_ICONS="${HOME}/.local/share/icons/hicolor/scalable/apps"
INSTALL_DESKTOP="${HOME}/.local/share/applications"

echo "Building Zerkalo (release)..."
cargo build --release

echo "Installing binary..."
mkdir -p "${INSTALL_BIN}"
cp "target/release/${BINARY_NAME}" "${INSTALL_BIN}/${BINARY_NAME}"
chmod +x "${INSTALL_BIN}/${BINARY_NAME}"

echo "Installing icon..."
mkdir -p "${INSTALL_ICONS}"
cp "packaging/zerkalo.svg" "${INSTALL_ICONS}/${BINARY_NAME}.svg"

echo "Installing .desktop file..."
mkdir -p "${INSTALL_DESKTOP}"
cp "packaging/${APP_ID}.desktop" "${INSTALL_DESKTOP}/${APP_ID}.desktop"

echo "Updating desktop database..."
update-desktop-database "${INSTALL_DESKTOP}" 2>/dev/null || true

echo "Updating icon cache..."
gtk-update-icon-cache -f -t "${HOME}/.local/share/icons/hicolor" 2>/dev/null || true

echo ""
echo "Zerkalo installed."
echo "  Binary : ${INSTALL_BIN}/${BINARY_NAME}"
echo ""
echo "If ${INSTALL_BIN} is not in your PATH, add this to ~/.bashrc or ~/.zshrc:"
echo "  export PATH=\"\${HOME}/.local/bin:\${PATH}\""
