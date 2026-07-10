#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'USAGE'
usage: scripts/type1-qemu-smoke.sh [--print-command] [--expect-markers CSV | --expect-marker TEXT ...] BOOT_IMAGE

BOOT_IMAGE may also be supplied through AEGISHV_TYPE1_BOOT_IMAGE.
Expected markers may be supplied through AEGISHV_TYPE1_EXPECTED_MARKERS.
Markers must appear as complete serial-log lines in the configured order.
BOOT_IMAGE must be a bootable Limine ISO. This script does not build it.
USAGE
}

print_command=false
image="${AEGISHV_TYPE1_BOOT_IMAGE:-}"
default_expected_markers="aegishv:type1:host-tables-ok,aegishv:type1:backend-vmx,aegishv:type1:vmxon-cycle-ok,aegishv:type1:vmcs-load-ok,aegishv:type1:host-paging-ok,aegishv:type1:guest-config-ok,aegishv:type1:guest-preempt-exit-ok,aegishv:type1:guest-io-exit-ok,aegishv:type1:guest-io-b-exit-ok,aegishv:type1:guest-cpuid-exit-ok,aegishv:type1:guest-rdmsr-exit-ok,aegishv:type1:guest-pat-state-ok,aegishv:type1:guest-nm-x87-exit-ok,aegishv:type1:guest-nm-simd-exit-ok,aegishv:type1:guest-ud-inject-ok,aegishv:type1:guest-hlt-exit-ok,aegishv:type1:guest-run-ok"
expected_marker_csv="${AEGISHV_TYPE1_EXPECTED_MARKERS:-${AEGISHV_TYPE1_EXPECTED_SERIAL:-$default_expected_markers}}"
expected_markers=()
marker_option_mode=""

fail_usage() {
  echo "type1 qemu smoke: $1" >&2
  exit 64
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --print-command)
      print_command=true
      shift
      ;;
    --expect-markers)
      if [ "$marker_option_mode" = "repeated" ]; then
        fail_usage "--expect-markers cannot be combined with --expect-marker"
      fi
      marker_option_mode="csv"
      expected_marker_csv="${2:-}"
      shift 2
      ;;
    --expect-marker)
      if [ "$marker_option_mode" = "csv" ]; then
        fail_usage "--expect-marker cannot be combined with --expect-markers"
      fi
      if [ "$marker_option_mode" != "repeated" ]; then
        expected_markers=()
        marker_option_mode="repeated"
      fi
      expected_markers+=("${2:-}")
      shift 2
      ;;
    --expect-serial)
      if [ "$marker_option_mode" = "repeated" ]; then
        fail_usage "--expect-serial cannot be combined with --expect-marker"
      fi
      marker_option_mode="csv"
      expected_marker_csv="${2:-}"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      image="$1"
      shift
      ;;
  esac
done

if [ "$marker_option_mode" != "repeated" ]; then
  case "$expected_marker_csv" in
    ""|,*|*,|*,,*)
      fail_usage "expected serial marker list is empty or contains an empty item"
      ;;
  esac
  IFS=',' read -r -a expected_markers <<< "$expected_marker_csv"
fi

if [ "${#expected_markers[@]}" -eq 0 ]; then
  fail_usage "expected serial marker list is empty"
fi

for marker in "${expected_markers[@]}"; do
  if [ -z "$marker" ]; then
    fail_usage "expected serial marker is empty"
  fi
  case "$marker" in
    *$'\n'*|*$'\r'*|*,*)
      fail_usage "expected serial markers cannot contain commas or newlines"
      ;;
  esac
done

