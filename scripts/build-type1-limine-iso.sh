#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

out_dir="${AEGISHV_TYPE1_OUT:-target/type1}"
iso_root="${AEGISHV_TYPE1_ISO_ROOT:-$out_dir/limine-iso-root}"
iso_image="${AEGISHV_TYPE1_ISO_IMAGE:-$out_dir/aegishv-type1.iso}"
manifest="${AEGISHV_TYPE1_ISO_MANIFEST:-$out_dir/aegishv-type1-iso-build.txt}"
limine_dir="${AEGISHV_LIMINE_DIR:-}"

usage() {
  cat >&2 <<'USAGE'
usage: scripts/build-type1-limine-iso.sh [--build-kernel]

Builds a Limine ISO from the staged type-1 kernel tree when xorriso, limine,
and AEGISHV_LIMINE_DIR are available. It does not run QEMU.
USAGE
}

build_kernel=false
while [ "$#" -gt 0 ]; do
  case "$1" in
    --build-kernel)
      build_kernel=true
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

if ! command -v xorriso >/dev/null 2>&1; then
  echo "type1 limine iso: xorriso was not found" >&2
  exit 69
fi

if ! command -v limine >/dev/null 2>&1; then
  echo "type1 limine iso: limine command was not found" >&2
  exit 69
fi

if [ -z "$limine_dir" ]; then
  echo "type1 limine iso: AEGISHV_LIMINE_DIR is not set" >&2
  exit 64
fi

for file in limine-bios.sys limine-bios-cd.bin limine-uefi-cd.bin; do
  if [ ! -f "$limine_dir/$file" ]; then
    echo "type1 limine iso: missing Limine file: $limine_dir/$file" >&2
    exit 66
  fi
done

if [ "$build_kernel" = true ]; then
  bash scripts/build-type1-kernel.sh >/dev/null
fi
bash scripts/stage-type1-limine-iso.sh >/dev/null

cp "$limine_dir/limine-bios.sys" "$iso_root/boot/limine/limine-bios.sys"
cp "$limine_dir/limine-bios-cd.bin" "$iso_root/boot/limine/limine-bios-cd.bin"
cp "$limine_dir/limine-uefi-cd.bin" "$iso_root/boot/limine/limine-uefi-cd.bin"

bootx64_present=false
if [ -f "$limine_dir/BOOTX64.EFI" ]; then
  cp "$limine_dir/BOOTX64.EFI" "$iso_root/EFI/BOOT/BOOTX64.EFI"
  bootx64_present=true
fi

xorriso -as mkisofs \
  -b boot/limine/limine-bios-cd.bin \
  -no-emul-boot \
  -boot-load-size 4 \
  -boot-info-table \
  --efi-boot boot/limine/limine-uefi-cd.bin \
  -efi-boot-part \
  --efi-boot-image \
  --protective-msdos-label \
  "$iso_root" \
  -o "$iso_image"

limine bios-install "$iso_image"

cat > "$manifest" <<PLAN
aegishv type-1 Limine ISO build

iso_image=$iso_image
iso_root=$iso_root
limine_dir=$limine_dir
bootx64_present=$bootx64_present
bootable_iso=true
qemu_evidence=false

This manifest records a locally built Limine ISO. It is not QEMU boot evidence.
PLAN

echo "$iso_image"
