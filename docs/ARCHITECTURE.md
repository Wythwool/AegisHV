# Architecture

The sensor path is:

`collector -> parser -> identity -> wx -> policy/actions -> event`

`collector` reads tracefs `trace_pipe` or a replay file. Telemetry uses a bounded
channel; EOF and errors use a separate channel so a full queue cannot swallow
shutdown. `parser` distinguishes unrelated lines, unsupported formats, and
malformed KVM exits. `trace_format` reads tracepoint metadata but is not part of
the active text collector.

## Identity and correlation

`identity` reads QEMU arguments, cgroups, PID/TID start times, and optional
domain XML snapshots with host PID/TID metadata. Preferred IDs are `libvirt:<uuid>` and `name:<domain>`;
the fallback is `host-pid:<pid>:start:<ticks>`. XML input is file based, with no
connection to libvirt's daemon.

An event's `identity` records its sources, confidence, ambiguity, and start-time
verification. QMP dispatch requires a stable ID mapping by default and refuses
ambiguous or insufficiently verified targets.

`wx` keeps page-aligned state per VM and address space. Its window, capacity,
allowlist, and alert cooldown are configurable. Policy cooldown is separate:
its key includes the rule, VM, reason, trap type, page, and action set.

## Output and loss

JSONL is the primary output. Syslog and journald can mirror it; an optional
bounded disk spool handles primary-output failures. `/metrics` exposes queue,
parser, output, policy, and health counters.

Queue drops happen before event sequence numbers exist. The next emitted event
therefore reports an aggregate loss count with null sequence bounds. Gaps among
already numbered events instead report `range_kind=sequence_gap` and the missing
range. Neither case is silently treated as complete telemetry.

## Other crates

The VMI modules provide offline readers, translation, profiles, and inspection.
They consume snapshots supplied by the caller. The sensor has no live guest
memory or register reader.

The kernel is a separate `no_std` binary. Limine provides the initial mappings,
then the Intel path switches to its own page tables before guest entry.
[Kernel notes](TYPE1_BOOT_BOUNDARY.md) describe the fixed guest and current limits.

The core crate holds allocation, ownership, ring, VM, and scheduler models.
The device crate holds virtio and device-state models. These are library code;
the kernel does not yet wire them into running guests.
