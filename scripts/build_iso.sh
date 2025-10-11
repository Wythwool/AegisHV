#!/usr/bin/env bash
set -euo pipefail
make -C hv
cp hv/aegishv.elf hv/iso/boot/
grub-mkrescue -o AegisHV.iso hv/iso
echo "ISO ready: AegisHV.iso"
