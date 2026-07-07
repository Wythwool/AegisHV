# PMU Sampling Boundary

The current runtime still emits a PMU heartbeat from `/proc` thread CPU ticks when `[pmu].enable=true`. That heartbeat keeps hardware counters as `null` and reports `source=proc_stat_fallback`. It is not grouped perf sampling.

## Grouped Counters

The `pmu_sampling` module models grouped counter snapshots for cycles, instructions, cache references, cache misses, branches, and branch misses. It preserves unavailable counters as `None`, rejects counters that move backwards, and records `time_enabled` / `time_running` deltas so callers can scale values when the kernel reports multiplexing.

A live Linux backend still has to open `perf_event` groups, read the kernel group format, and prove target identity before emitting grouped PMU events. Until that backend exists, runtime JSONL must continue to say that grouped hardware counters are unavailable.

## Stable Target Identity

PMU target identity must include:

- stable VM id;
- vCPU id;
- host PID and TID;
- PID start time ticks;
- TID start time ticks.

If the observed start times change, the target is stale and must be rejected. PID-only PMU targeting is not safe enough for action or detection claims.

## Ring Buffer Sampling

The ring model records a bounded set of samples and increments a loss counter when it is full. It does not mmap a kernel perf ring, consume AUX buffers, or decode precise sample records. A live backend must report ring loss separately from JSONL queue loss.

## PEBS, IBS, And SPE

Intel PEBS, AMD IBS, and ARM SPE are represented as capability flags only:

- PEBS: precise IP, load latency, store latency;
- IBS: fetch sampling, op sampling, branch target;
- SPE: profiling, physical address reporting, timestamps.

No fake PEBS, IBS, or SPE samples are generated. A platform without one of these capabilities must leave that capability absent instead of inventing partial data.

## Offline Baseline Model

The PMU anomaly detector model tracks cycles per thousand instructions for synthetic or offline snapshots. It waits for a minimum history before reporting a finding. It is useful for testing scoring, dedupe, and incident plumbing. It is not a runtime PMU anomaly detector.
