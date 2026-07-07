# Performance Plan

Performance measurements must be local and repeatable. Do not commit invented numbers. Commit scripts, inputs, host details, and raw outputs together when recording a result.

## Benchmarks

| Area | Script | Measures | Notes |
| --- | --- | --- | --- |
| Trace ingestion | `scripts/bench-trace-ingest.sh` | replay runtime and event count | Uses committed trace fixtures. |
| W^X state | `scripts/bench-wx-state.sh` | replay runtime and W^X event count | Uses committed W^X corpus and config. |
| VMI translation | `scripts/bench-vmi-translate.sh` | repeated offline translate CLI calls | Fixture-only benchmark. |
| Trap controller | `scripts/bench-trap-synthetic.sh` | synthetic trap state transitions | Does not measure VM exits. |

## Required Metadata

Every saved result should include:

- Git commit;
- CPU model and core count;
- kernel version;
- Rust toolchain;
- command line;
- input fixture path and hash;
- raw CSV or log output;
- whether the run used debug or release binaries.

## Non-Claims

The current benchmarks do not measure bare-metal VM exits, live guest reads, hardware invalidations, real PMU groups, QMP latency on a fleet, or libvirt lifecycle behavior. A release note must keep those limits visible.

## Gate

Before publishing a performance claim, rerun the matching script at least three times on the same host, keep raw output, and mention variance. If the benchmark depends on a host resource such as live KVM, keep the opt-in script output separate from normal CI output.
