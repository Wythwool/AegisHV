# Policy bundles

`src/policy_bundle.rs` verifies bundle version, issuer, issued-at time, config
digest, signature, and config bytes through a caller-supplied verifier. Empty
signatures and versions below the current policy version are rejected. Equal
versions may be reverified.

The caller owns canonicalization, trusted issuers, key storage, and applying the
accepted config. No keys or network update service ship here. Validate the
config and run policy/action dry runs before replacing the active file. Keep
the accepted bundle and audit record together.