for ((marker_index = 0; marker_index < ${#expected_markers[@]}; marker_index++)); do
  for ((other_index = marker_index + 1; other_index < ${#expected_markers[@]}; other_index++)); do
    if [ "${expected_markers[$marker_index]}" = "${expected_markers[$other_index]}" ]; then
      fail_usage "expected serial marker list contains a duplicate: ${expected_markers[$marker_index]}"
    fi
  done
done

required_vmx_markers=(
  "aegishv:type1:host-tables-ok"
  "aegishv:type1:backend-vmx"
  "aegishv:type1:vmxon-cycle-ok"
  "aegishv:type1:vmcs-load-ok"
  "aegishv:type1:host-paging-ok"
  "aegishv:type1:guest-config-ok"
  "aegishv:type1:guest-preempt-exit-ok"
  "aegishv:type1:guest-io-exit-ok"
  "aegishv:type1:guest-io-b-exit-ok"
  "aegishv:type1:guest-cpuid-exit-ok"
  "aegishv:type1:guest-rdmsr-exit-ok"
  "aegishv:type1:guest-pat-state-ok"
  "aegishv:type1:guest-nm-x87-exit-ok"
  "aegishv:type1:guest-nm-simd-exit-ok"
  "aegishv:type1:guest-ud-inject-ok"
  "aegishv:type1:guest-hlt-exit-ok"
  "aegishv:type1:guest-run-ok"
)
required_marker_index=0
for marker in "${expected_markers[@]}"; do
  if [ "$marker" = "${required_vmx_markers[$required_marker_index]}" ]; then
    required_marker_index=$((required_marker_index + 1))
    if [ "$required_marker_index" -eq "${#required_vmx_markers[@]}" ]; then
      break
    fi
  fi
done
if [ "$required_marker_index" -ne "${#required_vmx_markers[@]}" ]; then
  fail_usage "expected serial marker list must include the complete host-table, VMX backend/VMXON/VMCS-load, owned-paging, guest-configuration, preemption, both I/O bitmaps, CPUID, RDMSR, PAT, x87/SIMD #NM, fixed #UD injection, HLT, and completion proof chain in order"
fi

if [ -z "$image" ]; then
  usage
  exit 64
fi

if [ ! -f "$image" ]; then
  echo "type1 qemu smoke: boot image does not exist: $image" >&2
  exit 66
fi
case "$image" in
  *.iso) ;;
  *) fail_usage "boot image must be a Limine ISO; a raw ELF has no Limine handoff" ;;
esac

qemu="${AEGISHV_QEMU:-qemu-system-x86_64}"
if ! command -v "$qemu" >/dev/null 2>&1; then
  echo "type1 qemu smoke: qemu-system-x86_64 was not found" >&2
  exit 69
fi

serial_log="${AEGISHV_QEMU_SERIAL_LOG:-/tmp/aegishv-type1-serial.log}"
timeout_seconds="${AEGISHV_QEMU_TIMEOUT_SECONDS:-15}"
requested_timeout="${AEGISHV_TIMEOUT:-}"
timeout_command=""
if [ -n "$requested_timeout" ]; then
  if command -v "$requested_timeout" >/dev/null 2>&1 \
    && "$requested_timeout" --help >/dev/null 2>&1; then
    timeout_command="$requested_timeout"
  fi
else
  for candidate in timeout /usr/bin/timeout gtimeout; do
    if command -v "$candidate" >/dev/null 2>&1 \
      && "$candidate" --help >/dev/null 2>&1; then
      timeout_command="$candidate"
      break
    fi
  done
fi
boot_mode="iso"

cmd=(
  "$qemu"
  -machine "${AEGISHV_QEMU_MACHINE:-q35,accel=kvm}"
  -cpu "${AEGISHV_QEMU_CPU:-host,+vmx}"
  -m 256M
  -serial "file:$serial_log"
  -display none
  -no-reboot
  -no-shutdown
)

cmd+=(-cdrom "$image" -boot d)

if [ "$print_command" = true ]; then
  printf '%q ' "${cmd[@]}"
  printf '\n'
  exit 0
fi

rm -f "$serial_log"
status=0
if [ -z "$timeout_command" ]; then
  echo "type1 qemu smoke: a compatible timeout command was not found: ${requested_timeout:-timeout}" >&2
  exit 69
fi
"$timeout_command" "$timeout_seconds" "${cmd[@]}" || status=$?

if [ ! -f "$serial_log" ]; then
  echo "type1 qemu smoke: serial log was not written: $serial_log" >&2
  exit 70
fi

serial_has_marker() {
  local log_path="$1"
  local expected="$2"
  local line
  while IFS= read -r line || [ -n "$line" ]; do
    line="${line%$'\r'}"
    if [ "$line" = "$expected" ]; then
      return 0
    fi
  done < "$log_path"
  return 1
}

serial_marker_count() {
  local log_path="$1"
  local expected="$2"
  local line
  local count=0
  while IFS= read -r line || [ -n "$line" ]; do
    line="${line%$'\r'}"
    if [ "$line" = "$expected" ]; then
      count=$((count + 1))
    fi
  done < "$log_path"
  printf '%d\n' "$count"
}

forbidden_markers=(
  "aegishv:type1:backend-none"
  "aegishv:type1:backend-svm"
  "aegishv:type1:runtime-plan-error"
  "aegishv:type1:runtime-preflight-error"
  "aegishv:type1:runtime-enable-error"
  "aegishv:type1:runtime-regions-error"
  "aegishv:type1:vmxon-cycle-error"
  "aegishv:type1:vmxon-cycle-skipped"
  "aegishv:type1:vmcs-load-error"
  "aegishv:type1:vmcs-load-skipped"
  "aegishv:type1:limine-missing"
  "aegishv:type1:host-tables-error"
  "aegishv:type1:host-paging-error"
  "aegishv:type1:host-exception"
  "aegishv:type1:host-fatal"
  "aegishv:type1:guest-timeout"
  "aegishv:type1:guest-entry-error"
  "aegishv:type1:guest-exit-error"
  "aegishv:type1:guest-resume-error"
  "aegishv:type1:guest-pat-state-error"
  "aegishv:type1:guest-nm-x87-exit-error"
  "aegishv:type1:guest-nm-simd-exit-error"
  "aegishv:type1:guest-ud-inject-error"
  "aegishv:type1:panic"
)
for forbidden_marker in "${forbidden_markers[@]}"; do
  if serial_has_marker "$serial_log" "$forbidden_marker"; then
    echo "type1 qemu smoke: forbidden serial marker was observed: $forbidden_marker" >&2
    exit 70
  fi
done

for expected_marker in "${expected_markers[@]}"; do
  observed_count="$(serial_marker_count "$serial_log" "$expected_marker")"
  if [ "$observed_count" -ne 1 ]; then
    echo "type1 qemu smoke: expected serial marker must appear exactly once: $expected_marker (observed $observed_count)" >&2
    exit 70
  fi
done

next_marker=0
while IFS= read -r line || [ -n "$line" ]; do
  line="${line%$'\r'}"
  if [ "$line" = "${expected_markers[$next_marker]}" ]; then
    next_marker=$((next_marker + 1))
    if [ "$next_marker" -eq "${#expected_markers[@]}" ]; then
      break
    fi
  fi
done < "$serial_log"

if [ "$next_marker" -ne "${#expected_markers[@]}" ]; then
  echo "type1 qemu smoke: expected serial marker was not observed in required order: ${expected_markers[$next_marker]}" >&2
  exit 70
fi

if [ "$status" -ne 0 ] && [ "$status" -ne 124 ]; then
  echo "type1 qemu smoke: qemu exited before marker review with status $status" >&2
  exit "$status"
fi
