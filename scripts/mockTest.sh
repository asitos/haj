#!/usr/bin/env bash
set -e

TEST_ROOT="/tmp/haj-test-root"

echo "==> Setting up mock pacman root at $TEST_ROOT..."
mkdir -p "$TEST_ROOT/var/lib/pacman/local"
mkdir -p "$TEST_ROOT/var/cache/pacman/pkg"
mkdir -p "$TEST_ROOT/etc/pacman.d"

cat <<EOF > "$TEST_ROOT/etc/pacman.conf"
[options]
DBPath = /var/lib/pacman/
CacheDir = /var/cache/pacman/pkg/

[core]
Server = file:///tmp/haj-dummy-repo
EOF

echo "==> Testing haj against mock root..."
cargo build --profile dev

target/debug/haj --root "$TEST_ROOT" ls

echo "==> Mock root verification setup complete!"
