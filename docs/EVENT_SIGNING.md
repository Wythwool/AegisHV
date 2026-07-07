# Event Signing Design

This document describes a possible tamper-evident signing path for AegisHV events. It is design-only. The current runtime does not sign events, does not verify signed event streams, and does not add signing fields to `schema/event.schema.json`.

Tamper-evident means an operator can detect likely deletion, modification, reordering, truncation, or replay when the signed stream and verification material are available. It is not tamper-proof. An attacker with the active signing key can produce valid signatures. An attacker who can stop the sensor can still prevent later events from being created.

JSONL remains the source event contract. Syslog, journald, spool, and any later export sink should mirror the same signed event line or an envelope derived from it.

## Scope

The signing boundary should sit after event construction, deterministic replay normalization, loss annotation, and policy/action audit enrichment, but before any `EventSink` writes the event. Signing must not change parser, W^X, identity, QMP, PMU, or tracefs behavior.

Out of scope for this design:

- implementing runtime signing;
- changing event or snapshot schemas;
- adding cryptographic dependencies without dependency review;
- adding backend, type-1, VMI, EPT/NPT, syscall integrity, or hardware PMU claims;
- claiming guaranteed delivery;
- putting private keys, secrets, raw host paths, socket paths, VM names, UUIDs, command lines, XML, raw trace text, or arbitrary raw errors into signing metadata.

## Canonical Event Bytes

The implementation should sign canonical bytes, not whatever a sink happens to write.

Recommended canonicalization:

1. Build the normal `Event` object.
2. Apply deterministic replay normalization only when deterministic replay mode is explicitly enabled.
3. Attach any loss metadata the runtime already knows.
4. Serialize with the existing stable JSON field order used by `Event::to_json`.
5. Encode as UTF-8 without a trailing newline.
6. Compute `event_hash = SHA-256(canonical_event_bytes)`.

The hash covers the full JSON event body. If an existing event field contains a VM identity value, the hash covers it because it is part of the event contract. Signing metadata must not duplicate VM names, UUIDs, socket paths, command lines, XML, paths, or free-form errors.

Do not canonicalize through a generic map serializer unless tests prove byte-for-byte stability across supported Rust versions and feature sets. Golden fixtures should make this visible.

## Hash Chain

Each emitted event should carry or be paired with a bounded signing envelope:

```json
{
  "signing_version": 1,
  "chain_id": "chain-20260427-0001",
  "key_id": "sha256:6b1f...32hex",
  "algorithm": "ed25519-sha256",
  "sequence": 42,
  "event_hash": "sha256:<64 hex>",
  "prev_chain_hash": "sha256:<64 hex>",
  "chain_hash": "sha256:<64 hex>",
  "signature": "base64..."
}
```

This shape is illustrative and not implemented.

Recommended chain input:

```text
signing_version || chain_id || key_id || sequence || event_hash || prev_chain_hash
```

`chain_hash = SHA-256(chain_input)`. The first event in a chain uses a fixed genesis value such as `sha256:000...000` as `prev_chain_hash`. A verifier checks:

- the JSON event hash matches the canonical event bytes;
- each `prev_chain_hash` equals the prior accepted `chain_hash`;
- sequence numbers increase by one except where a signed loss event explicitly reports a bounded sequence gap;
- the signature validates under the key identified by `key_id`;
- the chain starts with a startup lifecycle event or an explicit verifier-approved resume marker;
- the chain ends with a shutdown lifecycle event, a signed truncation marker, or an unexplained end-of-file warning.

The chain detects local deletion, insertion, modification, and reordering inside the signed stream. It cannot prove events were generated before the signing code was initialized.

## Signature Options

Two signing modes are reasonable.

### Per-Event Signature

Every event envelope carries a signature over `chain_hash`.

Properties:

- simplest verifier state;
- strongest localization of the first bad event;
- higher CPU and byte overhead;
- easier to mirror through syslog and journald because every line is self-contained.

