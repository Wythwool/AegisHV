# x86_64 kernel

The kernel boots with Limine base revision 6. It installs GDT/TSS/IDT, fault
stacks, a 256 KiB boot stack, and a VM-exit stack before entering Rust. The
Intel path owns one BSP and one fixed 64-bit guest. The userspace sensor does
not use this path.

## Build and boot

```sh
bash scripts/check-type1-lab-tools.sh
bash scripts/build-type1-kernel.sh
```

For an ISO, set `AEGISHV_LIMINE_DIR` to a local Limine distribution and install
xorriso. Then:

```sh
bash scripts/build-type1-limine-iso.sh --build-kernel
AEGISHV_RUN_TYPE1_LAB=1 scripts/run-type1-lab.sh --skip-build-kernel --timeout 20
```

The runner uses `scripts/type1-qemu-evidence.sh` and writes logs/manifests under
`target/type1`. Check its `--help` for alternate output paths and expected markers.
Keep the default strict markers for a VMX run.

## Guest path

One allocation ledger owns fifteen pages below 4 GiB: VMXON, VMCS, ten guest/EPT
pages, I/O bitmap A/B, and one MSR bitmap. The linked kernel and inherited CR3
root are reserved before allocation. After all HHDM reads and writes finish,
the Intel path switches to a checked four-page host hierarchy over the linked
2 MiB kernel window. Permissions are RX/R/RW with NX where appropriate and
five unmapped lower stack guards.

Guest execution starts with a zero-timer sentinel and a finite deadline probe.
Only a nonzero VMX timer exit before fallback permits the payload. Reload is at
least two, and `reload << rate` cannot exceed `0x01000000` TSC ticks. Known
broken CPU signatures are rejected.

The payload writes to ports `0xe9` and `0x8000`, executes CPUID, reads EFER
(answered with zero), and directly reads PAT. Port requests are not replayed
on the host. The MSR bitmap traps every write and every read except PAT.
VMCS PAT load/save controls and host/guest values are checked on every exit.

`FNOP` and `MOVDQA xmm0,xmm0` must each exit as #NM with TS set. The handler
then injects one #UD through a fixed CPL0 IDT gate. An integer-only handler
advances the UD2 frame and returns by IRETQ to the final HLT. The exit handler
checks RIP, stack, cookie, and delivery state. Any unexpected exit is terminal.

## Run evidence

TCG has reached descriptor setup and preflight. It did not expose VMX, so there
is no successful hardware guest run recorded here. Compilation and a bootable
ISO alone do not establish guest execution.

A strict run needs matching valid SHA-256 image digests before and after QEMU,
one valid CPU/timer diagnostic set, and these serial markers exactly once in
order:

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
aegishv:type1:guest-ud-inject-ok
aegishv:type1:guest-hlt-exit-ok
aegishv:type1:guest-run-ok
```

Host faults, paging failures, timeouts, guest entry/exit/resume errors, or invalid
timer diagnostics fail validation even if some success markers appeared.

## Missing pieces

The current guest is fixed. General loading, SMP/per-CPU state, APIC and guest
timers, scheduler preemption, an independent watchdog, dynamic map/unmap with
TLB invalidation, device backends, and IOMMU isolation are unfinished.

There is no XSAVE/FXSAVE context management, general MSR/WRMSR virtualization,
MTRR/PAT/MMIO policy, or general exception delivery. The fixed #UD test does
not cover error-code exceptions, nested faults, privilege/IST transitions, or
external interrupts. AMD SVM and ARM64 entry are not wired. Hardware soak,
measured boot, and secure update support are also absent.
