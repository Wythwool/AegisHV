#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
test -s Cargo.lock
if grep -qiE 'placeholder|bootstrap' Cargo.lock; then
  echo "Cargo.lock is not a committed production lockfile" >&2
  exit 1
fi
if grep -R -E --exclude=check-lockfile.sh 'cargo[[:space:]]+generate-lockfile' .github Dockerfile scripts 2>/dev/null; then
  echo "CI/release must not generate Cargo.lock on the fly" >&2
  exit 1
fi
cargo metadata --locked --format-version 1 >/dev/null
