#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

out_dir="${AEGISHV_TYPE1_OUT:-target/type1}"
manifest="$out_dir/aegishv-type1-build-plan.txt"

mkdir -p "$out_dir"

cargo test --locked -p aegishv-type1-boot --all-features

cat > "$manifest" <<'PLAN'
aegishv type-1 boot skeleton

bootable_image=false
runtime_backend=false
limine_config=boot/limine/limine.conf
linker_script=boot/linker/x86_64-type1.ld
x86_entry_stub=boot/x86_64/entry.S
handoff_crate=crates/aegishv-type1-boot

This manifest records the current boot boundary artifacts. It is not a bootable hypervisor image.
PLAN

echo "$manifest"
