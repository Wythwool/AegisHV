# PMU

With `[pmu].enable=true`, the runtime discovers QEMU threads and reports CPU-tick
heartbeats from `/proc`. Events use `source=proc_stat_fallback`; hardware counters
remain `null`. PID and TID start times are checked to reject reused processes.

There is no `perf_event_open` collector, kernel ring reader, or PEBS/IBS/SPE
decoder. Enabling this setting does not enable hardware PMU collection.

The offline anomaly detector can score caller-supplied counter snapshots after
collecting a minimum history. It has no source of live hardware counters.
