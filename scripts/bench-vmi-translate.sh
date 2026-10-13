#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

iterations="${AEGISHV_BENCH_ITERATIONS:-100}"
fixture="${AEGISHV_BENCH_VMI_FIXTURE:-./tests/fixtures/vmi/x86_64_basic.vmi}"
gva="${AEGISHV_BENCH_VMI_GVA:-0x0}"
mode="${AEGISHV_BENCH_VMI_MODE:-x86_64-4level}"
if [[ -n "${AEGISHV_BENCH_OUT:-}" ]]; then
  out_dir="$AEGISHV_BENCH_OUT"
else
  out_dir="target/tmp/aegishv-bench-vmi-translate"
fi

if [[ ! "$iterations" =~ ^[1-9][0-9]*$ ]]; then
  echo "AEGISHV_BENCH_ITERATIONS must be a positive integer" >&2
  exit 2
fi

if [[ ! -f "$fixture" ]]; then
  echo "VMI fixture does not exist: $fixture" >&2
  exit 66
fi

mkdir -p "$out_dir"
summary="$out_dir/vmi-translate.csv"
echo "case,iterations,elapsed_ms,fixture,mode" > "$summary"

start_ns="$(date +%s%N)"
for _ in $(seq 1 "$iterations"); do
  cargo run --locked -- vmi translate --fixture "$fixture" --gva "$gva" --mode "$mode" --json >/dev/null
done
end_ns="$(date +%s%N)"
elapsed_ms="$(((end_ns - start_ns) / 1000000))"
echo "vmi_translate,$iterations,$elapsed_ms,$fixture,$mode" >> "$summary"

echo "$summary"
