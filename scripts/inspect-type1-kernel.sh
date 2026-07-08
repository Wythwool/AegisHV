#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

kernel_elf="${1:-${AEGISHV_TYPE1_KERNEL_ELF:-target/type1/aegishv-type1.elf}}"
out_dir="${AEGISHV_TYPE1_OUT:-target/type1}"
manifest="${AEGISHV_TYPE1_INSPECT_MANIFEST:-$out_dir/aegishv-type1-kernel-inspect.txt}"
expected_entry="${AEGISHV_TYPE1_EXPECTED_ENTRY:-0xFFFFFFFF80200000}"
expected_serial="${AEGISHV_TYPE1_EXPECTED_SERIAL:-aegishv:type1:halt}"
expected_limine_missing="${AEGISHV_TYPE1_LIMINE_MISSING_SERIAL:-aegishv:type1:limine-missing}"
limine_failure_markers=(
  "aegishv:type1:limine-base-revision"
  "aegishv:type1:limine-hhdm-missing"
  "aegishv:type1:limine-hhdm-revision"
  "aegishv:type1:limine-hhdm-offset"
  "aegishv:type1:limine-memmap-missing"
  "aegishv:type1:limine-memmap-revision"
  "aegishv:type1:limine-memmap-empty"
  "aegishv:type1:limine-memmap-entries"
  "aegishv:type1:limine-executable-missing"
  "aegishv:type1:limine-executable-revision"
  "aegishv:type1:limine-executable-empty"
  "aegishv:type1:limine-executable-physical"
  "aegishv:type1:limine-executable-virtual"
)
required_layout_sections=(
  ".text"
  ".rodata"
  ".limine_requests"
  ".data"
  ".bss"
  ".boot_stack"
)

section_block() {
  local section="$1"
  printf '%s\n' "$sections" | awk -v section="$section" '
    index($0, "Name: " section " ") { capture = 1 }
    capture { print }
    capture && $0 ~ /^    }/ { exit }
  '
}

require_section_field() {
  local section="$1"
  local expected="$2"
  local block
  block="$(section_block "$section")"
  if [ -z "$block" ]; then
    echo "type1 kernel inspect: section was not found: $section" >&2
    exit 70
  fi
  if ! printf '%s\n' "$block" | grep -Fq "$expected"; then
    echo "type1 kernel inspect: section $section did not contain expected field: $expected" >&2
    exit 70
  fi
}

usage() {
  cat >&2 <<'USAGE'
usage: scripts/inspect-type1-kernel.sh [KERNEL_ELF]

Checks the minimal type-1 kernel ELF artifact for the expected entry address
and Limine request section when llvm-readobj is available. It always checks
the serial marker bytes.
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
limine_requests_section="skipped"
layout_section_check="skipped"
if command -v llvm-readobj >/dev/null 2>&1; then
  entry_value="$(llvm-readobj --file-headers "$kernel_elf" | awk '/Entry:/ {print $2; exit}')"
  entry_check="passed"
  if [ "$entry_value" != "$expected_entry" ]; then
    echo "type1 kernel inspect: unexpected entry address: $entry_value" >&2
    exit 70
  fi
  limine_requests_section="present"
  if ! llvm-readobj --sections "$kernel_elf" | grep -Fq ".limine_requests"; then
    echo "type1 kernel inspect: .limine_requests section was not found" >&2
    exit 70
  fi
  layout_section_check="passed"
  sections="$(llvm-readobj --sections "$kernel_elf")"
  for section in "${required_layout_sections[@]}"; do
    require_section_field "$section" "Name: $section"
  done
  require_section_field ".text" "Address: $expected_entry"
  require_section_field ".boot_stack" "Size: 65536"
fi

if ! grep -Fqa "$expected_serial" "$kernel_elf"; then
  echo "type1 kernel inspect: serial marker was not found: $expected_serial" >&2
  exit 70
fi

if ! grep -Fqa "$expected_limine_missing" "$kernel_elf"; then
  echo "type1 kernel inspect: Limine fallback marker was not found: $expected_limine_missing" >&2
  exit 70
fi

for marker in "${limine_failure_markers[@]}"; do
  if ! grep -Fqa "$marker" "$kernel_elf"; then
    echo "type1 kernel inspect: Limine status marker was not found: $marker" >&2
    exit 70
  fi
done

mkdir -p "$(dirname "$manifest")"
cat > "$manifest" <<PLAN
aegishv type-1 kernel inspect

kernel_elf=$kernel_elf
entry_value=$entry_value
entry_check=$entry_check
expected_entry=$expected_entry
limine_requests_section=$limine_requests_section
layout_section_check=$layout_section_check
layout_section_count=${#required_layout_sections[@]}
serial_marker=$expected_serial
serial_marker_present=true
limine_missing_marker=$expected_limine_missing
limine_missing_marker_present=true
limine_failure_marker_count=${#limine_failure_markers[@]}
limine_failure_markers_present=true
bootable_image=false
qemu_evidence=false

This manifest records local ELF inspection. It is not QEMU boot evidence.
PLAN

echo "$manifest"
