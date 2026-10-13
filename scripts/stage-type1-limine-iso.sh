#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

out_dir="${AEGISHV_TYPE1_OUT:-target/type1}"
kernel_elf="${AEGISHV_TYPE1_KERNEL_ELF:-$out_dir/aegishv-type1.elf}"
iso_root="${AEGISHV_TYPE1_ISO_ROOT:-$out_dir/limine-iso-root}"
manifest="${AEGISHV_TYPE1_ISO_STAGE_MANIFEST:-$out_dir/aegishv-type1-iso-stage.txt}"
limine_config="boot/limine/limine.conf"

usage() {
  cat >&2 <<'USAGE'
usage: scripts/stage-type1-limine-iso.sh [--build-kernel]

Stages the current kernel ELF and Limine config into target/type1/limine-iso-root.
It does not create a bootable ISO unless future tooling is added.
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

if [ "$build_kernel" = true ]; then
  bash scripts/build-type1-kernel.sh >/dev/null
fi

if [ ! -s "$kernel_elf" ]; then
  echo "type1 iso stage: kernel ELF is missing or empty: $kernel_elf" >&2
  exit 66
fi

if [ ! -f "$limine_config" ]; then
  echo "type1 iso stage: Limine config is missing: $limine_config" >&2
  exit 66
fi

rm -rf "$iso_root"
mkdir -p "$iso_root/boot/limine" "$iso_root/EFI/BOOT"
cp "$kernel_elf" "$iso_root/aegishv-type1.elf"
cp "$kernel_elf" "$iso_root/boot/aegishv-type1.elf"
cp "$limine_config" "$iso_root/limine.conf"
cp "$limine_config" "$iso_root/boot/limine/limine.conf"

limine_available=false
xorriso_available=false
if command -v limine >/dev/null 2>&1; then
  limine_available=true
fi
if command -v xorriso >/dev/null 2>&1; then
  xorriso_available=true
fi

cat > "$manifest" <<PLAN
aegishv type-1 Limine ISO stage

iso_root=$iso_root
kernel_source=$kernel_elf
kernel_staged=$iso_root/aegishv-type1.elf
kernel_staged_boot=$iso_root/boot/aegishv-type1.elf
limine_config_source=$limine_config
limine_config_staged=$iso_root/limine.conf
limine_config_staged_boot=$iso_root/boot/limine/limine.conf
limine_available=$limine_available
xorriso_available=$xorriso_available
bootable_iso=false
qemu_evidence=false

This manifest records a staged ISO root. It is not a bootable ISO and not QEMU boot evidence.
PLAN

echo "$manifest"
