#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

iterations="${AEGISHV_BENCH_ITERATIONS:-3}"
replay="${AEGISHV_BENCH_REPLAY:-./examples/traces/kvm_exit_sample.log}"
out_dir="${AEGISHV_BENCH_OUT:-${TMPDIR:-/tmp}/aegishv-bench-trace-ingest}"

if [[ ! "$iterations" =~ ^[1-9][0-9]*$ ]]; then
  echo "AEGISHV_BENCH_ITERATIONS must be a positive integer" >&2
  exit 2
fi

if [[ ! -f "$replay" ]]; then
  echo "replay fixture does not exist: $replay" >&2
  exit 66
fi

mkdir -p "$out_dir"
summary="$out_dir/trace-ingest.csv"
echo "case,iteration,elapsed_ms,events" > "$summary"

for i in $(seq 1 "$iterations"); do
  jsonl="$out_dir/trace-ingest-$i.jsonl"
  rm -f "$jsonl"
  start_ns="$(date +%s%N)"
  cargo run --locked -- run --replay "$replay" --listen '' --jsonl "$jsonl" --quiet >/dev/null
  end_ns="$(date +%s%N)"
  python3 scripts/validate-jsonl-schema.py --schema schema/event.schema.json --jsonl "$jsonl" >/dev/null
  events="$(wc -l < "$jsonl" | tr -d ' ')"
  elapsed_ms="$(((end_ns - start_ns) / 1000000))"
  echo "trace_ingest,$i,$elapsed_ms,$events" >> "$summary"
done

echo "$summary"
