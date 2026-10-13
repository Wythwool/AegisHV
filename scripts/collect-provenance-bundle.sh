#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-}"
BUNDLE="${2:-}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ -z "$TARGET" ]]; then
  echo "release target is required to collect provenance bundle" >&2
  exit 2
fi

if [[ -z "$BUNDLE" ]]; then
  echo "actions/attest did not report a provenance bundle path" >&2
  exit 2
fi

if [[ ! -s "$BUNDLE" ]]; then
  echo "provenance bundle is missing or empty: $BUNDLE" >&2
  exit 2
fi

VERSION="$(grep '^version = ' Cargo.toml | head -1 | cut -d '"' -f2)"
OUT="dist/aegishv-${VERSION}-${TARGET}.slsa-provenance.sigstore.json"

mkdir -p dist
rm -f "$OUT"
cp "$BUNDLE" "$OUT"

python3 - "$OUT" "$ROOT" <<'PY'
import json
import sys
from pathlib import Path

bundle_path = Path(sys.argv[1])
root = sys.argv[2].replace("\\", "/")
text = bundle_path.read_text(encoding="utf-8")

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
        print(f"provenance bundle contains forbidden host/build path fragment: {fragment}", file=sys.stderr)
        sys.exit(3)

document = json.loads(text)
if not isinstance(document, dict):
    print("provenance bundle is not a JSON object", file=sys.stderr)
    sys.exit(3)

if "mediaType" not in document and "dsseEnvelope" not in document:
    print("provenance bundle does not look like a Sigstore bundle", file=sys.stderr)
    sys.exit(3)
PY

echo "wrote $OUT"
