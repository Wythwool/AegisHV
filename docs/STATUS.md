# Status

This tree is a hardened host-side KVM sensor and a cleaner platform for the next VMI/trap work. It is not being mislabeled as a finished type-1 hypervisor.

## Implemented in this tree

- Committed release `Cargo.lock`; no CI/release lockfile generation before `--locked` builds.
- Dependency-free main crate so the source archive is self-contained for `cargo metadata --locked`.
- Hardened tracefs/replay collector with a separate non-lossy control channel for EOF and collector errors.
- Strict config validation and normalization.
- `validate-config` CLI command.
- JSONL event pipeline with event IDs, monotonic timestamps, sequence numbers, host metadata, VM identity fields, host CPU, guest vCPU field, address-space fields, rule/action state, and data-loss objects.
- Optional disk spool segments for JSONL write failures, with compatible plaintext v1 records and opt-in RLE-compressed v2 records.
- Optional UDP syslog and Linux journald mirroring of emitted JSON events. JSONL remains the primary event stream.
- Snapshot schema version 2 with tracefs diagnostics and bounded VM inventory from configured identity discovery.
- Local management CLI for version, health, policy explanation, policy dry-runs, and action dry-runs. It does not start a remote management service.
- Library primitives for role checks, bounded append-only audit records, manual approval files, policy bundle verification, startup hash events, and dump evidence state separation.
- Benchmark helper scripts for replay ingest, W^X state, offline VMI translation, and synthetic trap-controller timing. They do not commit benchmark result numbers.
- Release gate documents for hardware coverage, performance evidence, security review, host sensor release scope, VMI alpha scope, and type-1 readiness.
- Unsupported/unrelated trace lines separated from malformed `kvm_exit` parse errors.
- Queue-loss watermark propagation through `data_loss=true` on the next emitted event, with aggregate drop counts and exact emitted-sequence gaps only when the runtime can prove the gap.
- Page-aligned W^X correlation scoped by VM identity and address space (`cr3/asid/vmid/vpid`), with detector cooldown separate from policy cooldown.
- Explicit split between trace header `host_cpu` and real `vcpu_id`.
- Best-effort VM identity enrichment from `/proc`, PID start time, QEMU command line, cgroups/systemd, libvirt-style UUID/name hints, and QMP socket hints, with bounded source and confidence metadata.
- Tracepoint format autodiscovery parser for `events/*/*/format` files. The active collector still uses text `trace_pipe`.
- Policy priority, entity-scoped cooldown, dry-run, suppress, and enforce modes.
- Multiple actions per rule.
- QMP action retries, timeout handling, stable `vm_id` matching by default, VM-name fallback refusal when `identity.require_stable_qmp_match=true`, dump-root checks, structured action audit fields, manual-approval/noop actions, and mock tests.
- Replay EOF unit tests, including the queue-full EOF case.
- Prometheus text metrics + JSON health/readiness endpoints.
- PMU target rediscovery fallback with unavailable hardware counters represented as `null`, not fake zeroes.
- PMU sampling models for grouped counter deltas, stable target validation, bounded ring loss accounting, PEBS/IBS/SPE capability flags, and offline CPI baseline anomaly checks.
- CI/release wiring, systemd unit, Docker build smoke, docs, and packaging scripts.
- VMI/trap/type-1 interface boundaries in `src/vmi.rs` and `src/hypervisor.rs`.
- Synthetic/offline Linux x86_64 VMI helpers for profile parsing, KASLR anchors, task/module walking, syscall table and LSTAR checks, IDT/GDT/control-register checks, text hashing, ftrace/kprobe/BPF inventory, and an off-hot-path detector runner.
- Synthetic/offline Windows x86_64 VMI helpers for exact build/PDB profile parsing, pre-extracted symbol caches, ntoskrnl base checks, EPROCESS/module walking, SSDT and LSTAR checks, IDT/GDT checks, process callback inventory, text hashing, protection-limit reporting, and an off-hot-path detector runner.
- Detector engine library layer with a generic detector trait, scheduler, severity/confidence scoring, budget accounting, kernel text and syscall-hook normalizers, hidden process/module inventory comparison, executable anonymous and RWX mapping checks, JIT allow rules, W^X detection bridging, dedupe, incident objects, and versioned detector state parsing.
- Synthetic trap-engine library layer with architecture-neutral Stage-2 permissions, a synthetic permission table, trap controller states, invalidation planning, single-step strategy selection, storm control, JIT temporary-window policy, trap event metadata, and backend capability negotiation.
- `no_std` workspace crates for type-1 boundary models: core IDs, memory-map validation, physical page allocation, crash records, per-CPU state, event and command ABI rings, VM lifecycle, vCPU scheduling, x86 serial logging, x86 page-table plans, and AP startup plan validation.
- Planned type-1 boot skeleton artifacts: a no-std boot handoff crate, Limine config, x86_64 linker script, x86_64 entry symbol stub, image-plan helper, QEMU serial-marker contract, and build-plan helper. These artifacts do not produce a bootable image.
- Device isolation model code for physical page ownership, huge-page split/merge planning, DMA domains, PCI inventory, ACPI DMAR/IVRS parsing, virtio-mmio state, bounded console queues, read-only block images, and virtio-net quarantine decisions.
- Intel VMX lab models for feature detection, VMXON/VMCS region checks, VMCS lifecycle, VMX control adjustment, explicit exit handlers, EPT mapping plans, VPID/INVEPT invalidation plans, execute/write traps, Monitor Trap Flag fallback behavior, and minimal Linux lab coverage validation.
- AMD SVM lab models for feature detection, EFER.SVME value handling, VMCB layout checks, VMRUN/VMLOAD/VMSAVE/INVLPGA instruction facades, explicit intercept handlers, NPT map plans, nested page fault routing, ASID management, execute/write traps, and tiny guest lab validation.
- ARM64 EL2 lab models for capability decoding, vector table validation, 4K Stage-2 map plans, VTCR/VTTBR construction, ESR/FAR/HPFAR abort decode, TLBI planning, HVC/SMC/WFI/WFE traps, execute/write traps, GIC virtualization planning, virtual timer state, and toy guest coverage validation.

