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
- Bootable x86_64 Type-1 lab artifacts: a no-std kernel with a modern Limine request block and configuration, validated HHDM/memory-map/executable-address handoff, aligned physical relocation support, a page-separated linker layout, owned GDT/TSS/IDT, guarded stacks, early serial diagnostics, and one bounded early allocation ledger carried from VMX preflight into guest/EPT setup. The ledger excludes the linked kernel image and inherited active CR3 root before allocating from Limine `USABLE` memory. The final Intel path then switches to a live-validated four-page CR3 with only 4K RX/R/RW kernel mappings and five absent guards. Kernel and ISO builders, ELF inspection, tool probing, strict QEMU evidence capture, and an opt-in lab runner are present. A local QEMU TCG run booted the ISO through owned descriptor tables and runtime preflight, but did not expose VMX and therefore did not execute the new CR3 path; normal CI still does not build the ISO or run QEMU.
- Device isolation model code for physical page ownership, huge-page split/merge planning, DMA domains, PCI inventory, ACPI DMAR/IVRS parsing, virtio-mmio state, bounded console queues, read-only block images, and virtio-net quarantine decisions.
- Intel VMX lab models and runtime pieces for feature detection, true-control and fixed-bit validation, VMXON/VMCS region checks, complete minimal host/guest VMCS state, VMX control adjustment, four-level guest paging and EPT, explicit exit handlers, VPID/INVEPT plans, execute/write traps, x86_64 VMX instruction wrappers, and a VMXON/VMCLEAR/VMPTRLD/VMLAUNCH/VMRESUME sequencing layer with an assembly entry/exit trampoline.
- A wired Intel toy guest with a finite TSC-or-count deadline probe and HLT fallback followed by byte `OUT` operations to ports `0xe9` and `0x8000`, CPUID leaf/subleaf 0, trapped `RDMSR IA32_EFER`, direct `RDMSR IA32_PAT`, side-effect-minimized `FNOP` and `MOVDQA`-self probes, and HLT. One allocation ledger reserves the linked kernel image and inherited active CR3 root, then allocates fifteen distinct `USABLE` pages below 4 GiB: VMXON, VMCS, ten guest/EPT pages, trap-all I/O A/B pages, and one fixed MSR page. The MSR page allows exactly the direct PAT read; every write and every other read remains trapped. Bitmap materialization is read back exactly before HHDM removal. The VMCS requires both bitmap controls, rejects invalid or aliased bitmap addresses, and requires exact live bitmap-address and PAT-field readback before `guest-config-ok`.
- The fixed VMCS loads a deliberate valid guest PAT on entry, saves it on exit, and restores the captured host PAT. The live host MSR and saved guest PAT are checked on every exit; the direct-read result is checked at both guard exits. Guest `CR0.TS=1`, `CR0.EM=0`, and `CR4.OSFXSR=1` turn the exact `FNOP` and `MOVDQA`-self stages into validated vector-7 `#NM` exits before execution. The host ELF inspection gate disassembles host `.text` and rejects FPU/SIMD/state-save instructions; this is a fixed build boundary, not context switching. Strict execution evidence requires matching valid pre/post-run SHA-256 image digests, the complete ordered sixteen-marker chain through PAT, x87 `#NM`, SIMD `#NM`, HLT, and completion, plus one internally consistent CPU-signature/timer diagnostic set.
- AMD SVM lab models for feature detection, EFER.SVME value handling, VMCB layout checks, VMRUN/VMLOAD/VMSAVE/INVLPGA instruction facades, x86_64 hardware instruction wrappers, SVM runtime sequencing, explicit intercept handlers, NPT map plans, nested page fault routing, ASID management, execute/write traps, and tiny guest lab validation.
- ARM64 EL2 lab models for capability decoding, vector table validation, 4K Stage-2 map plans, VTCR/VTTBR construction, ESR/FAR/HPFAR abort decode, TLBI planning, HVC/SMC/WFI/WFE traps, execute/write traps, GIC virtualization planning, virtual timer state, and toy guest coverage validation.

## Still missing or unproven

