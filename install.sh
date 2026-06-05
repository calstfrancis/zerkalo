#!/usr/bin/env bash
set -euo pipefail

BINARY_NAME="zerkalo"
APP_ID="io.github.calstfrancis.Zerkalo"
INSTALL_BIN="${HOME}/.local/bin"
ICONS_BASE="${HOME}/.local/share/icons/hicolor"
INSTALL_DESKTOP="${HOME}/.local/share/applications"
GITHUB_REPO="calstfrancis/zerkalo"

# ── Prefer a locally-built binary; only fall back to downloading ──────────────
# If you ran `cargo build --release` before this script, that binary is used
# directly — no network access needed.  Only when there is no local build does
# the script try to download a pre-built AppImage from the latest GitHub release.

if [ -f "target/release/${BINARY_NAME}" ]; then
    echo "Local build found — installing target/release/${BINARY_NAME}..."
    mkdir -p "${INSTALL_BIN}"
    cp "target/release/${BINARY_NAME}" "${INSTALL_BIN}/${BINARY_NAME}"
    chmod +x "${INSTALL_BIN}/${BINARY_NAME}"
    echo "  Installed local build."
else
    USE_APPIMAGE=0
    if command -v curl &>/dev/null || command -v wget &>/dev/null; then
        echo "No local build found — checking for pre-built release on GitHub..."
        LATEST_URL="https://api.github.com/repos/${GITHUB_REPO}/releases/latest"
        if command -v curl &>/dev/null; then
            RELEASE_JSON=$(curl -fsSL "${LATEST_URL}" 2>/dev/null || true)
        else
            RELEASE_JSON=$(wget -qO- "${LATEST_URL}" 2>/dev/null || true)
        fi
        APPIMAGE_URL=$(echo "${RELEASE_JSON}" | grep -o '"browser_download_url": *"[^"]*\.AppImage"' \
            | head -1 | grep -o 'https://[^"]*' || true)
        if [ -n "${APPIMAGE_URL}" ]; then
            echo "Downloading pre-built AppImage..."
            APPIMAGE_FILE="${INSTALL_BIN}/zerkalo.AppImage"
            mkdir -p "${INSTALL_BIN}"
            if command -v curl &>/dev/null; then
                curl -fsSL -o "${APPIMAGE_FILE}" "${APPIMAGE_URL}"
            else
                wget -qO "${APPIMAGE_FILE}" "${APPIMAGE_URL}"
            fi
            chmod +x "${APPIMAGE_FILE}"
            cat > "${INSTALL_BIN}/${BINARY_NAME}" << WRAPPER
#!/bin/sh
exec "${APPIMAGE_FILE}" "\$@"
WRAPPER
            chmod +x "${INSTALL_BIN}/${BINARY_NAME}"
            USE_APPIMAGE=1
            echo "  Downloaded: ${APPIMAGE_FILE}"
        fi
    fi

    if [ "${USE_APPIMAGE}" -eq 0 ]; then
        echo "Building from source (this takes a few minutes)..."
        cargo build --release
        mkdir -p "${INSTALL_BIN}"
        cp "target/release/${BINARY_NAME}" "${INSTALL_BIN}/${BINARY_NAME}"
        chmod +x "${INSTALL_BIN}/${BINARY_NAME}"
        echo "  Binary built and installed."
    fi
fi

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

# ── Optional: install tinymist LSP for autocomplete ──────────────────────────
if command -v tinymist &>/dev/null; then
    echo ""
    echo "tinymist LSP is already installed."
else
    echo ""
    if [ -t 0 ]; then
        read -r -p "Install tinymist LSP for autocomplete in Zerkalo? [y/N] " _install_tinymist
    else
        _install_tinymist="n"
    fi
    if [[ "${_install_tinymist}" =~ ^[Yy]$ ]]; then
        echo "Installing tinymist via official installer..."
        TINYMIST_INSTALLER_URL="https://github.com/Myriad-Dreamin/tinymist/releases/latest/download/tinymist-installer.sh"
        if command -v curl &>/dev/null; then
            curl --proto '=https' --tlsv1.2 -fsSL "${TINYMIST_INSTALLER_URL}" | sh
        elif command -v wget &>/dev/null; then
            wget -qO- "${TINYMIST_INSTALLER_URL}" | sh
        else
            echo "  Neither curl nor wget found. Install tinymist manually:"
            echo "  https://github.com/Myriad-Dreamin/tinymist/releases/latest"
        fi
        if command -v tinymist &>/dev/null; then
            echo "  tinymist installed."
        else
            echo "  tinymist binary not found in PATH after install."
            echo "  You may need to add ~/.cargo/bin or the install dir to your PATH."
        fi
    else
        echo "  Skipped. To install later, run:"
        echo "  curl -fsSL https://github.com/Myriad-Dreamin/tinymist/releases/latest/download/tinymist-installer.sh | sh"
    fi
fi
