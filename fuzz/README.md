# AegisHV Fuzz Harnesses

This directory contains cargo-fuzz harness structure for local parser hardening. It is not part of normal PR CI, and the repository does not claim fuzz coverage unless an operator runs a campaign and keeps the evidence.

Install cargo-fuzz locally:

```bash
cargo install cargo-fuzz
```

Run short local checks from the repository root:

```bash
cargo fuzz run trace_parser_line -- -max_total_time=60
cargo fuzz run config_input -- -max_total_time=60
cargo fuzz run trace_format_metadata -- -max_total_time=60
cargo fuzz run qmp_action_safety -- -max_total_time=60
```

Targets:

- `trace_parser_line` feeds bounded UTF-8 trace lines through `parser::parse_line` and classifies parsed exits.
- `config_input` writes bounded UTF-8 TOML input to a temporary file and calls `Config::load`.
- `trace_format_metadata` feeds bounded UTF-8 tracepoint metadata through `trace_format::parse_tracepoint_format`.
- `qmp_action_safety` constructs an `ActionDispatcher` with a fixed QMP mapping and exercises bounded action refusal paths for missing, ambiguous, stale, PID-only, low-confidence, conflicting, and unverified identity states. It does not connect to a QMP socket.

The QMP action-safety corpus uses `.seed` files. The first byte intentionally maps through `selector % 7` to one refusal reason, and the fourth byte is even so the harness uses `execute=true` rather than dry-run.

The Python JSON Schema validator is not fuzzed here. Adding a Rust JSON Schema stack would add dependency weight to this structure task, and the committed validator tests already exercise the current schemas with deterministic fixtures.

The harnesses do not require live KVM, tracefs, libvirt, root, network services, secrets, package installs, or host-specific paths. They do not prove vulnerability absence, parser completeness, runtime safety, type-1 support, full VMI, EPT/NPT enforcement, syscall-path integrity, live libvirt integration, or hardware PMU support.
