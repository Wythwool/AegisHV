#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

out_dir="${AEGISHV_TYPE1_OUT:-target/type1}"
boot_image="${AEGISHV_TYPE1_BOOT_IMAGE:-}"
manifest="${AEGISHV_TYPE1_QEMU_MANIFEST:-$out_dir/aegishv-type1-qemu-evidence.txt}"
serial_log="${AEGISHV_QEMU_SERIAL_LOG:-$out_dir/aegishv-type1-serial.log}"
default_expected_markers="aegishv:type1:backend-vmx,aegishv:type1:vmxon-cycle-ok,aegishv:type1:vmcs-load-ok"
expected_marker_csv="${AEGISHV_TYPE1_EXPECTED_MARKERS:-${AEGISHV_TYPE1_EXPECTED_SERIAL:-$default_expected_markers}}"
expected_markers=()
marker_option_mode=""
timeout_seconds="${AEGISHV_QEMU_TIMEOUT_SECONDS:-15}"

usage() {
  cat >&2 <<'USAGE'
usage: scripts/type1-qemu-evidence.sh [--image PATH] [--manifest PATH] [--serial-log PATH] [--expect-markers CSV | --expect-marker TEXT ...] [--timeout SECONDS] [--print-command]

Runs the opt-in type-1 QEMU smoke path and writes an evidence manifest with the
boot image digest, serial log path, ordered marker state, and smoke exit code.
USAGE
}

print_command=false
fail_usage() {
  echo "type1 qemu evidence: $1" >&2
  exit 64
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --image)
      boot_image="${2:-}"
      shift 2
      ;;
    --manifest)
      manifest="${2:-}"
      shift 2
      ;;
    --serial-log)
      serial_log="${2:-}"
      shift 2
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
    --timeout)
      timeout_seconds="${2:-}"
      shift 2
      ;;
    --print-command)
      print_command=true
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      boot_image="$1"
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

required_vmx_markers=(
  "aegishv:type1:backend-vmx"
  "aegishv:type1:vmxon-cycle-ok"
  "aegishv:type1:vmcs-load-ok"
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
  fail_usage "expected serial marker list must include backend-vmx, vmxon-cycle-ok, and vmcs-load-ok in order"
fi

expected_marker_csv="$(IFS=','; printf '%s' "${expected_markers[*]}")"

require_nonempty() {
  local value="$1"
  local label="$2"
  if [ -z "$value" ]; then
    echo "type1 qemu evidence: missing $label" >&2
    exit 64
  fi
}

require_nonempty "$boot_image" "boot image"
require_nonempty "$manifest" "manifest path"
require_nonempty "$serial_log" "serial log path"
require_nonempty "$timeout_seconds" "timeout seconds"

if [ ! -f "$boot_image" ]; then
  echo "type1 qemu evidence: boot image does not exist: $boot_image" >&2
  exit 66
fi

if [ "$print_command" = true ]; then
  smoke_marker_args=()
  for marker in "${expected_markers[@]}"; do
    smoke_marker_args+=(--expect-marker "$marker")
  done
  AEGISHV_QEMU_SERIAL_LOG="$serial_log" \
  AEGISHV_QEMU_TIMEOUT_SECONDS="$timeout_seconds" \
    bash scripts/type1-qemu-smoke.sh --print-command "${smoke_marker_args[@]}" "$boot_image"
  exit 0
fi

command_path() {
  command -v "$1" 2>/dev/null || true
}

first_line_or_unavailable() {
  local command_path="$1"
  local command_arg="$2"
  local line
  line="$("$command_path" "$command_arg" 2>/dev/null | head -n 1 || true)"
  if [ -n "$line" ]; then
    echo "$line"
  else
    echo unavailable
  fi
}

sha256_file() {
  local path="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$path" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$path" | awk '{print $1}'
  else
    echo unavailable
  fi
}

qemu="${AEGISHV_QEMU:-qemu-system-x86_64}"
qemu_path="$(command_path "$qemu")"
qemu_available=false
qemu_version=unavailable
if [ -n "$qemu_path" ]; then
  qemu_available=true
  qemu_version="$(first_line_or_unavailable "$qemu_path" --version)"
fi
qemu_machine="${AEGISHV_QEMU_MACHINE:-q35,accel=kvm}"
qemu_cpu="${AEGISHV_QEMU_CPU:-host,+vmx}"
qemu_boot_mode=kernel
case "$boot_image" in
  *.iso)
    qemu_boot_mode=iso
    ;;
esac

mkdir -p "$(dirname "$manifest")"
mkdir -p "$(dirname "$serial_log")"

