# Trap Engine

AegisHV has a synthetic trap-engine model. It is a library and test surface for Stage-2 permission logic. The active daemon still runs as a host-side tracefs sensor.

## Implemented

- Architecture-neutral permission bits for read, write, and execute.
- Conceptual Stage-2 page sizes: 4K, 2M, and 1G.
- Synthetic Stage-2 table with overlap checks, lookup, permission updates, and one-level splits.
- Backend limit descriptions for synthetic, Intel EPT, AMD NPT, and ARM Stage-2 semantics.
- Trap controller states: armed, hit, classifying, allowed-step, denied, rearmed, disabled, and storm-throttled.
- Synthetic execute and write trap lifecycles with single-step/retrap accounting.
- TLB invalidation planning for synthetic records, INVEPT, INVLPGA, and ARM TLBI names.
- Single-step strategy selection for Intel MTF, x86 TF fallback, AMD VMCB single-step, ARM software step, and synthetic steps.
- Storm control by VM, address space, page, and vCPU with fail-open and fail-closed decisions.
- JIT temporary-window policy that requires guest process attribution and bounded page/window limits.
- Trap metadata in event JSON under the `trap` object.
- W^X correlation can be marked as correlation-only or trap-engine observed. The default runtime path remains correlation-only.
- Capability negotiation refuses enforcement when a backend lacks traps, Stage-2 permission writes, invalidation, single-step, or huge-page split support.

## Configuration

`config.example.toml` includes a disabled `[trap]` section. The parser validates storm limits, storm mode, JIT limits, and backend name. The only accepted backend value is `synthetic` until a real backend is implemented and tested.

Setting `trap.enable=true` does not make the daemon enforce guest memory permissions. The current runtime does not wire the trap controller into KVM, EPT, NPT, or ARM Stage-2.

## Bench Harness

`trap_synthetic_bench` measures the synthetic state machine and permission model in the current process:

```bash
cargo run --locked --bin trap_synthetic_bench -- --iterations 10000
```

The output reports the requested iteration count, elapsed microseconds, and synthetic transition count. It is not a hardware benchmark and does not measure VM exits, TLB invalidations, EPT/NPT writes, or guest performance.

## Limits

No code in this trap-engine layer writes hardware page tables, performs INVEPT/INVLPGA/TLBI, controls VMX/SVM/EL2 execution, or injects real single-step traps. Unsupported capability paths return typed errors or negotiation failures instead of reporting success.
