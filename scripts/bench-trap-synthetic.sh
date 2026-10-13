#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

iterations="${AEGISHV_BENCH_ITERATIONS:-10000}"
if [[ -n "${AEGISHV_BENCH_OUT:-}" ]]; then
  out_dir="$AEGISHV_BENCH_OUT"
else
  out_dir="target/tmp/aegishv-bench-trap-synthetic"
fi

if [[ ! "$iterations" =~ ^[1-9][0-9]*$ ]]; then
  echo "AEGISHV_BENCH_ITERATIONS must be a positive integer" >&2
  exit 2
fi

mkdir -p "$out_dir"
log="$out_dir/trap-synthetic.log"
cargo run --locked --bin trap_synthetic_bench -- --iterations "$iterations" | tee "$log"
echo "$log"
