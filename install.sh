#!/bin/sh
# Install claude-memory
# Usage: curl -fsSL https://raw.githubusercontent.com/Algiras/claude-memory/master/install.sh | sh

set -e

REPO="Algiras/claude-memory"
BINARY="claude-memory"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Detect OS and architecture
detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Linux)  PLATFORM="linux" ;;
        Darwin) PLATFORM="darwin" ;;
        MINGW*|MSYS*|CYGWIN*) PLATFORM="windows" ;;
        *) echo "Error: unsupported OS: $OS" >&2; exit 1 ;;
    esac

    case "$ARCH" in
        x86_64|amd64)   ARCH="x86_64" ;;
        arm64|aarch64)   ARCH="arm64" ;;
        *) echo "Error: unsupported architecture: $ARCH" >&2; exit 1 ;;
    esac

    # Map to release asset names
    case "${PLATFORM}-${ARCH}" in
        linux-x86_64)   ARCHIVE="${BINARY}-linux-x86_64.tar.gz" ;;
        darwin-arm64)    ARCHIVE="${BINARY}-darwin-arm64.tar.gz" ;;
        darwin-x86_64)   ARCHIVE="${BINARY}-darwin-arm64.tar.gz" ;; # Rosetta
        windows-x86_64)  ARCHIVE="${BINARY}-windows-x86_64.zip" ;;
        *) echo "Error: no prebuilt binary for ${PLATFORM}-${ARCH}" >&2; exit 1 ;;
    esac
}

# Get latest release tag
get_latest_version() {
    VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | head -1 \
        | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"

    if [ -z "$VERSION" ]; then
        echo "Error: could not determine latest version" >&2
        echo "No releases found. Install from source: cargo install --git https://github.com/${REPO}" >&2
        exit 1
    fi
}

main() {
    detect_platform
    get_latest_version

    URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}"
    TMPDIR="$(mktemp -d)"
    trap 'rm -rf "$TMPDIR"' EXIT

    echo "Installing ${BINARY} ${VERSION} (${PLATFORM}/${ARCH})..."
    echo "  Downloading ${URL}"

    curl -fsSL "$URL" -o "${TMPDIR}/${ARCHIVE}"

    # Extract
    cd "$TMPDIR"
    case "$ARCHIVE" in
        *.tar.gz) tar xzf "$ARCHIVE" ;;
        *.zip)    unzip -q "$ARCHIVE" ;;
    esac

    # Install
    if [ -w "$INSTALL_DIR" ]; then
        mv "${BINARY}" "${INSTALL_DIR}/${BINARY}"
    else
        echo "  Installing to ${INSTALL_DIR} (requires sudo)"
        sudo mv "${BINARY}" "${INSTALL_DIR}/${BINARY}"
    fi

    chmod +x "${INSTALL_DIR}/${BINARY}"

    echo ""
    echo "Installed ${BINARY} to ${INSTALL_DIR}/${BINARY}"
    echo ""
    echo "Get started:"
    echo "  ${BINARY} auth login              # Configure LLM provider"
    echo "  ${BINARY} ingest --skip-knowledge  # Archive conversations"
    echo "  ${BINARY} projects                 # List projects"
}

main
