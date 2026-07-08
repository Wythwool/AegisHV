#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

out_dir="${AEGISHV_TYPE1_OUT:-target/type1}"
boot_image="${AEGISHV_TYPE1_BOOT_IMAGE:-}"
manifest="${AEGISHV_TYPE1_QEMU_MANIFEST:-$out_dir/aegishv-type1-qemu-evidence.txt}"
serial_log="${AEGISHV_QEMU_SERIAL_LOG:-$out_dir/aegishv-type1-serial.log}"
expected_serial="${AEGISHV_TYPE1_EXPECTED_SERIAL:-aegishv:type1:halt}"
timeout_seconds="${AEGISHV_QEMU_TIMEOUT_SECONDS:-15}"

usage() {
  cat >&2 <<'USAGE'
usage: scripts/type1-qemu-evidence.sh [--image PATH] [--manifest PATH] [--serial-log PATH] [--expect-serial TEXT] [--timeout SECONDS] [--print-command]

Runs the opt-in type-1 QEMU smoke path and writes an evidence manifest with the
boot image digest, serial log path, observed marker state, and smoke exit code.
USAGE
}

print_command=false
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
    --expect-serial)
      expected_serial="${2:-}"
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
require_nonempty "$expected_serial" "expected serial marker"
require_nonempty "$timeout_seconds" "timeout seconds"

if [ ! -f "$boot_image" ]; then
  echo "type1 qemu evidence: boot image does not exist: $boot_image" >&2
  exit 66
fi

if [ "$print_command" = true ]; then
  AEGISHV_QEMU_SERIAL_LOG="$serial_log" \
  AEGISHV_QEMU_TIMEOUT_SECONDS="$timeout_seconds" \
    bash scripts/type1-qemu-smoke.sh --print-command --expect-serial "$expected_serial" "$boot_image"
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

mkdir -p "$(dirname "$manifest")"
mkdir -p "$(dirname "$serial_log")"

boot_image_sha256="$(sha256_file "$boot_image")"

smoke_status=0
set +e
AEGISHV_QEMU_SERIAL_LOG="$serial_log" \
AEGISHV_QEMU_TIMEOUT_SECONDS="$timeout_seconds" \
  bash scripts/type1-qemu-smoke.sh --expect-serial "$expected_serial" "$boot_image"
smoke_status=$?
set -e

serial_log_present=false
serial_marker_observed=false
if [ -f "$serial_log" ]; then
  serial_log_present=true
  if grep -Fq "$expected_serial" "$serial_log"; then
    serial_marker_observed=true
  fi
fi

qemu_evidence=false
if [ "$smoke_status" -eq 0 ] && [ "$serial_marker_observed" = true ]; then
  qemu_evidence=true
fi

cat > "$manifest" <<PLAN
aegishv type-1 QEMU smoke evidence

boot_image=$boot_image
boot_image_sha256=$boot_image_sha256
qemu_available=$qemu_available
qemu_path=$qemu_path
qemu_version=$qemu_version
serial_log=$serial_log
serial_log_present=$serial_log_present
expected_serial=$expected_serial
serial_marker_observed=$serial_marker_observed
timeout_seconds=$timeout_seconds
qemu_smoke_exit_status=$smoke_status
qemu_evidence=$qemu_evidence

This manifest records a local QEMU smoke attempt. A true qemu_evidence value requires the expected serial marker in the captured log.
PLAN

echo "$manifest"
exit "$smoke_status"
