#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'USAGE'
usage: scripts/vmx-linux-lab-smoke.sh [--print-command]

Required environment:
  AEGISHV_TYPE1_BOOT_IMAGE     boot image produced by a separate type-1 build
  AEGISHV_VMX_LAB_KERNEL       Linux kernel image for the lab guest

Optional environment:
  AEGISHV_VMX_LAB_INITRD       initrd passed to the lab guest
  AEGISHV_QEMU                 qemu binary, default qemu-system-x86_64
  AEGISHV_VMX_LAB_REQUIRE_KVM  require /dev/kvm, default 1
  AEGISHV_VMX_LAB_TIMEOUT      command timeout seconds, default 30

This script is opt-in lab plumbing. It does not build a boot image or claim that
the repository currently boots a type-1 runtime.
USAGE
}

print_command=false

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
      usage
      exit 64
      ;;
  esac
done

boot_image="${AEGISHV_TYPE1_BOOT_IMAGE:-}"
kernel="${AEGISHV_VMX_LAB_KERNEL:-}"
initrd="${AEGISHV_VMX_LAB_INITRD:-}"
qemu="${AEGISHV_QEMU:-qemu-system-x86_64}"
require_kvm="${AEGISHV_VMX_LAB_REQUIRE_KVM:-1}"
timeout_seconds="${AEGISHV_VMX_LAB_TIMEOUT:-30}"

if [ -z "$boot_image" ] || [ -z "$kernel" ]; then
  usage
  exit 64
fi

if [ ! -f "$boot_image" ]; then
  echo "vmx linux lab: boot image does not exist: $boot_image" >&2
  exit 66
fi

if [ ! -f "$kernel" ]; then
  echo "vmx linux lab: kernel image does not exist: $kernel" >&2
  exit 66
fi

if [ -n "$initrd" ] && [ ! -f "$initrd" ]; then
  echo "vmx linux lab: initrd does not exist: $initrd" >&2
  exit 66
fi

if ! command -v "$qemu" >/dev/null 2>&1; then
  echo "vmx linux lab: qemu-system-x86_64 was not found" >&2
  exit 69
fi

if [ "$require_kvm" = "1" ] && [ ! -e /dev/kvm ]; then
  echo "vmx linux lab: /dev/kvm is required for nested VMX lab execution" >&2
  exit 78
fi

serial_log="${AEGISHV_VMX_LAB_SERIAL_LOG:-/tmp/aegishv-vmx-linux-lab-serial.log}"
accel="tcg"
cpu="qemu64"
if [ "$require_kvm" = "1" ]; then
  accel="kvm"
  cpu="host,+vmx"
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
  -append "aegishv.vmx_lab_kernel=$kernel"
)

if [ -n "$initrd" ]; then
  cmd+=(-initrd "$initrd")
fi

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
