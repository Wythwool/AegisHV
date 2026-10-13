#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'USAGE'
usage: scripts/arm64-el2-lab-smoke.sh [--check-host] [--print-command] [--log-dir DIR]

Required for --print-command:
  AEGISHV_ARM64_BOOT_IMAGE   ARM64 boot image produced by a separate type-1 build

Optional environment:
  AEGISHV_QEMU_ARM64          qemu binary, default qemu-system-aarch64
  AEGISHV_ARM64_REQUIRE_KVM   require /dev/kvm, default 0
  AEGISHV_ARM64_TIMEOUT       command timeout seconds, default 30

This script is opt-in lab plumbing. It checks ARM64 host prerequisites and does
not build a boot image or claim a working EL2 runtime.
USAGE
}

check_host=false
print_command=false
log_dir="${AEGISHV_ARM64_LAB_LOG_DIR:-/tmp/aegishv-arm64-lab}"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --check-host)
      check_host=true
      shift
      ;;
    --print-command)
      print_command=true
      shift
      ;;
    --log-dir)
      if [ "$#" -lt 2 ]; then
        usage
        exit 64
      fi
      log_dir="$2"
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

mkdir -p "$log_dir"
host_log="$log_dir/host.txt"
cmd_log="$log_dir/qemu-command.txt"

{
  uname -a || true
  if [ -r /proc/cpuinfo ]; then
    grep -m1 -E '^(CPU architecture|Features|model name)' /proc/cpuinfo || true
  fi
  test -e /dev/kvm && ls -l /dev/kvm || true
} >"$host_log"

qemu="${AEGISHV_QEMU_ARM64:-qemu-system-aarch64}"
require_kvm="${AEGISHV_ARM64_REQUIRE_KVM:-0}"
timeout_seconds="${AEGISHV_ARM64_TIMEOUT:-30}"

if [ "$require_kvm" = "1" ] && [ ! -e /dev/kvm ]; then
  echo "arm64 el2 lab: /dev/kvm is required for accelerated ARM64 lab execution" >&2
  exit 78
fi

if [ "$check_host" = true ] && [ "$print_command" = false ]; then
  echo "arm64 el2 lab: host prerequisite check completed"
  exit 0
fi

boot_image="${AEGISHV_ARM64_BOOT_IMAGE:-}"
if [ -z "$boot_image" ]; then
  usage
  exit 64
fi

if [ ! -f "$boot_image" ]; then
  echo "arm64 el2 lab: boot image does not exist: $boot_image" >&2
  exit 66
fi

if ! command -v "$qemu" >/dev/null 2>&1; then
  echo "arm64 el2 lab: qemu-system-aarch64 was not found" >&2
  exit 69
fi

serial_log="$log_dir/serial.log"
accel="tcg"
if [ "$require_kvm" = "1" ]; then
  accel="kvm"
fi

cmd=(
  "$qemu"
  -machine "virt,virtualization=on,gic-version=3,accel=$accel"
  -cpu max
  -m 512M
  -display none
  -no-reboot
  -no-shutdown
  -serial "file:$serial_log"
  -kernel "$boot_image"
)

printf '%q ' "${cmd[@]}" >"$cmd_log"
printf '\n' >>"$cmd_log"

if [ "$print_command" = true ]; then
  cat "$cmd_log"
  exit 0
fi

if command -v timeout >/dev/null 2>&1; then
  timeout "$timeout_seconds" "${cmd[@]}"
else
  "${cmd[@]}"
fi
