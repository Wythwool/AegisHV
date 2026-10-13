# Benchmarks

| Script | Workload |
| --- | --- |
| `scripts/bench-trace-ingest.sh` | Replay time and event count |
| `scripts/bench-wx-state.sh` | W^X replay time and alert count |
| `scripts/bench-vmi-translate.sh` | Repeated offline translation CLI calls |
| `scripts/bench-trap-synthetic.sh` | In-memory trap state transitions |

Keep the raw output, commit, command, fixture hash, CPU, kernel, toolchain, and
build mode with a result. Run several samples on the same host to see variance.
These scripts do not measure live VM exits, hardware invalidation, guest memory
reads, or fleet QMP latency.
