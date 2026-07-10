# Security Review Checklist

Use this checklist before release branches, backend changes, and policy/action changes.

## Input Handling

- Config parsing rejects malformed sections, duplicate keys, unsafe action fields, and bad ranges.
- Replay and tracefs inputs are classified as parsed, unsupported, malformed, degraded, or unrelated.
- VMI fixtures reject unsafe paths and inconsistent architecture data.
- JSON and JSONL validators reject empty or malformed inputs.

## Output Handling

- JSONL write failures are fatal when spool is disabled.
- Spool-enabled write failures preserve or count the failed event.
- Spool limits reject records that cannot fit.
- Syslog and journald outputs are bounded.
- Redaction docs cover shared artifacts.

## Actions

- QMP actions require stable VM identity by default.
- Dump paths stay inside the configured dump root.
- Manual approval and dry-run states are explicit.
- Action audit fields record decision, status, retry, timeout, and refusal state.

## Backend Claims

- Production/general Type-1 support and demonstrated Intel guest execution must remain distinct from the bootable lab kernel and code-only toy path until reviewed hardware evidence and the production gates exist.
- Full VMI, general direct EPT/NPT/Stage-2 enforcement, syscall-path integrity, libvirt lifecycle integration, and hardware PMU sampling must remain documented as not implemented until real backend code and evidence exist.

## Release Checks

- `cargo fmt --all -- --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked --all --all-features`
- `scripts/smoke-replay.sh`
- `scripts/check-doc-links.sh`
- marker scan for unsupported wording
