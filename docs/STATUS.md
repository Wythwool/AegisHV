# Status

This tree contains a hardened host-side KVM sensor and a separate bootable x86_64 Type-1 lab kernel. The lab kernel is a bring-up target with a wired Intel toy-guest path, not a production or general-purpose hypervisor.

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
- Bootable x86_64 Type-1 lab artifacts: a no-std kernel with a modern Limine request block and configuration, validated HHDM/memory-map/executable-address handoff, aligned physical relocation support, a page-separated linker layout, owned GDT/TSS/IDT and VM-exit stacks, early serial diagnostics, and one bounded early allocation ledger carried from VMX preflight into guest/EPT setup. The ledger excludes the linked kernel image and inherited active CR3 root before allocating from Limine `USABLE` memory. Kernel and ISO builders, ELF inspection, tool probing, strict QEMU evidence capture, and an opt-in lab runner are present. A local QEMU TCG run has booted the ISO through owned host tables and runtime preflight; normal CI still does not build the ISO or run QEMU.
- Device isolation model code for physical page ownership, huge-page split/merge planning, DMA domains, PCI inventory, ACPI DMAR/IVRS parsing, virtio-mmio state, bounded console queues, read-only block images, and virtio-net quarantine decisions.
- Intel VMX lab models and runtime pieces for feature detection, true-control and fixed-bit validation, VMXON/VMCS region checks, complete minimal host/guest VMCS state, VMX control adjustment, four-level guest paging and EPT, explicit exit handlers, VPID/INVEPT plans, execute/write traps, x86_64 VMX instruction wrappers, and a VMXON/VMCLEAR/VMPTRLD/VMLAUNCH/VMRESUME sequencing layer with an assembly entry/exit trampoline.
- A wired Intel toy guest with a finite TSC-or-count deadline probe and HLT fallback followed by an `AL='A'; OUT 0xE9,AL; CPUID leaf/subleaf 0; HLT` payload. One allocation ledger reserves the linked kernel image and inherited active CR3 root, allocates all twelve distinct VMX/guest/EPT pages from `USABLE` memory, and is retained across the preflight and guest paths. The runtime refuses CPUID signatures known by Linux KVM to have broken VMX preemption timers and enables unconditional I/O exiting. An initial zero-value sentinel exits before the first instruction; the handler then resumes the probe with a reload derived from a hard `0x01000000`-TSC-tick budget and the `IA32_VMX_MISC` timer rate. The effective deadline never exceeds that budget, and the CPU is refused if the resulting reload is less than 2. The probe reaches its HLT fallback at either the `0x08000000`-TSC-tick horizon or a `0x01000000`-iteration limit. Only a real expiration of the nonzero VMX deadline advances RIP to the payload and emits the preemption marker. An HLT or timer exit at the exact fallback RIP emits `guest-timeout` instead of wedging the BSP; other unexpected probe exits remain `guest-exit-error`. Later stages remain bounded. The handler validates the byte `OUT` to port `0xe9` and advances guest RIP without performing a host port write, then handles CPUID, HLT, and VMXOFF. Strict evidence requires the complete ordered host-table, VMX, configuration, preemption-exit, I/O-exit, CPUID-exit, HLT-exit, and completion markers plus one internally consistent CPU-signature/timer diagnostic set.
- AMD SVM lab models for feature detection, EFER.SVME value handling, VMCB layout checks, VMRUN/VMLOAD/VMSAVE/INVLPGA instruction facades, x86_64 hardware instruction wrappers, SVM runtime sequencing, explicit intercept handlers, NPT map plans, nested page fault routing, ASID management, execute/write traps, and tiny guest lab validation.
- ARM64 EL2 lab models for capability decoding, vector table validation, 4K Stage-2 map plans, VTCR/VTTBR construction, ESR/FAR/HPFAR abort decode, TLBI planning, HVC/SMC/WFI/WFE traps, execute/write traps, GIC virtualization planning, virtual timer state, and toy guest coverage validation.

## Still missing or unproven

