#!/usr/bin/env sh
# rig installer - https://github.com/dipendra-sharma/rig
# Usage: curl -fsSL https://raw.githubusercontent.com/dipendra-sharma/rig/main/install.sh | sh

set -e

REPO="dipendra-sharma/rig"
BINARY_NAME="rig"
INSTALL_DIR="${RIG_INSTALL_DIR:-$HOME/.local/bin}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info()  { printf "${GREEN}[INFO]${NC} %s\n" "$1"; }
warn()  { printf "${YELLOW}[WARN]${NC} %s\n" "$1"; }
error() { printf "${RED}[ERROR]${NC} %s\n" "$1"; exit 1; }

detect_os() {
    case "$(uname -s)" in
        Linux*)  OS="linux";;
        Darwin*) OS="darwin";;
        *)       error "Unsupported OS: $(uname -s)";;
    esac
}

detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64)  ARCH="x86_64";;
        arm64|aarch64) ARCH="aarch64";;
        *)             error "Unsupported arch: $(uname -m)";;
    esac
}

get_latest_version() {
    VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    if [ -z "$VERSION" ]; then
        error "Failed to get latest version from GitHub"
    fi
}

get_target() {
    case "$OS" in
        linux)
            TARGET="${ARCH}-unknown-linux-musl"
            ;;
        darwin)
            TARGET="${ARCH}-apple-darwin"
            ;;
    esac
}

install() {
    info "Detected: $OS $ARCH"
    info "Target:  $TARGET"
    info "Version: $VERSION"

    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${BINARY_NAME}-${TARGET}.tar.gz"
    TEMP_DIR=$(mktemp -d)
    ARCHIVE="${TEMP_DIR}/${BINARY_NAME}.tar.gz"

    info "Downloading..."
    if ! curl -fsSL "$DOWNLOAD_URL" -o "$ARCHIVE"; then
        error "Download failed — check that the release exists at ${DOWNLOAD_URL}"
    fi

    info "Extracting..."
    tar -xzf "$ARCHIVE" -C "$TEMP_DIR"

    mkdir -p "$INSTALL_DIR"
    mv "${TEMP_DIR}/${BINARY_NAME}" "${INSTALL_DIR}/"
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

    rm -rf "$TEMP_DIR"

    info "Installed to ${INSTALL_DIR}/${BINARY_NAME}"
}

verify() {
    if command -v "$BINARY_NAME" >/dev/null 2>&1; then
        info "Verification: $($BINARY_NAME --help 2>&1 | head -1)"
    else
        warn "Not in PATH. Add to your shell profile:"
        warn "  export PATH=\"\$HOME/.local/bin:\$PATH\""
    fi
}

main() {
    info "Installing $BINARY_NAME..."
    detect_os
    detect_arch
    get_latest_version
    get_target
    install
    verify
    echo ""
    info "Done! Run '$BINARY_NAME' to launch."
}

main