This should be the default design for security-sensitive deployments if performance tests are acceptable.

### Batch Signature

Each event carries `event_hash`, `prev_chain_hash`, and `chain_hash`, but only periodic batch marker events carry a signature over the last `chain_hash` and the batch range.

Properties:

- lower signature overhead;
- verifier can still detect chain damage but trust is anchored at batch boundaries;
- a crash between batch signatures leaves the tail verifiable only by hash continuity, not by public-key signature;
- truncation near the tail is harder to distinguish from a crash unless shutdown markers are present.

Batch signature metadata must include bounded fields only:

- `batch_start_sequence`
- `batch_end_sequence`
- `event_count`
- `chain_hash`
- `algorithm`
- `key_id`
- `signature`

Do not include raw event bodies, paths, sockets, VM names, UUIDs, command lines, XML, or error strings in batch metadata.

## Key Storage

Key handling must be explicit and boring.

Recommended key sources:

- an OS key service or hardware-backed key where available;
- a root-readable file with strict ownership and mode checks;
- an HSM/KMS signing adapter only if its failure behavior is bounded and testable.

Validation rules:

- signing is disabled by default;
- enabled signing requires a configured key source;
- file keys must reject group/world-readable private key material;
- the runtime never emits private key bytes, raw key source paths, or provider error strings;
- startup events report only `signing=enabled`, `algorithm`, `key_id`, and `chain_id`;
- metrics labels use bounded values such as `reason="key_unavailable"` or `reason="sign_failed"`.

`key_id` should be a non-secret stable identifier, such as a SHA-256 fingerprint of the public key truncated to a documented length. It must not be a file path, account name, socket path, or provider-specific raw error.

## Key Rotation

Rotation must be visible in the chain.

Recommended behavior:

1. Finish the old chain with a signed `sensor` event reason such as `signing_key_rotation`.
2. Include bounded metadata: old `key_id`, new `key_id`, old terminal `chain_hash`, and new `chain_id`.
3. Start the new chain with `prev_chain_hash` set to the old terminal `chain_hash` or with a dedicated `previous_chain_hash` field in the rotation event.
4. Reject rotation if the new key cannot sign a test challenge.

A verifier should require both old and new public keys for the rotated range. Missing key material should produce `unknown_key`, not a silent pass.

## Lifecycle Interaction

Startup lifecycle events are the natural chain start. When signing is enabled, the startup event should be signed and include bounded signing state:

- enabled or disabled;
- algorithm enum;
- key ID;
- chain ID;
- signature mode: `per_event` or `batch`;
- verifier policy version.

Shutdown lifecycle events should close the chain. A clean shutdown signs the final shutdown event. A signal shutdown should keep existing `shutdown_signal` behavior and then sign the final lifecycle event if signing still works.

Failure shutdown has two cases:

- If signing still works, emit and sign `sensor_shutdown` with the known failure reason already used by runtime lifecycle logic.
- If signing failed, emit an unsigned terminal diagnostic only if the output path can still write it. The diagnostic must be clearly marked unsigned and must not pretend the chain closed cleanly.

If startup fails before JSONL output opens or before a key is available, there may be no signed startup event. The process should fail with an operator-useful startup error rather than creating an unsigned stream while signing is required.

## Deterministic Replay And Golden Fixtures

Signing must not make normal deterministic replay unstable.

Recommended modes:

- default deterministic replay keeps signing disabled;
- a separate deterministic signing test mode uses a checked-in public test key and fixed private test key only for tests;
- test key material must be named as test-only and must not be used by examples that look deployable;
- deterministic signed fixtures freeze `chain_id`, timestamps, event IDs, and key IDs.

Golden JSONL fixtures should remain unsigned unless the fixture is specifically for signing behavior. This avoids changing every event fixture when signing is not part of the tested contract.

Live tracefs output must not be made fake-deterministic. Live signing uses real chain IDs and real keys.

## Sink Behavior

Signing should happen once before sink fan-out.

