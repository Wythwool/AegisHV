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

`target/linux-guest/serial.log` contains the Linux console and monitor exits.
The adjacent JSON file records the command, input digests and fault counters.
The `OS boot` workflow runs the same test on Linux with 1, 2 and 4 CPUs.

## Machine

The current machine has 256 MiB guest RAM at IPA `0x40000000`, backed by a
separate physical range at `0x50000000`. The host's code, tables and stacks are
not mapped into the guest. The console is a PL011 UART. GICv2 distributor and
CPU-interface accesses trap to EL2; IPIs use virtual SGIs and physical SGIs to
wake the destination CPU. Each vCPU has a dedicated host CPU and guarded stack.
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
