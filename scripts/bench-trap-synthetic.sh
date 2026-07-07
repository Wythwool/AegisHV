#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

iterations="${AEGISHV_BENCH_ITERATIONS:-10000}"
out_dir="${AEGISHV_BENCH_OUT:-${TMPDIR:-/tmp}/aegishv-bench-trap-synthetic}"

if [[ ! "$iterations" =~ ^[1-9][0-9]*$ ]]; then
  echo "AEGISHV_BENCH_ITERATIONS must be a positive integer" >&2
  exit 2
fi

mkdir -p "$out_dir"
log="$out_dir/trap-synthetic.log"
cargo run --locked --bin trap_synthetic_bench -- --iterations "$iterations" | tee "$log"
echo "$log"
