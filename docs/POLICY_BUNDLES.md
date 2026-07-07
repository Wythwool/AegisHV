# Policy Bundles

Policy bundle verification is modeled in `src/policy_bundle.rs`. A bundle records a policy version, issuer, issued-at timestamp, config digest, signature, and the raw config text.

## Verification Rules

The verifier rejects:

- empty signatures;
- versions older than the currently applied policy version;
- invalid signatures reported by the caller-provided verifier.

The verifier accepts a bundle only after the signature verifier approves the config bytes and metadata. The code does not ship a signing key, does not hard-code a trust root, and does not fetch keys from the network.

## Rollback Handling

Bundle versions are monotonic. A bundle with a lower version than the current version is rejected before the config is applied. Equal versions are allowed so operators can re-verify the current bundle during recovery.

## Integration Contract

An embedding service should:

- canonicalize the config before signing;
- keep the trusted issuer and key material outside the bundle;
- verify the signature before writing the active config;
- preserve the accepted bundle, audit record, and operator decision together;
- run `aegishv validate-config --config FILE` before restart or reload.

## Boundary

This is a local verification contract, not a live update service. Online key rotation, certificate transparency, threshold approvals, and live daemon reloads are not implemented in this tree.
