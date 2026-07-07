#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'USAGE'
usage: scripts/svm-amd-lab-smoke.sh [--check-host] [--print-command] [--log-dir DIR]

Required for --print-command:
  AEGISHV_TYPE1_BOOT_IMAGE   boot image produced by a separate type-1 build
  AEGISHV_SVM_LAB_KERNEL     Linux kernel image for the AMD SVM lab guest

Optional environment:
  AEGISHV_SVM_LAB_INITRD       initrd passed to the lab guest
  AEGISHV_QEMU                 qemu binary, default qemu-system-x86_64
  AEGISHV_SVM_LAB_REQUIRE_KVM  require /dev/kvm, default 1
  AEGISHV_SVM_LAB_TIMEOUT      command timeout seconds, default 30

This script is opt-in lab plumbing. It checks AMD SVM host prerequisites and
does not build a boot image or claim a working SVM runtime.
USAGE
}

check_host=false
print_command=false
log_dir="${AEGISHV_SVM_LAB_LOG_DIR:-/tmp/aegishv-amd-lab}"

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
    grep -m1 -E '^(vendor_id|flags|Features)' /proc/cpuinfo || true
  fi
  test -e /dev/kvm && ls -l /dev/kvm || true
} >"$host_log"

if [ ! -r /proc/cpuinfo ] || ! grep -Eq '(^flags[[:space:]]*:.*\bsvm\b|^Features[[:space:]]*:.*\bsvm\b)' /proc/cpuinfo; then
  echo "svm amd lab: CPU flags do not report AMD SVM" >&2
  exit 78
fi

qemu="${AEGISHV_QEMU:-qemu-system-x86_64}"
require_kvm="${AEGISHV_SVM_LAB_REQUIRE_KVM:-1}"
timeout_seconds="${AEGISHV_SVM_LAB_TIMEOUT:-30}"

if [ "$require_kvm" = "1" ] && [ ! -e /dev/kvm ]; then
  echo "svm amd lab: /dev/kvm is required for AMD SVM lab execution" >&2
  exit 78
fi

if [ "$check_host" = true ] && [ "$print_command" = false ]; then
  echo "svm amd lab: host prerequisite check passed"
  exit 0
fi

boot_image="${AEGISHV_TYPE1_BOOT_IMAGE:-}"
kernel="${AEGISHV_SVM_LAB_KERNEL:-}"
initrd="${AEGISHV_SVM_LAB_INITRD:-}"

if [ -z "$boot_image" ] || [ -z "$kernel" ]; then
  usage
  exit 64
fi

if [ ! -f "$boot_image" ]; then
  echo "svm amd lab: boot image does not exist: $boot_image" >&2
  exit 66
fi

if [ ! -f "$kernel" ]; then
  echo "svm amd lab: kernel image does not exist: $kernel" >&2
  exit 66
fi

if [ -n "$initrd" ] && [ ! -f "$initrd" ]; then
  echo "svm amd lab: initrd does not exist: $initrd" >&2
  exit 66
fi

if ! command -v "$qemu" >/dev/null 2>&1; then
  echo "svm amd lab: qemu-system-x86_64 was not found" >&2
  exit 69
fi

serial_log="$log_dir/serial.log"
accel="tcg"
cpu="qemu64"
if [ "$require_kvm" = "1" ]; then
  accel="kvm"
  cpu="host,+svm"
fi

cmd=(
  "$qemu"
  -machine "q35,accel=$accel"
  -cpu "$cpu"
  -m 512M
  -display none
  -no-reboot
  -no-shutdown
  -serial "file:$serial_log"
  -kernel "$boot_image"
  -append "aegishv.svm_lab_kernel=$kernel"
)

if [ -n "$initrd" ]; then
  cmd+=(-initrd "$initrd")
fi

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
