#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

out_dir="${AEGISHV_TYPE1_OUT:-target/type1}"
boot_image="${AEGISHV_TYPE1_BOOT_IMAGE:-}"
manifest="${AEGISHV_TYPE1_QEMU_MANIFEST:-$out_dir/aegishv-type1-qemu-evidence.txt}"
serial_log="${AEGISHV_QEMU_SERIAL_LOG:-$out_dir/aegishv-type1-serial.log}"
default_expected_markers="aegishv:type1:host-tables-ok,aegishv:type1:backend-vmx,aegishv:type1:vmxon-cycle-ok,aegishv:type1:vmcs-load-ok,aegishv:type1:host-paging-ok,aegishv:type1:guest-config-ok,aegishv:type1:guest-preempt-exit-ok,aegishv:type1:guest-io-exit-ok,aegishv:type1:guest-io-b-exit-ok,aegishv:type1:guest-cpuid-exit-ok,aegishv:type1:guest-rdmsr-exit-ok,aegishv:type1:guest-pat-state-ok,aegishv:type1:guest-nm-x87-exit-ok,aegishv:type1:guest-nm-simd-exit-ok,aegishv:type1:guest-ud-inject-ok,aegishv:type1:guest-hlt-exit-ok,aegishv:type1:guest-run-ok"
expected_marker_csv="${AEGISHV_TYPE1_EXPECTED_MARKERS:-${AEGISHV_TYPE1_EXPECTED_SERIAL:-$default_expected_markers}}"
expected_markers=()
marker_option_mode=""
timeout_seconds="${AEGISHV_QEMU_TIMEOUT_SECONDS:-15}"
vmx_timer_budget_limit="0x0000000001000000"

usage() {
  cat >&2 <<'USAGE'
usage: scripts/type1-qemu-evidence.sh [--image PATH] [--manifest PATH] [--serial-log PATH] [--expect-markers CSV | --expect-marker TEXT ...] [--timeout SECONDS] [--print-command]

Runs the opt-in type-1 QEMU smoke path and writes an evidence manifest with the
pre/post-run boot image SHA-256 digests, serial log path, ordered marker state,
and smoke exit code.
USAGE
}

print_command=false
fail_usage() {
  echo "type1 qemu evidence: $1" >&2
  exit 64
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --image)
      boot_image="${2:-}"
      shift 2
      ;;
    --manifest)
      manifest="${2:-}"
      shift 2
      ;;
    --serial-log)
      serial_log="${2:-}"
      shift 2
      ;;
    --expect-markers)
      if [ "$marker_option_mode" = "repeated" ]; then
        fail_usage "--expect-markers cannot be combined with --expect-marker"
      fi
      marker_option_mode="csv"
      expected_marker_csv="${2:-}"
      shift 2
      ;;
    --expect-marker)
      if [ "$marker_option_mode" = "csv" ]; then
        fail_usage "--expect-marker cannot be combined with --expect-markers"
      fi
      if [ "$marker_option_mode" != "repeated" ]; then
        expected_markers=()
        marker_option_mode="repeated"
      fi
      expected_markers+=("${2:-}")
      shift 2
      ;;
    --expect-serial)
      if [ "$marker_option_mode" = "repeated" ]; then
        fail_usage "--expect-serial cannot be combined with --expect-marker"
      fi
      marker_option_mode="csv"
      expected_marker_csv="${2:-}"
      shift 2
      ;;
    --timeout)
      timeout_seconds="${2:-}"
      shift 2
      ;;
    --print-command)
      print_command=true
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      boot_image="$1"
      shift
      ;;
  esac
done

if [ "$marker_option_mode" != "repeated" ]; then
  case "$expected_marker_csv" in
    ""|,*|*,|*,,*)
      fail_usage "expected serial marker list is empty or contains an empty item"
      ;;
  esac
  IFS=',' read -r -a expected_markers <<< "$expected_marker_csv"
fi

if [ "${#expected_markers[@]}" -eq 0 ]; then
  fail_usage "expected serial marker list is empty"
