#!/usr/bin/env bash
set -euo pipefail

REPO_URL="https://github.com/asitos/haj.git"
PREFIX="${PREFIX:-/usr/local}"
UNINSTALL=false
NO_ALPM=false
WITH_DISPLAY3D=false
TMP_CLONE=""

usage() {
    cat <<'EOF'
Usage: ./install.sh [OPTIONS]

Build and install haj from source, or uninstall an existing installation.

Options:
  --uninstall, -u     Remove installed haj binary and completions
  --prefix <dir>      Install prefix (default: /usr/local)
  --no-alpm           Build without libalpm (--no-default-features)
  --with-display3d    Also install optional display3d 3D shark renderer
  --help, -h          Show this help message
EOF
}

cleanup() {
    if [[ -n "$TMP_CLONE" && -d "$TMP_CLONE" ]]; then
        rm -rf "$TMP_CLONE"
    fi
}
trap cleanup EXIT

while [[ $# -gt 0 ]]; do
    case "$1" in
        --uninstall|-u)
            UNINSTALL=true
            shift
            ;;
        --no-alpm)
            NO_ALPM=true
            shift
            ;;
        --with-display3d)
            WITH_DISPLAY3D=true
            shift
            ;;
        --prefix)
            if [[ $# -lt 2 || -z "$2" ]]; then
                echo "error: --prefix requires a directory argument" >&2
                exit 1
            fi
            PREFIX="${2%/}"
            shift 2
            ;;
        --prefix=*)
            PREFIX="${1#*=}"
            PREFIX="${PREFIX%/}"
            if [[ -z "$PREFIX" ]]; then
                echo "error: --prefix requires a non-empty path" >&2
                exit 1
            fi
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            echo "error: unknown option '$1'" >&2
            usage >&2
            exit 1
            ;;
    esac
done

run_priv() {
    local probe_dir="$1"
    shift
    if [[ "$(id -u)" -eq 0 ]] || [[ -d "$probe_dir" && -w "$probe_dir" ]]; then
        "$@"
    elif mkdir -p "$probe_dir" 2>/dev/null; then
        "$@"
    else
        sudo "$@"
    fi
}

completion_paths() {
    echo "$PREFIX/share/bash-completion/completions/haj"
    echo "$PREFIX/share/fish/vendor_completions.d/haj.fish"
    echo "$PREFIX/share/zsh/site-functions/_haj"
}

is_haj_repo() {
    local dir="$1"
    [[ -f "$dir/Cargo.toml" ]] && grep -q '^name = "haj"' "$dir/Cargo.toml"
}

uninstall_haj() {
    local bin_path="$PREFIX/bin/haj"
    local removed=false

    if [[ -e "$bin_path" || -L "$bin_path" ]]; then
        echo "==> removing $bin_path..."
        run_priv "$(dirname "$bin_path")" rm -f "$bin_path"
        removed=true
    fi

    while IFS= read -r comp_path; do
        if [[ -e "$comp_path" || -L "$comp_path" ]]; then
            echo "==> removing $comp_path..."
            run_priv "$(dirname "$comp_path")" rm -f "$comp_path"
            removed=true
        fi
    done < <(completion_paths)

    if [[ "$removed" == true ]]; then
        echo "haj uninstalled from $PREFIX."
    else
        echo "no haj installation found under $PREFIX."
        if [[ "$PREFIX" != "/usr" && -e "/usr/bin/haj" ]]; then
            echo "note: found /usr/bin/haj; pass '--prefix /usr' if you installed there manually."
        fi
    fi
}

if [[ "$UNINSTALL" == true ]]; then
    uninstall_haj
    exit 0
fi

command -v cargo >/dev/null 2>&1 || {
    echo "error: rust toolchain (cargo) is required. Install it from https://rustup.rs" >&2
    exit 1
}

if is_haj_repo "$PWD"; then
    REPO_DIR="$PWD"
elif [[ -n "${BASH_SOURCE[0]:-}" ]] && is_haj_repo "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; then
    REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
else
    command -v git >/dev/null 2>&1 || {
        echo "error: git is required when running outside a haj checkout" >&2
        exit 1
    }
    TMP_CLONE="$(mktemp -d)"
    echo "==> cloning haj into temporary directory..."
    git clone --depth 1 "$REPO_URL" "$TMP_CLONE"
    REPO_DIR="$TMP_CLONE"
fi

cd "$REPO_DIR"

BUILD_ARGS=(build --release --locked)
if [[ "$NO_ALPM" == true ]]; then
    BUILD_ARGS+=(--no-default-features)
elif ! command -v pkg-config >/dev/null 2>&1 || ! pkg-config --exists libalpm; then
    echo "==> libalpm development headers not found; building with pure-CLI backend (--no-default-features)"
    BUILD_ARGS+=(--no-default-features)
fi

echo "==> building haj from source (cargo ${BUILD_ARGS[*]})..."
cargo "${BUILD_ARGS[@]}"

echo "==> installing haj to $PREFIX/bin/haj..."
run_priv "$PREFIX/bin" install -Dm755 target/release/haj "$PREFIX/bin/haj"

if [[ -f "src/completions/bash.sh" ]]; then
    run_priv "$PREFIX/share/bash-completion/completions" \
        install -Dm644 src/completions/bash.sh "$PREFIX/share/bash-completion/completions/haj"
fi
if [[ -f "src/completions/fish.sh" ]]; then
    run_priv "$PREFIX/share/fish/vendor_completions.d" \
        install -Dm644 src/completions/fish.sh "$PREFIX/share/fish/vendor_completions.d/haj.fish"
fi
if [[ -f "src/completions/zsh.sh" ]]; then
    run_priv "$PREFIX/share/zsh/site-functions" \
        install -Dm644 src/completions/zsh.sh "$PREFIX/share/zsh/site-functions/_haj"
fi

if [[ "$WITH_DISPLAY3D" == true ]] && ! command -v display3d >/dev/null 2>&1; then
    echo "==> installing optional display3d animation renderer..."
    cargo install display3d
elif ! command -v display3d >/dev/null 2>&1; then
    echo "note: optional 3D shark renderer 'display3d' not installed (tui falls back to static art)."
    echo "      re-run with --with-display3d or 'cargo install display3d' to enable it."
fi

echo "haj installed successfully at $PREFIX/bin/haj."
case ":$PATH:" in
    *":$PREFIX/bin:"*) ;;
    *) echo "note: add $PREFIX/bin to your PATH to run 'haj' directly." ;;
esac
