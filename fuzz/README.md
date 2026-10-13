# Fuzzing

Install cargo-fuzz and run from the repository root with a supported nightly
toolchain:

```sh
cargo install cargo-fuzz
cargo +nightly fuzz run trace_parser_line -- -max_total_time=60
cargo +nightly fuzz run config_input -- -max_total_time=60
cargo +nightly fuzz run trace_format_metadata -- -max_total_time=60
cargo +nightly fuzz run qmp_action_safety -- -max_total_time=60
```

The targets parse bounded UTF-8 trace lines, temporary config files, tracepoint
metadata, and QMP action inputs. The QMP target exercises missing, ambiguous,
stale, PID-only, low-confidence, conflicting, and unverified identities without
opening a socket.

For QMP corpus `.seed` files, the first byte selects a refusal case with `% 7`;
an even fourth byte selects execution validation instead of dry-run.

CI compiles the harnesses; it does not run a fuzz campaign. Retain any failing
input and the reproducing command. These targets need no live KVM or root access.
