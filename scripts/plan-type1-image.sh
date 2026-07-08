#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

usage() {
  cat >&2 <<'USAGE'
usage: scripts/plan-type1-image.sh [--require-kernel] [--manifest PATH] [--kernel-elf PATH] [--output-image PATH]

Writes a type-1 image input manifest. It verifies checked-in boot inputs and can
optionally require the future kernel ELF to already exist.
USAGE
}

require_kernel=false
out_dir="${AEGISHV_TYPE1_OUT:-target/type1}"
manifest="$out_dir/aegishv-type1-image-plan.txt"
kernel_elf="${AEGISHV_TYPE1_KERNEL_ELF:-target/type1/aegishv-type1.elf}"
output_image="${AEGISHV_TYPE1_OUTPUT_IMAGE:-target/type1/aegishv-type1.iso}"
limine_config="boot/limine/limine.conf"
linker_script="boot/linker/x86_64-type1.ld"
entry_stub="boot/x86_64/entry.S"
expected_serial="${AEGISHV_TYPE1_EXPECTED_SERIAL:-aegishv:type1:halt}"
expected_kernel_physical_base="${AEGISHV_TYPE1_EXPECTED_PHYSICAL_BASE:-0x00200000}"
expected_kernel_virtual_base="${AEGISHV_TYPE1_EXPECTED_VIRTUAL_BASE:-0xFFFFFFFF80200000}"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --require-kernel)
      require_kernel=true
      shift
      ;;
    --manifest)
      manifest="${2:-}"
      shift 2
      ;;
    --kernel-elf)
      kernel_elf="${2:-}"
      shift 2
      ;;
    --output-image)
      output_image="${2:-}"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      usage
      exit 64
      ;;
  esac
done

require_nonempty() {
  local value="$1"
  local label="$2"
  if [ -z "$value" ]; then
    echo "type1 image plan: missing $label" >&2
    exit 64
  fi
}

require_file() {
  local path="$1"
  local label="$2"
  if [ ! -f "$path" ]; then
    echo "type1 image plan: missing $label: $path" >&2
    exit 66
  fi
}

require_suffix() {
  local path="$1"
  local suffix="$2"
  local label="$3"
  case "$path" in
    *"$suffix") ;;
    *)
      echo "type1 image plan: $label must end with $suffix: $path" >&2
      exit 65
      ;;
  esac
}

require_nonempty "$manifest" "manifest path"
require_nonempty "$kernel_elf" "kernel ELF path"
require_nonempty "$output_image" "output image path"
require_nonempty "$expected_serial" "expected serial marker"
require_nonempty "$expected_kernel_physical_base" "expected physical base"
require_nonempty "$expected_kernel_virtual_base" "expected virtual base"

require_suffix "$kernel_elf" ".elf" "kernel ELF path"
require_suffix "$output_image" ".iso" "output image path"

require_file "$limine_config" "Limine config"
require_file "$linker_script" "linker script"
require_file "$entry_stub" "entry stub"

kernel_elf_present=false
if [ -f "$kernel_elf" ]; then
  kernel_elf_present=true
elif [ "$require_kernel" = true ]; then
  echo "type1 image plan: kernel ELF does not exist: $kernel_elf" >&2
  exit 66
fi

mkdir -p "$(dirname "$manifest")"

cat > "$manifest" <<PLAN
aegishv type-1 image plan

bootable_image=false
runtime_backend=false
kernel_elf=$kernel_elf
kernel_elf_present=$kernel_elf_present
output_image=$output_image
limine_config=$limine_config
linker_script=$linker_script
x86_entry_stub=$entry_stub
qemu_smoke=scripts/type1-qemu-smoke.sh
qemu_expected_serial=$expected_serial
expected_kernel_physical_base=$expected_kernel_physical_base
expected_kernel_virtual_base=$expected_kernel_virtual_base

This manifest records the current image inputs and QEMU evidence contract. It is not a boot evidence record.
PLAN

echo "$manifest"
