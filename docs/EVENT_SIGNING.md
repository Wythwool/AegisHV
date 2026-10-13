# Event integrity

Runtime JSONL events are unsigned. There is no event hash chain, event-signing
key store, or `verify-events` command.

Release archives have a separate signing and provenance workflow described in
[RELEASE.md](../RELEASE.md). Those signatures do not authenticate a running
sensor's event stream.

Store raw events under restricted permissions and ship copies outside the
monitored host if loss or host compromise matters. Preserve sequence and loss
records. The digest helpers used for local change detection are not signatures.
