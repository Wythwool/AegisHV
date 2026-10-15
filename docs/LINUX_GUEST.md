# ARM64 Linux

Linux runs at EL1; AegisHV owns EL2, the host page tables and Stage-2 mappings.
There is no host Linux underneath this kernel. QEMU supplies the machine and
firmware for the test.

```sh
rustup target add aarch64-unknown-none
AEGISHV_LINUX=1 bash scripts/build-type1-arm64.sh
python3 scripts/prepare-linux-guest.py --cpus 4
python3 scripts/test-type1-linux.py target/type1/aegishv-el2.elf
```

The preparation script downloads Alpine 3.22.0's ARM64 kernel and minirootfs,
checks pinned SHA256 digests, unwraps the distribution's compressed Image,
and builds an initramfs. The kernel itself is unchanged. `/init` mounts the
root filesystem services, checks all CPUs, runs atomic workers pinned to each
CPU, takes CPU 1 offline and online, then powers off. The small worker is a test
program, not part of the hypervisor's operation.

The console test sends input over a TCP serial connection, checks that Linux
receives it, moves the UART IRQ to the last CPU, and repeats the exchange.
`/proc/interrupts` must show an increment on the selected CPU each time.

`target/linux-guest/serial.log` contains the Linux console and monitor exits.
The adjacent JSON file records the command, input digests and fault counters.
`edr.jsonl` holds EL2 events; `edr.prom` is their Prometheus snapshot. The `OS boot`
workflow runs on Linux with 1, 2 and 4 CPUs, plus PMU-off, telemetry isolation
and vector modification cases.

## Monitoring

EL2 writes JSON to a second PL011 at `0x09040000`. The guest DTB and Stage-2
map do not expose it. QEMU needs two `-serial` backends; the runner sets them
up. [QEMU virt](https://www.qemu.org/docs/master/system/arm/virt.html) creates
the second non-secure UART when its backend is supplied and TrustZone is off.
Use the second stream as the collector input:

```sh
python3 tools/type1_events.py --input target/linux-guest/edr.jsonl \
  --metrics-file target/linux-guest/collected.prom
```

Each Stage-2 permission fault records the vCPU, PC, IPA, access and decision.
`allow` records an RX/RW-NX transition; `emulate` records a scalar text patch
through the host mapping; `deny` records an access outside guest RAM and devices.
The sequence is shared across CPUs. Metric labels contain CPU IDs and event
types, never guest addresses. The guest console is untrusted and must not feed
the event collector.

Each CPU samples its PMU cycle counter every 25 host timer ticks, and before
CPU_OFF or poweroff. The counter includes EL0/EL1 and excludes EL2. EL2 owns
the PMU; guest accesses trap and read as zero or discard writes. No virtual PMU
device is advertised to Linux. `pmu_status` reports availability, including
after CPU hotplug. A missing PMU produces no cycle samples.

The same timer samples VBAR_EL1, translates it through the current guest page
tables, checks that the 2 KiB vector table lies within the loaded kernel Image,
and compares its bytes, VA and IPA with the previous sample on that CPU.
Translation restores PAR_EL1 before returning to Linux. `vector_sample` reports
validity and observed changes. This is sampled syscall/exception-entry monitoring;
changes between samples can escape detection, the first sample is the baseline,
and a valid address does not establish that the code is trusted. Concurrent
guest stores can race the byte snapshot. It does not inspect the syscall table.

```sh
python3 scripts/test-type1-linux.py target/type1/aegishv-el2.elf --pmu off
python3 scripts/test-type1-linux.py target/type1/aegishv-el2.elf --vector-change
python3 scripts/prepare-linux-guest.py --cpus 1 --probe telemetry
python3 scripts/test-type1-linux.py target/type1/aegishv-el2.elf --probe telemetry
```

The vector test changes an unused AArch32 vector byte through QEMU's local GDB
stub, requires an EL2 change event, restores the byte and requires a second
event before Linux completes. The telemetry probe uses `/dev/mem` in the guest
to attempt a UART write; a Stage-2 denial must stop it. Every guest also prints
a forged event on its console. Neither that line nor its counter value may
appear in the EL2 stream. These workloads are test programs; monitoring requires
no guest service or module.

## Machine

The current machine has 256 MiB guest RAM at IPA `0x40000000`, backed by a
separate physical range at `0x50000000`. The host's code, tables and stacks are
not mapped into the guest. The console is a PL011 UART. GICv2 distributor and
CPU-interface accesses trap to EL2; IPIs use virtual SGIs and physical SGIs to
wake the destination CPU. UART accesses also trap so EL2 can rearm the physical
interrupt after the guest drains the FIFO. Shared interrupt state and affinity
are common to all CPUs; SGIs retain each sender's ID. Each vCPU has a dedicated
host CPU and guarded stack.
PSCI CPU_ON/OFF and affinity queries support Linux secondary startup and hotplug.
There is no vCPU oversubscription or CPU migration yet.

Linux owns its EL1 page tables. Stage-2 starts as RW/NX; instruction faults switch
pages to RX and write faults switch them back. Linux sometimes patches the page
it is executing. EL2 handles those scalar stores through its private mapping
and synchronizes the instruction caches, avoiding an RX/RW fault loop. This mode
allows guest code changes; it is not an immutable-code policy. The static ELF
guest mode still denies writes to code and execution from data.

The boot envelope is eight little-endian u64 words at physical `0x50000000`:
`AEGLNX01`, entry IPA, DTB IPA, RAM size, CPU count, initramfs length, Image size,
and a timer budget in 10 ms ticks. Zero disables the budget. QEMU loads the Image,
DTB and initramfs into the backing range before entering EL2. The helper sets a
120-second budget; `--budget-seconds 0` disables it. It is not a disk bootloader.

Only the QEMU `virt` GICv2/Cortex-A57 layout is supported here. Virtual timers are
sampled at 100 Hz, devices are limited to the console and interrupt controller,
and there is no storage or network backend. SMP and Linux boot have been tested
under TCG, not on a physical ARM board. Windows and x86 Linux are not supported
by this boot path.