boot_image_sha256="$(sha256_file "$boot_image")"

smoke_marker_args=()
for marker in "${expected_markers[@]}"; do
  smoke_marker_args+=(--expect-marker "$marker")
done

qemu_command=unavailable
set +e
rendered_command="$(
  AEGISHV_QEMU_SERIAL_LOG="$serial_log" \
  AEGISHV_QEMU_TIMEOUT_SECONDS="$timeout_seconds" \
    bash scripts/type1-qemu-smoke.sh --print-command "${smoke_marker_args[@]}" "$boot_image"
)"
rendered_command_status=$?
set -e
if [ "$rendered_command_status" -eq 0 ] && [ -n "$rendered_command" ]; then
  qemu_command="$rendered_command"
fi

smoke_status=0
set +e
AEGISHV_QEMU_SERIAL_LOG="$serial_log" \
AEGISHV_QEMU_TIMEOUT_SECONDS="$timeout_seconds" \
  bash scripts/type1-qemu-smoke.sh "${smoke_marker_args[@]}" "$boot_image"
smoke_status=$?
set -e

serial_log_present=false
serial_markers_present=false
serial_markers_in_order=false
forbidden_backend_none_observed=false
forbidden_marker_observed=false
forbidden_marker=""
marker_observed=()

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

if [ -f "$serial_log" ]; then
  serial_log_present=true

  all_markers_present=true
  for marker in "${expected_markers[@]}"; do
    if serial_has_marker "$serial_log" "$marker"; then
      marker_observed+=(true)
    else
      marker_observed+=(false)
      all_markers_present=false
    fi
  done
  serial_markers_present="$all_markers_present"

  next_marker=0
  while IFS= read -r line || [ -n "$line" ]; do
    line="${line%$'\r'}"
    if [ "$line" = "${expected_markers[$next_marker]}" ]; then
      next_marker=$((next_marker + 1))
      if [ "$next_marker" -eq "${#expected_markers[@]}" ]; then
        serial_markers_in_order=true
        break
      fi
    fi
  done < "$serial_log"

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
    "aegishv:type1:panic"
  )
  for candidate in "${forbidden_markers[@]}"; do
    if serial_has_marker "$serial_log" "$candidate"; then
      forbidden_marker_observed=true
      if [ -z "$forbidden_marker" ]; then
        forbidden_marker="$candidate"
      fi
      if [ "$candidate" = "aegishv:type1:backend-none" ]; then
        forbidden_backend_none_observed=true
      fi
    fi
  done
else
  for _marker in "${expected_markers[@]}"; do
    marker_observed+=(false)
  done
fi

qemu_evidence=false
if [ "$smoke_status" -eq 0 ] \
  && [ "$serial_markers_present" = true ] \
  && [ "$serial_markers_in_order" = true ] \
  && [ "$forbidden_marker_observed" = false ]; then
  qemu_evidence=true
fi

{
  cat <<PLAN
aegishv type-1 QEMU smoke evidence

boot_image=$boot_image
boot_image_sha256=$boot_image_sha256
qemu_available=$qemu_available
qemu_path=$qemu_path
qemu_version=$qemu_version
qemu_machine=$qemu_machine
qemu_cpu=$qemu_cpu
qemu_boot_mode=$qemu_boot_mode
qemu_command=$qemu_command
serial_log=$serial_log
serial_log_present=$serial_log_present
expected_serial=$expected_marker_csv
expected_serial_marker_count=${#expected_markers[@]}
expected_serial_markers=$expected_marker_csv
serial_marker_observed=$serial_markers_in_order
serial_markers_present=$serial_markers_present
serial_markers_in_order=$serial_markers_in_order
forbidden_backend_none_observed=$forbidden_backend_none_observed
forbidden_marker_observed=$forbidden_marker_observed
forbidden_marker=$forbidden_marker
PLAN
  for index in "${!expected_markers[@]}"; do
    manifest_index=$((index + 1))
    echo "expected_serial_marker_$manifest_index=${expected_markers[$index]}"
    echo "serial_marker_${manifest_index}_observed=${marker_observed[$index]}"
  done
  cat <<PLAN
timeout_seconds=$timeout_seconds
qemu_smoke_exit_status=$smoke_status
qemu_evidence=$qemu_evidence

This manifest records a local QEMU smoke attempt. A true qemu_evidence value requires every expected marker in order and rejects contradictory backend, failure, skipped, and panic markers. It does not prove guest entry or VM-exit handling.
PLAN
} > "$manifest"

echo "$manifest"
exit "$smoke_status"
