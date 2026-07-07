#!/usr/bin/env bash
set -euo pipefail
TARGET="${1:-x86_64-unknown-linux-gnu}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
mkdir -p dist
BIN="target/${TARGET}/release/aegishv"
if [[ ! -x "$BIN" ]]; then
  BIN="target/release/aegishv"
fi
VERSION="$(grep '^version = ' Cargo.toml | head -1 | cut -d '"' -f2)"
PKG="dist/aegishv-${VERSION}-${TARGET}"
rm -rf "$PKG"
mkdir -p "$PKG/schema" "$PKG/docs"
cp "$BIN" "$PKG/aegishv"
cp Cargo.toml Cargo.lock config.example.toml README.md LICENSE-BINARY CHANGELOG.md RELEASE.md "$PKG/"
cp schema/*.json "$PKG/schema/"
cp docs/*.md "$PKG/docs/"
( cd dist && tar -czf "aegishv-${VERSION}-${TARGET}.tar.gz" "$(basename "$PKG")" )
