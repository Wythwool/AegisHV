#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

target="x86_64-unknown-none"
out_dir="${AEGISHV_TYPE1_OUT:-target/type1}"
kernel_elf="${AEGISHV_TYPE1_KERNEL_ELF:-$out_dir/aegishv-type1.elf}"
manifest="$out_dir/aegishv-type1-kernel-build.txt"
linker_script="boot/linker/x86_64-type1.ld"
expected_kernel_physical_base="${AEGISHV_TYPE1_EXPECTED_PHYSICAL_BASE:-0x00200000}"
expected_kernel_virtual_base="${AEGISHV_TYPE1_EXPECTED_VIRTUAL_BASE:-0xFFFFFFFF80200000}"

usage() {
  cat >&2 <<'USAGE'
usage: scripts/build-type1-kernel.sh

Builds the minimal x86_64 type-1 kernel ELF artifact. This does not create a
bootable ISO and does not run QEMU.
USAGE
}

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
  usage
  exit 0
fi

if ! rustup target list --installed | grep -Fxq "$target"; then
  echo "type1 kernel build: missing Rust target $target" >&2
  echo "type1 kernel build: run: rustup target add $target" >&2
  exit 69
fi

if [ ! -f "$linker_script" ]; then
  echo "type1 kernel build: missing linker script: $linker_script" >&2
  exit 66
fi

mkdir -p "$out_dir"

cargo rustc \
  --locked \
  -p aegishv-type1-kernel \
  --bin aegishv-type1-kernel \
  --target "$target" \
  --release \
  -- \
  -C panic=abort \
  -C relocation-model=static \
  -C code-model=kernel \
  -C strip=none \
  -C link-arg=-T"$linker_script"

built="target/$target/release/aegishv-type1-kernel"
if [ ! -s "$built" ]; then
  echo "type1 kernel build: expected ELF was not written: $built" >&2
  exit 70
fi

cp "$built" "$kernel_elf"
bash scripts/plan-type1-image.sh --require-kernel --kernel-elf "$kernel_elf" >/dev/null
inspect_manifest="$(bash scripts/inspect-type1-kernel.sh "$kernel_elf")"

cat > "$manifest" <<PLAN
aegishv type-1 kernel build

kernel_elf=$kernel_elf
kernel_elf_present=true
target=$target
linker_script=$linker_script
expected_kernel_physical_base=$expected_kernel_physical_base
expected_kernel_virtual_base=$expected_kernel_virtual_base
relocation_model=static
code_model=kernel
serial_marker=aegishv:type1:halt
inspect_manifest=$inspect_manifest
bootable_image=false
qemu_evidence=false

This manifest records a built kernel ELF artifact. It is not a bootable ISO and not QEMU boot evidence.
PLAN

echo "$kernel_elf"
