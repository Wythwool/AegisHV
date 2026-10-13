#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-x86_64-unknown-linux-gnu}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if ! command -v cosign >/dev/null 2>&1; then
  echo "cosign is required to sign release artifacts; install the pinned release workflow version and rerun this script" >&2
  exit 127
fi

VERSION="$(grep '^version = ' Cargo.toml | head -1 | cut -d '"' -f2)"
ARTIFACTS=(
  "dist/aegishv-${VERSION}-${TARGET}.tar.gz"
  "dist/aegishv-${VERSION}-${TARGET}.sbom.cdx.json"
  "dist/SHA256SUMS-${TARGET}.txt"
)

for artifact in "${ARTIFACTS[@]}"; do
  if [[ ! -f "$artifact" ]]; then
    echo "release artifact is missing and cannot be signed: $artifact" >&2
    exit 2
  fi
  if [[ ! -s "$artifact" ]]; then
    echo "release artifact is empty and cannot be signed: $artifact" >&2
    exit 2
  fi
done

for artifact in "${ARTIFACTS[@]}"; do
  bundle="${artifact}.sigstore.json"
  rm -f "$bundle"
  cosign sign-blob --yes --bundle "$bundle" "$artifact"
  if [[ ! -s "$bundle" ]]; then
    echo "cosign did not create a signature bundle for: $artifact" >&2
    exit 3
  fi
  echo "signed $artifact -> $bundle"
done
