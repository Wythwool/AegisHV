# Changelog

## Unreleased

- Added local management CLI commands for version, health, policy review, policy dry-runs, and action dry-runs.
- Added role, audit, approval, policy bundle, dump evidence, and startup hash helper primitives.
- Added benchmark helper scripts for replay ingest, W^X state handling, offline VMI translation, and synthetic trap transitions.
- Added hardware, performance, security, release, VMI alpha, and type-1 readiness gate documents.
- Added a bootable x86_64 Type-1 lab kernel with current Limine configuration, a validated HHDM/memory-map/executable-address handoff, aligned physical relocation support, and a page-separated RX/R/RW linker layout.
- Added early transition fault handling plus owned GDT, TSS, IDT, double-fault/NMI/machine-check stacks, boot stack, and VM-exit stack state before the Rust and VMX paths run.
- Added kernel ELF inspection, ISO staging/building, tool probing, image manifests, bounded QEMU execution, strict serial-log review, and opt-in evidence capture. Raw ELF QEMU boot is refused because it cannot provide the Limine handoff.
- Added explicit host-table, runtime, VMXON, VMCS-load, guest configuration, CPUID-exit, HLT-exit, completion, and failure markers.
- Added Intel VMX VMLAUNCH/VMRESUME lifecycle handling, hardware instruction wrappers, complete minimal host/guest/control VMCS construction, four-level guest paging, four-level EPT, and an assembly VM-entry/exit trampoline.
- Added a fixed isolated Intel guest containing `mov eax, 0; cpuid; hlt`; the runtime handles the CPUID exit, resumes the guest, handles HLT, and shuts VMX down.
- Allocated VMXON, VMCS, guest, paging, and EPT pages only from bounded Limine `USABLE` memory below 4 GiB; bootloader-reclaimable pages remain reserved.
- Validated VMX CR0/CR4 fixed bits, true-control MSRs including mandatory default-one bits, host architectural state, and required write-back four-level EPT capabilities before guest entry.
- Tightened Intel QEMU evidence to require the complete ordered eight-marker host/VMX/CPUID/resume/HLT chain and reject contradictory backends, skipped operations, host faults, and guest entry/exit/resume errors.
- Booted the modern Limine ISO locally under QEMU TCG through owned host-table installation and runtime preflight. TCG did not expose VMX and WHPX was unavailable, so this is not Intel guest-execution evidence.
- Added AMD SVM hardware instruction wrappers and a checked runtime sequence for EFER.SVME, VMLOAD, VMRUN, VMSAVE, and INVLPGA.
- Corrected guest CR3 typing, VM-entry failure decoding, and EPT qualification semantics in the Intel model layer.
- Expanded synthetic Linux and Windows VMI fixture corpus.

## 0.4.0

- Replaced the bootstrap lock marker with a committed production `Cargo.lock`.
- Removed dependency generation from CI/release paths; locked builds now use the committed lockfile.
- Made the main crate dependency-free so the source archive is self-contained for `cargo metadata --locked`.
- Added event `sequence`, `monotonic_ms`, and structured `loss` metadata.
- Split unsupported/unrelated trace lines from malformed `kvm_exit` parse errors.
- Propagated queue-loss watermarks onto the next emitted event.
- Added PID start-time based identity cache defense.
- Changed QMP action dispatch to prefer stable `vm_id` mappings over VM-name fallback.
- Scoped policy cooldowns by rule, VM, reason, trap type, page, and action set.
- Made unavailable PMU counters serialize as `null` instead of fake zeroes.
- Fixed `NoHypervisorBackend` to report `BackendArch::None`.
- Updated CI, Docker, release, schema, and docs for locked reproducible builds.

## 0.3.1

- Hardened collector EOF/error control messages.
- Split host `host_cpu` from guest `vcpu_id`.
- Added best-effort VM identity enrichment.
- Added W^X page alignment and per-VM/address-space scoping.
- Added strict config validation, policy modes, QMP action tests, and deployment docs.
