# Boot Boundary Runtime

This directory contains the x86_64 entry, linker, host-state, VMX trampoline, and Limine configuration used by the separate `aegishv-type1-kernel` lab target. These are live boot inputs, not future image sketches.

The default `aegishv` binary remains the Linux host-side sensor. Building or booting the lab kernel does not turn that userspace binary into a Type-1 hypervisor.

## Files

- `limine/limine.conf` contains the current Limine menu entry and `boot():` kernel path used by the ISO.
- `linker/x86_64-type1.ld` defines page-separated RX text, R rodata/GOT, and RW request/data/BSS load segments plus a 256 KiB NOLOAD boot stack and exported layout symbols.
- `x86_64/entry.S` disables interrupts, installs the transition IDT, clears BSS, switches to the boot stack, installs owned host tables, and enters Rust.
- `x86_64/host_tables.S` defines the owned GDT, 64-bit TSS, IDT, transition IDT, dedicated fault stacks, selector reload, descriptor verification support, and terminal host exception path.
- `x86_64/vmx_entry.S` defines the non-returning VMLAUNCH/VMRESUME entry points and VM-exit GPR frame on the dedicated host stack. VM exit reloads the owned descriptor tables before Rust dispatch.
- The kernel ELF carries the Limine base-revision block and memory-map, HHDM, executable-address, RSDP, bootloader-info, and command-line requests used by the checked handoff.

## Build And Inspection

`scripts/build-type1-skeleton.sh` validates the boot-handoff crate and writes a review manifest under `target/type1`. That manifest alone is not a boot image or execution evidence.

`scripts/plan-type1-image.sh` validates the checked-in boot inputs and writes the kernel ELF, output ISO, expected bases, and serial contract to `target/type1/aegishv-type1-image-plan.txt`.

`scripts/build-type1-kernel.sh` builds `target/type1/aegishv-type1.elf` for `x86_64-unknown-none`. The ELF validates the Limine handoff, owned host state, CPU capabilities, VMX controls, runtime regions, and explicit error paths. On an eligible Intel host, the runtime proceeds through VMXON, complete VMCS/EPT setup, VMLAUNCH into a fixed `CPUID; HLT` guest, CPUID exit handling, VMRESUME, HLT exit handling, and VMXOFF.

The ELF is not a standalone QEMU boot input. Passing it through QEMU `-kernel` does not provide the required Limine handoff.

`scripts/inspect-type1-kernel.sh` checks the ELF entry, page-separated load layout, request section, boot stack, and required host/runtime/guest marker strings. Artifact inspection proves neither boot nor privileged VMX execution.

`scripts/stage-type1-limine-iso.sh` copies the kernel and Limine configuration into `target/type1/limine-iso-root`.

`scripts/build-type1-limine-iso.sh` uses the Limine command, reviewed Limine files, and xorriso to produce `target/type1/aegishv-type1.iso`. A successful ISO build proves image assembly only.

## Execution Evidence

`scripts/type1-qemu-smoke.sh` accepts the Limine ISO, runs QEMU under a bounded timeout, and requires this complete ordered serial chain by default:

```text
aegishv:type1:host-tables-ok
aegishv:type1:backend-vmx
aegishv:type1:vmxon-cycle-ok
aegishv:type1:vmcs-load-ok
aegishv:type1:guest-config-ok
aegishv:type1:guest-cpuid-exit-ok
aegishv:type1:guest-hlt-exit-ok
aegishv:type1:guest-run-ok
```

Contradictory backends, skipped VMX operations, host faults, runtime failures, guest entry/exit/resume errors, missing handoff, or panic invalidate the run. `scripts/type1-qemu-evidence.sh` records the image digest, environment, command, log, and marker results; `scripts/run-type1-lab.sh` drives the opt-in build and evidence chain.

A modern Limine ISO has booted locally under QEMU TCG through owned host-table installation and runtime preflight. TCG exposed no VMX in the available environment, and WHPX was unavailable, so the observed run followed the non-VMX/skipped path and is not Intel guest-execution evidence.

A valid eight-marker log from a reviewed nested-VMX or bare-metal host would prove only this fixed BSP toy-guest sequence. It would not prove SMP, a general loader, full architectural context, devices/IOMMU isolation, production host paging, AMD/ARM runtime support, hardware soak, or a secure production lifecycle. See `docs/TYPE1_BOOT_BOUNDARY.md` and `docs/TYPE1_READINESS_GATE.md` for the claim boundary.