- Reviewed Intel guest-execution evidence is not present. The available QEMU TCG environment boots the Limine/host-table path but does not expose VMX, and WHPX is unavailable, so it cannot prove VMXON, VMLAUNCH, the preemption or I/O exits, CPUID, VMRESUME, or HLT.
- A general guest loader, reusable VM/vCPU lifecycle, multiple guests, scheduling, and recovery are not implemented; the live Intel path is one fixed BSP-only toy guest.
- The AMD SVM instruction/runtime layer is wired into the type-1 kernel as a checked runtime plan only after a CPUID capability snapshot, EFER.SVME preflight, a controlled EFER.SVME enable write, and HHDM materialization of the VMCB page; the kernel entry path does not execute VMRUN or claim QEMU/hardware evidence.
- Booted guest execution through the AMD SVM lab models is not implemented.
- ARM64 EL2 runtime and vectors.
- Bare-metal execution of the ARM64 EL2 lab models is not implemented.
- SMP/AP startup, per-CPU VMX state, APIC and interrupt routing, guest-timer virtualization, scheduler-driven preemption, and interrupt injection are not implemented. The VMX preemption timer only bounds stages of the fixed Intel toy guest on the BSP.
- There is no independent host watchdog for timer failure. The finite in-guest TSC-or-count fallback prevents this fixed probe from wedging the BSP even if one fallback source stalls, but it is not a general hostile-guest watchdog. The known-broken CPU-signature denylist cannot cover unknown errata, so hardware evidence and broader CPU/firmware qualification remain required.
- PAT, XSAVE/FPU, comprehensive MSR context, selective I/O and MSR bitmap policy, broad exit coverage, and hostile-guest recovery are not implemented. The fixed guest uses unconditional I/O exiting rather than a general device or port policy.
- Live device assignment, SMMU/VT-d/AMD-Vi programming, virtual switch enforcement, and SR-IOV quarantine are not implemented.
- Guest physical memory reader.
- Guest virtual-to-physical translation.
- vCPU register reader from a real backend.
- Real Linux/Windows guest OS profile extraction and live profile distribution.
- Live Windows guest process/module/syscall/callback reads are not implemented.
- Runtime syscall-path integrity monitoring is not implemented.
- Runtime detector engine integration is not implemented; the detector layer is currently a library surface with tests.
- General direct hardware EPT/NPT/Stage-2 permission flips, real TLB invalidation, and real single-step/retrap execution are not implemented. The Intel toy guest has a fixed EPT, while the general trap engine remains a synthetic model with tests.
- Host page-table plans are data models. The lab kernel still uses Limine's mappings; it does not install a hypervisor-owned CR3, enforce runtime W^X across all aliases, or provide guard pages.
- Libvirt API integration with lifecycle events is not implemented.
- True PMU grouped-counter/ring-buffer sampling with PEBS/IBS/SPE semantics is not implemented.
- Remote management service, multi-user authentication backend, online policy update service, and hardware attestation are not implemented.
- Hardware soak, broad CPU/firmware qualification, secure/measured boot, signed and rollback-safe hypervisor updates, and a production incident/crash lifecycle are not implemented.
- OTLP runtime export is not implemented. `docs/EVENT_EXPORT.md` is design-only.
- OCSF and ECS runtime output is not implemented. `docs/EVENT_MAPPINGS.md` is mapping guidance only.

## Claim discipline

Describe the default `aegishv` binary as a Linux host-side KVM telemetry sensor. It reads tracefs text, emits JSONL events, exposes metrics, correlates W^X patterns, and can call configured QMP actions.

The separate no-std target may be described as a bootable x86_64 Type-1 lab kernel with an Intel VMX toy-guest path wired in code. The local TCG run may be cited only as Limine, owned-host-table, and preflight boot evidence. Do not claim demonstrated Intel guest execution without the full strict marker chain and validated CPU/timer diagnostics from a reviewed nested-VMX or bare-metal host.

Do not describe this tree as a production or general-purpose Type-1 hypervisor, full VMI, general direct EPT/NPT/Stage-2 enforcement, syscall-path integrity, hardware PMU sampling, libvirt lifecycle integration, or a finished EDR product. Those claims require runtime code and release evidence that are not present.

Roadmap documents may discuss those targets, but they must keep implemented code, locally observed boot behavior, hardware-proven execution, and production qualification separate.
