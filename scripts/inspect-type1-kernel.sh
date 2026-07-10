#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

kernel_elf="${1:-${AEGISHV_TYPE1_KERNEL_ELF:-target/type1/aegishv-type1.elf}}"
out_dir="${AEGISHV_TYPE1_OUT:-target/type1}"
manifest="${AEGISHV_TYPE1_INSPECT_MANIFEST:-$out_dir/aegishv-type1-kernel-inspect.txt}"
expected_entry="${AEGISHV_TYPE1_EXPECTED_ENTRY:-0xFFFFFFFF80200000}"
expected_serial="${AEGISHV_TYPE1_EXPECTED_SERIAL:-aegishv:type1:handoff-ok}"
expected_runtime_backend="${AEGISHV_TYPE1_EXPECTED_RUNTIME_BACKEND:-aegishv:type1:backend-none}"
expected_limine_missing="${AEGISHV_TYPE1_LIMINE_MISSING_SERIAL:-aegishv:type1:limine-missing}"
runtime_backend_markers=(
  "aegishv:type1:backend-none"
  "aegishv:type1:backend-vmx"
  "aegishv:type1:backend-svm"
  "aegishv:type1:runtime-plan-error"
)
runtime_preflight_markers=(
  "aegishv:type1:runtime-preflight-ok"
  "aegishv:type1:runtime-preflight-error"
)
runtime_enable_markers=(
  "aegishv:type1:runtime-enable-ok"
  "aegishv:type1:runtime-enable-error"
)
runtime_region_markers=(
  "aegishv:type1:runtime-regions-ok"
  "aegishv:type1:runtime-regions-error"
)
runtime_vmxon_markers=(
  "aegishv:type1:vmxon-cycle-ok"
  "aegishv:type1:vmxon-cycle-error"
  "aegishv:type1:vmxon-cycle-skipped"
)
runtime_vmcs_load_markers=(
  "aegishv:type1:vmcs-load-ok"
  "aegishv:type1:vmcs-load-error"
  "aegishv:type1:vmcs-load-skipped"
)
host_state_markers=(
  "aegishv:type1:host-tables-ok"
  "aegishv:type1:host-tables-error"
  "aegishv:type1:host-exception"
  "aegishv:type1:host-fatal"
)
vmx_guest_markers=(
  "aegishv:type1:guest-config-ok"
  "aegishv:type1:guest-cpuid-exit-ok"
  "aegishv:type1:guest-hlt-exit-ok"
  "aegishv:type1:guest-run-ok"
  "aegishv:type1:guest-entry-error"
  "aegishv:type1:guest-exit-error"
  "aegishv:type1:guest-resume-error"
  "aegishv:type1:vm-instruction-error=0x"
)
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
load_segment_page_alignment="skipped"
load_segment_permissions="skipped"
static_elf_check="skipped"
symbol_table_check="skipped"
if command -v llvm-readobj >/dev/null 2>&1; then
  file_headers="$(llvm-readobj --file-headers "$kernel_elf")"
  entry_value="$(printf '%s\n' "$file_headers" | awk '/Entry:/ {print $2; exit}')"
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
  require_section_field ".bss" "Type: SHT_NOBITS"
  require_section_field ".boot_stack" "Size: 262144"
  require_section_field ".boot_stack" "Type: SHT_NOBITS"
  load_segment_page_alignment="passed"
  load_segment_count=0
  while IFS= read -r address; do
    load_segment_count=$((load_segment_count + 1))
    case "$address" in
      0x*000) ;;
      *)
        echo "type1 kernel inspect: PT_LOAD is not page aligned: $address" >&2
        exit 70
        ;;
    esac
  done < <(
    llvm-readobj --program-headers "$kernel_elf" | awk '
      /Type: PT_LOAD/ { load = 1; next }
      load && /VirtualAddress:/ { print $2; load = 0 }
    '
  )
  if [ "$load_segment_count" -ne 3 ]; then
    echo "type1 kernel inspect: expected exactly three PT_LOAD segments, found $load_segment_count" >&2
    exit 70
  fi
  load_segment_permissions="passed"
  load_permissions="$(
    llvm-readobj --program-headers "$kernel_elf" | awk '
      /Type: PT_LOAD/ { load = 1; read = 0; write = 0; execute = 0; next }
      load && /PF_R/ { read = 1 }
      load && /PF_W/ { write = 1 }
      load && /PF_X/ { execute = 1 }
      load && /Alignment:/ { print read, write, execute; load = 0 }
    '
  )"
  expected_load_permissions="1 0 1
