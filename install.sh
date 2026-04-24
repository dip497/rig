#!/usr/bin/env sh
# rig installer - https://github.com/dipendra-sharma/rig
# Usage: curl -fsSL https://raw.githubusercontent.com/dipendra-sharma/rig/main/install.sh | sh
#
# Env overrides:
#   RIG_VERSION      Pin a specific release tag (e.g. v0.2.0-alpha.1).
#   RIG_INSTALL_DIR  Install destination (default: $HOME/.local/bin).

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
error() { printf "${RED}[ERROR]${NC} %s\n" "$1" >&2; exit 1; }

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || error "required command not found: $1"
}

detect_os() {
    case "$(uname -s)" in
        Linux*)  OS="linux";;
        Darwin*) OS="darwin";;
        *)       error "Unsupported OS: $(uname -s). Try: cargo install --path crates/rig-cli --git https://github.com/${REPO}";;
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
    if [ -n "$RIG_VERSION" ]; then
        VERSION="$RIG_VERSION"
        return
    fi
    VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name":' \
        | sed -E 's/.*"([^"]+)".*/\1/')
    if [ -z "$VERSION" ]; then
        error "Failed to resolve latest version. Set RIG_VERSION=vX.Y.Z explicitly."
    fi
}

get_target() {
    case "$OS" in
        linux)  TARGET="${ARCH}-unknown-linux-musl";;
        darwin) TARGET="${ARCH}-apple-darwin";;
    esac
}

sha256_verify() {
    archive="$1"
    expected="$2"
    if command -v sha256sum >/dev/null 2>&1; then
        actual=$(sha256sum "$archive" | awk '{print $1}')
    elif command -v shasum >/dev/null 2>&1; then
        actual=$(shasum -a 256 "$archive" | awk '{print $1}')
    else
        warn "Neither sha256sum nor shasum available; skipping checksum verification."
        return 0
    fi
    if [ "$actual" != "$expected" ]; then
        error "Checksum mismatch for $(basename "$archive"). Expected $expected, got $actual."
    fi
    info "Checksum OK."
}

install() {
    info "Detected: $OS $ARCH"
    info "Target:  $TARGET"
    info "Version: $VERSION"

    ARCHIVE_NAME="${BINARY_NAME}-${TARGET}.tar.gz"
    BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"
    DOWNLOAD_URL="${BASE_URL}/${ARCHIVE_NAME}"
    CHECKSUMS_URL="${BASE_URL}/checksums.txt"

    TEMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TEMP_DIR"' EXIT
    ARCHIVE="${TEMP_DIR}/${ARCHIVE_NAME}"
    CHECKSUMS="${TEMP_DIR}/checksums.txt"

    info "Downloading ${DOWNLOAD_URL}..."
    if ! curl -fsSL "$DOWNLOAD_URL" -o "$ARCHIVE"; then
        error "Download failed. Release asset may not exist yet: ${DOWNLOAD_URL}"
    fi

    if curl -fsSL "$CHECKSUMS_URL" -o "$CHECKSUMS" 2>/dev/null; then
        expected=$(grep " ${ARCHIVE_NAME}\$" "$CHECKSUMS" | awk '{print $1}' | head -1)
        if [ -n "$expected" ]; then
            sha256_verify "$ARCHIVE" "$expected"
        else
            warn "No checksum entry for ${ARCHIVE_NAME}; skipping verification."
        fi
    else
        warn "checksums.txt not published for this release; skipping verification."
    fi

    info "Extracting..."
    tar -xzf "$ARCHIVE" -C "$TEMP_DIR"

    mkdir -p "$INSTALL_DIR"
    mv "${TEMP_DIR}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

    info "Installed to ${INSTALL_DIR}/${BINARY_NAME}"
}

verify() {
    if "${INSTALL_DIR}/${BINARY_NAME}" --version >/dev/null 2>&1; then
        info "Verification: $("${INSTALL_DIR}/${BINARY_NAME}" --version)"
    else
        warn "Binary installed but --version check failed."
    fi

    case ":$PATH:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            warn "${INSTALL_DIR} is not on your PATH. Add this to your shell rc:"
            warn "  export PATH=\"${INSTALL_DIR}:\$PATH\""
            ;;
    esac
}

main() {
    need_cmd curl
    need_cmd tar
    info "Installing ${BINARY_NAME}..."
    detect_os
    detect_arch
    get_latest_version
    get_target
    install
    verify
    echo ""
    info "Done! Run '${BINARY_NAME} --help' to get started."
}

main
