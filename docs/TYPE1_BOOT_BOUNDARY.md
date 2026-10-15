# Kernels

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

Intel's full guest sequence runs in Bochs 3.1 with VMX enabled. The `OS boot`
workflow builds the pinned emulator and runs `scripts/test-type1-vmx.py` on
Linux. This caught a missing CR0.ET bit in the guest configuration.

The same sequence passed with nested KVM on an
[Intel Xeon Platinum 8573C](https://github.com/Wythwool/AegisHV/actions/runs/38596593726),
using Ubuntu 22.04 and QEMU 6.2.0 at commit `ee6c7e0`. KVM rejected an unnecessary
SMBASE write during setup; the normal guest no longer writes that SMM field.
The retained artifact includes the CPU report, serial log, ISO and matching
before/after SHA256 digests. This is nested VMX, not a direct bare-metal boot.

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

The Intel guest is fixed. x86 OS loading, SMP/per-CPU state, APIC and guest
timers, an independent watchdog, dynamic map/unmap with
TLB invalidation and device backends are unfinished. [VT-d quarantine](IOMMU.md)
is active on supported x86 machines; general device assignment is unfinished.
The separate [ARM64 Linux mode](LINUX_GUEST.md) supports OS boot and SMP.

The Intel path has no XSAVE/FXSAVE context management or general MSR/WRMSR virtualization,
MTRR/PAT/MMIO policy, or general exception delivery. The fixed #UD test does
not cover error-code exceptions, nested faults, privilege/IST transitions, or
external interrupts. Hardware soak,
measured boot, and secure update support are also absent.

## AMD and Arm

The SVM path uses a separate guest VMCB, host-save page, host VMCB, NPT root,
I/O/MSR permission maps and FXSAVE areas. Guest RAM is the only memory exposed
through NPT. The host drops the bootloader mappings before VMRUN. A guest
round trip checks GPR and SIMD state, LSTAR interception, code write denial,
and data execute denial. Invalid or unexpected exits stop the guest.

The ARM64 image loads at `0x40080000` on QEMU `virt`. It enters EL1 through
ERET with Stage-2 enabled, saves GPR/FP/SIMD state on an EL2 exception, checks
the guest vector address on HVC, and handles actual instruction/data aborts.
Its PMU sample is the guest cycle-counter delta across the run. EL2 installs
its own Stage-1 tables with RX text, read-only constants,
RW/NX data and an unmapped stack guard. Only the linked kernel, PL011 and the two
GIC interfaces are mapped. Boot checks read/write translations with AT S1E2R/W
before entering the guest.

```sh
rustup target add aarch64-unknown-none
bash scripts/build-type1-arm64.sh
python3 scripts/test-type1-boot.py el2 target/type1/aegishv-el2.elf
python3 scripts/test-type1-boot.py svm target/type1/aegishv-type1.iso
```

The `boot` workflow runs these guests under Linux QEMU TCG and keeps serial
logs and image hashes. These are bounded one-vCPU execution tests, not full
OS boots. Windows QEMU 10.1 truncates high segment bases in VMLOAD; the host
state check rejects that build rather than continuing with a damaged TSS.

The seven SVM cases also passed with nested KVM on AMD EPYC 7763, on both
[Ubuntu 26.04](https://github.com/Wythwool/AegisHV/actions/runs/38478722179) and
[Ubuntu 24.04](https://github.com/Wythwool/AegisHV/actions/runs/38478725524).
These runs used hardware SVM through KVM, not a direct boot on a physical machine.
The manual `nested KVM` workflow records the host CPU and selects SVM or VMX from
its advertised features. A host without nested virtualization fails preflight.

## ELF guests

SVM and EL2 accept a static ELF64 image through `AEGISHV_GUEST_ELF` at build time.
PT_LOAD addresses must be identity linked, page aligned and non-overlapping.
The loader rejects writable executable pages, dynamic linking, wrong machine
types and entries outside file-backed executable code. It validates the whole
image before changing RAM, copies data and clears BSS.

The SVM allocation is 128 KiB. Image pages occupy GPA `0x1000..0x18000`, followed
by a 16 KiB stack and four guest page-table pages. CR writes are intercepted;
CR0.WP and read-only guest mappings protect the page tables while NPT permits
hardware A/D updates. The EL2 image range is `0x40000000..0x4001c000`, followed
by a 16 KiB stack. EL1 starts with its MMU disabled; TVM prevents the guest from
changing that address space. Unused image pages and host memory remain unmapped.

SVM uses the legacy PIC/PIT on the BSP, disables the inherited LAPIC and sets
V_INTR_MASKING. EL2 uses CNTHP and GICv2 PPI 26 on QEMU `virt` without security
extensions. Both stop after 20 timer ticks (about 200 ms), even with guest IRQs
masked. A separate 4096-exit limit bounds exit storms. These are fixed run
budgets; there is no multi-vCPU scheduler yet.

```sh
python3 scripts/build-type1-guest.py arm64 return
AEGISHV_GUEST_ELF="$PWD/target/type1/guest-arm64-return.elf" bash scripts/build-type1-arm64.sh
python3 scripts/test-type1-boot.py el2 target/type1/aegishv-el2.elf --guest return
```

Use `x86_64` and `scripts/build-type1-limine-iso.sh --build-kernel` for SVM.
An unset variable builds the instruction probes. The external examples cover
a computed result, an IRQ-masked loop, code writes, data execution and out-of-range
reads. `boot.yml` runs all seven cases per backend and retains their logs and images.
EL1 guests return in x0 with `HVC #1`; SVM guests return in RAX with HLT. The
examples have no OS runtime or devices. The `syscall` case also checks rejection
of LSTAR/VBAR targets in a writable page. ARM checks VBAR on each exit; it does
not trap the VBAR write itself on Cortex-A57. A transient change between exits
can escape that check. SVM intercepts LSTAR writes before applying them.

Linux boot CI uses Ubuntu 26.04. QEMU 8.2 drops PIC delivery after the inherited
LAPIC is disabled with LINT0 masked, so the IRQ-masked SVM loop hangs there.
Do not remove the timer probe to make that version pass.

## Serial collection

`tools/type1_events.py` runs outside the guest and needs only Python 3.10 or newer.
It filters boot messages, validates kernel JSON, tracks missing sequence numbers
and serves `/metrics`, `/healthz` and `/readyz` on the requested address.
Sequence 1 begins a new boot. RIP and guest addresses are kept in JSON and never
used as metric labels. The HTTP listener stays up at EOF; `stream_up` becomes 0.
SMP shares one sequence and records `vcpu` on every event. Aggregate counters
remain available alongside per-CPU counters. The Linux mode uses a dedicated
telemetry UART; its guest console is a separate, untrusted stream.

```sh
python3 tools/type1_events.py --input target/type1/el2-serial.log \
  --jsonl kernel-events.jsonl --metrics-file kernel.prom
cat /dev/ttyUSB0 | python3 tools/type1_events.py --listen 127.0.0.1:9465
```

PMU cycle counters are present only when the kernel supplies a sample. No
counter is manufactured for an unsupported backend. Serial JSON is a separate
format from the Linux trace sensor's events.
