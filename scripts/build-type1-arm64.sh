#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

cargo rustc --locked -p aegishv-type1-kernel --bin aegishv-el2 \
  --target aarch64-unknown-none --profile type1 -- \
  -C panic=abort -C relocation-model=static -C strip=none \
  -C link-arg=-Tboot/linker/arm64-type1.ld
mkdir -p target/type1
cp target/aarch64-unknown-none/type1/aegishv-el2 target/type1/aegishv-el2.elf
