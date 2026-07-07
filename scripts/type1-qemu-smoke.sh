#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'USAGE'
usage: scripts/type1-qemu-smoke.sh [--print-command] BOOT_IMAGE

BOOT_IMAGE may also be supplied through AEGISHV_TYPE1_BOOT_IMAGE.
This script is opt-in lab plumbing. It does not build a boot image.
USAGE
}

print_command=false
image="${AEGISHV_TYPE1_BOOT_IMAGE:-}"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --print-command)
      print_command=true
      shift
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

if [ -z "$image" ]; then
  usage
  exit 64
fi

if [ ! -f "$image" ]; then
  echo "type1 qemu smoke: boot image does not exist: $image" >&2
  exit 66
fi

qemu="${AEGISHV_QEMU:-qemu-system-x86_64}"
if ! command -v "$qemu" >/dev/null 2>&1; then
  echo "type1 qemu smoke: qemu-system-x86_64 was not found" >&2
  exit 69
fi

serial_log="${AEGISHV_QEMU_SERIAL_LOG:-/tmp/aegishv-type1-serial.log}"
timeout_seconds="${AEGISHV_QEMU_TIMEOUT_SECONDS:-15}"

cmd=(
  "$qemu"
  -machine q35
  -cpu qemu64
  -m 256M
  -serial "file:$serial_log"
  -display none
  -no-reboot
  -no-shutdown
  -kernel "$image"
)

if [ "$print_command" = true ]; then
  printf '%q ' "${cmd[@]}"
  printf '\n'
  exit 0
fi

if command -v timeout >/dev/null 2>&1; then
  timeout "$timeout_seconds" "${cmd[@]}"
else
  "${cmd[@]}"
fi
