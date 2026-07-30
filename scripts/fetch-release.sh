#!/bin/sh
# fetch-release.sh — install step for `herdr plugin install`.
#
# herdr's `plugin install` clones this repo and runs the manifest's [[build]]
# commands as plain argv (no shell) after user confirmation; this script is
# that build step. It downloads the prebuilt release tarball for this
# platform, verifies it against the release's SHA256SUMS, and only then
# places the binary at target/release/herdr-window-title — the path the
# manifest's commands expect. On any download or verification failure it
# exits nonzero without leaving a partial binary behind.
#
# `herdr plugin link` never runs builds: developers working from a checkout
# build with `cargo build --release` themselves.
#
# Environment:
#   HWT_RELEASE_BASE_URL  Override the release download base URL (tests point
#                         this at a file:// URL). Defaults to the GitHub
#                         release matching the version in herdr-plugin.toml.
#   GITHUB_TOKEN          If set, sent as `Authorization: Bearer` (needed
#                         while the repo is private).

set -eu

fail() {
    echo "fetch-release: error: $*" >&2
    exit 1
}

download() {
    # $1: source URL, $2: destination path
    if [ -n "${GITHUB_TOKEN:-}" ]; then
        curl -fsSL -H "Authorization: Bearer ${GITHUB_TOKEN}" -o "$2" "$1"
    else
        curl -fsSL -o "$2" "$1"
    fi
}

plugin_root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd) \
    || fail "cannot resolve plugin root from $0"
cd "$plugin_root" || fail "cannot cd to plugin root $plugin_root"

[ -f herdr-plugin.toml ] || fail "herdr-plugin.toml not found in $plugin_root"
command -v curl >/dev/null 2>&1 || fail "curl is required to download the release binary"

version=$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' herdr-plugin.toml | head -n 1)
[ -n "$version" ] || fail "could not read version from herdr-plugin.toml"

os=$(uname -s)
arch=$(uname -m)
case "$os" in
    Darwin)
        case "$arch" in
            arm64 | aarch64) target=aarch64-apple-darwin ;;
            x86_64) target=x86_64-apple-darwin ;;
            *) fail "unsupported macOS architecture: $arch" ;;
        esac
        ;;
    Linux)
        case "$arch" in
            aarch64 | arm64) target=aarch64-unknown-linux-gnu ;;
            x86_64) target=x86_64-unknown-linux-gnu ;;
            *) fail "unsupported Linux architecture: $arch" ;;
        esac
        ;;
    *)
        fail "unsupported operating system: $os (this plugin supports macOS and Linux)"
        ;;
esac

tag="v${version}"
base_url=${HWT_RELEASE_BASE_URL:-"https://github.com/mackt/herdr-window-title/releases/download/${tag}"}
archive="herdr-window-title-${tag}-${target}.tar.gz"

workdir=$(mktemp -d "${TMPDIR:-/tmp}/herdr-window-title-fetch.XXXXXX") \
    || fail "could not create a temporary work directory"
trap 'rm -rf "$workdir"' EXIT INT TERM

echo "fetch-release: downloading $archive from $base_url" >&2
download "${base_url}/${archive}" "${workdir}/${archive}" \
    || fail "failed to download ${base_url}/${archive}"
download "${base_url}/SHA256SUMS" "${workdir}/SHA256SUMS" \
    || fail "failed to download ${base_url}/SHA256SUMS"

if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "${workdir}/${archive}")
elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "${workdir}/${archive}")
else
    fail "neither sha256sum nor shasum is available to verify the download"
fi
actual=${actual%% *}

expected=$(awk -v name="$archive" \
    '{ f = $2; sub(/^\*/, "", f); if (f == name) { print $1; exit } }' \
    "${workdir}/SHA256SUMS")
[ -n "$expected" ] || fail "SHA256SUMS has no entry for $archive"
[ "$expected" = "$actual" ] \
    || fail "checksum mismatch for $archive: expected $expected, got $actual"

mkdir -p "${workdir}/extract"
tar -xzf "${workdir}/${archive}" -C "${workdir}/extract" \
    || fail "failed to extract $archive"
binary="${workdir}/extract/target/release/herdr-window-title"
[ -f "$binary" ] || fail "$archive does not contain target/release/herdr-window-title"
chmod +x "$binary"

mkdir -p target/release
mv -f "$binary" target/release/herdr-window-title
echo "fetch-release: installed target/release/herdr-window-title ($tag, $target)" >&2
