# Event output

JSONL is the primary event stream. Each line follows
[`event.schema.json`](../schema/event.schema.json). `snapshot` uses
[`snapshot.schema.json`](../schema/snapshot.schema.json).

Optional mirrors in `config.example.toml`:

- `[syslog]`: UDP to a numeric `ip:port`, with facility and message-size limits.
- `[journald]`: Linux datagram socket, identifier, and message-size limits.
- `[spool]`: bounded local segments for primary JSONL write failures. `none`
  keeps v1 plaintext; `rle` writes compressed v2 segments.

Mirror failures have separate counters. UDP and local datagram writes do not
acknowledge receipt by a log store. Spool size limits can still cause loss; keep
the emitted loss records with the stream.

There is no built-in OTLP exporter. A separate collector can ship JSONL and
perform format conversion. Preserve event sequence, schema version, identity
confidence, null counters, and loss metadata when converting.
