# Type-1 Boundary Invariants

This document separates invariants enforced by the bootable x86_64 lab path from model-only or production requirements. A local TCG boot has exercised the Limine and owned-descriptor-table boundary, but no reviewed run has exercised the final owned-CR3 or Intel VMX guest path.

## Memory Ownership

- Limine memory-map structure and ranges are validated before allocation.
- Overlapping regions with different types are rejected by the core memory model.
- One early physical-allocation ledger survives the VMXON/VMCS smoke cycle and is reused for guest, guest-page-table, EPT, and interception-bitmap allocation. The live path does not rebuild an allocator from the Limine map.
- Before that ledger allocates any page, it excludes the complete page-covered linker image and the 4K root page named by the inherited active CR3. Invalid linker bounds, an invalid CR3 root, or a changed root before guest setup fails closed.
- Runtime pages are allocated only from bounded Limine `USABLE` regions between 1 MiB and 4 GiB. Bootloader-reclaimable pages are excluded while Limine responses and boot page tables may still reference them.
- VMXON, VMCS, ten guest/EPT pages, and the I/O A, I/O B, and MSR bitmap pages occupy fifteen distinct 4K-aligned physical pages below 4 GiB.
- Physical-to-HHDM conversions reject misalignment, overflow, and non-canonical ranges before raw memory access.
- Guest page materialization zeros a full page before writing its bounded contents. Both I/O pages are filled with `0xff`. The MSR page also starts at `0xff`, then clears exactly the low-read bit for `IA32_PAT`; all three pages are read back against their exact fixed patterns.
- Freeing a page that was not allocated by the core allocator is an error.

The reusable memory and allocator rules are covered by `aegishv-hypervisor-core::memory` and `aegishv-hypervisor-core::allocator` tests. A source test is not evidence that a particular firmware map or HHDM behaved correctly on hardware.

## Host Architectural State

- An early transition IDT is installed before BSS clearing and the main boot stack transition.
- The lab kernel installs owned GDT, 64-bit TSS, IDT, double-fault, NMI, machine-check, a 256 KiB boot stack, and VM-exit stack state before VMX guest entry.
- Loaded descriptor-table bases, selectors, and the available 64-bit TSS descriptor are verified before the host-table success marker.
- VM exit reloads the owned GDTR and IDTR before Rust exit handling because VM exit changes their architectural limits.
- Terminal host exceptions fail closed and halt.

These are BSP bring-up invariants. AP startup, per-CPU descriptor/VMX ownership and paging roots, recoverable guard-fault testing, watchdog recovery, and a production crash path are not implemented.

## W^X Mapping Boundary

- The linker separates executable text, read-only data/GOT, and writable data into page-aligned RX, R, and RW load segments.
- After every Limine/HHDM runtime, guest, and interception-bitmap write and readback completes, the final Intel path materializes a linker-owned PML4/PDPT/PD/PT pool and switches CR3 before capturing VMCS host state. The root maps only the linked higher-half 2 MiB kernel window with 4K supervisor leaves; null, HHDM, and identity aliases are absent.
- Text is RX, rodata/GOT is R/NX, writable state and stacks are RW/NX, and the table pool is RW/NX. Five lower guard pages for double-fault, NMI, machine-check, VM-exit, and boot stacks remain non-present.
- NX support, 4-level mode, physical width, CR0.WP, EFER.NXE, CR3 readback, descriptor-table state, and the live table contents are checked. Clearing PGE before the switch invalidates inherited global translations; unexpected aliases, W+X leaves, changed physical mappings, and non-hardware table mutations fail closed.
- The toy guest maps code executable and non-writable; its stack and page-table pages are writable and non-executable.
- The toy EPT maps only the fixed guest pages with the permissions required by that guest.
- The kernel ELF inspection gate disassembles host `.text` and rejects FPU, MMX/SIMD, and FXSAVE/XSAVE-family state-save instructions. This static host-code invariant reduces the fixed lab path's untracked state surface; it is not a host/guest FPU context switch.
- x86 page-table plan models reject mappings that are both writable and executable.

Early boot, handoff validation, the VMXON/VMCS smoke cycle, and all HHDM materialization still execute with Limine-provided mappings. The owned root covers only the final BSP Intel toy-guest/VM-exit path; it has no general physical direct map, dynamic map/unmap, TLB shootdown, SMP roots, PAT/MMIO policy, teardown, or hardware execution evidence. It must not be described as production-wide host paging.

All owned host leaves select IA32_PAT entry zero. CR3 activation is refused unless that entry is write-back, and VMCS host-state validation repeats the same check for the value restored on every VM exit.

## Intel VMX Entry

