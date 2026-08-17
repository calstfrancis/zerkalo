#!/usr/bin/env bash
set -euo pipefail

BINARY_NAME="zerkalo"
APP_ID="io.github.calstfrancis.Zerkalo"
INSTALL_BIN="${HOME}/.local/bin"

# ── Helpers ───────────────────────────────────────────────────────────────────

_install_icons_and_desktop() {
    local ICONS_BASE="${HOME}/.local/share/icons/hicolor"
    local INSTALL_DESKTOP="${HOME}/.local/share/applications"

    echo "Installing icons..."
    mkdir -p "${ICONS_BASE}/scalable/apps"
    cp "packaging/zerkalo.svg" "${ICONS_BASE}/scalable/apps/${BINARY_NAME}.svg"

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
    fi

    echo "Installing .desktop file..."
    mkdir -p "${INSTALL_DESKTOP}"
    sed "s|Exec=zerkalo %U|Exec=${INSTALL_BIN}/${BINARY_NAME} %U|" \
        "packaging/${APP_ID}.desktop" > "${INSTALL_DESKTOP}/${APP_ID}.desktop"

    update-desktop-database "${INSTALL_DESKTOP}" 2>/dev/null || true
    gtk-update-icon-cache -f -t "${ICONS_BASE}" 2>/dev/null || true
    xdg-mime default "${APP_ID}.desktop" text/x-typst 2>/dev/null || true
    kbuildsycoca6 --noincremental 2>/dev/null || kbuildsycoca5 --noincremental 2>/dev/null || true

    rm -f "${HOME}/.local/share/zerkalo/.welcome_version" 2>/dev/null || true
}

# ── Developer fast-path: local cargo build ────────────────────────────────────
if [ -f "target/release/${BINARY_NAME}" ]; then
    echo "Local build found — installing target/release/${BINARY_NAME}..."
    mkdir -p "${INSTALL_BIN}"
    cp "target/release/${BINARY_NAME}" "${INSTALL_BIN}/${BINARY_NAME}"
    chmod +x "${INSTALL_BIN}/${BINARY_NAME}"
    echo "  Installed local build."
    _install_icons_and_desktop
    echo ""
    echo "Zerkalo installed to ${INSTALL_BIN}/${BINARY_NAME}"
    exit 0
fi

# ── Build from source ──────────────────────────────────────────────────────────
# Zerkalo's pre-built packages are the flatpak at
# https://calstfrancis.github.io/flatpak/ (see README.md) — GitHub Releases
# don't carry .deb/.rpm assets. This script is for building locally instead.
echo "Building from source (this takes a few minutes)..."
echo "Tip: for a pre-built package, install the flatpak instead — see README.md."
if ! command -v cargo &>/dev/null; then
    echo "Error: cargo not found. Install Rust from https://rustup.rs then re-run this script."
    exit 1
fi
cargo build --release

mkdir -p "${INSTALL_BIN}"
cp "target/release/${BINARY_NAME}" "${INSTALL_BIN}/${BINARY_NAME}"
chmod +x "${INSTALL_BIN}/${BINARY_NAME}"
echo "  Binary built and installed."
_install_icons_and_desktop

echo ""
echo "Zerkalo installed to ${INSTALL_BIN}/${BINARY_NAME}"
echo ""
echo "If ${INSTALL_BIN} is not in your PATH, add this to ~/.bashrc or ~/.zshrc:"
echo "  export PATH=\"\${HOME}/.local/bin:\${PATH}\""

# The flatpak bundles tinymist automatically; a source build needs it separately.
if command -v tinymist &>/dev/null; then
    echo ""
    echo "tinymist (autocomplete/error-checking engine) is already installed."
else
    echo ""
    if [ -t 0 ]; then
        read -r -p "Install tinymist for autocomplete and error-checking? [y/N] " _install_tinymist
    else
        _install_tinymist="n"
    fi
    if [[ "${_install_tinymist}" =~ ^[Yy]$ ]]; then
        echo "Installing tinymist..."
        TINYMIST_URL="https://github.com/Myriad-Dreamin/tinymist/releases/latest/download/tinymist-installer.sh"
        if command -v curl &>/dev/null; then
            curl --proto '=https' --tlsv1.2 -fsSL "${TINYMIST_URL}" | sh
        else
            wget -qO- "${TINYMIST_URL}" | sh
        fi
        command -v tinymist &>/dev/null && echo "  tinymist installed." || \
            echo "  Add ~/.cargo/bin to PATH if tinymist is not found."
    else
        echo "  Skipped. Install later: https://github.com/Myriad-Dreamin/tinymist/releases/latest"
    fi
fi
