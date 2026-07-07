# MITRE mapping

This document maps implemented detection records to ATT&CK technique candidates. The mapping is evidence guidance only. It is not a claim that AegisHV proves intent or root cause.

## Implemented mappings

| Detection kind | Technique candidate | Confidence | Reason |
| --- | --- | --- | --- |
| `kernel_text_tamper` | T1014 Rootkit | Medium | Kernel text drift can be consistent with rootkit behavior, but it can also come from legitimate patching or stale baselines. |
| `syscall_hook` | T1014 Rootkit | Medium | Syscall entry or table drift can be consistent with kernel interception. A symbol/profile mismatch can produce the same signal. |
| `hidden_process` | T1014 Rootkit | Low | Inventory mismatch can indicate hiding. Snapshot skew and terminated processes are common false-positive cases. |
| `hidden_module` | T1014 Rootkit | Low | Driver/module inventory mismatch can indicate hiding. Snapshot skew and profile mismatch must be ruled out. |
| `executable_anonymous_memory` | T1055 Process Injection | Low | Anonymous executable memory can occur during injection, unpacking, or JIT compilation. Process context is required before raising confidence. |
| `rwx_mapping` | T1055 Process Injection | Low | RWX memory can be suspicious, but short-lived loader transitions and JIT runtimes can be legitimate. |
| `wx_correlation` | T1055 Process Injection | Low | Write-then-execute correlation can support process-injection triage. It does not prove injection by itself. |

## Unmapped areas

No mapping is provided for detectors that are unsupported for a given input. Unsupported detector outcomes must remain explicit in reports and state.

No mapping is provided for type-1 behavior, direct EPT/NPT enforcement, hardware PMU sampling, or live VMI backend behavior because those paths are not implemented.
