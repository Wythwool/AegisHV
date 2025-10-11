#!/usr/bin/env bash
set -euo pipefail
./scripts/build_dev.sh
RUST_LOG=info ./target/debug/devharness --policy configs/policies.yaml --json events.jsonl &
./target/debug/aegisd --events events.jsonl --listen 127.0.0.1:9108 &
echo "curl http://127.0.0.1:9108/metrics"
wait
