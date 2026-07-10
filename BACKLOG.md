# Backlog

This backlog tracks planned work after AegisHV 0.4.0. The default runtime remains a Linux userspace KVM telemetry sensor; the repository also has a separate bootable x86_64 lab kernel with a BSP-only Intel VMX toy-guest path.

Creator: https://github.com/Wythwool  
Organization: https://github.com/Nullbit1

No item below is marked complete. An item may move out of open status only when the code, tests, docs, and release evidence satisfy its acceptance criteria and production gate.

## Phase 0 - Host Sensor Cleanup

### B001 - Replay and tracefs ingestion contract

- Status: open
- Scope: Keep live tracefs and replay ingestion behavior explicit, bounded, and measurable.
- Acceptance criteria: replay EOF, collector errors, unsupported tracepoints, malformed `kvm_exit` lines, queue pressure, and JSON write failures have tests that preserve current event semantics.
- Production gate: a release candidate passes locked tests and replay smoke with `examples/traces/kvm_exit_sample.log` on a clean checkout.

### B002 - Event contract audit

- Status: open
- Scope: Audit emitted JSONL fields, schema versions, monotonic timestamps, identity fields, loss metadata, and policy fields.
- Acceptance criteria: schema validation covers representative `exit`, `wx`, `pmu`, `policy`, and `sensor` events, including negative cases for malformed output.
- Production gate: event schemas, examples, compatibility notes, and replay validation agree on the same public contract.

### B003 - Operator failure messages

- Status: open
- Scope: Replace vague startup, config, tracefs, QMP, and output errors with typed errors that tell an operator what to fix.
- Acceptance criteria: failure tests cover bad config, missing tracefs paths, unwritable output, unsupported action configuration, and invalid QMP socket paths.
- Production gate: no supported CLI path panics on ordinary operator input.

### B004 - Metrics and health contract

- Status: open
- Scope: Define stable metric names, health/readiness states, and counter behavior for ingest, parse, policy, QMP, output, and PMU fallback paths.
- Acceptance criteria: tests cover successful metrics encoding, degraded readiness, unavailable PMU counters as `null`, and queue-loss metrics.
- Production gate: dashboards and alerts can be pinned to documented metric names for one minor release line.

### B005 - Packaging and service hardening

- Status: open
- Scope: Tighten systemd, Docker, filesystem permissions, tracefs access, log paths, and release package contents.
- Acceptance criteria: package tests confirm required files, service flags, writable directories, and absence of generated local artifacts.
- Production gate: release artifacts install, start, stop, and uninstall cleanly on documented Linux targets.

## Phase 1 - VMI Foundation

### B006 - Backend capability model

- Status: open
- Scope: Extend backend contracts so each backend reports exact supported capabilities and refusal reasons.
- Acceptance criteria: tests cover no-backend refusal, partial capability sets, ambiguous capability requests, and operator-facing error text.
- Production gate: unsupported VMI, trap, and type-1 paths cannot be reported as accepted work.

### B007 - Guest physical memory reads

- Status: open
- Scope: Add a tested interface for bounded guest physical memory reads through a real or simulated backend.
- Acceptance criteria: tests cover valid reads, out-of-range reads, short reads, backend refusal, and unmapped memory.
- Production gate: every memory-read result carries backend identity, address range, and a typed failure reason when data is unavailable.

### B008 - vCPU register reads

- Status: open
- Scope: Add a backend register-read contract for guest general-purpose, control, model-specific, and architecture-specific registers.
- Acceptance criteria: tests cover valid reads, unavailable registers, paused/running vCPU state conflicts, and unsupported architecture requests.
- Production gate: register data is never synthesized when a backend cannot provide it.

### B009 - Guest virtual address translation

- Status: open
- Scope: Implement guest virtual-to-physical translation with architecture-specific page-table walks behind the VMI contract.
- Acceptance criteria: tests cover 4 KiB pages, huge pages, not-present entries, permission bits, reserved bits, and malformed page tables.
- Production gate: translation results include permissions and fault reasons that match the backend architecture.

### B010 - Guest OS profiles and symbols

- Status: open
- Scope: Add Linux-first OS profile loading for kernel layout, modules, syscall tables, and symbol resolution.
- Acceptance criteria: tests cover profile version matching, missing symbols, duplicate symbols, untrusted profile input, and profile refusal.
- Production gate: symbol attribution is disabled unless the loaded profile is version-matched and integrity-checked.

## Phase 2 - Trap Engine

### B011 - Trap lifecycle state machine

- Status: open
- Scope: Define trap install, arm, trigger, single-step, rearm, disarm, and failure states without changing current tracefs behavior.
- Acceptance criteria: tests cover normal lifecycle, duplicate trap requests, backend refusal, guest race conditions, and cleanup after failure.
- Production gate: trap state cannot silently drift from backend permission state.

### B012 - Stage-2 permission manager

- Status: open
- Scope: Add a backend-agnostic interface for EPT, NPT, and Stage-2 permission updates.
- Acceptance criteria: tests cover read/write/execute transitions, huge-page split requirements, invalid ranges, TLB invalidation requests, and rollback on failure.
- Production gate: permission changes are confirmed by backend state or reported as refused.

### B013 - Trap storm control

- Status: open
- Scope: Bound trap rates per VM, vCPU, page, reason, and policy rule so a noisy guest cannot starve the sensor.
- Acceptance criteria: tests cover rate limits, cooldowns, allowlisted JIT-like patterns, queue pressure, and degraded telemetry.
- Production gate: storm control emits metrics and events without blocking hot paths.

### B014 - Syscall-path integrity checks

