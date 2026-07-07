#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/live-tracefs-smoke.sh [--timeout SECONDS] [--tracefs PATH]

Checks whether this host can provide live KVM kvm_exit data through tracefs.
The script is Linux-only and opt-in. It restores kvm_exit/enable and tracing_on
before it exits.

Environment:
  AEGISHV_TRACEFS                 Override tracefs path.
  AEGISHV_LIVE_TRACEFS_TIMEOUT   Seconds to wait for a live kvm_exit event.
USAGE
}

fail() {
  echo "live tracefs smoke: $*" >&2
  exit 1
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command '$1'"
  fi
}

timeout_s="${AEGISHV_LIVE_TRACEFS_TIMEOUT:-15}"
tracefs="${AEGISHV_TRACEFS:-}"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --timeout)
      [ "$#" -ge 2 ] || fail "--timeout requires a value"
      timeout_s="$2"
      shift 2
      ;;
    --tracefs)
      [ "$#" -ge 2 ] || fail "--tracefs requires a path"
      tracefs="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      fail "unsupported argument: $1"
      ;;
  esac
done

case "$timeout_s" in
  ''|*[!0-9]*)
    fail "--timeout must be a positive integer"
    ;;
esac
[ "$timeout_s" -gt 0 ] || fail "--timeout must be greater than zero"

[ "$(uname -s)" = "Linux" ] || fail "requires Linux tracefs; this host reports $(uname -s)"

require_cmd timeout
require_cmd mktemp
require_cmd grep

if [ -z "$tracefs" ]; then
  for candidate in /sys/kernel/tracing /sys/kernel/debug/tracing; do
    if [ -d "$candidate/events/kvm/kvm_exit" ]; then
      tracefs="$candidate"
      break
    fi
  done
fi

[ -n "$tracefs" ] || fail "KVM tracefs events are not mounted; mount tracefs and run on a KVM-capable Linux host"
[ -d "$tracefs" ] || fail "tracefs path does not exist: $tracefs"

event_dir="$tracefs/events/kvm/kvm_exit"
enable_file="$event_dir/enable"
format_file="$event_dir/format"
trace_pipe="$tracefs/trace_pipe"
trace_marker="$tracefs/trace_marker"
tracing_on="$tracefs/tracing_on"

[ -d "$event_dir" ] || fail "missing KVM kvm_exit tracepoint at $event_dir"
[ -r "$format_file" ] || fail "cannot read $format_file; check tracefs permissions"
[ -r "$trace_pipe" ] || fail "cannot read $trace_pipe; check tracefs permissions"
[ -w "$trace_marker" ] || fail "cannot write $trace_marker; run with permissions that can write trace markers"
[ -r "$enable_file" ] && [ -w "$enable_file" ] || fail "cannot read/write $enable_file; run with permissions that can enable kvm_exit"
[ -r "$tracing_on" ] && [ -w "$tracing_on" ] || fail "cannot read/write $tracing_on; run with permissions that can control tracing"

grep -q '^name: kvm_exit$' "$format_file" || fail "$format_file is not kvm_exit metadata"

original_enable="$(cat "$enable_file")"
original_tracing_on="$(cat "$tracing_on")"
sample_file="$(mktemp "${TMPDIR:-/tmp}/aegishv-live-tracefs.XXXXXX")"
restore_ready=1

restore_tracefs_state() {
  status=$?
  if [ "${restore_ready:-0}" -eq 1 ]; then
    printf '%s\n' "$original_enable" >"$enable_file" 2>/dev/null || \
      echo "live tracefs smoke: warning: could not restore $enable_file" >&2
    printf '%s\n' "$original_tracing_on" >"$tracing_on" 2>/dev/null || \
      echo "live tracefs smoke: warning: could not restore $tracing_on" >&2
  fi
  rm -f "$sample_file"
  exit "$status"
}
trap restore_tracefs_state EXIT INT TERM

printf '1\n' >"$tracing_on" || fail "could not enable tracefs tracing"
printf '1\n' >"$enable_file" || fail "could not enable kvm_exit tracepoint"

marker="aegishv-live-tracefs-smoke-$$-${RANDOM}"

if ! timeout "$timeout_s" bash -c '
  marker="$1"
  sample_file="$2"
  trace_pipe="$3"
  trace_marker="$4"
  seen_marker=0

  ( sleep 0.1; printf "%s\n" "$marker" >"$trace_marker" ) &
  marker_writer=$!

  while IFS= read -r line; do
    case "$line" in
      *"$marker"*)
        seen_marker=1
        ;;
      *kvm_exit*)
        if [ "$seen_marker" -eq 1 ]; then
          printf "%s\n" "$line" >"$sample_file"
          wait "$marker_writer" 2>/dev/null || true
          exit 0
        fi
        ;;
    esac
  done <"$trace_pipe"

  wait "$marker_writer" 2>/dev/null || true
  exit 1
' bash "$marker" "$sample_file" "$trace_pipe" "$trace_marker"; then
  fail "no live kvm_exit tracefs data observed within ${timeout_s}s after the marker; start or exercise a KVM guest and rerun"
fi

sample="$(cat "$sample_file")"
[ -n "$sample" ] || fail "tracefs returned an empty kvm_exit sample"

echo "live tracefs smoke: passed; observed kvm_exit data after trace marker"