### JSONL

The JSONL sink should write either:

- the existing event object with a `signing` object added after a schema update; or
- an envelope line containing `{ "event": <event>, "signing": <metadata> }` under a new schema.

The implementation must choose one format and update schema, fixtures, docs, and compatibility notes in the same change. It must not silently produce mixed signed and unsigned JSONL lines in one stream unless a signed state-change event explains the transition.

### Disk Spool

The spool should preserve the already-signed line when the main JSONL write fails. Spool compression must compress signed bytes as data; it must not recompute signatures inside the spool writer.

If a spooled segment is copied later, the verifier should treat it as part of the same signed stream only when sequence and chain continuity match. Segment headers are storage metadata, not cryptographic trust anchors.

### Syslog And Journald

Syslog and journald should mirror signed event lines after JSONL or spool handling. They must not strip signing metadata. Datagram truncation or message-size refusal must be explicit and counted as sink failure.

Do not put `key_id`, `chain_id`, VM names, UUIDs, PIDs, sockets, paths, command lines, XML, or arbitrary errors into metric labels. `key_id` can be an event field but should not become a label.

### OTLP And Other Export Sinks

Any later exporter should carry the signed JSON event body and signing metadata as body fields. Index only bounded fields such as `signing_version`, `algorithm`, and signature mode. Do not index full hashes or signatures unless the downstream owner explicitly accepts the cardinality and storage cost.

## Loss Ranges And Sequence Gaps

Loss events must be part of the signed stream.

Rules:

- Queue overflow aggregate loss remains aggregate. Do not invent missing event hashes.
- Known emitted-sequence gaps are signed as the runtime currently reports them.
- Intentional policy filtering, such as ignored VM events, must be accounted so it does not create false loss signatures.
- Spool/syslog/journald/export failure after JSONL success is sink loss, not source telemetry loss.
- If signing itself drops or blocks event emission, emit a bounded `sensor` diagnostic when policy allows degraded signing.

A verifier should report:

- `sequence_gap_without_loss_event`;
- `loss_event_with_aggregate_only`;
- `loss_event_with_exact_range`;
- `chain_break_after_loss_event`;
- `truncated_after_sequence`.

It must not infer exact missing event content from a range.

## Failure Behavior

The implementation must choose one startup policy.

### Required Signing

If signing is configured as required:

- key load failure fails startup;
- signature failure at runtime is fatal after an explicit signed or unsigned diagnostic attempt;
- unsigned event emission is refused;
- readiness is failed when signing is unavailable.

This mode is appropriate when unsigned telemetry is worse than no telemetry.

### Best-Effort Signing

If signing is configured as best-effort:

- key load failure starts the sensor in degraded mode only when the config explicitly permits it;
- unsigned events carry a bounded `signing_status="unavailable"` field after schema support exists;
- metrics count signing failures with bounded reasons;
- readiness is degraded while signing is unavailable.

Best-effort must not be the implicit result of a signing failure. The operator must configure it.

Runtime failure reasons should be bounded:

- `key_unavailable`
- `key_permission`
- `sign_failed`
- `unsupported_algorithm`
- `chain_state_corrupt`
- `clock_or_sequence_invalid`
- `verifier_config_invalid`

Do not expose raw provider errors, paths, socket names, command lines, XML, or secrets in labels or signing metadata.

## Verification Tool Shape

A verifier should be a separate command or script, not part of hot-path ingestion.

Suggested command shape:

```text
aegishv verify-events --jsonl events.jsonl --public-key aegishv-signing.pub --policy verify.toml
```

This command is not implemented.

Verifier output should be machine-readable and bounded:

```json
{
  "ok": false,
  "events_checked": 1024,
  "first_bad_sequence": 781,
  "reason": "chain_break",
  "details": "prev_chain_hash did not match prior chain_hash"
}
```

Verifier checks:

