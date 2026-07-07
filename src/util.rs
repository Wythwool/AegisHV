use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static EVENT_COUNTER: AtomicU64 = AtomicU64::new(1);
static SEQUENCE_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMetadata {
    pub host_id: Option<String>,
    pub sensor_id: Option<String>,
    pub tenant_id: Option<String>,
}

pub fn next_event_id() -> String {
    let n = EVENT_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("evt-{n:016x}")
}

pub fn next_sequence() -> u64 {
    SEQUENCE_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub fn now_rfc3339() -> String {
    // Dependency-free UTC timestamp. Good enough for event ordering; monotonic_ms is emitted
    // separately for same-host correlation.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0));
    let secs = now.as_secs() as i64;
    let millis = now.subsec_millis();
    let (year, month, day, hour, min, sec) = unix_to_utc(secs);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}.{millis:03}Z")
}

pub fn monotonic_ms() -> u128 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis()
}

fn unix_to_utc(mut secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    let sec = secs.rem_euclid(60) as u32;
    secs = secs.div_euclid(60);
    let min = secs.rem_euclid(60) as u32;
    secs = secs.div_euclid(60);
    let hour = secs.rem_euclid(24) as u32;
    let days = secs.div_euclid(24);
    let (year, month, day) = civil_from_days(days);
    (year, month, day, hour, min, sec)
}

fn civil_from_days(z: i64) -> (i32, u32, u32) {
    // Howard Hinnant's civil-from-days algorithm, shifted from Unix epoch.
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    (y as i32, m as u32, d as u32)
}

pub fn runtime_metadata() -> &'static RuntimeMetadata {
    static METADATA: OnceLock<RuntimeMetadata> = OnceLock::new();
    METADATA.get_or_init(load_runtime_metadata)
}

pub fn host_id() -> Option<String> {
    runtime_metadata().host_id.clone()
}

pub fn sensor_id() -> Option<String> {
    runtime_metadata().sensor_id.clone()
}

pub fn tenant_id() -> Option<String> {
    runtime_metadata().tenant_id.clone()
}

fn load_runtime_metadata() -> RuntimeMetadata {
    metadata_from_sources(
        std::env::var("AEGISHV_HOST_ID").ok().as_deref(),
        std::env::var("AEGISHV_SENSOR_ID").ok().as_deref(),
        std::env::var("AEGISHV_TENANT_ID").ok().as_deref(),
        std::fs::read_to_string("/etc/machine-id").ok().as_deref(),
    )
}

fn metadata_from_sources(
    host_env: Option<&str>,
    sensor_env: Option<&str>,
    tenant_env: Option<&str>,
    machine_id: Option<&str>,
) -> RuntimeMetadata {
    RuntimeMetadata {
        host_id: metadata_value(host_env).or_else(|| metadata_value(machine_id)),
        sensor_id: metadata_value(sensor_env),
        tenant_id: metadata_value(tenant_env),
    }
}

fn metadata_value(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn parse_comm_pid(raw: &str) -> (String, Option<i32>) {
    let mut split = raw.rsplitn(2, '-');
    let tail = split.next().unwrap_or(raw);
    let head = split.next();
    if let (Some(name), Ok(pid)) = (head, tail.parse::<i32>()) {
        (name.to_string(), Some(pid))
    } else {
        (raw.to_string(), None)
    }
}

pub fn parse_hex_u64(s: &str) -> Option<u64> {
    let trimmed = s.trim().trim_end_matches(',');
    let stripped = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    u64::from_str_radix(stripped, 16).ok()
}

pub fn format_hex(v: u64) -> String {
    format!("0x{v:x}")
}

pub fn page_align(addr: u64, page_size: u64) -> u64 {
    let size = if page_size == 0 { 4096 } else { page_size };
    addr & !(size - 1)
}

pub fn clamp_u64(v: u64, min: u64, max: u64) -> u64 {
    v.max(min).min(max)
}

pub fn clamp_usize(v: usize, min: usize, max: usize) -> usize {
    v.max(min).min(max)
}

pub fn json_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 8);
    for c in input.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

