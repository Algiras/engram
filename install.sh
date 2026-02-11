#!/bin/sh
# Install claude-memory
#
# Public repo:  curl -fsSL https://raw.githubusercontent.com/Algiras/claude-memory/master/install.sh | sh
# Private repo: GH_TOKEN=ghp_... sh install.sh
#               or: gh auth token | INSTALL_DIR=~/.local/bin sh install.sh

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

# Build auth header if token available
auth_header() {
    if [ -n "$GH_TOKEN" ]; then
        echo "Authorization: token ${GH_TOKEN}"
    elif command -v gh >/dev/null 2>&1; then
        TOKEN="$(gh auth token 2>/dev/null || true)"
        if [ -n "$TOKEN" ]; then
            echo "Authorization: token ${TOKEN}"
        fi
    fi
}

# Get latest release tag
get_latest_version() {
    AUTH="$(auth_header)"
    if [ -n "$AUTH" ]; then
        CURL_AUTH="-H"
    else
        CURL_AUTH=""
        AUTH=""
    fi

    VERSION="$(curl -fsSL ${CURL_AUTH:+"$CURL_AUTH"} ${AUTH:+"$AUTH"} \
        "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | head -1 \
        | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"

    if [ -z "$VERSION" ]; then
        echo "Error: could not determine latest version." >&2
        echo "If the repo is private, set GH_TOKEN or install gh CLI first." >&2
        exit 1
    fi
}

main() {
    detect_platform
    get_latest_version

    AUTH="$(auth_header)"
    URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}"
    TMPDIR="$(mktemp -d)"
    trap 'rm -rf "$TMPDIR"' EXIT

    echo "Installing ${BINARY} ${VERSION} (${PLATFORM}/${ARCH})..."
    echo "  Downloading ${ARCHIVE}"

    if [ -n "$AUTH" ]; then
        # Private repo: find asset ID via API, download with octet-stream accept
        RELEASE_JSON="$(curl -fsSL -H "$AUTH" \
            "https://api.github.com/repos/${REPO}/releases/tags/${VERSION}")"

        # Extract the API url for our asset (sits on the line before the asset name)
        ASSET_URL="$(echo "$RELEASE_JSON" \
            | grep -B5 "\"name\": *\"${ARCHIVE}\"" \
            | grep '"url"' \
            | head -1 \
            | sed 's/.*"url": *"\([^"]*\)".*/\1/')"

        if [ -n "$ASSET_URL" ]; then
            curl -fsSL -H "$AUTH" -H "Accept: application/octet-stream" \
                "$ASSET_URL" -o "${TMPDIR}/${ARCHIVE}"
        else
            echo "Error: could not find asset ${ARCHIVE} in release ${VERSION}" >&2
            exit 1
        fi
    else
        curl -fsSL -L "$URL" -o "${TMPDIR}/${ARCHIVE}"
    fi

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
    echo "Installed ${BINARY} ${VERSION} to ${INSTALL_DIR}/${BINARY}"
    echo ""
    echo "Get started:"
    echo "  ${BINARY} auth login              # Configure LLM provider"
    echo "  ${BINARY} ingest --skip-knowledge  # Archive conversations"
    echo "  ${BINARY} projects                 # List projects"
}

main
