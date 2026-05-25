#!/usr/bin/env bash
set -euo pipefail

BINARY_NAME="zerkalo"
APP_ID="io.github.calstfrancis.Zerkalo"
INSTALL_BIN="${HOME}/.local/bin"
INSTALL_ICONS="${HOME}/.local/share/icons/hicolor/scalable/apps"
INSTALL_DESKTOP="${HOME}/.local/share/applications"

echo "Removing Zerkalo..."

rm -f "${INSTALL_BIN}/${BINARY_NAME}"
rm -f "${INSTALL_ICONS}/${BINARY_NAME}.svg"
rm -f "${INSTALL_DESKTOP}/${APP_ID}.desktop"

update-desktop-database "${INSTALL_DESKTOP}" 2>/dev/null || true
gtk-update-icon-cache -f -t "${HOME}/.local/share/icons/hicolor" 2>/dev/null || true

echo "Zerkalo uninstalled."
echo ""
echo "User config and data are preserved at:"
echo "  ~/.config/zerkalo/"
echo "  ~/.local/share/zerkalo/"
echo "Remove those directories manually if you want a clean slate."
