# Threat model

## Good fit

- KVM hosts where tracefs is available and the operator accepts host-side sensor limits.
- Detection pipelines that want host-side signals without guest agents.
- W^X-like patterns, suspicious exit activity, and policy-driven QMP response.

## Weak fit

- Guests protected by technologies that intentionally hide guest memory from the host-side sensor layer.
- Environments that need per-process, per-module attribution without a VMI layer.
- Claims of full syscall-path integrity without guest memory and symbol access.

## Honest limits

This code does not yet implement full VMI, a trap-based execution engine, or a true type-1 hypervisor core. It should be described as a hardened KVM host-side sensor with a roadmap, not as a finished VMX/SVM/EL2 EDR hypervisor.
