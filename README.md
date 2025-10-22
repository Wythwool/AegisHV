# AegisHV — Hypervisor EDR via KVM tracefs (x86-64/arm64)

Type‑1 style EDR without in‑guest agents. Host‑side daemon tailing KVM tracepoints (`kvm:kvm_exit` etc.) and exporting events as JSON Lines and Prometheus metrics. Heuristics detect EPT/NPT W^X breaks (exec from non‑X, write+exec toggling), bursty exit patterns, and suspicious headless VMs. PMU (cycles) sampling for vCPU threads is optional.

**Scope**: Linux hosts with KVM. Works on x86‑64 and arm64 (pKVM) when tracefs provides KVM events. No guest changes.

## Build
Rust 1.77+.
```bash
cargo build --release
```

## CLI
```
aegishv run [--tracefs /sys/kernel/tracing] [--jsonl out/events.jsonl] [--listen 0.0.0.0:9108]
            [--rules config.example.toml] [--replay FILE] [--quiet]

aegishv snapshot --out out/snapshot.json         # one‑shot sample (counts)
aegishv self-check                               # env sanity
aegishv dump-schemas                             # print JSON schemas
```

- `run`: tail live tracefs (or `--replay` file) and emit events + `/metrics` (Prometheus).
- `--rules`: thresholds and allowlists per VM name.
- `--jsonl`: append JSON Lines for SIEM ingestion.
- `--listen`: HTTP server for metrics.
- `--replay`: parse an example trace log (no root needed), good for CI.

## Enable tracing (root)
```bash
sudo ./tools/enable_tracefs.sh start    # enable kvm:* and hyp/* (arm64) tracepoints
# ... run aegishv ...
sudo ./tools/enable_tracefs.sh stop     # cleanup
```

## Output
- **JSONL** (schema `schema/event.schema.json`): one line per event with fields like `vm`, `vcpu`, `reason`, `ept.access` (`read|write|exec`), and `gpa` if available.
- **Prometheus**: counters/gauges under `/metrics`:
  - `aegishv_ept_violations_total{type="exec|write|read"}`
  - `aegishv_wx_violation_total` (write then exec on same GPA in short window)
  - `aegishv_vm_exits_total{reason="..."}`
  - `aegishv_vcpu_cycles_total{tid="...",vm="..."}` (if PMU sampling is enabled)

## Examples
```bash
# Replay sample log (no root, no GPU): deterministic output
aegishv run --replay examples/traces/kvm_exit_sample.log --listen 127.0.0.1:9108 --jsonl out/events.jsonl

# Live (root). Expose Prometheus and write JSONL
sudo aegishv run --tracefs /sys/kernel/tracing --jsonl /var/log/aegishv.jsonl --listen 0.0.0.0:9108
```

## Limitations
- Needs KVM tracepoints available to user‑space (`tracefs`), typically root or tracing group.
- EPT/NPT decoding uses fields exposed by kvm_exit lines; address/GPA presence depends on kernel version.
- PMU uses `perf_event_open` on vCPU threads by name; if thread naming differs, sampling may be skipped.

License: MIT.




