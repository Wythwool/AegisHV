#!/usr/bin/env bash
set -euo pipefail
pushd devharness >/dev/null
cargo build --release
popd >/dev/null
pushd userspace/aegisd >/dev/null
cargo build --release
popd >/dev/null
echo "OK"
