# Event data

There is no runtime redaction filter. JSONL, syslog, journald, and spool output
can contain VM names and IDs, host PID/TID and start times, command names, guest
addresses, policy IDs, paths, and diagnostic text. Snapshots also expose host
and VM inventory information.

Restrict access to these files and sinks. To share a sample, redact a copy and
retain the original separately. Keep null values, loss metadata, and ordering
intact; consistently map VM IDs if cross-event correlation is needed.

Prometheus labels use fixed categories and error kinds. Raw trace lines, guest
addresses, VM names, paths, build strings, and free-form errors do not belong
in labels.
