# Type-1 Boot Boundary

The repository now contains a bootable x86_64 Type-1 lab kernel and a wired Intel VMX toy-guest path. This is a bring-up milestone, not a production hypervisor and not hardware evidence by itself.

## Implemented Boundary

- Limine base revision 6 handoff with checked HHDM, memory-map, and executable-address responses. The kernel accepts Limine's aligned physical relocation while retaining the fixed higher-half virtual layout.
- A page-separated RX/R/RW ELF layout, current Limine configuration syntax, and an ISO builder gated on reviewed Limine and xorriso inputs.
- An owned GDT, 64-bit TSS, IDT, double-fault IST, NMI IST, 256 KiB boot stack, and VM-exit stack installed before Rust code runs. The kernel verifies the loaded tables and selectors before VMX bring-up.
- Allocation of VMXON, VMCS, guest code/stack/page-table, and EPT pages only from bounded Limine `USABLE` memory. Bootloader-reclaimable memory remains excluded.
- Validation of `IA32_VMX_BASIC`, true control MSRs, CR0/CR4 fixed bits, and required four-level write-back EPT capabilities.
- A complete VMCS for one isolated 64-bit guest. The guest executes `CPUID` followed by `HLT`; the VM-exit trampoline handles CPUID, performs `VMRESUME`, verifies the HLT exit, then executes `VMXOFF`.
- Guest code is RX, stack and page tables are RW/NX, and VMXE is present in hardware guest CR4 only as required by the fixed bits while its guest-visible shadow is clear.
- Strict serial evidence requires the ordered host-table, VMXON, VMCS-load, guest-configuration, CPUID-exit, HLT-exit, and final run markers. Host faults and every guest entry/exit/resume error marker invalidate the run.

The main artifacts are `crates/aegishv-type1-kernel`, `boot/x86_64`, `boot/linker/x86_64-type1.ld`, `boot/limine/limine.conf`, and the `scripts/type1-*` build and evidence helpers.

## Evidence Boundary

Normal Rust tests and the bare-metal build do not execute privileged VMX instructions. A successful ISO build proves only that the image was assembled. A TCG boot can prove the Limine and owned-host-table path, but TCG does not provide VMX. Intel guest execution is established only by a reviewed nested-VMX or bare-metal serial log containing this complete ordered chain:

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

Such a log is not production qualification. It proves one BSP, one VMCS, one CPUID exit, one resume, and one HLT exit on the recorded CPU/QEMU configuration.

## Still Missing

- Per-CPU VMX state, AP startup, APIC/interrupt routing, vCPU scheduling, timers, and preemption.
- General guest loading, multiple address spaces, device emulation, block/network/console backends, and an IOMMU-backed DMA boundary.
- XSAVE/FPU state, MSR and I/O bitmap policy, interrupt injection, broad exit coverage, EPT invalidation, memory overcommit, and recovery from a guest crash.
- Guard pages and a hypervisor-owned host page-table root; the bring-up kernel still uses Limine's mappings.
- Live AMD SVM guest entry, ARM64 EL2 entry, device assignment, suspend/resume, firmware diversity, fuzzed hostile-guest coverage, and long-duration hardware testing.
- Secure boot, measured boot/attestation, signed updates, rollback, crash dumps, operational telemetry, and a supported production lifecycle are not implemented.

## Next Gate

Run `scripts/run-type1-lab.sh` on a reviewed Intel nested-VMX host, retain the generated manifest and serial log, and diagnose any `guest-entry-error` using the captured VM-instruction error before expanding the toy guest. After that, the next engineering gate is per-CPU host state plus interrupt/timer virtualization; a general-purpose guest should not be attempted before those foundations exist.
