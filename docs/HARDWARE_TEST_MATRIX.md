# Hardware Test Matrix

This matrix separates checked paths, locally observed bring-up, implemented-but-unproven runtime paths, planned coverage, degraded cases, and unsupported cases. It is a release gate: README and release notes must not claim coverage that is missing here.

## Status Terms

- `checked`: covered by normal locked tests or reviewable committed evidence.
- `observed`: completed in a local run, but not yet backed by a reviewable committed evidence package.
- `implemented`: runtime code exists and is covered by build/model checks, but the required hardware execution evidence is absent.
- `planned`: designed but not implemented or checked to the required level.
- `degraded`: expected to run with reduced visibility or reduced action scope.
- `unsupported`: must fail with a clear error or documented refusal.

## CPU And Hypervisor Paths

| Area | Status | Current Evidence | Release Gate |
| --- | --- | --- | --- |
| Linux tracefs KVM text replay | checked | `scripts/smoke-replay.sh`, golden JSONL tests | Required for host sensor release |
| Live Linux tracefs KVM smoke | planned | `scripts/live-tracefs-smoke.sh`, `scripts/live-kvm-integration.sh` | Manual runner evidence required before broad live claims |
| Intel VMX model and VMCS/EPT construction tests | checked | `aegishv-arch-x86::vmx` tests and bare-metal kernel build checks | Model/build wording required |
| x86_64 Limine ISO boot through owned descriptor tables and preflight | observed | Local modern-Limine ISO boot under QEMU TCG; VMX unavailable | Retain a reviewable manifest and serial log before moving to `checked` |
| Intel final-path owned host paging | implemented | Four-page root, 4K RX/R/RW leaves, NXE/WP/CR3/live-table checks, no HHDM/identity alias, and five guards are wired and build-tested | `host-paging-ok` plus complete hardware chain required |
| Intel VMX toy-guest runtime | implemented | One ledger owns 15 distinct pages including trap-all I/O A/B/MSR pages; the VMCS requires both bitmap controls and exact address readback; bounded I/O-A, I/O-B, CPUID, synthetic RDMSR, and HLT exits are wired in code | Matching valid pre/post-run SHA-256 image digests, complete thirteen-marker evidence, and validated CPU/timer diagnostics required |
| Intel VMX guest execution | planned | No reviewed nested-VMX or bare-metal guest-execution log; TCG does not expose VMX and WHPX is unavailable | Must not be claimed as demonstrated |
| AMD SVM model tests | checked | `aegishv-arch-x86::svm` unit tests | Lab-model wording required |
| AMD SVM live guest path | unsupported | No VMRUN-backed guest execution path | Must not be claimed |
| ARM64 EL2 model tests | checked | `aegishv-arch-arm64` unit tests | Lab-model wording required |
| ARM64 EL2 live guest path | unsupported | No live EL2 entry or guest execution path | Must not be claimed |
| Bare-metal physical-host qualification | planned | No retained hardware boot/guest evidence | Required before hardware coverage claims |
| General direct EPT/NPT/Stage-2 enforcement | unsupported | Intel toy EPT exists; general trap runtime remains synthetic | Must not be claimed |
| Hardware PMU sampling | unsupported | Grouped/ring models only | Must not be claimed |

## Guest And Tooling Paths

| Area | Status | Current Evidence | Release Gate |
| --- | --- | --- | --- |
| Fixed Intel deadline-probe/I/O-A/I/O-B/`CPUID`/`RDMSR`/`HLT` toy guest | implemented | Guest bytes, finite timeout fallback, four-level guest paging, EPT, VMCS bitmap fields, two suppressed port exits, synthetic-zero RDMSR, and ordered exit state machine build and test | Hardware marker chain required before execution claim |
| General guest loader and lifecycle | unsupported | No kernel/module loader or production vCPU lifecycle | Must not be claimed |
| Offline x86_64 VMI translation fixtures | checked | `vmi_cli_tests`, VMI fixture tests | Fixture-only wording required |
| Offline ARM64 VMI translation fixtures | checked | `vmi_cli_tests`, VMI fixture tests | Fixture-only wording required |
| Linux synthetic VMI profile corpus | checked | `tests/fixtures/vmi/linux` | Synthetic-only wording required |
| Windows synthetic VMI profile corpus | checked | `tests/fixtures/vmi/windows` | Synthetic-only wording required |
| Real Linux guest profile extraction | unsupported | No extractor | Must not be claimed |
| Real Windows symbol download or PDB parsing | unsupported | Pre-extracted synthetic cache only | Must not be claimed |
| QEMU/QMP action dry-run | checked | `management_security_tests` | Stable identity gate remains required |
| Libvirt lifecycle integration | unsupported | File-backed XML discovery only | Must not be claimed |

## Platform Inputs

| Input | Status | Notes |
| --- | --- | --- |
| QEMU TCG boot coverage | observed | One local modern-Limine ISO boot reached owned descriptor tables and runtime preflight; TCG supplied no VMX and did not reach the final owned root. |
| Nested-VMX QEMU/KVM coverage | planned | Record exact host CPU, kernel, KVM, QEMU, machine, CPU model, and marker log. |
| WHPX coverage | unsupported | WHPX was unavailable in the current environment. |
| Libvirt version coverage | planned | Record exact version when lifecycle work is added. |
| Firmware and physical boot chain | planned | Required for bare-metal qualification. |
| Confidential guest modes | degraded | SEV, SNP, TDX, pKVM, and CCA-style protection can block reads. |

## Still Missing For Production Coverage

The following coverage is still missing: owned paging throughout early boot, dynamic/per-CPU roots and invalidation, teardown/recovery, executed guard-fault evidence, SMP/per-CPU VMX, APIC/interrupt/guest-timer virtualization, devices/IOMMU isolation, a general guest loader, PAT/XSAVE/FPU/full stateful MSR context, WRMSR and selective/dynamic bitmap policy, live AMD/ARM paths, hardware soak, and the secure update/attestation/incident-response lifecycle. These remain release blockers for production wording.

Any row moved from `planned`, `observed`, or `implemented` to `checked` needs a script, test, or attached lab log that a reviewer can reproduce.
