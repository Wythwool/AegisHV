#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

out_dir="${AEGISHV_TYPE1_OUT:-target/type1}"
manifest="${AEGISHV_TYPE1_TOOL_MANIFEST:-$out_dir/aegishv-type1-lab-tools.txt}"
limine_dir="${AEGISHV_LIMINE_DIR:-}"
require_all=false

usage() {
  cat >&2 <<'USAGE'
usage: scripts/check-type1-lab-tools.sh [--require-all] [--manifest PATH]

Checks the local tools needed for the opt-in type-1 ISO and QEMU lab path.
Without --require-all it writes a manifest and exits successfully.
USAGE
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --require-all)
      require_all=true
      shift
      ;;
    --manifest)
      manifest="${2:-}"
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

if [ -z "$manifest" ]; then
  echo "type1 lab tools: manifest path is empty" >&2
  exit 64
fi

command_path() {
  command -v "$1" 2>/dev/null || true
}

bool_for_path() {
  if [ -n "$1" ]; then
    echo true
  else
    echo false
  fi
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

rustup_path="$(command_path rustup)"
llvm_objdump_path="$(command_path llvm-objdump)"
qemu_path="$(command_path "${AEGISHV_QEMU:-qemu-system-x86_64}")"
requested_timeout="${AEGISHV_TIMEOUT:-}"
timeout_command=""
timeout_path=""
if [ -n "$requested_timeout" ]; then
  candidate_path="$(command_path "$requested_timeout")"
  if [ -n "$candidate_path" ] && "$requested_timeout" --help >/dev/null 2>&1; then
    timeout_command="$requested_timeout"
    timeout_path="$candidate_path"
  fi
else
  for candidate in timeout /usr/bin/timeout gtimeout; do
    candidate_path="$(command_path "$candidate")"
    if [ -n "$candidate_path" ] && "$candidate" --help >/dev/null 2>&1; then
      timeout_command="$candidate"
      timeout_path="$candidate_path"
      break
    fi
  done
fi
xorriso_path="$(command_path xorriso)"
limine_path="$(command_path limine)"

rust_target_installed=false
if [ -n "$rustup_path" ] && rustup target list --installed | grep -Fxq x86_64-unknown-none; then
  rust_target_installed=true
fi

qemu_available="$(bool_for_path "$qemu_path")"
llvm_objdump_available="$(bool_for_path "$llvm_objdump_path")"
timeout_compatible="$(bool_for_path "$timeout_path")"
xorriso_available="$(bool_for_path "$xorriso_path")"
limine_command_available="$(bool_for_path "$limine_path")"

limine_dir_set=false
limine_dir_files_present=false
if [ -n "$limine_dir" ]; then
  limine_dir_set=true
  if [ -f "$limine_dir/limine-bios.sys" ] \
    && [ -f "$limine_dir/limine-bios-cd.bin" ] \
    && [ -f "$limine_dir/limine-uefi-cd.bin" ]; then
    limine_dir_files_present=true
  fi
fi

qemu_version=unavailable
if [ -n "$qemu_path" ]; then
  qemu_version="$(first_line_or_unavailable "$qemu_path" --version)"
fi

missing=""
add_missing() {
  if [ -n "$missing" ]; then
    missing="$missing,$1"
  else
    missing="$1"
  fi
}

[ "$rust_target_installed" = true ] || add_missing rust_target_x86_64_unknown_none
[ "$llvm_objdump_available" = true ] || add_missing llvm_objdump
[ "$qemu_available" = true ] || add_missing qemu_system_x86_64
[ "$timeout_compatible" = true ] || add_missing timeout_command
[ "$xorriso_available" = true ] || add_missing xorriso
[ "$limine_command_available" = true ] || add_missing limine_command
[ "$limine_dir_set" = true ] || add_missing AEGISHV_LIMINE_DIR
[ "$limine_dir_files_present" = true ] || add_missing limine_iso_files

lab_ready=false
if [ -z "$missing" ]; then
  lab_ready=true
else
  missing="[$missing]"
fi

mkdir -p "$(dirname "$manifest")"
cat > "$manifest" <<PLAN
aegishv type-1 lab tools

rustup_present=$(bool_for_path "$rustup_path")
rust_target_x86_64_unknown_none=$rust_target_installed
llvm_objdump_available=$llvm_objdump_available
llvm_objdump_path=$llvm_objdump_path
qemu_available=$qemu_available
qemu_path=$qemu_path
qemu_version=$qemu_version
timeout_command=$timeout_command
timeout_path=$timeout_path
timeout_compatible=$timeout_compatible
xorriso_available=$xorriso_available
xorriso_path=$xorriso_path
limine_command_available=$limine_command_available
limine_path=$limine_path
limine_dir_set=$limine_dir_set
limine_dir=$limine_dir
limine_dir_files_present=$limine_dir_files_present
lab_ready=$lab_ready
missing=$missing

This manifest records local tool availability. It does not build an ISO or run QEMU.
PLAN

echo "$manifest"

if [ "$require_all" = true ] && [ "$lab_ready" != true ]; then
  echo "type1 lab tools: missing required lab tools: $missing" >&2
  exit 69
fi
