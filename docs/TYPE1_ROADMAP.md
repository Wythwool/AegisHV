# Type-1 / VMI / trap backend roadmap

A real type-1 EDR is a separate runtime, not a bigger tracefs parser. The current code keeps the host sensor honest and adds interface boundaries for the backend work.

The phase backlog for this work lives in `BACKLOG.md`. That file tracks IDs, scope, acceptance criteria, and production gates. This document explains the backend boundary and implementation order.

VMI safety and consistency rules for the offline infrastructure live in `docs/VMI.md`.

## Backend contract now present

`src/hypervisor.rs` defines the minimum shape of a backend that can eventually drive VMX/SVM/EL2 exits:

- VM identity and vCPU identity.
- VM-exit classification.
- Stage-2 permission updates.
- Backend event pump.
- Backend health snapshot.

`src/vmi.rs` defines the semantic layer needed above that backend:

- guest physical memory reads;
- vCPU register reads;
- guest virtual-to-physical translation;
- OS profile loading;
- process/module/symbol attribution;
- syscall-path reporting;
- trap lifecycle calls.

The current implementation intentionally provides contracts and a no-backend boundary, not fake VMX/SVM/EL2 code.

## Required runtime subsystems for actual type-1

- Boot path, measured init, and early allocator.
- Per-CPU virtualization state.
- Intel VMX backend: VMXON, VMCS setup, VMLAUNCH/VMRESUME, VM-exit handlers, EPT, VPID, INVEPT.
- AMD SVM backend: VMCB, VMRUN, intercept vectors, NPT, ASIDs, INVLPGA, MSRPM/IOPM.
- ARM64 backend: EL2 entry, vectors, HCR_EL2, VTCR_EL2, VTTBR_EL2, Stage-2 tables, GIC virtualization.
- Memory manager and Stage-2 permission manager.
- Interrupt/device/IOMMU isolation.
- Guest lifecycle, SMP, crash recovery, secure update.
- Hardware test matrix on Intel, AMD, and arm64.

## Next implementation sequence

1. Replace best-effort identity with real libvirt lifecycle discovery and QMP socket discovery.
2. Add guest memory/register access through a backend that can be tested before bare metal.
3. Add OS profile database and symbol resolution for Linux first.
4. Implement syscall path checks against LSTAR, kernel text, syscall table, modules, ftrace/kprobe/eBPF surfaces.
5. Build a trap engine with permission flips, huge-page split, TLB invalidation, single-step/retrap, storm control, and JIT allowlists.
6. Only then split or port the backend into a real type-1 runtime.
