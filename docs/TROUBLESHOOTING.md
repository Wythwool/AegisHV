# Troubleshooting

| Symptom | Check |
| --- | --- |
| No live events | tracefs is mounted, `kvm_exit` is enabled, the collector can read `trace_pipe`, and a guest is producing exits |
| Config rejected | Run `aegishv validate-config --config config.example.toml`; errors include the line number |
| Metrics bind fails | Another process may own the address; choose another `--listen` address or use `--listen ''` |
| Process alive, readiness false | Inspect output, collector, PMU, and action failure counters in `/metrics` |
| Queue drops | Check collector rate, output latency, and `--queue`; the next event carries loss metadata |
| W^X alert missing | Both accesses must share VM identity, address space, and page, and fit within the configured window; also check cooldown and allowlists |
| QMP action refused | Inspect the action's `failure_class`, identity confidence, start-time verification, and stable ID/socket mapping |
| Dump refused | The path must be new, below `dump_root`, and free of symlink components |
| Dump accepted but absent | QMP acceptance is asynchronous; check QEMU and the expected file separately |
| PMU counters are null | The current sampler reports thread heartbeats, not hardware counters |
| VMI operation unsupported | Live guest reads are unavailable; offline commands need matching memory/register/profile fixtures |
| Journald startup fails | The sink requires Linux and access to its configured datagram socket |
| Spool fills up | Check size limits and output failures; spooling is bounded and does not acknowledge downstream delivery |
| Reload rejected | `config_reload_failed` retains the old config; startup-only settings need a restart |
| Rotation does not switch files | Send `SIGHUP` after renaming; check `jsonl_reopen_failed` and directory permissions |

For parser problems, retain a minimal trace sample and the host kernel version.
For identity problems, compare event identity with QEMU arguments and process
start-time ticks. Remove private VM names, paths, and addresses before sharing.

For kernel boot problems, keep the ELF/ISO hashes, serial log, QEMU command,
CPU model, and timer diagnostics. `backend-none` under TCG means VMX was not
available. A partial marker sequence is not a completed guest run; see
[kernel notes](TYPE1_BOOT_BOUNDARY.md).