- CPUID, `IA32_FEATURE_CONTROL`, `IA32_VMX_BASIC`, `IA32_VMX_MISC`, CR0/CR4 fixed-bit MSRs, true control MSRs, host state, and required write-back four-level EPT capabilities are validated before guest entry. CPU signatures on the Linux KVM known-broken VMX preemption-timer denylist are refused.
- True-control construction preserves architectural mandatory default-one bits while rejecting unsupported functional controls.
- VMXON and VMCS regions use the required revision identifier, size, alignment, and memory type.
- The VMCS contains explicit host state, guest state, control fields, EPTP, three interception-bitmap addresses, guest and host PAT fields, entry RIP/RSP, and VM-exit trampoline state for one 64-bit guest. `use I/O bitmaps` and `use MSR bitmaps` are mandatory; bitmap addresses must be nonzero, 4K-aligned, pairwise distinct, below 4 GiB, and recovered exactly by live `VMREAD` before launch.
- Successful VMLAUNCH and VMRESUME are treated as non-returning; instruction failure is decoded and reported explicitly.
- `IA32_VMX_MISC` supplies the VMX preemption-timer rate. The first timer value is exactly zero to force a sentinel exit before the guest's first instruction. The handler derives a reload from a hard `0x01000000`-TSC-tick budget at that architectural rate and refuses values below 2; the resulting effective deadline cannot exceed the requested budget. It then resumes a finite TSC-or-count probe whose HLT fallback uses a `0x08000000`-TSC-tick horizon and a `0x01000000`-iteration limit. Only the resulting nonzero VMX deadline exit advances RIP to the payload; later payload resumes rearm the same bounded deadline.
- Trap-all I/O bitmap A contains the immediate byte `OUT` of `A` to port `0xe9`; trap-all bitmap B contains the byte `OUT DX, AL` with `DX=0x8000`. The handler validates both exact exits and advances guest RIP without performing either physical port write.
- The fixed MSR page allows exactly the low-range read bit for `IA32_PAT`. Every MSR write and other read traps. The only accepted RDMSR exit remains the exact `IA32_EFER` stage, which returns synthetic zero without replaying that request on the host; direct guest `RDMSR IA32_PAT` must not exit.
- The VMCS enables guest PAT load on entry, guest PAT save and host PAT restore on exit, and exact control/field readback. The guest PAT is a deliberate value whose eight entries are validated and which differs from the captured host value. Every exit must expose the restored live host PAT and expected saved guest PAT; both guard exits must also expose the direct-read register value before the PAT marker is emitted.
- Guest `CR0.TS=1`, `CR0.EM=0`, and `CR4.OSFXSR=1` are part of the exact VMCS state and CR mask/shadow contract. `FNOP` and `MOVDQA xmm0,xmm0` must each produce a hardware-exception VM exit with valid vector 7, no error code, its exact fault RIP, and TS still set. Fixed continuation RIPs skip the faulting probes after validation.
- The only accepted successful exit order is the zero-value preemption-timer sentinel, nonzero timer expiry, I/O bitmap A, I/O bitmap B, CPUID, trapped EFER read, x87 `#NM`, SIMD `#NM`, and HLT; the direct PAT read executes between the EFER and x87 exits. An HLT or timer exit at the probe fallback RIP is classified as `guest-timeout`; a later timer expiry, wrong exception, unexpected exit, or out-of-order exit is terminal.
- VMXOFF follows the successful HLT completion path.

These are code invariants, not execution evidence. The available TCG environment exposes no VMX and WHPX is unavailable.

The signature denylist covers known failures only. It is not proof that every other CPU implements the timer or TSC correctly. The finite TSC-or-count fallback prevents the fixed test from waiting forever if the VMX timer fails or the TSC stalls, but the lab path still has no independent host watchdog.

## Evidence

- Evidence capture requires valid SHA-256 image digests before and after QEMU and rejects the run unless they match.
- Intel guest evidence requires the complete ordered host-table, VMX backend, VMXON, VMCS-load, owned-host-paging, guest-configuration, preemption-exit, I/O-A-exit, I/O-B-exit, CPUID-exit, RDMSR-exit, PAT-state, x87-`#NM`, SIMD-`#NM`, HLT-exit, and guest-run marker chain.
- Evidence also requires exactly one well-formed CPU-signature and timer-diagnostic set. The timer rate, reload, and effective deadline must be internally consistent, the reload must be at least 2, and the effective value must not exceed the hard `0x01000000`-TSC-tick budget. These diagnostics do not change the sixteen-marker count.
- Contradictory backends, skipped VMX operations, host faults, runtime failures, the `aegishv:type1:guest-timeout` marker, guest entry/exit/resume failures, missing Limine handoff, or panic invalidate the run.
- A raw kernel ELF is not accepted as QEMU evidence because it does not receive the Limine handoff.
- A TCG boot that reaches host preflight without VMX is boot-boundary evidence only.
- Even a digest-stable valid sixteen-marker run proves only the fixed toy guest, its two trap-all I/O exits, one-read MSR allowlist, deliberate PAT transition, two fixed `#NM` probes, and final-path owned root on the recorded CPU/firmware/accelerator configuration. It does not prove general PAT policy, FPU/SIMD virtualization or context switching, selective device access, general MSR/exception virtualization, or WRMSR, and it does not prove a general or production hypervisor.

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