- Status: open
- Scope: Validate guest syscall entry points, syscall tables, kernel text, modules, ftrace, kprobe, and eBPF surfaces through VMI.
- Acceptance criteria: tests cover clean profiles, tampered pointers, missing symbols, unsupported guests, and ambiguous attribution.
- Production gate: syscall findings are disabled unless VMI reads, translation, and symbol profiles are all verified.

## Phase 3 - Type-1 CPU Backends

### B015 - Intel VMX backend

- Status: partial; fixed BSP toy-guest entry is implemented, general runtime and hardware evidence remain open.
- Scope: Evolve the VMXON, complete VMCS/EPT, VMLAUNCH, CPUID-exit, VMRESUME, HLT-exit, and VMXOFF bring-up path into per-CPU VMX with general exits, VPID, INVEPT, scheduling, and recovery.
- Acceptance criteria: existing model/build tests cover capability checks, control fields, VMCS validation, EPT, exit decoding, and teardown; retained nested-VMX or bare-metal evidence and hostile-guest negative tests are still required.
- Production gate: bare-metal Intel hardware tests pass without claiming support on unsupported CPUs.

### B016 - AMD SVM backend

- Status: open
- Scope: Implement AMD SVM support for VMCB, VMRUN, intercept vectors, NPT, ASIDs, INVLPGA, MSRPM, and IOPM.
- Acceptance criteria: tests cover SVM capability checks, VMCB validation, NPT faults, intercept decoding, nested refusal, and teardown.
- Production gate: bare-metal AMD hardware tests pass without claiming support on unsupported CPUs.

### B017 - ARM64 EL2 backend

- Status: open
- Scope: Implement ARM64 EL2 entry, vectors, HCR_EL2, VTCR_EL2, VTTBR_EL2, Stage-2 tables, timer, and GIC virtualization boundaries.
- Acceptance criteria: tests cover EL2 capability checks, Stage-2 faults, exception decoding, interrupt routing, unsupported host refusal, and teardown.
- Production gate: arm64 hardware tests pass on documented platforms without claiming support elsewhere.

## Phase 4 - Isolation, IOMMU, and Devices

### B018 - IOMMU isolation

- Status: open
- Scope: Add DMA isolation requirements and interfaces for VT-d, AMD-Vi, and SMMU before any device passthrough claim.
- Acceptance criteria: tests cover missing IOMMU, unsafe device assignment, domain teardown, DMA fault reporting, and refusal paths.
- Production gate: passthrough stays disabled unless IOMMU isolation is verified.

### B019 - Interrupt and timer model

- Status: open
- Scope: Define interrupt routing, timer virtualization, vCPU wakeups, and host signal handling for a type-1 runtime.
- Acceptance criteria: tests cover timer injection, interrupt masking, pending interrupt delivery, lost-interrupt detection, and shutdown.
- Production gate: guests continue making forward progress under interrupt load in hardware tests.

### B020 - Device emulation boundary

- Status: open
- Scope: Define which devices are emulated, passed through, or refused by the runtime.
- Acceptance criteria: tests cover unsupported devices, unsafe passthrough requests, reset handling, MMIO exits, and clean refusal.
- Production gate: device support is documented per device and disabled by default until tested on hardware.

## Phase 5 - Management Plane

### B021 - VM lifecycle management

- Status: open
- Scope: Add explicit VM create, start, stop, pause, resume, snapshot, and destroy lifecycle contracts.
- Acceptance criteria: tests cover invalid state transitions, partial failure cleanup, idempotent stop, identity collisions, and audit records.
- Production gate: management operations are transactional enough to recover after process restart or host reboot.

### B022 - Libvirt and QMP integration boundary

- Status: open
- Scope: Replace best-effort discovery with explicit libvirt lifecycle integration and audited QMP socket mapping.
- Acceptance criteria: tests cover ambiguous identity, stale sockets, permission errors, VM rename, UUID mismatch, and QMP refusal.
- Production gate: actions use stable VM identity before any name fallback.

### B023 - Policy and audit log management

- Status: open
- Scope: Add signed policy bundles, policy versioning, dry-run review, audit logs, and rollback semantics.
- Acceptance criteria: tests cover invalid signatures, downgrade attempts, conflicting rules, rollback after failure, and audit integrity.
- Production gate: policy changes are attributable, reversible, and blocked when integrity checks fail.

## Phase 6 - Release and Hardware Tests

### B024 - Release qualification

- Status: open
- Scope: Define release blocking checks for lockfile, fmt, clippy, tests, replay smoke, schema validation, package contents, and docs.
- Acceptance criteria: release scripts fail on missing artifacts, dirty generated output, schema drift, missing licenses, or undocumented operator changes.
- Production gate: each release candidate has reproducible artifacts and recorded command output.

### B025 - Hardware test matrix

- Status: open
- Scope: Build a repeatable hardware test matrix for Intel, AMD, and arm64 hosts with documented CPU, firmware, kernel, and mitigation settings.
- Acceptance criteria: tests cover boot, VM lifecycle, exits, memory permissions, interrupts, IOMMU behavior, recovery, and unsupported hardware refusal.
- Production gate: hardware results are recorded from real machines, not inferred from mocks.

### B026 - Crash recovery and update safety

- Status: open
- Scope: Define crash recovery, state cleanup, log durability, rollback, and update safety for host sensor and backend runtimes.
- Acceptance criteria: tests cover interrupted writes, process crash, backend restart, host reboot, stale locks, and corrupted state.
- Production gate: recovery paths leave VMs in a documented state and do not report success before cleanup is complete.
