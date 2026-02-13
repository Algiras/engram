#!/bin/sh
# Install engram
#
# Usage: curl -fsSL https://raw.githubusercontent.com/Algiras/engram/master/install.sh | sh
#
# Options:
#   INSTALL_DIR=~/.local/bin  - Custom install directory (default: /usr/local/bin)
#   VERSION=v0.3.0            - Install a specific release tag (default: latest)

set -e

REPO="${REPO:-Algiras/engram}"
BINARY="engram"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
RELEASES_API_URL="${RELEASES_API_URL:-https://api.github.com/repos/${REPO}/releases}"
RELEASE_BASE_URL="${RELEASE_BASE_URL:-https://github.com/${REPO}/releases/download}"

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Error: required command '$1' not found." >&2
        exit 1
    fi
}

check_dependencies() {
    require_cmd curl
    require_cmd uname
    require_cmd mktemp
    require_cmd install
    require_cmd awk
}

sha256_file() {
    file="$1"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$file" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$file" | awk '{print $1}'
    else
        echo "Error: SHA-256 tool not found (need sha256sum or shasum)." >&2
        exit 1
    fi
}

verify_checksum() {
    archive_name="$1"
    archive_path="$2"
    checksum_file="$3"

    expected="$(awk -v name="$archive_name" '$2 == name {print $1}' "$checksum_file" | head -n 1)"
    if [ -z "$expected" ]; then
        echo "Error: checksum for $archive_name not found in checksums.txt." >&2
        exit 1
    fi

    actual="$(sha256_file "$archive_path")"
    if [ "$actual" != "$expected" ]; then
        echo "Error: checksum verification failed for $archive_name." >&2
        echo "Expected: $expected" >&2
        echo "Actual:   $actual" >&2
        exit 1
    fi
}

detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"
    LEGACY_ARCHIVE=""

    case "${OS}-${ARCH}" in
        Linux-x86_64|Linux-amd64)
            TARGET="x86_64-unknown-linux-gnu"
            EXT="tar.gz"
            LEGACY_ARCHIVE="engram-linux-x86_64.tar.gz"
            ;;
        Linux-aarch64|Linux-arm64)
            echo "Error: no prebuilt binary for Linux ARM64 yet." >&2
            echo "Build from source: cargo install --git https://github.com/${REPO}" >&2
            exit 1
            ;;
        Darwin-arm64|Darwin-aarch64)
            TARGET="aarch64-apple-darwin"
            EXT="tar.gz"
            LEGACY_ARCHIVE="engram-darwin-arm64.tar.gz"
            ;;
        Darwin-x86_64)
            echo "Error: no prebuilt binary for macOS Intel (x86_64)." >&2
            echo "Build from source: cargo install --git https://github.com/${REPO}" >&2
            exit 1
            ;;
        MINGW*-x86_64|MSYS*-x86_64|CYGWIN*-x86_64)
            TARGET="x86_64-pc-windows-msvc"
            EXT="zip"
            BINARY="engram.exe"
            LEGACY_ARCHIVE="engram-windows-x86_64.zip"
            ;;
        *)
            echo "Error: no prebuilt binary for ${OS}/${ARCH}" >&2
            echo "Build from source: cargo install --git https://github.com/${REPO}" >&2
            exit 1
            ;;
    esac

    ARCHIVE_PRIMARY="engram-${TARGET}.${EXT}"
}

get_latest_version() {
    if [ -n "$VERSION" ]; then
        return
    fi

    VERSION="$(curl -fsSL "${RELEASES_API_URL}/latest" \
        | grep '"tag_name"' \
        | head -1 \
        | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"

    if [ -z "$VERSION" ]; then
        echo "Error: could not determine latest version." >&2
        exit 1
    fi
}

main() {
    check_dependencies
    detect_platform
    get_latest_version

    case "$EXT" in
        tar.gz) require_cmd tar ;;
        zip) require_cmd unzip ;;
    esac

    TMPDIR="$(mktemp -d)"
    trap 'rm -rf "$TMPDIR"' EXIT

    CHECKSUMS_URL="${RELEASE_BASE_URL}/${VERSION}/checksums.txt"
    CHECKSUMS_PATH="${TMPDIR}/checksums.txt"
    if ! curl -fsSL -L "$CHECKSUMS_URL" -o "$CHECKSUMS_PATH"; then
        echo "Error: could not download checksums.txt for ${VERSION}." >&2
        exit 1
    fi

    ARCHIVE=""
    for candidate in "$ARCHIVE_PRIMARY" "$LEGACY_ARCHIVE"; do
        [ -n "$candidate" ] || continue
        URL="${RELEASE_BASE_URL}/${VERSION}/${candidate}"
        CANDIDATE_PATH="${TMPDIR}/${candidate}"
        if curl -fsSL -L "$URL" -o "$CANDIDATE_PATH"; then
            verify_checksum "$candidate" "$CANDIDATE_PATH" "$CHECKSUMS_PATH"
            ARCHIVE="$candidate"
            break
        fi
    done

    if [ -z "$ARCHIVE" ]; then
        echo "Error: could not download a compatible binary for ${TARGET}." >&2
        exit 1
    fi

    echo "Detected platform: ${OS}/${ARCH} -> ${TARGET}"
    echo "Installing ${BINARY} ${VERSION} (${TARGET})..."

    # Extract
    cd "$TMPDIR"
    case "$ARCHIVE" in
        *.tar.gz) tar xzf "$ARCHIVE" ;;
        *.zip)    unzip -q "$ARCHIVE" ;;
    esac

    SRC="${BINARY}"
    if [ ! -f "$SRC" ]; then
        SRC="$(find . -type f -name "$BINARY" | head -n 1)"
    fi

    if [ -z "$SRC" ] || [ ! -f "$SRC" ]; then
        echo "Error: extracted archive did not contain ${BINARY}." >&2
        exit 1
    fi

    mkdir -p "$INSTALL_DIR"

    # Install
    if [ -w "$INSTALL_DIR" ]; then
        install -m 755 "$SRC" "${INSTALL_DIR}/${BINARY}"
    else
        echo "  Installing to ${INSTALL_DIR} (requires sudo)"
        sudo install -m 755 "$SRC" "${INSTALL_DIR}/${BINARY}"
    fi

    echo ""
    echo "Installed ${BINARY} ${VERSION} to ${INSTALL_DIR}/${BINARY}"
    echo ""
    echo "Get started:"
    echo "  engram auth login              # Configure LLM provider"
    echo "  engram ingest --skip-knowledge  # Archive conversations"
    echo "  engram projects                 # List projects"
    echo "  engram tui                      # Interactive browser"
}

main
