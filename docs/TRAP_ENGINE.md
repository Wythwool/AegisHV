# Trap model

`src/trap` models page permissions, overlap checks, lookup, one-level huge-page
splits, and trap lifecycle. States cover armed, hit, classifying, allowed-step,
denied, rearmed, disabled, and storm-throttled pages.

Invalidation and single-step plans describe Intel EPT/MTF, AMD NPT/ASID, and ARM
Stage-2/TLBI operations. They do not execute those instructions. Backend
negotiation requires permission writes, invalidation, stepping, and huge-page
split support before accepting enforcement.

Storm control is keyed by VM, address space, page, and vCPU. JIT windows require
process attribution and limit both pages and duration. The event schema can
carry trap metadata, but the sensor's W^X path remains correlation-only.

The config accepts only `backend="synthetic"`. Setting `trap.enable=true` does
not connect the controller to live KVM page tables.

```sh
cargo run --locked --bin trap_synthetic_bench -- --iterations 10000
```

This measures in-process model transitions, not VM exits or guest performance.