pub fn json_str(s: &str) -> String {
    format!("\"{}\"", json_escape(s))
}

pub fn json_opt_string(v: &Option<String>) -> String {
    match v {
        Some(s) => json_str(s),
        None => "null".to_string(),
    }
}

pub fn json_opt_i32(v: Option<i32>) -> String {
    v.map(|n| n.to_string())
        .unwrap_or_else(|| "null".to_string())
}

pub fn json_opt_u64(v: Option<u64>) -> String {
    v.map(|n| n.to_string())
        .unwrap_or_else(|| "null".to_string())
}

pub fn json_opt_f64(v: Option<f64>) -> String {
    v.map(|n| format!("{n:.6}"))
        .unwrap_or_else(|| "null".to_string())
}

pub fn parse_bool(s: &str) -> Option<bool> {
    match s.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

pub fn parse_string_value(s: &str) -> String {
    let t = s.trim().trim_end_matches(',').trim();
    if t.starts_with('"') && t.ends_with('"') && t.len() >= 2 {
        t[1..t.len() - 1].replace("\\\"", "\"").replace("\\n", "\n")
    } else {
        t.to_string()
    }
}

pub fn parse_string_array(s: &str) -> Vec<String> {
    let t = s.trim();
    let inner = t
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .unwrap_or(t);
    inner
        .split(',')
        .map(parse_string_value)
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_comm_pid() {
        assert_eq!(
            parse_comm_pid("qemu-system-x86-1234"),
            ("qemu-system-x86".to_string(), Some(1234))
        );
        assert_eq!(
            parse_comm_pid("qemu-system-x86"),
            ("qemu-system-x86".to_string(), None)
        );
    }

    #[test]
    fn aligns_page() {
        assert_eq!(page_align(0x1abc, 4096), 0x1000);
    }

    #[test]
    fn monotonic_ms_does_not_move_backward() {
        let first = monotonic_ms();
        let second = monotonic_ms();
        assert!(second >= first);
    }

    #[test]
    fn monotonic_ms_is_not_unix_epoch_time() {
        let mono = monotonic_ms();
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("host clock must be after Unix epoch")
            .as_millis();

        assert!(
            mono < wall,
            "monotonic_ms must be process-relative timing, not wall-clock epoch time"
        );
    }

    #[test]
    fn runtime_metadata_cache_returns_same_instance() {
        let first = runtime_metadata() as *const RuntimeMetadata;
        let second = runtime_metadata() as *const RuntimeMetadata;
        assert_eq!(first, second);
    }

    #[test]
    fn metadata_uses_host_env_before_machine_id() {
        let metadata = metadata_from_sources(
            Some(" host-override "),
            Some(" sensor-a "),
            Some(" tenant-a "),
            Some("machine-id"),
        );

        assert_eq!(metadata.host_id.as_deref(), Some("host-override"));
        assert_eq!(metadata.sensor_id.as_deref(), Some("sensor-a"));
        assert_eq!(metadata.tenant_id.as_deref(), Some("tenant-a"));
    }

    #[test]
    fn metadata_uses_machine_id_when_host_env_is_empty() {
        let metadata = metadata_from_sources(Some("  "), None, None, Some(" machine-id "));

        assert_eq!(metadata.host_id.as_deref(), Some("machine-id"));
        assert_eq!(metadata.sensor_id, None);
        assert_eq!(metadata.tenant_id, None);
    }

    #[test]
    fn metadata_rejects_empty_sensor_and_tenant_values() {
        let metadata = metadata_from_sources(None, Some("  "), Some(""), None);

        assert_eq!(
            metadata,
            RuntimeMetadata {
                host_id: None,
                sensor_id: None,
                tenant_id: None,
            }
        );
    }
}
