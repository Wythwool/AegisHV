# Changelog

## Unreleased

- Added local management CLI commands for version, health, policy review, policy dry-runs, and action dry-runs.
- Added role, audit, approval, policy bundle, dump evidence, and startup hash helper primitives.
- Added benchmark helper scripts for replay ingest, W^X state handling, offline VMI translation, and synthetic trap transitions.
- Added hardware, performance, security, release, VMI alpha, and type-1 readiness gate documents.
- Added a bootable x86_64 Type-1 lab kernel with current Limine configuration, a validated HHDM/memory-map/executable-address handoff, aligned physical relocation support, and a page-separated RX/R/RW linker layout.
- Added early transition fault handling plus owned GDT, TSS, IDT, double-fault/NMI/machine-check stacks, boot stack, and VM-exit stack state before the Rust and VMX paths run.
- Added kernel ELF inspection, ISO staging/building, tool probing, image manifests, bounded QEMU execution, strict serial-log review, and opt-in evidence capture. Raw ELF QEMU boot is refused because it cannot provide the Limine handoff.
- Added explicit host-table, runtime, VMXON, VMCS-load, guest configuration, preemption-exit, I/O-exit, CPUID-exit, HLT-exit, completion, and failure markers.
- Added Intel VMX VMLAUNCH/VMRESUME lifecycle handling, hardware instruction wrappers, complete minimal host/guest/control VMCS construction, four-level guest paging, four-level EPT, and an assembly VM-entry/exit trampoline.
- Added a fixed isolated Intel guest with a finite TSC-or-count deadline probe and HLT fallback followed by an `AL='A'; OUT 0xe9,AL; CPUID leaf/subleaf 0; HLT` payload; the runtime contains the port write, handles the ordered exits, keeps every resume bounded, and shuts VMX down.
- Kept VMXON/VMCS and toy-guest/EPT allocations under one early physical-allocation ledger. It reserves the linked kernel span and current inherited CR3 root page before consuming bounded Limine `USABLE` memory below 4 GiB; bootloader-reclaimable pages remain excluded.
- Added a linker-owned four-page host paging root for the final Intel toy-guest path. After all HHDM materialization, the BSP enables NXE and CR0.WP, rejects LA57, flushes inherited global translations, switches to 4K RX/R/RW kernel leaves with no HHDM/identity alias, validates the live tables, and leaves five lower stack guards non-present.
- Validated VMX CR0/CR4 fixed bits, true-control MSRs including mandatory default-one bits, host architectural state, and required write-back four-level EPT capabilities before guest entry.
- Tightened Intel QEMU evidence to require matching valid pre/post-run SHA-256 image digests, the ordered eleven-marker host/VMX/owned-paging/guest chain, and consistent CPU/timer diagnostics, and to reject changed images, contradictory backends, paging failures, skipped operations, host faults, guest timeouts, and guest entry/exit/resume errors.
- Booted the modern Limine ISO locally under QEMU TCG through owned descriptor-table installation and runtime preflight. TCG did not expose VMX and WHPX was unavailable, so this run stopped before the final owned-CR3 path and is not owned-paging or Intel guest-execution evidence.
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
