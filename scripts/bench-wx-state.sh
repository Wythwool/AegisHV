#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

iterations="${AEGISHV_BENCH_ITERATIONS:-3}"
replay="${AEGISHV_BENCH_WX_REPLAY:-./corpus/malicious/wx_same_vm_same_as.log}"
config="${AEGISHV_BENCH_CONFIG:-./config.example.toml}"
out_dir="${AEGISHV_BENCH_OUT:-${TMPDIR:-/tmp}/aegishv-bench-wx-state}"

if [[ ! "$iterations" =~ ^[1-9][0-9]*$ ]]; then
  echo "AEGISHV_BENCH_ITERATIONS must be a positive integer" >&2
  exit 2
fi

if [[ ! -f "$replay" ]]; then
  echo "W^X replay fixture does not exist: $replay" >&2
  exit 66
fi

mkdir -p "$out_dir"
summary="$out_dir/wx-state.csv"
echo "case,iteration,elapsed_ms,wx_events" > "$summary"

for i in $(seq 1 "$iterations"); do
  jsonl="$out_dir/wx-state-$i.jsonl"
  rm -f "$jsonl"
  start_ns="$(date +%s%N)"
  cargo run --locked -- run --replay "$replay" --config "$config" --listen '' --jsonl "$jsonl" --quiet >/dev/null
  end_ns="$(date +%s%N)"
  python3 scripts/validate-jsonl-schema.py --schema schema/event.schema.json --jsonl "$jsonl" >/dev/null
  wx_events="$(grep -c '"category":"wx"' "$jsonl" || true)"
  elapsed_ms="$(((end_ns - start_ns) / 1000000))"
  echo "wx_state,$i,$elapsed_ms,$wx_events" >> "$summary"
done

echo "$summary"
