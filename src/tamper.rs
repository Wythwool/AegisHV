use crate::event::{Category, Event, Severity};
use crate::util::{json_escape, now_rfc3339};
use std::fs;
use std::io::Read;
use std::path::Path;

pub const HASH_READ_LIMIT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TamperDigest {
    pub label: String,
}

pub fn digest_bytes(bytes: &[u8]) -> TamperDigest {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    TamperDigest {
        label: format!("fnv1a64:{hash:016x}"),
    }
}

pub fn digest_file_bounded(path: &Path) -> Result<TamperDigest, String> {
    let metadata = fs::metadata(path).map_err(|e| format!("stat {}: {e}", path.display()))?;
    if metadata.len() > HASH_READ_LIMIT_BYTES {
        return Err(format!(
            "refusing to hash {} because it exceeds {} bytes",
            path.display(),
            HASH_READ_LIMIT_BYTES
        ));
    }
    let mut file = fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut data = Vec::with_capacity(metadata.len().min(1024 * 1024) as usize);
    file.read_to_end(&mut data)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    Ok(digest_bytes(&data))
}

pub fn tamper_hash_event(kind: &str, path: &Path, digest: &TamperDigest) -> Event {
    let mut ev = Event::base(
        Category::Sensor,
        Severity::Info,
        now_rfc3339(),
        "host".to_string(),
    );
    ev.reason = Some("tamper_hash".to_string());
    ev.tags.push(format!("hash_kind:{kind}"));
    ev.message = Some(format!(
        "tamper hash kind={} path={} digest={}",
        json_escape(kind),
        json_escape(&path.display().to_string()),
        digest.label
    ));
    ev
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_bytes_is_stable_and_labeled() {
        let a = digest_bytes(b"config=1");
        let b = digest_bytes(b"config=1");
        let c = digest_bytes(b"config=2");

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.label.starts_with("fnv1a64:"));
    }

    #[test]
    fn tamper_hash_event_carries_kind_and_digest_without_schema_change() {
        let digest = digest_bytes(b"schema");
        let event = tamper_hash_event("schema", Path::new("schema/event.schema.json"), &digest);

        assert_eq!(event.reason.as_deref(), Some("tamper_hash"));
        assert!(event.message.unwrap().contains("fnv1a64:"));
    }
}
