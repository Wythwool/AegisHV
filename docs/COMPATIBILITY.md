# Compatibility

## Current implementation status

| Layer | Status |
| --- | --- |
| Linux host + KVM tracefs | Supported as host-side telemetry |
| Replay mode | Supported |
| x86 EPT-like exit parsing | Supported on text trace/replay path |
| AMD NPF-like exit parsing | Supported on text trace/replay path |
| arm64 Stage-2 abort parsing | Supported on text trace/replay path |
| Tracepoint format autodiscovery | KVM `kvm_exit` metadata is checked in live startup diagnostics and snapshots; not wired into binary/perf ingestion |
| VM identity from PID/cmdline/cgroup | Best-effort supported |
| Libvirt API lifecycle discovery | Not implemented |
| QEMU QMP actions | Supported for mapped sockets |
| PMU fallback heartbeat | Supported as host-thread target discovery with unavailable hardware counters reported as `null` |
| Guest memory introspection | Not implemented |
| x86_64 Limine lab boot | Bootable ISO path implemented; locally observed under QEMU TCG through owned descriptor tables and runtime preflight, before the final CR3 path |
| Intel VMX toy-guest runtime | VMXON/VMCS/EPT, a zero-value timer sentinel followed by a nonzero deadline exit from a finite TSC-or-count probe with an HLT timeout fallback, unconditional port-I/O trapping, and bounded resumes through I/O, CPUID, and HLT exits are implemented in the lab kernel; reviewed hardware execution evidence is not present |
| Intel VMX guest execution coverage | Not established; the observed TCG environment exposed no VMX and WHPX was unavailable |
| General VMX/SVM/EL2 backends | Intel has one fixed toy-guest path; AMD SVM has instruction/runtime models; live AMD guest entry and ARM64 EL2 are not implemented |
| EPT/NPT/Stage-2 permission enforcement | A fixed EPT is built for the Intel toy guest; general live permission flips, invalidation, and retrapping are not implemented |
| AMD SVM lab models | Implemented as library models; booted guest execution is not implemented |
| SEV, SEV-ES, SEV-SNP guest inspection | Degraded or unsupported; no bypass is claimed |
| ARM64 EL2 lab models | Implemented as library models; bare-metal execution is not implemented |
| pKVM, Arm CCA, protected guest memory | Degraded or unsupported; no introspection claim is made |

## Guest and platform caveats

- SEV/SEV-ES/SEV-SNP, TDX, VBS/HVCI, pKVM and similar protections can limit or block memory introspection.
- SEV can make guest memory unavailable to host inspection; SEV-ES can make register state unavailable; SEV-SNP adds integrity and isolation checks that must be treated as a boundary. AegisHV does not claim a bypass for these protections.
- pKVM, Arm CCA realms, vendor protected guests, and similar ARM64 protections can make protected guest memory unavailable. AegisHV does not claim introspection for protected guest memory.
- Huge pages, live migration, snapshots, nested virtualization and multi-tenant QMP policies need dedicated test coverage before stronger deployment claims.
- Tracefs text formats depend on kernel tracepoint formatting. Use replay and format autodiscovery tests for every kernel family you support.

The Type-1 lab target is BSP-only and has no compatibility claim for SMP, APIC/interrupt/guest-timer virtualization, scheduler-driven preemption, general guest loading, PAT/XSAVE/FPU context, full MSR state, device emulation, passthrough, or IOMMU isolation. Its VMX preemption timer only bounds the fixed guest's stages. Limine mappings remain active through preflight; only the final Intel path switches to the fixed four-level owned root, so there is no compatibility claim for LA57, dynamic/per-CPU paging, teardown, or guard-fault recovery.

A source build, model test, or TCG boot is not Intel VMX execution coverage. That claim requires matching valid pre/post-run SHA-256 image digests, the complete strict marker chain, and a validated CPU/timer diagnostic set from a recorded nested-VMX or bare-metal configuration. Even that evidence covers only the fixed deadline probe and `AL='A'; OUT 0xE9,AL; CPUID leaf/subleaf 0; HLT` payload and is not production qualification.

Treat this matrix literally. Unsupported means unsupported.

## Snapshot schema notes

Snapshot schema version 2 includes `tracepoints_ok` and `tracepoints` for tracefs metadata diagnostics plus `vm_inventory` for the current identity discovery state. `vm_inventory` reports file-backed or mock lifecycle metadata already known to the identity layer: UUID/name, host task ids, vCPU mappings, QMP socket presence, source/confidence, and bounded conflict state. It does not include raw XML, command lines, socket paths, host paths, or live libvirt freshness claims.

## Event schema notes

Policy action events include structured action audit fields inside the existing `action` object. Identity-enriched events include a nullable `identity` object with bounded source and confidence fields. Loss objects may include `range_kind`, `sequence_gap_start`, and `sequence_gap_end`. Queue drops are reported as `range_kind=aggregate_counter` because the dropped trace lines never received event sequence numbers. Exact sequence gaps are only reported when the emitted event stream itself has a known discontinuity. The event schema version remains `2`; older consumers that ignore unknown optional properties remain compatible with the JSON shape.

## Schema compatibility examples

Current-schema examples live under `schema/examples/`:

- `event-v2-compatibility.jsonl` covers current event schema version 2 shapes for `exit`, `wx`, `pmu`, `policy`, `snapshot`, and `sensor` categories, including action audit, identity metadata, lifecycle, tracefs diagnostic, and loss-range fields.
- `snapshot-v2-inventory.json` covers current snapshot schema version 2 with tracefs diagnostics and bounded VM inventory fields.

The repository validator accepts these examples against the current schemas. That is the compatibility guarantee: current schema files accept these current example shapes. It does not prove support for older schemas, future schemas, external SIEM schemas, OCSF, ECS, OTLP, or every field combination a downstream consumer may accept. Event schema version 2 still allows additional event properties; validation of an extra property is not a promise that AegisHV runtime code emits or interprets it. Snapshot schema version 2 rejects extra top-level fields.
