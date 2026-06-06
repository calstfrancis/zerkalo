#!/usr/bin/env bash
set -euo pipefail

BINARY_NAME="zerkalo"
APP_ID="io.github.calstfrancis.Zerkalo"
GITHUB_REPO="calstfrancis/zerkalo"
INSTALL_BIN="${HOME}/.local/bin"

# ── Helpers ───────────────────────────────────────────────────────────────────

_download() {
    local url="$1" dest="$2"
    if command -v curl &>/dev/null; then
        curl -fsSL -o "$dest" "$url"
    else
        wget -qO "$dest" "$url"
    fi
}

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

# ── Fetch latest release JSON ─────────────────────────────────────────────────
LATEST_URL="https://api.github.com/repos/${GITHUB_REPO}/releases/latest"
if command -v curl &>/dev/null; then
    RELEASE_JSON=$(curl -fsSL "${LATEST_URL}" 2>/dev/null || true)
else
    RELEASE_JSON=$(wget -qO- "${LATEST_URL}" 2>/dev/null || true)
fi

# ── Prefer native packages ────────────────────────────────────────────────────

if command -v dpkg &>/dev/null; then
    DEB_URL=$(echo "${RELEASE_JSON}" | grep -o '"browser_download_url": *"[^"]*\.deb"' \
        | head -1 | grep -o 'https://[^"]*' || true)
    if [ -n "${DEB_URL}" ]; then
        echo "Downloading .deb package..."
        TMP_DEB=$(mktemp /tmp/zerkalo_XXXXXX.deb)
        _download "${DEB_URL}" "${TMP_DEB}"
        echo "Installing (you may be prompted for your password)..."
        if command -v apt &>/dev/null; then
            sudo apt install -y "${TMP_DEB}"
        else
            sudo dpkg -i "${TMP_DEB}"
            sudo apt-get install -f -y 2>/dev/null || true
        fi
        rm -f "${TMP_DEB}"
        echo "Zerkalo installed."
        exit 0
    fi
fi

if command -v rpm &>/dev/null; then
    RPM_URL=$(echo "${RELEASE_JSON}" | grep -o '"browser_download_url": *"[^"]*\.rpm"' \
        | head -1 | grep -o 'https://[^"]*' || true)
    if [ -n "${RPM_URL}" ]; then
        echo "Downloading .rpm package..."
        TMP_RPM=$(mktemp /tmp/zerkalo_XXXXXX.rpm)
        _download "${RPM_URL}" "${TMP_RPM}"
        echo "Installing (you may be prompted for your password)..."
        if command -v dnf &>/dev/null; then
            sudo dnf install -y "${TMP_RPM}"
        else
            sudo rpm -i "${TMP_RPM}"
        fi
        rm -f "${TMP_RPM}"
        echo "Zerkalo installed."
        exit 0
    fi
fi

# ── Last resort: build from source ───────────────────────────────────────────
echo "No pre-built package found — building from source (this takes a few minutes)..."
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

# tinymist prompt only for source-build path (deb/rpm bundle it automatically)
if command -v tinymist &>/dev/null; then
    echo ""
    echo "tinymist LSP is already installed."
else
    echo ""
    if [ -t 0 ]; then
        read -r -p "Install tinymist LSP for autocomplete? [y/N] " _install_tinymist
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
