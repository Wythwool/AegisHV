#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if [[ "${AEGISHV_RUN_LIVE_KVM:-}" != "1" ]]; then
  echo "set AEGISHV_RUN_LIVE_KVM=1 to run host-dependent live KVM checks" >&2
  exit 77
fi

if [[ -n "${AEGISHV_LIVE_KVM_LOG_DIR:-}" ]]; then
  log_dir="$AEGISHV_LIVE_KVM_LOG_DIR"
else
  log_dir="target/tmp/aegishv-live-kvm"
fi
timeout_s="${AEGISHV_LIVE_KVM_TIMEOUT:-30}"

mkdir -p "$log_dir"

if [[ ! -e /dev/kvm ]]; then
  echo "/dev/kvm is required for this live integration check" >&2
  exit 78
fi

cargo metadata --locked --format-version 1 > "$log_dir/cargo-metadata.json"
cargo run --locked -- validate-config --config config.example.toml > "$log_dir/validate-config.log"
scripts/live-tracefs-smoke.sh --timeout "$timeout_s" > "$log_dir/live-tracefs-smoke.log"

echo "$log_dir"
