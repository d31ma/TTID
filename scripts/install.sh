#!/bin/sh
# TTID installer for macOS and Linux.
#   curl -fsSL https://github.com/d31ma/TTID/releases/latest/download/install.sh | sh
# Downloads the right binary from the latest GitHub release, verifies its
# checksum, and installs it to a directory on your PATH.
set -eu

REPO="d31ma/TTID"
RELEASES="${TTID_BASE_URL:-https://github.com/${REPO}/releases}"

# Defaults to the latest release. Set TTID_VERSION to install a specific one --
# this is the rollback path when a release turns out bad:
#   TTID_VERSION=26.28.02 curl -fsSL .../install.sh | sh
# Tags carry a leading "v", which is added here when it is left off.
if [ -n "${TTID_VERSION:-}" ]; then
    tag="$TTID_VERSION"
    case "$tag" in v*) : ;; *) tag="v${tag}" ;; esac
    BASE="${RELEASES}/download/${tag}"
else
    BASE="${RELEASES}/latest/download"
fi

os=$(uname -s)
arch=$(uname -m)

case "$os" in
    Darwin) os_tag="macos" ;;
    Linux) os_tag="linux" ;;
    *) echo "Unsupported OS: $os (use install.ps1 on Windows)" >&2; exit 1 ;;
esac

case "$arch" in
    x86_64 | amd64) arch_tag="x64" ;;
    arm64 | aarch64) arch_tag="arm64" ;;
    *) echo "Unsupported architecture: $arch" >&2; exit 1 ;;
esac

asset="ttid-${os_tag}-${arch_tag}"
url="${BASE}/${asset}"

# Pick an install dir on PATH we can write to; fall back to ~/.local/bin.
if [ -w /usr/local/bin ]; then
    dest="/usr/local/bin"
else
    dest="${HOME}/.local/bin"
    mkdir -p "$dest"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "Downloading ${asset}..."
if ! curl -fSL "$url" -o "$tmp/ttid"; then
    echo "Failed to download ${asset} from ${url}" >&2
    if [ -n "${TTID_VERSION:-}" ]; then
        echo "Is TTID_VERSION=${TTID_VERSION} a released version? See ${RELEASES}" >&2
    fi
    exit 1
fi

# Verify the download against the release SHA256SUMS. This fails closed: any
# step that cannot complete aborts the install rather than skipping the check.
# Set TTID_SKIP_CHECKSUM=1 to install without verification.
if [ "${TTID_SKIP_CHECKSUM:-}" = "1" ]; then
    echo "Warning: skipping checksum verification (TTID_SKIP_CHECKSUM=1)." >&2
else
    if command -v sha256sum >/dev/null 2>&1; then
        hash_cmd="sha256sum"
    elif command -v shasum >/dev/null 2>&1; then
        hash_cmd="shasum -a 256"
    else
        echo "Cannot verify ${asset}: neither sha256sum nor shasum is available." >&2
        echo "Install one, or re-run with TTID_SKIP_CHECKSUM=1 to bypass." >&2
        exit 1
    fi

    if ! curl -fsSL "${BASE}/SHA256SUMS" -o "$tmp/SHA256SUMS"; then
        echo "Cannot verify ${asset}: failed to download SHA256SUMS." >&2
        exit 1
    fi

    # sha256sum writes "<hash>  <name>"; binary mode prefixes the name with '*'.
    expected=$(awk -v a="$asset" '$2 == a || $2 == "*" a { print $1 }' "$tmp/SHA256SUMS")
    if [ -z "$expected" ]; then
        echo "Cannot verify ${asset}: no entry for it in SHA256SUMS." >&2
        exit 1
    fi

    actual=$($hash_cmd "$tmp/ttid" | awk '{print $1}')
    if [ "$expected" != "$actual" ]; then
        echo "Checksum mismatch for ${asset}. Aborting." >&2
        echo "  expected: ${expected}" >&2
        echo "  actual:   ${actual}" >&2
        exit 1
    fi
    echo "Checksum verified."
fi

chmod +x "$tmp/ttid"
mv "$tmp/ttid" "$dest/ttid"

echo "Installed ttid to ${dest}/ttid"
case ":$PATH:" in
    *":$dest:"*) : ;;
    *) echo "Note: ${dest} is not on your PATH. Add it, e.g.:"; echo "  export PATH=\"${dest}:\$PATH\"" ;;
esac
echo "Run 'ttid --help' to get started."
