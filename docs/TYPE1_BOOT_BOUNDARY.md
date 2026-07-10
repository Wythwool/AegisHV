# Type-1 Boot Boundary

The repository now contains a bootable x86_64 Type-1 lab kernel and a wired Intel VMX toy-guest path. This is a bring-up milestone, not a production hypervisor and not hardware evidence by itself.

## Implemented Boundary

- Limine base revision 6 handoff with checked HHDM, memory-map, and executable-address responses. The kernel accepts Limine's aligned physical relocation while retaining the fixed higher-half virtual layout.
- A page-separated RX/R/RW ELF layout, current Limine configuration syntax, and an ISO builder gated on reviewed Limine and xorriso inputs.
- An owned GDT, 64-bit TSS, IDT, double-fault IST, NMI IST, machine-check IST, 256 KiB boot stack, and VM-exit stack installed before Rust code runs. The kernel verifies the loaded tables and selectors before VMX bring-up.
- One early allocation ledger is retained from the VMXON/VMCS smoke cycle through guest/EPT and interception setup. Before allocating from bounded Limine `USABLE` memory, it excludes the full linker-owned kernel image and the 4K inherited active CR3 root; the root is checked again before guest setup. The ledger owns fifteen distinct pages below 4 GiB: VMXON, VMCS, ten guest/EPT pages, trap-all I/O A/B bitmap pages, and one fixed MSR bitmap page. Bootloader-reclaimable memory remains excluded.
- After all Limine and HHDM reads/writes, the final Intel path requires NX and four-level paging, sets EFER.NXE and CR0.WP, materializes and reads back a four-page owned hierarchy, flushes inherited global translations, and switches CR3 before VMCS host-state capture. The root maps only the linked 2 MiB higher-half kernel window: text RX, rodata/GOT R/NX, writable state/stacks/table pages RW/NX, with null, HHDM, identity, and five lower stack-guard pages absent.
- Every owned host leaf selects IA32_PAT entry zero. The current entry-zero type must be write-back before CR3 activation, and the captured host PAT is validated again for VM-exit restoration.
- Validation of `IA32_VMX_BASIC`, `IA32_VMX_MISC`, true control MSRs, CR0/CR4 fixed bits, required four-level write-back EPT capabilities, and the CPU signature against the known-broken VMX preemption-timer denylist used by Linux KVM.
- A complete VMCS for one isolated 64-bit guest. Its code begins with a finite TSC-or-count deadline probe and HLT fallback followed by an immediate byte `OUT 0xe9, AL`, a byte `OUT DX, AL` with `DX=0x8000`, CPUID leaf/subleaf 0, trapped `RDMSR IA32_EFER`, direct `RDMSR IA32_PAT`, `FNOP`, `MOVDQA xmm0,xmm0`, and HLT. An initial timer value of exactly zero forces a sentinel exit before the first instruction. The handler derives a reload from a hard `0x01000000`-TSC-tick budget and the `IA32_VMX_MISC` timer-rate field and resumes at the probe. The effective VMX deadline cannot exceed that budget, and a reload below 2 is refused. Only a real nonzero timer exit before the finite fallback moves guest RIP to the payload.
- The VMCS requires `use I/O bitmaps` and `use MSR bitmaps`. It rejects zero, non-4K-aligned, duplicate, and at/above-4-GiB bitmap addresses, and exact live `VMREAD` must recover all three physical addresses before `guest-config-ok` is emitted. Both I/O pages are read back as trap-all. The MSR page starts trap-all, clears exactly the low-read bit for `IA32_PAT`, and is read back against that fixed pattern before the inherited HHDM is removed.
- I/O bitmap A contains the `0xe9` access and bitmap B contains the `0x8000` access. The exit handler accepts only the two expected byte writes in their exact stages and advances guest RIP without issuing a host `OUT`. The high-read MSR quadrant traps `RDMSR IA32_EFER`, and the handler returns synthetic zero without executing that guest request on the host. Direct `RDMSR IA32_PAT` is the sole MSR access allowed to execute in the guest; every MSR write and every other read remains trapped and is rejected outside the fixed EFER stage.
- The VMCS requires PAT capability bits, a deliberate valid guest PAT, the captured host PAT, VM-entry guest-PAT load, VM-exit guest-PAT save, and VM-exit host-PAT load. Exact pre-entry `VMREAD` covers the controls and both PAT fields. Live restored host `IA32_PAT` and the saved guest PAT field must match on every observed exit. The direct guest PAT read must also match at both guard exits before `guest-pat-state-ok` is emitted.
- Guest `CR0.TS=1`, `CR0.EM=0`, and `CR4.OSFXSR=1` are written and read back under the fixed mask/shadow policy. The side-effect-minimized `FNOP` and `MOVDQA`-self instructions must each exit at its exact fault RIP with valid hardware-exception metadata for vector 7, no error code, and TS still set. The handler moves RIP to fixed compile-time continuations; it does not treat `VM_EXIT_INSTRUCTION_LENGTH` as proof for these faults.
- The VM-exit trampoline handles the zero-value sentinel, nonzero probe deadline, I/O-A, I/O-B, CPUID, trapped EFER read, x87 `#NM`, SIMD `#NM`, and HLT exits in that order, with the direct PAT read executing between the EFER and x87 stages. Bounded `VMRESUME` operations separate every exit before `VMXOFF`. An HLT or timer exit at the probe fallback RIP, a later timer expiry, any wrong exception, or any out-of-order exit is terminal.
- Guest code is RX, stack and page tables are RW/NX, and VMXE is present in hardware guest CR4 only as required by the fixed bits while its guest-visible shadow is clear. The ELF inspection gate disassembles host `.text` and rejects FPU/SIMD/state-save instructions; that static host restriction does not implement guest context switching.
- Strict serial evidence requires the ordered host-table, VMX-backend, VMXON, VMCS-load, owned-host-paging, guest-configuration, preemption-exit, I/O-A-exit, I/O-B-exit, CPUID-exit, RDMSR-exit, PAT-state, x87-`#NM`, SIMD-`#NM`, HLT-exit, and final run markers. Paging failures, host faults, `aegishv:type1:guest-timeout`, and every guest entry/exit/resume error marker invalidate the run.