1 0 0
1 1 0"
  if [ "$load_permissions" != "$expected_load_permissions" ]; then
    echo "type1 kernel inspect: expected RX, R, and RW PT_LOAD permissions" >&2
    exit 70
  fi
  static_elf_check="passed"
  if ! printf '%s\n' "$file_headers" | grep -Fq "Type: Executable"; then
    echo "type1 kernel inspect: expected an executable ET_EXEC image" >&2
    exit 70
  fi
  relocations="$(llvm-readobj --relocations "$kernel_elf")"
  if printf '%s\n' "$relocations" | grep -Fq "Section ("; then
    echo "type1 kernel inspect: static kernel contains relocations" >&2
    exit 70
  fi
  dynamic_table="$(llvm-readobj --dynamic-table "$kernel_elf")"
  if printf '%s\n' "$dynamic_table" | grep -Fq "DynamicSection ["; then
    echo "type1 kernel inspect: static kernel contains a dynamic section" >&2
    exit 70
  fi
  symbol_table_check="passed"
  symbols="$(llvm-readobj --symbols "$kernel_elf")"
  for symbol in aegishv_type1_start __aegishv_boot_stack_bottom __aegishv_boot_stack_top aegishv_vmx_vmexit_entry; do
    if ! grep -Fq "Name: $symbol" <<< "$symbols"; then
      echo "type1 kernel inspect: diagnostic symbol was not retained: $symbol" >&2
      exit 70
    fi
  done
fi

if ! grep -Fqa "$expected_serial" "$kernel_elf"; then
  echo "type1 kernel inspect: serial marker was not found: $expected_serial" >&2
  exit 70
fi

if ! grep -Fqa "$expected_runtime_backend" "$kernel_elf"; then
  echo "type1 kernel inspect: runtime backend marker was not found: $expected_runtime_backend" >&2
  exit 70
fi

for marker in "${runtime_backend_markers[@]}"; do
  if ! grep -Fqa "$marker" "$kernel_elf"; then
    echo "type1 kernel inspect: runtime backend marker was not found: $marker" >&2
    exit 70
  fi
done

for marker in "${runtime_preflight_markers[@]}"; do
  if ! grep -Fqa "$marker" "$kernel_elf"; then
    echo "type1 kernel inspect: runtime preflight marker was not found: $marker" >&2
    exit 70
  fi
done

for marker in "${runtime_enable_markers[@]}"; do
  if ! grep -Fqa "$marker" "$kernel_elf"; then
    echo "type1 kernel inspect: runtime enable marker was not found: $marker" >&2
    exit 70
  fi
done

for marker in "${runtime_region_markers[@]}"; do
  if ! grep -Fqa "$marker" "$kernel_elf"; then
    echo "type1 kernel inspect: runtime region marker was not found: $marker" >&2
    exit 70
  fi
done

for marker in "${runtime_vmxon_markers[@]}"; do
  if ! grep -Fqa "$marker" "$kernel_elf"; then
    echo "type1 kernel inspect: VMXON cycle marker was not found: $marker" >&2
    exit 70
  fi
done

for marker in "${runtime_vmcs_load_markers[@]}"; do
  if ! grep -Fqa "$marker" "$kernel_elf"; then
    echo "type1 kernel inspect: VMCS load marker was not found: $marker" >&2
    exit 70
  fi
done

for marker in "${host_state_markers[@]}" "${vmx_guest_markers[@]}"; do
  if ! grep -Fqa "$marker" "$kernel_elf"; then
    echo "type1 kernel inspect: host/guest runtime marker was not found: $marker" >&2
    exit 70
  fi
done

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
load_segment_page_alignment=$load_segment_page_alignment
load_segment_permissions=$load_segment_permissions
static_elf_check=$static_elf_check
symbol_table_check=$symbol_table_check
layout_section_count=${#required_layout_sections[@]}
serial_marker=$expected_serial
serial_marker_present=true
runtime_backend_marker=$expected_runtime_backend
runtime_backend_marker_present=true
runtime_backend_probe=cpuid-msr
runtime_backend_marker_count=${#runtime_backend_markers[@]}
runtime_backend_markers_present=true
runtime_preflight=checked
runtime_preflight_marker_count=${#runtime_preflight_markers[@]}
runtime_preflight_markers_present=true
runtime_enable=controlled
runtime_enable_marker_count=${#runtime_enable_markers[@]}
runtime_enable_markers_present=true
runtime_regions=materialized
runtime_region_marker_count=${#runtime_region_markers[@]}
runtime_region_markers_present=true
runtime_vmxon=smoke-cycle
runtime_vmxon_marker_count=${#runtime_vmxon_markers[@]}
runtime_vmxon_markers_present=true
runtime_vmcs_load=smoke-cycle
runtime_vmcs_load_marker_count=${#runtime_vmcs_load_markers[@]}
runtime_vmcs_load_markers_present=true
host_state_marker_count=${#host_state_markers[@]}
host_state_markers_present=true
vmx_guest_marker_count=${#vmx_guest_markers[@]}
vmx_guest_markers_present=true
limine_missing_marker=$expected_limine_missing
limine_missing_marker_present=true
limine_failure_marker_count=${#limine_failure_markers[@]}
limine_failure_markers_present=true
bootable_image=false
qemu_evidence=false

This manifest records local ELF inspection. It is not QEMU boot evidence.
PLAN

echo "$manifest"
