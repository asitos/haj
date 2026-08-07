#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "Usage: $0 <haj-version>" >&2
    exit 1
fi

VERSION="$1"
DISPLAY3D_VERSION="0.2.3"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"
OUTPUT_DIR="$PROJECT_ROOT/dist"
STAGING_DIR="$(mktemp -d)"
ARCHIVE_ROOT="haj-${VERSION}-source"

cleanup() {
    rm -rf "$STAGING_DIR"
}
trap cleanup EXIT

command -v cargo >/dev/null || { echo "cargo is required" >&2; exit 1; }
command -v git >/dev/null || { echo "git is required" >&2; exit 1; }
command -v tar >/dev/null || { echo "tar is required" >&2; exit 1; }

cd "$PROJECT_ROOT"
DISPLAY3D_SOURCE="$(find "$CARGO_HOME_DIR/registry/src" -mindepth 2 -maxdepth 2 -type d -name "display3d-${DISPLAY3D_VERSION}" -print -quit)"
if [[ -z "$DISPLAY3D_SOURCE" ]]; then
    cargo info "display3d@${DISPLAY3D_VERSION}" >/dev/null
    DISPLAY3D_SOURCE="$(find "$CARGO_HOME_DIR/registry/src" -mindepth 2 -maxdepth 2 -type d -name "display3d-${DISPLAY3D_VERSION}" -print -quit)"
fi

if [[ -z "$DISPLAY3D_SOURCE" ]]; then
    echo "unable to locate downloaded display3d ${DISPLAY3D_VERSION} source" >&2
    exit 1
fi

RELEASE_DIR="$STAGING_DIR/$ARCHIVE_ROOT"
mkdir -p "$RELEASE_DIR/third_party"
git archive --format=tar HEAD | tar -xf - -C "$RELEASE_DIR"
cp -a "$DISPLAY3D_SOURCE" "$RELEASE_DIR/third_party/display3d"

printf '%s\n' \
    '# Third-party software' \
    '' \
    'This source release bundles display3d v0.2.3 in third_party/display3d.' \
    'display3d is licensed under MIT OR Apache-2.0; its license is included' \
    'in third_party/display3d/LICENSE.' \
    > "$RELEASE_DIR/THIRD_PARTY_NOTICES.md"

mkdir -p "$OUTPUT_DIR"
tar -C "$STAGING_DIR" -czf "$OUTPUT_DIR/${ARCHIVE_ROOT}.tar.gz" "$ARCHIVE_ROOT"
echo "created $OUTPUT_DIR/${ARCHIVE_ROOT}.tar.gz"