fi

for marker in "${expected_markers[@]}"; do
  if [ -z "$marker" ]; then
    fail_usage "expected serial marker is empty"
  fi
  case "$marker" in
    *$'\n'*|*$'\r'*|*,*)
      fail_usage "expected serial markers cannot contain commas or newlines"
      ;;
  esac
done

for ((marker_index = 0; marker_index < ${#expected_markers[@]}; marker_index++)); do
  for ((other_index = marker_index + 1; other_index < ${#expected_markers[@]}; other_index++)); do
    if [ "${expected_markers[$marker_index]}" = "${expected_markers[$other_index]}" ]; then
      fail_usage "expected serial marker list contains a duplicate: ${expected_markers[$marker_index]}"
    fi
  done
done

required_vmx_markers=(
  "aegishv:type1:host-tables-ok"
  "aegishv:type1:backend-vmx"
  "aegishv:type1:vmxon-cycle-ok"
  "aegishv:type1:vmcs-load-ok"
  "aegishv:type1:host-paging-ok"
  "aegishv:type1:guest-config-ok"
  "aegishv:type1:guest-preempt-exit-ok"
  "aegishv:type1:guest-io-exit-ok"
  "aegishv:type1:guest-io-b-exit-ok"
  "aegishv:type1:guest-cpuid-exit-ok"
  "aegishv:type1:guest-rdmsr-exit-ok"
  "aegishv:type1:guest-pat-state-ok"
  "aegishv:type1:guest-nm-x87-exit-ok"
  "aegishv:type1:guest-nm-simd-exit-ok"
  "aegishv:type1:guest-ud-inject-ok"
  "aegishv:type1:guest-hlt-exit-ok"
  "aegishv:type1:guest-run-ok"
)
required_marker_index=0
for marker in "${expected_markers[@]}"; do
  if [ "$marker" = "${required_vmx_markers[$required_marker_index]}" ]; then
    required_marker_index=$((required_marker_index + 1))
    if [ "$required_marker_index" -eq "${#required_vmx_markers[@]}" ]; then
      break
    fi
  fi
done
if [ "$required_marker_index" -ne "${#required_vmx_markers[@]}" ]; then
  fail_usage "expected serial marker list must include the complete host-table, VMX backend/VMXON/VMCS-load, owned-paging, guest-configuration, preemption, both I/O bitmaps, CPUID, RDMSR, PAT, x87/SIMD #NM, fixed #UD injection, HLT, and completion proof chain in order"
fi

expected_marker_csv="$(IFS=','; printf '%s' "${expected_markers[*]}")"

require_nonempty() {
  local value="$1"
  local label="$2"
  if [ -z "$value" ]; then
    echo "type1 qemu evidence: missing $label" >&2
    exit 64
  fi
}

require_nonempty "$boot_image" "boot image"
require_nonempty "$manifest" "manifest path"
require_nonempty "$serial_log" "serial log path"
require_nonempty "$timeout_seconds" "timeout seconds"

if [ ! -f "$boot_image" ]; then
  echo "type1 qemu evidence: boot image does not exist: $boot_image" >&2
  exit 66
fi
case "$boot_image" in
  *.iso) ;;
  *) fail_usage "boot image must be a Limine ISO; a raw ELF has no Limine handoff" ;;
esac

if [ "$print_command" = true ]; then
  smoke_marker_args=()
  for marker in "${expected_markers[@]}"; do
    smoke_marker_args+=(--expect-marker "$marker")
  done
  AEGISHV_QEMU_SERIAL_LOG="$serial_log" \
  AEGISHV_QEMU_TIMEOUT_SECONDS="$timeout_seconds" \
    bash scripts/type1-qemu-smoke.sh --print-command "${smoke_marker_args[@]}" "$boot_image"
  exit 0
fi

command_path() {
  command -v "$1" 2>/dev/null || true
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

sha256_file() {
  local path="$1"
  local hash_command="${AEGISHV_SHA256_COMMAND:-}"
  local digest=""

  if [ -n "$hash_command" ]; then
    if command -v "$hash_command" >/dev/null 2>&1; then
      digest="$("$hash_command" "$path" 2>/dev/null | awk 'NR == 1 { print $1; exit }' || true)"
    fi
  elif command -v sha256sum >/dev/null 2>&1; then
    digest="$(sha256sum "$path" 2>/dev/null | awk 'NR == 1 { print $1; exit }' || true)"
  elif command -v shasum >/dev/null 2>&1; then
    digest="$(shasum -a 256 "$path" 2>/dev/null | awk 'NR == 1 { print $1; exit }' || true)"
  fi

  # GNU sha256sum prefixes escaped output with a backslash when the file name
  # itself contains a backslash, as it does for Windows paths passed to bash.
  if [ "${#digest}" -eq 65 ] && [ "${digest:0:1}" = "\\" ]; then
    digest="${digest:1}"
  fi

  if [[ "$digest" =~ ^[0-9A-Fa-f]{64}$ ]]; then
    printf '%s\n' "${digest,,}"
  elif [ -n "$digest" ]; then
    printf '%s\n' "$digest"
  else
    echo unavailable
  fi
}

sha256_is_valid() {
  local digest="$1"
  [[ "$digest" =~ ^[0-9a-f]{64}$ ]]
}

qemu="${AEGISHV_QEMU:-qemu-system-x86_64}"
qemu_path="$(command_path "$qemu")"
qemu_available=false
qemu_version=unavailable
if [ -n "$qemu_path" ]; then
  qemu_available=true
  qemu_version="$(first_line_or_unavailable "$qemu_path" --version)"
fi
qemu_machine="${AEGISHV_QEMU_MACHINE:-q35,accel=kvm}"
qemu_cpu="${AEGISHV_QEMU_CPU:-host,+vmx}"
qemu_boot_mode=iso

mkdir -p "$(dirname "$manifest")"
mkdir -p "$(dirname "$serial_log")"

boot_image_sha256_before="$(sha256_file "$boot_image")"
boot_image_sha256_before_valid=false
if sha256_is_valid "$boot_image_sha256_before"; then
  boot_image_sha256_before_valid=true
fi

smoke_marker_args=()
for marker in "${expected_markers[@]}"; do
  smoke_marker_args+=(--expect-marker "$marker")
done

qemu_command=unavailable
set +e
rendered_command="$(
  AEGISHV_QEMU_SERIAL_LOG="$serial_log" \
  AEGISHV_QEMU_TIMEOUT_SECONDS="$timeout_seconds" \
    bash scripts/type1-qemu-smoke.sh --print-command "${smoke_marker_args[@]}" "$boot_image"
)"
rendered_command_status=$?
set -e
if [ "$rendered_command_status" -eq 0 ] && [ -n "$rendered_command" ]; then
  qemu_command="$rendered_command"
fi

smoke_status=0
set +e
AEGISHV_QEMU_SERIAL_LOG="$serial_log" \
AEGISHV_QEMU_TIMEOUT_SECONDS="$timeout_seconds" \
  bash scripts/type1-qemu-smoke.sh "${smoke_marker_args[@]}" "$boot_image"
smoke_status=$?
set -e

boot_image_sha256_after="$(sha256_file "$boot_image")"
boot_image_sha256_after_valid=false
if sha256_is_valid "$boot_image_sha256_after"; then
  boot_image_sha256_after_valid=true
fi
boot_image_digest_valid=false
if [ "$boot_image_sha256_before_valid" = true ] \
  && [ "$boot_image_sha256_after_valid" = true ]; then
  boot_image_digest_valid=true
fi
boot_image_digest_match=false
if [ "$boot_image_digest_valid" = true ] \
  && [ "$boot_image_sha256_before" = "$boot_image_sha256_after" ]; then
  boot_image_digest_match=true
fi
# Keep the original field as the post-run digest for existing manifest consumers.
boot_image_sha256="$boot_image_sha256_after"

serial_log_present=false
serial_markers_present=false
serial_markers_in_order=false
serial_markers_exactly_once=false
forbidden_backend_none_observed=false
forbidden_marker_observed=false
forbidden_marker=""
marker_observed=()
vmx_cpu_signature_valid=false
vmx_cpu_signature=""
vmx_timer_rate_valid=false
vmx_timer_rate=""
vmx_timer_reload_valid=false
vmx_timer_reload=""
vmx_timer_effective_valid=false
vmx_timer_effective=""
vmx_timer_semantics_valid=false
vmx_diagnostics_valid=false

serial_has_marker() {
  local log_path="$1"
  local expected="$2"
  local line
  while IFS= read -r line || [ -n "$line" ]; do
    line="${line%$'\r'}"
    if [ "$line" = "$expected" ]; then
      return 0
    fi
  done < "$log_path"
  return 1
}

serial_marker_count() {
  local log_path="$1"
  local expected="$2"
  local line
  local count=0
  while IFS= read -r line || [ -n "$line" ]; do
    line="${line%$'\r'}"
    if [ "$line" = "$expected" ]; then
      count=$((count + 1))
    fi
  done < "$log_path"
  printf '%d\n' "$count"
}

capture_serial_hex_value() {
  local log_path="$1"
  local prefix="$2"
  local width="$3"
  local regex="^0x[0-9a-f]{${width}}$"
  local line
  local candidate
  local prefix_count=0
  local valid_count=0

  captured_hex_valid=false
  captured_hex_value=""
  while IFS= read -r line || [ -n "$line" ]; do
    line="${line%$'\r'}"
    case "$line" in
      "$prefix"*)
        prefix_count=$((prefix_count + 1))
        candidate="${line#"$prefix"}"
        if [[ "$candidate" =~ $regex ]]; then
          valid_count=$((valid_count + 1))
          captured_hex_value="$candidate"
        fi
        ;;
    esac
  done < "$log_path"

  if [ "$prefix_count" -eq 1 ] && [ "$valid_count" -eq 1 ]; then
    captured_hex_valid=true
  else
    captured_hex_value=""
  fi
}

validate_vmx_timer_semantics() {
  local LC_ALL=C
  local rate_digits
  local reload_digits
  local effective_digits
  local rate_value
  local reload_value
  local effective_value
  local expected_effective
  local max_reload
  local budget_digits="${vmx_timer_budget_limit#0x}"
  local hard_budget

  vmx_timer_semantics_valid=false
  if [ "$vmx_timer_rate_valid" != true ] \
    || [ "$vmx_timer_reload_valid" != true ] \
    || [ "$vmx_timer_effective_valid" != true ]; then
    return
  fi

  rate_digits="${vmx_timer_rate#0x}"
  reload_digits="${vmx_timer_reload#0x}"
  effective_digits="${vmx_timer_effective#0x}"
  rate_value=$((16#$rate_digits))
  reload_value=$((16#$reload_digits))
  if [ "$rate_value" -gt 31 ] || [ "$reload_value" -lt 2 ]; then
    return
  fi
  if [[ "$effective_digits" > "$budget_digits" ]]; then
    return
  fi

  hard_budget=$((16#$budget_digits))
  max_reload=$((hard_budget >> rate_value))
  if [ "$reload_value" -gt "$max_reload" ]; then
    return
  fi
  expected_effective=$((reload_value << rate_value))
  effective_value=$((16#$effective_digits))
  if [ "$expected_effective" -ne "$effective_value" ]; then
    return
  fi

  vmx_timer_semantics_valid=true
}

if [ -f "$serial_log" ]; then
  serial_log_present=true

  capture_serial_hex_value "$serial_log" "aegishv:type1:vmx-cpu-signature=" 8
  vmx_cpu_signature_valid="$captured_hex_valid"
  vmx_cpu_signature="$captured_hex_value"
  capture_serial_hex_value "$serial_log" "aegishv:type1:vmx-timer-rate=" 8
  vmx_timer_rate_valid="$captured_hex_valid"
  vmx_timer_rate="$captured_hex_value"
  capture_serial_hex_value "$serial_log" "aegishv:type1:vmx-timer-reload=" 8
  vmx_timer_reload_valid="$captured_hex_valid"
  vmx_timer_reload="$captured_hex_value"
  capture_serial_hex_value "$serial_log" "aegishv:type1:vmx-timer-effective=" 16
  vmx_timer_effective_valid="$captured_hex_valid"
  vmx_timer_effective="$captured_hex_value"
  validate_vmx_timer_semantics
  if [ "$vmx_cpu_signature_valid" = true ] \
    && [ "$vmx_timer_rate_valid" = true ] \
    && [ "$vmx_timer_reload_valid" = true ] \
    && [ "$vmx_timer_effective_valid" = true ] \
    && [ "$vmx_timer_semantics_valid" = true ]; then
    vmx_diagnostics_valid=true
  fi

  all_markers_present=true
  all_markers_exactly_once=true
  for marker in "${expected_markers[@]}"; do
    marker_count="$(serial_marker_count "$serial_log" "$marker")"
    if [ "$marker_count" -gt 0 ]; then
      marker_observed+=(true)
    else
      marker_observed+=(false)
      all_markers_present=false
    fi
    if [ "$marker_count" -ne 1 ]; then
      all_markers_exactly_once=false
    fi
  done
  serial_markers_present="$all_markers_present"
  serial_markers_exactly_once="$all_markers_exactly_once"

  next_marker=0
  while IFS= read -r line || [ -n "$line" ]; do
    line="${line%$'\r'}"
    if [ "$line" = "${expected_markers[$next_marker]}" ]; then
      next_marker=$((next_marker + 1))
      if [ "$next_marker" -eq "${#expected_markers[@]}" ]; then
        serial_markers_in_order=true
        break
      fi
    fi
  done < "$serial_log"

  forbidden_markers=(
    "aegishv:type1:backend-none"
    "aegishv:type1:backend-svm"
    "aegishv:type1:runtime-plan-error"
    "aegishv:type1:runtime-preflight-error"
    "aegishv:type1:runtime-enable-error"
    "aegishv:type1:runtime-regions-error"
    "aegishv:type1:vmxon-cycle-error"
    "aegishv:type1:vmxon-cycle-skipped"
    "aegishv:type1:vmcs-load-error"
    "aegishv:type1:vmcs-load-skipped"
    "aegishv:type1:limine-missing"
    "aegishv:type1:host-tables-error"
    "aegishv:type1:host-paging-error"
    "aegishv:type1:host-exception"
    "aegishv:type1:host-fatal"
    "aegishv:type1:guest-timeout"
    "aegishv:type1:guest-entry-error"
    "aegishv:type1:guest-exit-error"
    "aegishv:type1:guest-resume-error"
    "aegishv:type1:guest-pat-state-error"
    "aegishv:type1:guest-nm-x87-exit-error"
    "aegishv:type1:guest-nm-simd-exit-error"
    "aegishv:type1:guest-ud-inject-error"
    "aegishv:type1:panic"
  )
  for candidate in "${forbidden_markers[@]}"; do
    if serial_has_marker "$serial_log" "$candidate"; then
      forbidden_marker_observed=true
      if [ -z "$forbidden_marker" ]; then
        forbidden_marker="$candidate"
      fi
      if [ "$candidate" = "aegishv:type1:backend-none" ]; then
        forbidden_backend_none_observed=true
      fi
    fi
  done
else
  for _marker in "${expected_markers[@]}"; do
    marker_observed+=(false)
  done
fi

qemu_evidence=false
if [ "$smoke_status" -eq 0 ] \
  && [ "$boot_image_digest_valid" = true ] \
  && [ "$boot_image_digest_match" = true ] \
  && [ "$serial_markers_present" = true ] \
  && [ "$serial_markers_in_order" = true ] \
  && [ "$serial_markers_exactly_once" = true ] \
  && [ "$vmx_diagnostics_valid" = true ] \
  && [ "$forbidden_marker_observed" = false ]; then
  qemu_evidence=true
fi

evidence_status="$smoke_status"
if [ "$evidence_status" -eq 0 ] \
  && { [ "$boot_image_digest_valid" != true ] \
    || [ "$boot_image_digest_match" != true ] \
    || [ "$vmx_diagnostics_valid" != true ]; }; then
  evidence_status=70
fi

{
  cat <<PLAN
aegishv type-1 QEMU smoke evidence

boot_image=$boot_image
boot_image_sha256=$boot_image_sha256
boot_image_sha256_before=$boot_image_sha256_before
boot_image_sha256_after=$boot_image_sha256_after
boot_image_sha256_before_valid=$boot_image_sha256_before_valid
boot_image_sha256_after_valid=$boot_image_sha256_after_valid
boot_image_digest_valid=$boot_image_digest_valid
boot_image_digest_match=$boot_image_digest_match
qemu_available=$qemu_available
qemu_path=$qemu_path
qemu_version=$qemu_version
qemu_machine=$qemu_machine
qemu_cpu=$qemu_cpu
qemu_boot_mode=$qemu_boot_mode
qemu_command=$qemu_command
serial_log=$serial_log
serial_log_present=$serial_log_present
expected_serial=$expected_marker_csv
expected_serial_marker_count=${#expected_markers[@]}
expected_serial_markers=$expected_marker_csv
serial_marker_observed=$serial_markers_in_order
serial_markers_present=$serial_markers_present
serial_markers_in_order=$serial_markers_in_order
serial_markers_exactly_once=$serial_markers_exactly_once
vmx_cpu_signature_valid=$vmx_cpu_signature_valid
vmx_cpu_signature=$vmx_cpu_signature
vmx_timer_rate_valid=$vmx_timer_rate_valid
vmx_timer_rate=$vmx_timer_rate
vmx_timer_reload_valid=$vmx_timer_reload_valid
vmx_timer_reload=$vmx_timer_reload
vmx_timer_effective_valid=$vmx_timer_effective_valid
vmx_timer_effective=$vmx_timer_effective
vmx_timer_semantics_valid=$vmx_timer_semantics_valid
vmx_timer_budget_limit=$vmx_timer_budget_limit
vmx_diagnostics_valid=$vmx_diagnostics_valid
forbidden_backend_none_observed=$forbidden_backend_none_observed
forbidden_marker_observed=$forbidden_marker_observed
forbidden_marker=$forbidden_marker
PLAN
  for index in "${!expected_markers[@]}"; do
    manifest_index=$((index + 1))
    echo "expected_serial_marker_$manifest_index=${expected_markers[$index]}"
    echo "serial_marker_${manifest_index}_observed=${marker_observed[$index]}"
  done
  cat <<PLAN
timeout_seconds=$timeout_seconds
qemu_smoke_exit_status=$smoke_status
qemu_evidence_exit_status=$evidence_status
qemu_evidence=$qemu_evidence

This manifest records a local QEMU smoke attempt. A true qemu_evidence value requires a valid SHA-256 digest of the boot image before and after the run with both digests equal, every expected marker exactly once and in order, exactly one well-formed CPU/timer diagnostic set whose timer values are internally consistent and within the fixed budget, and no contradictory backend, failure, skipped, or panic markers. With the default contract it proves only the fixed toy guest's VMLAUNCH, forced preemption, trapped port-I/O, CPUID and selected MSR behavior, fixed PAT round trip, exact x87/SIMD #NM probes, one fixed vector-6 hardware exception injected at VM entry through the immutable CPL0 IDT gate, IRETQ to the fixed HLT, VMRESUME, and VMXOFF sequence on the recorded host. It is not evidence of general exceptions, error-code injection, reinjection, IST or privilege transitions, external interrupts, APIC, SMP, guest-OS support, a general runtime, or production readiness.
PLAN
} > "$manifest"

echo "$manifest"
exit "$evidence_status"
