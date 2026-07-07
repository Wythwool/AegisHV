use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyBundle<'a> {
    pub key_id: &'a str,
    pub policy_version: u64,
    pub rollback_counter: u64,
    pub payload: &'a [u8],
    pub signature: &'a [u8],
}

pub trait SignatureVerifier {
    fn verify(&self, key_id: &str, payload: &[u8], signature: &[u8]) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyBundleError {
    MissingKey,
    EmptyPayload,
    EmptySignature,
    Rollback,
    SignatureRejected,
}

impl fmt::Display for PolicyBundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingKey => write!(f, "policy bundle key id is empty"),
            Self::EmptyPayload => write!(f, "policy bundle payload is empty"),
            Self::EmptySignature => write!(f, "policy bundle signature is empty"),
            Self::Rollback => write!(f, "policy bundle rollback counter is older than required"),
            Self::SignatureRejected => write!(f, "policy bundle signature verification failed"),
        }
    }
}

impl std::error::Error for PolicyBundleError {}

pub fn verify_policy_bundle(
    bundle: &PolicyBundle<'_>,
    minimum_rollback_counter: u64,
    verifier: &impl SignatureVerifier,
) -> Result<(), PolicyBundleError> {
    if bundle.key_id.trim().is_empty() {
        return Err(PolicyBundleError::MissingKey);
    }
    if bundle.payload.is_empty() {
        return Err(PolicyBundleError::EmptyPayload);
    }
    if bundle.signature.is_empty() {
        return Err(PolicyBundleError::EmptySignature);
    }
    if bundle.rollback_counter < minimum_rollback_counter {
        return Err(PolicyBundleError::Rollback);
    }
    if !verifier.verify(bundle.key_id, bundle.payload, bundle.signature) {
        return Err(PolicyBundleError::SignatureRejected);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestVerifier;

    impl SignatureVerifier for TestVerifier {
        fn verify(&self, key_id: &str, payload: &[u8], signature: &[u8]) -> bool {
            key_id == "root-a" && payload == b"policy" && signature == b"sig-ok"
        }
    }

    fn bundle(signature: &'static [u8]) -> PolicyBundle<'static> {
        PolicyBundle {
            key_id: "root-a",
            policy_version: 7,
            rollback_counter: 4,
            payload: b"policy",
            signature,
        }
    }

    #[test]
    fn signed_policy_bundle_verifies_through_supplied_verifier() {
        verify_policy_bundle(&bundle(b"sig-ok"), 4, &TestVerifier).unwrap();
    }

    #[test]
    fn signed_policy_bundle_rejects_rollback_and_bad_signature() {
        assert_eq!(
            verify_policy_bundle(&bundle(b"sig-ok"), 5, &TestVerifier).unwrap_err(),
            PolicyBundleError::Rollback
        );
        assert_eq!(
            verify_policy_bundle(&bundle(b"bad"), 4, &TestVerifier).unwrap_err(),
            PolicyBundleError::SignatureRejected
        );
    }
}
