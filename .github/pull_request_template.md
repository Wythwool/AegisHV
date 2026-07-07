# Pull Request Checklist

## Summary

State what changed and why.

## Scope

- [ ] The PR does one reviewable thing.
- [ ] Runtime code, schemas, docs, config, and packaging changes are separated unless they are required for the same behavior.
- [ ] No unrelated modules or generated files were reformatted.

## Tests Run

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --locked --all-targets --all-features -- -D warnings`
- [ ] `cargo test --locked --all --all-features`
- [ ] `./scripts/smoke-replay.sh`
- [ ] Any skipped command has a concrete reason, such as a missing tool or host capability.

## Schema Impact

- [ ] No public schema changed.
- [ ] If a public schema changed, schemas, tests, docs, examples, and compatibility notes were updated in the same PR.

## Security Impact

- [ ] Identity, policy, QMP actions, backend contracts, trap logic, guest memory access, and refusal behavior were reviewed when touched.
- [ ] Unsupported behavior returns a typed error instead of silent success.
- [ ] Unsafe code, if added, documents the invariant that makes it valid.

## Docs Impact

- [ ] Operator-visible behavior is documented.
- [ ] Unsupported behavior remains described honestly.
- [ ] No fake backend, hardware, benchmark, compatibility, or release claim was added.

## Detector Impact

- [ ] False-positive impact was reviewed.
- [ ] False-negative impact was reviewed.
- [ ] Malformed, unsupported, and degraded inputs have tests when detector behavior changes.

## Production Claim Check

- [ ] This PR does not claim type-1 support, full VMI support, EPT/NPT/Stage-2 enforcement, syscall-path integrity, hardware PMU sampling, libvirt support, or hardware coverage unless it implements and tests that exact capability.

## Dependency Impact

- [ ] No dependency was added.
- [ ] If a dependency was added, the PR explains why the main crate should take it and updates `Cargo.lock`.
