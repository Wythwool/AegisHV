#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
out="${TMPDIR:-/tmp}/aegishv-smoke.jsonl"
rm -f "$out"
cargo run --locked -- run --replay ./examples/traces/kvm_exit_sample.log --listen '' --jsonl "$out" --quiet
python3 scripts/validate-jsonl-schema.py --schema schema/event.schema.json --jsonl "$out"
