# AegisHV

Type‑1 Hypervisor EDR (x86‑64 / arm64). **Zero in‑guest agents.**  
Introspection via VMX/SVM/EL2, EPT/NPT **W^X** enforcement, execute‑traps, syscall‑path checks, PMU sampling.  
Event pipeline: JSONL + Prometheus exporter.

> Reality check: a production‑grade bare‑metal hypervisor is months of work and lots of hardware bring‑up.
> This repo gives you two tracks:
>
> 1) **Microvisor (bare‑metal)**: real VMX/SVM/EL2 scaffolding with clean, readable C. Files under `hv/`.
>    Boot and page‑table setup are stubbed; hardware bring‑up TODOs are marked plainly.
> 2) **Dev Harness (hosted)**: a Rust KVM backend that lets you validate policies (W^X, exec traps,
>    syscall guards) and the **full telemetry path** (ring buffer → JSONL → Prometheus) on a normal Linux box.
>
> You can develop policies and collectors today, then swap the backend when the microvisor is ready.

## Features
- **W^X policy** at stage‑2 (EPT/NPT) with optional *exec‑trap* pages.
- **Syscall‑path guard**: configurable allow/deny by path patterns and callsite hashes.
- **PMU sampling**: perf‑driven sampling in dev harness; hooks for PEBS/IBS in microvisor.
- **No in‑guest agents** by design.
- **Events**: newline‑delimited JSON and `/metrics` Prometheus HTTP endpoint.
- **Linux out‑of‑tree driver** exposes a lock‑free ring buffer at `/dev/aegishv`.

## Layout
```
hv/             # type‑1 microvisor scaffolding (C, freestanding)
  x86/          # VMX + EPT + VMEXIT skeletons
  arm64/        # EL2 + Stage‑2 translation skeletons
  common/       # ring buffer, simple logger
devharness/     # KVM‑based backend (Rust) for policy + telemetry dev
drivers/linux/  # /dev/aegishv char device (ring buffer)
userspace/aegisd# Prometheus exporter + JSON sink (Rust, axum)
api/            # event.proto + JSON schema
docs/           # DESIGN, BUILD, THREAT_MODEL, METRICS, ROADMAP
configs/        # policies.yaml — W^X + syscall guards
```
## Quick start (Dev Harness)
```bash
# 1) build kernel driver (optional; harness can run without it)
make -C drivers/linux

# 2) run dev harness (simulated HV using /dev/kvm)
cd devharness
cargo run -- --policy ../configs/policies.yaml --json ../events.jsonl

# 3) start exporter
cd ../userspace/aegisd
cargo run -- --events ../../events.jsonl --listen 0.0.0.0:9108
# curl localhost:9108/metrics
```

### Status
- Microvisor: bring‑up stubs with clearly marked TODOs for real hardware init.
- Dev harness: usable for **policy** and **telemetry** work today.