#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-x86_64-unknown-linux-gnu}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if ! command -v syft >/dev/null 2>&1; then
  echo "syft is required to generate release SBOMs; install the pinned release workflow version and rerun this script" >&2
  exit 127
fi

VERSION="$(grep '^version = ' Cargo.toml | head -1 | cut -d '"' -f2)"
PKG="dist/aegishv-${VERSION}-${TARGET}"
OUT="dist/aegishv-${VERSION}-${TARGET}.sbom.cdx.json"

if [[ ! -d "$PKG" ]]; then
  echo "release package is missing: $PKG; run scripts/package-release.sh $TARGET first" >&2
  exit 2
fi

for required in Cargo.toml Cargo.lock; do
  if [[ ! -f "$PKG/$required" ]]; then
    echo "release package is missing $required; SBOM would not represent the locked Rust dependency graph" >&2
    exit 2
  fi
done

mkdir -p dist
rm -f "$OUT"
syft "dir:$PKG" \
  --source-name "aegishv-${TARGET}" \
  --source-version "$VERSION" \
  -o "cyclonedx-json=$OUT"

python3 - "$OUT" "$ROOT" <<'PY'
import json
import sys
from pathlib import Path

sbom_path = Path(sys.argv[1])
root = sys.argv[2].replace("\\", "/")
text = sbom_path.read_text(encoding="utf-8")

for fragment in [
    root,
    "/target/",
    "\\target\\",
    "/.git/",
    "\\.git\\",
    ".pytest_cache",
    "__pycache__",
    "node_modules",
    "/.cache/",
    "\\.cache\\",
]:
    if fragment and fragment in text:
        print(f"SBOM contains forbidden host/build path fragment: {fragment}", file=sys.stderr)
        sys.exit(3)

document = json.loads(text)
if document.get("bomFormat") != "CycloneDX":
    print("SBOM generator did not emit a CycloneDX JSON document", file=sys.stderr)
    sys.exit(3)

metadata = document.get("metadata", {})
component = metadata.get("component", {})
if not component.get("name"):
    print("SBOM metadata is missing a source component name", file=sys.stderr)
    sys.exit(3)
PY

echo "wrote $OUT"