- Reviewed Intel guest-execution evidence is not present. The available QEMU TCG environment boots the Limine/descriptor-table path but does not expose VMX, and WHPX is unavailable, so it cannot prove the final owned-CR3 switch, VMXON, VMLAUNCH, preemption, I/O-A, I/O-B, CPUID, RDMSR, PAT state, x87/SIMD `#NM`, VMRESUME, or HLT exits.
- A general guest loader, reusable VM/vCPU lifecycle, multiple guests, scheduling, and recovery are not implemented; the live Intel path is one fixed BSP-only toy guest.
- The AMD SVM instruction/runtime layer is wired into the type-1 kernel as a checked runtime plan only after a CPUID capability snapshot, EFER.SVME preflight, a controlled EFER.SVME enable write, and HHDM materialization of the VMCB page; the kernel entry path does not execute VMRUN or claim QEMU/hardware evidence.
- Booted guest execution through the AMD SVM lab models is not implemented.
- ARM64 EL2 runtime and vectors.
- Bare-metal execution of the ARM64 EL2 lab models is not implemented.
- SMP/AP startup, per-CPU VMX state, APIC and interrupt routing, guest-timer virtualization, scheduler-driven preemption, and interrupt injection are not implemented. The VMX preemption timer only bounds stages of the fixed Intel toy guest on the BSP.
- There is no independent host watchdog for timer failure. The finite in-guest TSC-or-count fallback prevents this fixed probe from wedging the BSP even if one fallback source stalls, but it is not a general hostile-guest watchdog. The known-broken CPU-signature denylist cannot cover unknown errata, so hardware evidence and broader CPU/firmware qualification remain required.
- The deliberate PAT transition and two `#NM` probes are narrow fixed-guest checks, not general architectural context support. XSAVE/FXSAVE, host FPU/SIMD preservation and context switching, lazy or multi-vCPU state, WRMSR PAT, MTRR/PAT/MMIO policy, SMP/per-CPU PAT, comprehensive stateful MSR handling, selective/dynamic bitmap policy, general exception injection, broad exit coverage, and hostile-guest recovery are not implemented.
- Live device assignment, SMMU/VT-d/AMD-Vi programming, virtual switch enforcement, and SR-IOV quarantine are not implemented.
- Guest physical memory reader.
- Guest virtual-to-physical translation.
- vCPU register reader from a real backend.
- Real Linux/Windows guest OS profile extraction and live profile distribution.
- Live Windows guest process/module/syscall/callback reads are not implemented.
- Runtime syscall-path integrity monitoring is not implemented.
- Runtime detector engine integration is not implemented; the detector layer is currently a library surface with tests.
- General direct hardware EPT/NPT/Stage-2 permission flips, real TLB invalidation, and real single-step/retrap execution are not implemented. The Intel toy guest has a fixed EPT, while the general trap engine remains a synthetic model with tests.
- The final BSP Intel path has a bounded owned CR3, but early handoff/preflight still uses Limine mappings. Dynamic/per-CPU roots, a general direct-map/MMIO policy, invalidation, teardown/reclamation, guard-fault recovery tests, and hardware execution evidence remain missing.
- Libvirt API integration with lifecycle events is not implemented.
- True PMU grouped-counter/ring-buffer sampling with PEBS/IBS/SPE semantics is not implemented.
- Remote management service, multi-user authentication backend, online policy update service, and hardware attestation are not implemented.
- Hardware soak, broad CPU/firmware qualification, secure/measured boot, signed and rollback-safe hypervisor updates, and a production incident/crash lifecycle are not implemented.
- OTLP runtime export is not implemented. `docs/EVENT_EXPORT.md` is design-only.
- OCSF and ECS runtime output is not implemented. `docs/EVENT_MAPPINGS.md` is mapping guidance only.

## Claim discipline

Describe the default `aegishv` binary as a Linux host-side KVM telemetry sensor. It reads tracefs text, emits JSONL events, exposes metrics, correlates W^X patterns, and can call configured QMP actions.

The separate no-std target may be described as a bootable x86_64 Type-1 lab kernel with Intel VMX toy-guest and final owned-CR3 paths wired in code. The local TCG run may be cited only as Limine, owned-descriptor-table, and preflight boot evidence. Do not claim demonstrated owned paging or Intel guest execution without the full strict marker chain and validated CPU/timer diagnostics from a reviewed nested-VMX or bare-metal host.

Do not describe this tree as a production or general-purpose Type-1 hypervisor, full VMI, general direct EPT/NPT/Stage-2 enforcement, syscall-path integrity, hardware PMU sampling, libvirt lifecycle integration, or a finished EDR product. Those claims require runtime code and release evidence that are not present.

Roadmap documents may discuss those targets, but they must keep implemented code, locally observed boot behavior, hardware-proven execution, and production qualification separate.
