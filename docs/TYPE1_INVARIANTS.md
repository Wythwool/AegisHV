# Type-1 Boundary Invariants

This document separates invariants enforced by the bootable x86_64 lab path from model-only or production requirements. A local TCG boot has exercised the Limine and owned-host-table boundary, but no reviewed run has exercised the Intel VMX guest path.

## Memory Ownership

- Limine memory-map structure and ranges are validated before allocation.
- Overlapping regions with different types are rejected by the core memory model.
- Runtime pages are allocated only from bounded Limine `USABLE` regions between 1 MiB and 4 GiB. Bootloader-reclaimable pages are excluded while Limine responses and boot page tables may still reference them.
- VMXON, VMCS, guest code, guest stack, guest page tables, and EPT structures occupy twelve distinct 4K-aligned physical pages.
- Physical-to-HHDM conversions reject misalignment, overflow, and non-canonical ranges before raw memory access.
- Page materialization zeros a full page before writing its bounded contents.
- Freeing a page that was not allocated by the core allocator is an error.

The reusable memory and allocator rules are covered by `aegishv-hypervisor-core::memory` and `aegishv-hypervisor-core::allocator` tests. A source test is not evidence that a particular firmware map or HHDM behaved correctly on hardware.

## Host Architectural State

- An early transition IDT is installed before BSS clearing and the main boot stack transition.
- The lab kernel installs owned GDT, 64-bit TSS, IDT, double-fault, NMI, machine-check, a 256 KiB boot stack, and VM-exit stack state before VMX guest entry.
- Loaded descriptor-table bases, selectors, and the available 64-bit TSS descriptor are verified before the host-table success marker.
- VM exit reloads the owned GDTR and IDTR before Rust exit handling because VM exit changes their architectural limits.
- Terminal host exceptions fail closed and halt.

These are BSP bring-up invariants. AP startup, per-CPU descriptor/VMX ownership, guard pages, watchdog recovery, and a production crash path are not implemented.

## W^X Mapping Boundary

- The linker separates executable text, read-only data/GOT, and writable data into page-aligned RX, R, and RW load segments.
- The toy guest maps code executable and non-writable; its stack and page-table pages are writable and non-executable.
- The toy EPT maps only the fixed guest pages with the permissions required by that guest.
- x86 page-table plan models reject mappings that are both writable and executable.

The lab kernel still executes with Limine-provided host mappings and HHDM aliases. It does not install a hypervisor-owned CR3, prove W^X across every alias, or provide guard pages. The linker and guest/EPT permissions must not be described as production host W^X enforcement.

## Intel VMX Entry

- CPUID, `IA32_FEATURE_CONTROL`, `IA32_VMX_BASIC`, `IA32_VMX_MISC`, CR0/CR4 fixed-bit MSRs, true control MSRs, host state, and required write-back four-level EPT capabilities are validated before guest entry. CPU signatures on the Linux KVM known-broken VMX preemption-timer denylist are refused.
- True-control construction preserves architectural mandatory default-one bits while rejecting unsupported functional controls.
- VMXON and VMCS regions use the required revision identifier, size, alignment, and memory type.
- The VMCS contains explicit host state, guest state, control fields, EPTP, entry RIP/RSP, and VM-exit trampoline state for one 64-bit guest.
- Successful VMLAUNCH and VMRESUME are treated as non-returning; instruction failure is decoded and reported explicitly.
- `IA32_VMX_MISC` supplies the VMX preemption-timer rate. The first timer value is exactly zero to force a sentinel exit before the guest's first instruction. The handler derives a reload from a hard `0x01000000`-TSC-tick budget at that architectural rate and refuses values below 2; the resulting effective deadline cannot exceed the requested budget. It then resumes a finite TSC-or-count probe whose HLT fallback uses a `0x08000000`-TSC-tick horizon and a `0x01000000`-iteration limit. Only the resulting nonzero VMX deadline exit advances RIP to the payload; later payload resumes rearm the same bounded deadline.
- Unconditional I/O exiting remains enabled. The only accepted I/O exit is the toy guest's immediate byte `OUT` of `A` to port `0xe9`; the handler validates it and advances guest RIP without performing the physical port write.
- The only accepted successful exit order is the zero-value preemption-timer sentinel, the nonzero timer expiry from the finite probe, the port-I/O exit, CPUID, and HLT. An HLT or timer exit at the exact probe fallback RIP is classified as `guest-timeout`; a timer expiry during a later payload stage, any other unexpected exit, or any out-of-order exit is terminal. Unexpected non-fallback probe exits remain guest-exit errors.
- VMXOFF follows the successful HLT completion path.

These are code invariants, not execution evidence. The available TCG environment exposes no VMX and WHPX is unavailable.

The signature denylist covers known failures only. It is not proof that every other CPU implements the timer or TSC correctly. The finite TSC-or-count fallback prevents the fixed test from waiting forever if the VMX timer fails or the TSC stalls, but the lab path still has no independent host watchdog.

## Evidence

- Intel guest evidence requires the complete ordered host-table, VMX backend, VMXON, VMCS-load, guest-configuration, preemption-exit, I/O-exit, CPUID-exit, HLT-exit, and guest-run marker chain.
- Evidence also requires exactly one well-formed CPU-signature and timer-diagnostic set. The timer rate, reload, and effective deadline must be internally consistent, the reload must be at least 2, and the effective value must not exceed the hard `0x01000000`-TSC-tick budget. These diagnostics do not change the ten-marker count.
- Contradictory backends, skipped VMX operations, host faults, runtime failures, the `aegishv:type1:guest-timeout` marker, guest entry/exit/resume failures, missing Limine handoff, or panic invalidate the run.
- A raw kernel ELF is not accepted as QEMU evidence because it does not receive the Limine handoff.
- A TCG boot that reaches host preflight without VMX is boot-boundary evidence only.
- Even a valid ten-marker run proves only the fixed toy guest on the recorded CPU/firmware/accelerator configuration. It does not prove a general or production hypervisor.

## Event Ring Loss

- The hypervisor-to-control-plane event ring is bounded.
- Event records carry ABI version and monotonic sequence numbers.
- When the event ring is full, the oldest entry is overwritten and the loss counter increments.
- The command ring is bounded and rejects unknown command codes before enqueue.

These model invariants are covered by `aegishv-hypervisor-core::abi` and `aegishv-event-abi` tests. The rings are not yet a live lab-kernel control plane.

## CPU And VM State Models

- A per-CPU slot cannot become online until stack and event-ring state exist.
- VM state transitions are explicit: created, configured, runnable, running, paused, stopping, stopped, crashed.
- The vCPU scheduler tracks queued, running, and halted states.

These rules are covered by `aegishv-hypervisor-core::percpu`, `vm`, and `scheduler` tests. They are not wired to SMP, a live scheduler, or the fixed VMX toy guest and do not implement scheduler-driven preemption; the fixed guest's VMX preemption timer is only a stage deadline.
