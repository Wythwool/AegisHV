# VMI safety and consistency

AegisHV has offline VMI infrastructure for tests and developer tooling. It is not a live VMI backend and it does not inspect a running guest. The separate Type-1 lab kernel does not change this boundary: its fixed toy guest and VM-exit frame are not connected to the VMI interfaces.

## Implemented scope

The current VMI code covers:

- typed VMI errors for unsupported backends, invalid addresses, missing memory, translation failures, inconsistent snapshots, unsupported architectures, and permission or availability limits;
- synthetic guest physical memory ranges for tests, including mapped, unmapped, denied, unavailable, invalid, and explicit partial-read cases;
- read-only offline memory snapshot manifests with tiny fixture backing files;
- x86_64 4-level guest virtual to guest physical translation;
- x86_64 LA57 5-level translation;
- ARM64 stage-1 translation for 4 KiB, 16 KiB, and 64 KiB granules;
- translation cache infrastructure with VM, address-space, mode, page, page-size, access, CR3/root, ASID, VMID, and full-flush invalidation rules;
- architecture-neutral register snapshots for x86_64 and ARM64;
- offline register fixture loading;
- OS profile identity and registry types;
- VMI fixture loading for memory, registers, profile identity, and expected translation records;
- offline `vmi translate` CLI for one address from one fixture;
- VMI metrics skeleton counters with bounded labels.

These pieces are offline/testable infrastructure. They are not evidence of live guest inspection.

The bootable x86_64 lab kernel has a wired Intel VMX path for one fixed deadline/I/O/`CPUID`/MSR/PAT/`#NM`/vector-6-injection/`HLT` guest. Its trap-all I/O pages, one-read PAT MSR allowlist, synthetic `IA32_EFER`, deliberate PAT transition, two fixed guard faults, and one immutable CPL0 `#UD` injection/`IRETQ` round trip are not a stable guest-memory reader, a reusable vCPU register source, general MSR or exception virtualization, FPU/SIMD context management, snapshot consistency, OS profiles, or an adapter to `src/vmi.rs`. The available TCG boot did not execute the VMX path, so it is not live VMI evidence either.

Linux profile metadata format notes live in `docs/VMI_LINUX.md`. Windows profile and pre-extracted symbol cache format notes live in `docs/VMI_WINDOWS.md` and `docs/VMI_WINDOWS_SYMBOLS.md`.

## Unsupported scope

- Live VMI backend support is not implemented.
- Live guest memory reads are not implemented.
- Live guest register reads are not implemented.
- Full VMI stack behavior is not implemented.
- Direct EPT/NPT/Stage-2 enforcement is not implemented. The Intel toy guest has a fixed EPT only; it is not a general permission-flip, invalidation, or retrap engine.
- General Type-1 runtime support is not implemented. The separate BSP-only toy-guest target is not wired to the live VMI interfaces.
- Syscall integrity implementation is not present.
- Hardware PMU sampling is not implemented.
- Production guest inspection is not implemented.
- Real Linux or Windows OS profile data is not shipped.

Unsupported operations must return typed errors or explicit degraded state. They must not return success placeholders.

## Snapshot consistency

Offline memory and register fixtures must describe the same guest point-in-time. CR3, TTBR, TCR, page-table memory, and expected translation records are only meaningful when they come from the same snapshot boundary.

Stale memory/register combinations can produce invalid translations. Expected translations are test data, not runtime proof. The fixture loader rejects malformed structure and unsafe paths, but it cannot prove that fixture bytes or register values are semantically true.

## Race limits

Offline fixtures have no live race with a running guest. They can still be stale or internally inconsistent.

Any later live-read path would need a pause, freeze, snapshot, or strict retry and validation policy. Live page-table walks can race CR3 or TTBR changes, page-table writes, TLB state, and guest CPU execution. A live backend would need clear rules for retry, abort, and evidence capture before any stronger claim.

## Encrypted and confidential guests

SEV, SEV-ES, SEV-SNP, TDX, pKVM, CCA-style protection, and similar designs can make guest memory or registers unreadable or unverifiable from the host side. AegisHV does not claim a bypass.

When protection blocks access, the VMI layer must report unsupported, permission denied, temporarily unavailable, missing memory, inconsistent snapshot, or another typed failure. It must not invent bytes or register values.

## Live-read requirements

Before a live VMI path can be claimed, the implementation needs:

- stable VM identity;
- stable vCPU and register snapshot source;
- memory read API with consistent typed error mapping;
- page-table read consistency rules;
- snapshot generation, pause/freeze, or retry policy;
- explicit permission and availability handling;
- architecture-specific profile validation;
- privacy and security handling for evidence, memory bytes, and dumps;
- metrics that do not leak guest addresses, fixture paths, host paths, VM names, kernel builds, secrets, or free-form error detail.

## Translation consistency

x86_64 paging mode selection is explicit: 4-level or LA57. ARM64 mode selection is explicit by granule and stage-1 mode.

CR3, TTBR, TCR, SCTLR, and descriptor memory must match the memory image being walked. Page-table entries and ARM64 descriptors require exact 8-byte reads. Partial reads must fail and must not be decoded from zero-padded buffers.

Unmapped, denied, unavailable, malformed, not-present, and unsupported table memory must fail closed with typed translation errors.

## Profile registry consistency

The OS profile registry starts empty. It does not ship real Linux or Windows profile data.

Profile lookup is exact by OS kind, architecture, kernel or build identity, and variant where present. Duplicate keys are rejected. Missing and unsupported profiles fail explicitly. There is no nearest-match fallback. Synthetic profiles used in tests are not OS support.

## Fixture safety

Fixture files are tiny synthetic inputs. Fixture references must stay relative, portable, and inside the fixture tree. Absolute paths, Windows drive paths, UNC-like paths, path traversal, backslashes, colon-separated paths, host paths, secrets, and real guest dumps are rejected or out of scope.

Zero-valued addresses and registers are valid only when explicitly present. Missing fields are missing; they are not interpreted as zero.

## Cache safety

The translation cache is bounded infrastructure. Keys include enough VM, root, mode, page, page-size, and access state to avoid cross-address-space hits. Cache lookup refuses a GVA outside the key page. Insert rejects mismatched key and value page sizes.

Invalidation by VMID, CR3/root, ASID, and full flush must remove matching entries before a cached translation can be reused. Failed translations are not cached.

## Metrics safety

VMI metrics are skeleton counters for offline/testable paths. They must not imply live backend coverage.

Metric names and labels must stay bounded. They must not expose raw GVA or GPA values, fixture paths, host paths, VM names, kernel build strings, secrets, or arbitrary backend error text.