The main artifacts are `crates/aegishv-type1-kernel`, `boot/x86_64`, `boot/linker/x86_64-type1.ld`, `boot/limine/limine.conf`, and the `scripts/type1-*` build and evidence helpers.

## Evidence Boundary

Normal Rust tests and the bare-metal build do not execute privileged VMX instructions. A successful ISO build proves only that the image was assembled. A TCG boot can prove the Limine and owned-descriptor-table path, but TCG does not provide VMX and stops before the final CR3 switch. Owned paging and Intel guest execution are established only by a reviewed nested-VMX or bare-metal evidence package with matching valid pre/post-run SHA-256 image digests and a serial log containing this complete ordered chain:

```text
aegishv:type1:host-tables-ok
aegishv:type1:backend-vmx
aegishv:type1:vmxon-cycle-ok
aegishv:type1:vmcs-load-ok
aegishv:type1:host-paging-ok
aegishv:type1:guest-config-ok
aegishv:type1:guest-preempt-exit-ok
aegishv:type1:guest-io-exit-ok
aegishv:type1:guest-io-b-exit-ok
aegishv:type1:guest-cpuid-exit-ok
aegishv:type1:guest-rdmsr-exit-ok
aegishv:type1:guest-pat-state-ok
aegishv:type1:guest-nm-x87-exit-ok
aegishv:type1:guest-nm-simd-exit-ok
aegishv:type1:guest-hlt-exit-ok
aegishv:type1:guest-run-ok
```

The strict evidence helper also requires exactly one well-formed serial diagnostic set for the CPUID signature, VMX timer rate, reload, and effective TSC-tick deadline. It verifies `reload >= 2`, `effective = reload << rate`, and `effective <= 0x01000000`. These values make the recorded timer configuration auditable but do not expand the sixteen-marker chain, replace the stable image-digest check, or replace hardware review.

Such an evidence package is not production qualification. It proves one BSP, one VMCS, a nonzero timer deadline exit from the finite probe, contained exits through I/O bitmaps A and B, one synthetic high-range RDMSR result, one direct guest PAT comparison plus VMCS PAT save/restore checks, exact `#NM` exits for the fixed `FNOP` and `MOVDQA`-self instructions, bounded resumes, and one HLT exit on the recorded configuration. It does not prove selective port pass-through, general MSR or exception virtualization, FPU/SIMD context management, or WRMSR handling.

## Still Missing

- Per-CPU VMX state, AP startup, APIC/interrupt routing, vCPU scheduling, guest-visible timer virtualization, and scheduler-driven preemption.
- An independent host watchdog for timer failure and qualification beyond the fixed known-broken CPU-signature denylist.
- General guest loading, multiple address spaces, device emulation, block/network/console backends, and an IOMMU-backed DMA boundary.
- XSAVE/FXSAVE, host FPU/SIMD preservation and context switching, lazy or multi-vCPU FPU state, full MSR context, WRMSR handling including WRMSR PAT, MTRR/PAT/MMIO policy, SMP/per-CPU PAT, selective or dynamic I/O/MSR bitmap policy, general exception injection, broad exit coverage, EPT invalidation, memory overcommit, and recovery from a guest crash.
- Owned paging throughout handoff/preflight, dynamic map/unmap and invalidation, per-CPU roots, teardown/reclamation, recoverable guard-fault testing, and a general physical/MMIO mapping policy. Limine mappings remain active until the final Intel-path switch, so the five guards do not protect early boot.
- Live AMD SVM guest entry, ARM64 EL2 entry, device assignment, suspend/resume, firmware diversity, fuzzed hostile-guest coverage, and long-duration hardware testing.
- Secure boot, measured boot/attestation, signed updates, rollback, crash dumps, operational telemetry, and a supported production lifecycle are not implemented.

## Next Gate

Run `scripts/run-type1-lab.sh` on a reviewed Intel nested-VMX host, retain the generated manifest and serial log, and diagnose any `guest-entry-error` using the captured VM-instruction error before expanding the toy guest. After that, the next engineering gate is per-CPU host state plus interrupt/timer virtualization; a general-purpose guest should not be attempted before those foundations exist.
