#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

out_dir="${AEGISHV_TYPE1_OUT:-target/type1}"
iso_image="${AEGISHV_TYPE1_ISO_IMAGE:-$out_dir/aegishv-type1.iso}"
summary="${AEGISHV_TYPE1_LAB_SUMMARY:-$out_dir/aegishv-type1-lab-summary.txt}"
evidence_manifest="${AEGISHV_TYPE1_QEMU_MANIFEST:-$out_dir/aegishv-type1-qemu-evidence.txt}"
timeout_seconds="${AEGISHV_QEMU_TIMEOUT_SECONDS:-20}"
build_kernel=true
evidence_marker_args=()
marker_option_mode=""

usage() {
  cat >&2 <<'USAGE'
usage: scripts/run-type1-lab.sh [--skip-build-kernel] [--timeout SECONDS] [--summary PATH] [--expect-markers CSV | --expect-marker TEXT ...]

Runs the opt-in type-1 lab chain:
  tool check -> Limine ISO build -> QEMU evidence capture

Set AEGISHV_RUN_TYPE1_LAB=1 to allow the run.
USAGE
}

fail_usage() {
  echo "type1 lab: $1" >&2
  exit 64
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
    --expect-markers)
      if [ "$marker_option_mode" = "repeated" ]; then
        fail_usage "--expect-markers cannot be combined with --expect-marker"
      fi
      marker_option_mode="csv"
      evidence_marker_args=(--expect-markers "${2:-}")
      shift 2
      ;;
    --expect-marker)
      if [ "$marker_option_mode" = "csv" ]; then
        fail_usage "--expect-marker cannot be combined with --expect-markers"
      fi
      if [ "$marker_option_mode" != "repeated" ]; then
        evidence_marker_args=()
        marker_option_mode="repeated"
      fi
      evidence_marker_args+=(--expect-marker "${2:-}")
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
bash scripts/type1-qemu-evidence.sh --image "$iso_image" \
  --timeout "$timeout_seconds" \
  "${evidence_marker_args[@]}"
qemu_status=$?
set -e

qemu_evidence=unknown
expected_serial_markers=unknown
serial_markers_in_order=unknown
serial_markers_exactly_once=unknown
forbidden_backend_none_observed=unknown
forbidden_marker_observed=unknown
forbidden_marker=unknown
qemu_machine=unknown
qemu_cpu=unknown
qemu_boot_mode=unknown
qemu_command=unknown
if [ -f "$evidence_manifest" ]; then
  manifest_value() {
    awk -F= -v key="$1" '$1 == key {sub(/^[^=]*=/, ""); print; exit}' "$evidence_manifest"
  }

  qemu_evidence="$(manifest_value qemu_evidence)"
  if [ -z "$qemu_evidence" ]; then
    qemu_evidence=unknown
  fi
  expected_serial_markers="$(manifest_value expected_serial_markers)"
  if [ -z "$expected_serial_markers" ]; then
    expected_serial_markers=unknown
  fi
  serial_markers_in_order="$(manifest_value serial_markers_in_order)"
  if [ -z "$serial_markers_in_order" ]; then
    serial_markers_in_order=unknown
  fi
  serial_markers_exactly_once="$(manifest_value serial_markers_exactly_once)"
  if [ -z "$serial_markers_exactly_once" ]; then
    serial_markers_exactly_once=unknown
  fi
  forbidden_backend_none_observed="$(manifest_value forbidden_backend_none_observed)"
  if [ -z "$forbidden_backend_none_observed" ]; then
    forbidden_backend_none_observed=unknown
  fi
  forbidden_marker_observed="$(manifest_value forbidden_marker_observed)"
  if [ -z "$forbidden_marker_observed" ]; then
    forbidden_marker_observed=unknown
  fi
  forbidden_marker="$(manifest_value forbidden_marker)"
  if [ -z "$forbidden_marker" ]; then
    forbidden_marker=none
  fi
  qemu_machine="$(manifest_value qemu_machine)"
  qemu_cpu="$(manifest_value qemu_cpu)"
  qemu_boot_mode="$(manifest_value qemu_boot_mode)"
  qemu_command="$(manifest_value qemu_command)"
fi

lab_complete=false
if [ "$qemu_status" -eq 0 ] \
  && [ "$qemu_evidence" = true ] \
  && [ "$serial_markers_in_order" = true ] \
  && [ "$serial_markers_exactly_once" = true ] \
  && [ "$forbidden_marker_observed" = false ]; then
  lab_complete=true
fi

mkdir -p "$(dirname "$summary")"
cat > "$summary" <<PLAN
aegishv type-1 lab summary

tool_manifest=${AEGISHV_TYPE1_TOOL_MANIFEST:-$out_dir/aegishv-type1-lab-tools.txt}
iso_image=$iso_image
qemu_evidence_manifest=$evidence_manifest
qemu_exit_status=$qemu_status
expected_serial_markers=$expected_serial_markers
serial_markers_in_order=$serial_markers_in_order
serial_markers_exactly_once=$serial_markers_exactly_once
forbidden_backend_none_observed=$forbidden_backend_none_observed
forbidden_marker_observed=$forbidden_marker_observed
forbidden_marker=$forbidden_marker
qemu_machine=$qemu_machine
qemu_cpu=$qemu_cpu
qemu_boot_mode=$qemu_boot_mode
qemu_command=$qemu_command
qemu_evidence=$qemu_evidence
lab_complete=$lab_complete

This summary records one local opt-in lab chain. It is only successful when the full fixed-guest marker sequence appears exactly once and in order and no contradictory marker is present. Success proves only the bounded VMX timer, bitmap, PAT and fixed #NM probes, plus one fixed vector-6 VM-entry injection through the immutable CPL0 IDT gate and an integer-only handler's IRETQ to the fixed HLT, in the recorded VMLAUNCH/VMRESUME/VMXOFF sequence. It is not evidence of general exceptions, error-code injection, reinjection, IST or privilege transitions, external interrupts, APIC, SMP, guest-OS support, a general runtime, or production readiness.
PLAN

echo "$summary"
exit "$qemu_status"
