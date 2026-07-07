# Schema Compatibility Examples

These files are small compatibility examples for the current schemas:

- `event-v2-compatibility.jsonl` is accepted by `schema/event.schema.json`.
- `snapshot-v2-inventory.json` is accepted by `schema/snapshot.schema.json`.

They prove only current-schema acceptance for the documented shapes. They do not prove older schema support, future schema support, runtime support for every field combination, export format compatibility, or backend capability.

The examples use fixed synthetic values. They avoid host-specific paths, raw sockets, raw XML, command lines, secrets, machine IDs, and environment-derived values.
