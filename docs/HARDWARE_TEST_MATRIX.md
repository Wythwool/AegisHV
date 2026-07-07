# Hardware Test Matrix

This matrix separates checked paths, planned lab paths, degraded cases, and unsupported cases. It is a release gate: README and release notes must not claim coverage that is missing here.

## Status Terms

- `checked`: covered by normal locked tests or an opt-in script that has committed evidence.
- `planned`: designed but not checked by committed evidence.
- `degraded`: expected to run with reduced visibility or reduced action scope.
- `unsupported`: must fail with a clear error or documented refusal.

## CPU And Hypervisor Paths

| Area | Status | Current Evidence | Release Gate |
| --- | --- | --- | --- |
| Linux tracefs KVM text replay | checked | `scripts/smoke-replay.sh`, golden JSONL tests | Required for host sensor release |
| Live Linux tracefs KVM smoke | planned | `scripts/live-tracefs-smoke.sh`, `scripts/live-kvm-integration.sh` | Manual runner evidence required before broad live claims |
| Intel VMX model tests | checked | `aegishv-arch-x86::vmx` unit tests | Lab-only wording required |
| AMD SVM model tests | checked | `aegishv-arch-x86::svm` unit tests | Lab-only wording required |
| ARM64 EL2 model tests | checked | `aegishv-arch-arm64` unit tests | Lab-only wording required |
| Bare-metal type-1 boot | unsupported | boot boundary skeleton only; no boot image | Must not be claimed |
| Direct EPT/NPT/Stage-2 enforcement | unsupported | synthetic trap model only | Must not be claimed |
| Hardware PMU sampling | unsupported | grouped/ring models only | Must not be claimed |

## Guest And Tooling Paths

| Area | Status | Current Evidence | Release Gate |
| --- | --- | --- | --- |
| Offline x86_64 VMI translation fixtures | checked | `vmi_cli_tests`, VMI fixture tests | Fixture-only wording required |
| Offline ARM64 VMI translation fixtures | checked | `vmi_cli_tests`, VMI fixture tests | Fixture-only wording required |
| Linux synthetic VMI profile corpus | checked | `tests/fixtures/vmi/linux` | Synthetic-only wording required |
| Windows synthetic VMI profile corpus | checked | `tests/fixtures/vmi/windows` | Synthetic-only wording required |
| Real Linux guest profile extraction | unsupported | no extractor | Must not be claimed |
| Real Windows symbol download or PDB parsing | unsupported | pre-extracted synthetic cache only | Must not be claimed |
| QEMU/QMP action dry-run | checked | `management_security_tests` | Stable identity gate remains required |
| Libvirt lifecycle integration | unsupported | file-backed XML discovery only | Must not be claimed |

## Platform Inputs

| Input | Status | Notes |
| --- | --- | --- |
| QEMU version coverage | planned | Record exact version in live test logs. |
| Libvirt version coverage | planned | Record exact version when lifecycle work is added. |
| Firmware and boot chain | planned | Required for any type-1 lab evidence. |
| Confidential guest modes | degraded | SEV, SNP, TDX, pKVM, and CCA-style protection can block reads. |

Any row moved from planned to checked needs a script, test, or attached lab log that a reviewer can reproduce.
