# Detector library

`src/detectors` accepts existing events or caller-supplied offline VMI reports.
It does not collect guest memory, registers, or profiles. Outcomes distinguish
clean, finding, degraded, unsupported, and error results.

Checks cover kernel text hashes, syscall targets, hidden process/module
inventory differences, anonymous executable/RWX mappings, PMU snapshots, and
existing W^X events. Attribution already present on an event is retained.
[Detection limits](DETECTION_LIMITS.md) lists common false results.

The scheduler has per-detector enable flags, elapsed-time accounting, and finding
caps. It checks time after the detector returns; it cannot interrupt a stuck
detector. Excess findings are truncated and counted.

Dedupe keys include detector, VM, entity, range, and symbol. Incident objects
correlate W^X, syscall, and text-hash signals within one VM. Versioned state files
store dedupe and incident summaries. Incident objects are not emitted by the
current sensor runtime.

`[detectors]` and `[[detectors.rules]]` are parsed and validated as library
settings. The daemon does not automatically run these detectors. Embedding code
must supply inputs and translate config into `DetectorRunConfig`.
