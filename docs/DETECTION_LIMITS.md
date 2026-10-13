# Detection limits

This registry lists implemented detector sources and the cases where the result can be wrong or unavailable. It is intentionally narrow.

## Source reliability

- `tracefs`: current Linux host tracefs events. Useful for W^X correlation. It is not guest memory truth.
- `offline_snapshot`: caller-provided offline memory/register/profile reports. Useful for deterministic checks. It can be stale or internally inconsistent.
- `verified_snapshot`: caller claims the memory, register, and profile inputs share one snapshot boundary. AegisHV still depends on the caller for that guarantee.
- `synthetic_fixture`: unit-test data only.
- `unsupported`: the detector cannot run for the supplied input.

## Detector registry

| Detector | Input source | Primary signal | False positives | False negatives | Unsupported cases |
| --- | --- | --- | --- | --- | --- |
| `kernel_text_tamper` | Linux or Windows text hash report | Hash mismatch or missing baseline for kernel text ranges | Legitimate hotpatching, changed kernel build, stale baseline, wrong profile | Tamper outside hashed ranges, stale snapshot, baseline already contains modified bytes | Empty hash report, missing profile identity, unreadable text range |
| `syscall_hook` | Linux syscall table/LSTAR or Windows SSDT/LSTAR report | Handler or entry target outside expected ranges | Legitimate kernel patching, wrong symbol profile, stale executable ranges | Hooks inside trusted ranges, stale snapshot, missing syscall records | Empty syscall table, missing SSDT/LSTAR data, unsupported profile |
| `hidden_process` | Memory process inventory compared with OS inventory | Process present in memory inventory but missing from OS list | Snapshot skew between inventories, terminated process during collection | Process hidden from both inventories, missing memory structure offsets | Either inventory unavailable or unsupported |
| `hidden_module` | Memory module or driver inventory compared with OS inventory | Module present in memory inventory but missing from OS list | Snapshot skew, unloaded module during collection, profile mismatch | Module hidden from both inventories, corrupted list that prevents walking | Either inventory unavailable or unsupported |
| `executable_anonymous_memory` | Caller-supplied memory mappings | Anonymous executable mapping outside JIT allowlist | Legitimate JIT runtime not allowlisted, unpacker or loader behavior with known software | Executable mapping mislabeled as file-backed, missing process mapping data | Empty or inverted mapping ranges |
| `rwx_mapping` | Caller-supplied memory mappings | Mapping has readable, writable, and executable permissions | Short-lived loader transitions, inaccurate permission snapshot | Permission flip missed between snapshots, incomplete mapping inventory | Empty or inverted mapping ranges |
| `wx_correlation` | Existing W^X events | Write then execute on the same guest page inside the configured window | Legitimate JIT or self-modifying code, broad page granularity, incomplete identity | Writes and executes outside the window, address-space mismatch, missing GPA | W^X event missing its payload |

## Confidence handling

Detections carry a source and confidence score. The score is reduced when attribution is weaker, profile confidence is lower, identity confidence is lower, or data loss was reported. Policy match can raise confidence, but it does not turn weak evidence into proof.

Low-confidence findings are still emitted by the detector layer. Operators should treat them as leads, not proof.

## State and aggregation

Dedupe keys use detector id, VM id, entity, range, and symbol. This prevents unrelated findings from collapsing together. It can still split the same root cause into multiple records when attribution changes between snapshots.

The incident model currently correlates W^X, syscall hook evidence, and kernel text hash drift on the same VM. It does not claim root cause. It only records that those signals co-occurred.
