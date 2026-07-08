#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

kernel_elf="${1:-${AEGISHV_TYPE1_KERNEL_ELF:-target/type1/aegishv-type1.elf}}"
out_dir="${AEGISHV_TYPE1_OUT:-target/type1}"
manifest="${AEGISHV_TYPE1_INSPECT_MANIFEST:-$out_dir/aegishv-type1-kernel-inspect.txt}"
expected_entry="${AEGISHV_TYPE1_EXPECTED_ENTRY:-0xFFFFFFFF80200000}"
expected_serial="${AEGISHV_TYPE1_EXPECTED_SERIAL:-aegishv:type1:halt}"

usage() {
  cat >&2 <<'USAGE'
usage: scripts/inspect-type1-kernel.sh [KERNEL_ELF]

Checks the minimal type-1 kernel ELF artifact for the expected entry address
when llvm-readobj is available and always checks the serial marker bytes.
USAGE
}

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
  usage
  exit 0
fi

if [ ! -s "$kernel_elf" ]; then
  echo "type1 kernel inspect: kernel ELF is missing or empty: $kernel_elf" >&2
  exit 66
fi

entry_value="unavailable"
entry_check="skipped"
if command -v llvm-readobj >/dev/null 2>&1; then
  entry_value="$(llvm-readobj --file-headers "$kernel_elf" | awk '/Entry:/ {print $2; exit}')"
  entry_check="passed"
  if [ "$entry_value" != "$expected_entry" ]; then
    echo "type1 kernel inspect: unexpected entry address: $entry_value" >&2
    exit 70
  fi
fi

if ! grep -Fqa "$expected_serial" "$kernel_elf"; then
  echo "type1 kernel inspect: serial marker was not found: $expected_serial" >&2
  exit 70
fi

mkdir -p "$(dirname "$manifest")"
cat > "$manifest" <<PLAN
aegishv type-1 kernel inspect

kernel_elf=$kernel_elf
entry_value=$entry_value
entry_check=$entry_check
expected_entry=$expected_entry
serial_marker=$expected_serial
serial_marker_present=true
bootable_image=false
qemu_evidence=false

This manifest records local ELF inspection. It is not QEMU boot evidence.
PLAN

echo "$manifest"