## Still not implemented as runtime code

- Bare-metal VMXON/VMLAUNCH/VMRESUME hypervisor backend.
- Bare-metal execution of the Intel VMX lab models is not implemented.
- AMD VMRUN/VMCB backend.
- Bare-metal execution of the AMD SVM lab models is not implemented.
- ARM64 EL2 runtime and vectors.
- Bare-metal execution of the ARM64 EL2 lab models is not implemented.
- Bootable type-1 image, APIC startup, real trampoline code, and QEMU boot evidence are not implemented. The Limine config, linker script, image-plan manifest, and serial-marker contract are boundary artifacts only.
- Live device assignment, SMMU/VT-d/AMD-Vi programming, virtual switch enforcement, and SR-IOV quarantine are not implemented.
- Guest physical memory reader.
- Guest virtual-to-physical translation.
- vCPU register reader from a real backend.
- Real Linux/Windows guest OS profile extraction and live profile distribution.
- Live Windows guest process/module/syscall/callback reads are not implemented.
- Runtime syscall-path integrity monitoring is not implemented.
- Runtime detector engine integration is not implemented; the detector layer is currently a library surface with tests.
- Direct hardware EPT/NPT/Stage-2 permission flips, real TLB invalidation, and real single-step/retrap execution are not implemented. The current trap engine is a synthetic model with tests.
- Host page-table plans are data models. They do not install CR3 or change live page tables.
- Libvirt API integration with lifecycle events is not implemented.
- True PMU grouped-counter/ring-buffer sampling with PEBS/IBS/SPE semantics is not implemented.
- Remote management service, multi-user authentication backend, online policy update service, and hardware attestation are not implemented.
- OTLP runtime export is not implemented. `docs/EVENT_EXPORT.md` is design-only.
- OCSF and ECS runtime output is not implemented. `docs/EVENT_MAPPINGS.md` is mapping guidance only.

## Claim discipline

Describe the current code as a Linux host-side KVM telemetry sensor. It reads tracefs text, emits JSONL events, exposes metrics, correlates W^X patterns, and can call configured QMP actions.

Do not describe this tree as type-1, full VMI, direct EPT/NPT/Stage-2 enforcement, syscall-path integrity, hardware PMU sampling, libvirt lifecycle integration, or a finished EDR product. Those claims require backend code, tests, docs, and release evidence that are not present in this tree.

Roadmap documents may discuss those targets, but they must keep planned work separate from implemented behavior.

This version removes several deployment footguns from the host sensor layer and makes the next backend work explicit. It does not fake a type-1 hypervisor.
