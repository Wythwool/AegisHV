#!/usr/bin/env bash
set -e
ISO="aegishv.iso"
if [ ! -f "$ISO" ]; then
  echo "ISO not found. Building..."
  make iso
fi

qemu-system-x86_64 \
  -machine q35,accel=kvm:xen:tcg \
  -cpu host,+vmx \
  -m 512 \
  -serial stdio \
  -display none \
  -no-reboot \
  -cdrom "$ISO"
