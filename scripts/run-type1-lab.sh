#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

out_dir="${AEGISHV_TYPE1_OUT:-target/type1}"
iso_image="${AEGISHV_TYPE1_ISO_IMAGE:-$out_dir/aegishv-type1.iso}"
summary="${AEGISHV_TYPE1_LAB_SUMMARY:-$out_dir/aegishv-type1-lab-summary.txt}"
evidence_manifest="${AEGISHV_TYPE1_QEMU_MANIFEST:-$out_dir/aegishv-type1-qemu-evidence.txt}"
timeout_seconds="${AEGISHV_QEMU_TIMEOUT_SECONDS:-20}"
build_kernel=true

usage() {
  cat >&2 <<'USAGE'
usage: scripts/run-type1-lab.sh [--skip-build-kernel] [--timeout SECONDS] [--summary PATH]

Runs the opt-in type-1 lab chain:
  tool check -> Limine ISO build -> QEMU evidence capture

Set AEGISHV_RUN_TYPE1_LAB=1 to allow the run.
USAGE
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --skip-build-kernel)
      build_kernel=false
      shift
      ;;
    --timeout)
      timeout_seconds="${2:-}"
      shift 2
      ;;
    --summary)
      summary="${2:-}"
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

if [ "${AEGISHV_RUN_TYPE1_LAB:-}" != "1" ]; then
  echo "type1 lab: set AEGISHV_RUN_TYPE1_LAB=1 to run the ISO/QEMU lab chain" >&2
  exit 64
fi

if [ -z "$timeout_seconds" ]; then
  echo "type1 lab: timeout seconds is empty" >&2
  exit 64
fi

if [ -z "$summary" ]; then
  echo "type1 lab: summary path is empty" >&2
  exit 64
fi

bash scripts/check-type1-lab-tools.sh --require-all

if [ "$build_kernel" = true ]; then
  bash scripts/build-type1-limine-iso.sh --build-kernel
else
  bash scripts/build-type1-limine-iso.sh
fi

qemu_status=0
set +e
bash scripts/type1-qemu-evidence.sh --image "$iso_image" --timeout "$timeout_seconds"
qemu_status=$?
set -e

qemu_evidence=unknown
if [ -f "$evidence_manifest" ]; then
  qemu_evidence="$(awk -F= '$1 == "qemu_evidence" {print $2; exit}' "$evidence_manifest")"
  if [ -z "$qemu_evidence" ]; then
    qemu_evidence=unknown
  fi
fi

lab_complete=false
if [ "$qemu_status" -eq 0 ] && [ "$qemu_evidence" = true ]; then
  lab_complete=true
fi

mkdir -p "$(dirname "$summary")"
cat > "$summary" <<PLAN
aegishv type-1 lab summary

tool_manifest=${AEGISHV_TYPE1_TOOL_MANIFEST:-$out_dir/aegishv-type1-lab-tools.txt}
iso_image=$iso_image
qemu_evidence_manifest=$evidence_manifest
qemu_exit_status=$qemu_status
qemu_evidence=$qemu_evidence
lab_complete=$lab_complete

This summary records one local opt-in lab chain. It is only successful when QEMU evidence is true.
PLAN

echo "$summary"
exit "$qemu_status"