- parse every JSONL line;
- validate event schema for the embedded event shape;
- reserialize canonical event bytes exactly as the signer did;
- recompute `event_hash`;
- recompute `chain_hash`;
- verify per-event or batch signature;
- check sequence monotonicity and loss-range consistency;
- check startup/shutdown lifecycle markers;
- reject unknown algorithms unless explicitly allowed;
- reject unknown keys unless policy allows archived key lookup;
- report truncation when the file ends without a signed shutdown or accepted batch boundary.

The verifier must never require live libvirt, live QEMU, tracefs, journald, syslog, OTLP, or host `/proc` state.

## Bounded Signing Metadata

Safe metadata fields:

| Field | Bound |
| --- | --- |
| `signing_version` | integer enum |
| `algorithm` | bounded enum |
| `signature_mode` | `per_event` or `batch` |
| `key_id` | fixed-length public key fingerprint |
| `chain_id` | generated bounded ID, not a path |
| `sequence` | event sequence number |
| `event_hash` | fixed-length hash string |
| `prev_chain_hash` | fixed-length hash string |
| `chain_hash` | fixed-length hash string |
| `signature` | bounded base64 signature |
| `batch_start_sequence` | integer |
| `batch_end_sequence` | integer |
| `event_count` | integer |
| `signing_status` | bounded enum |
| `signing_failure_reason` | bounded enum |

Unsafe signing metadata:

- private keys or key material;
- key source paths;
- socket paths;
- tracefs paths;
- dump paths;
- VM names;
- UUIDs copied only for signing;
- raw command lines;
- XML;
- raw trace text;
- arbitrary error strings;
- secrets from config headers or environment variables.

## Threat Model

This design helps detect:

- event body modification after signing;
- deletion of signed events in the middle of a stream;
- insertion of unsigned or differently signed events;
- reordering signed events;
- truncation without a signed shutdown or accepted batch boundary;
- replay of an old stream when `chain_id`, startup time, and key policy are checked;
- disagreement between reported sequence gaps and chain continuity.

This design does not prevent:

- an attacker with the active private key from signing forged events;
- an attacker from stopping AegisHV before an event is produced;
- kernel, hypervisor, firmware, or hardware compromise from hiding telemetry before userspace sees it;
- loss caused by full disks, disabled sinks, or network failures;
- tampering with unsigned old logs;
- false confidence from weak key storage;
- high-cardinality leakage already present in an event body.

Public-key verification proves only that a holder of the private key signed the canonical bytes in that order. It does not prove the event describes real guest state, type-1 enforcement, full VMI, syscall integrity, EPT/NPT permission enforcement, or hardware PMU sampling.

## Implementation Test Plan

A runtime implementation should add tests for:

- canonical JSON bytes are stable;
- event hash changes when any signed event field changes;
- chain hash detects deletion, insertion, modification, and reordering;
- per-event signatures verify and reject wrong keys;
- batch signatures verify ranges and report unsigned tails;
- key load rejects unsafe permissions;
- key rotation links old and new chains;
- startup and shutdown lifecycle events open and close the chain;
- signing failure behavior for required and best-effort modes;
- deterministic replay output remains unchanged when signing is disabled;
- deterministic signed fixtures are byte-stable when explicit test signing is enabled;
- spool preserves signed bytes across plaintext and compressed segment modes;
- syslog and journald mirror signed lines without modifying metadata;
- loss-range and sequence-gap events verify without inventing missing event content;
- verifier reports bounded reasons and no raw host paths, sockets, VM names, UUIDs, command lines, XML, or arbitrary errors.

Normal tests must not require live key services, live QEMU, live libvirt, tracefs, journald, syslog, OTLP, or host-specific process timing. Use test keys, fixture JSONL, and mock signers/verifiers.

## Current Status

No signing runtime exists in this tree. No config section enables signing. No schema fields carry signing metadata. No verifier command exists. This document is a design record for a later implementation and does not change JSONL, replay, deterministic replay, golden fixtures, spool, syslog, journald, metrics, snapshot, or action behavior.
