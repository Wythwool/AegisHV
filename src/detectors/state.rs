use std::fs;
use std::path::Path;

use crate::detectors::dedupe::{AggregatedDetection, DetectionDedupeKey};
use crate::detectors::{DetectionKind, DetectorError};
use crate::event::{severity_from_str, Severity};
use crate::incidents::{IncidentRecord, IncidentStatus};

pub const DETECTOR_STATE_VERSION: &str = "aegishv-detector-state-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedDedupeEntry {
    pub key: DetectionDedupeKey,
    pub kind: DetectionKind,
    pub count: u64,
    pub first_seen_ms: u64,
    pub last_seen_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedIncidentEntry {
    pub incident_id: String,
    pub vm_id: String,
    pub status: IncidentStatus,
    pub severity: Severity,
    pub first_seen_ms: u64,
    pub last_seen_ms: u64,
    pub detection_count: u64,
    pub kinds: Vec<DetectionKind>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DetectorState {
    pub dedupe: Vec<PersistedDedupeEntry>,
    pub incidents: Vec<PersistedIncidentEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectorStateEvent {
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectorStateLoad {
    pub state: DetectorState,
    pub sensor_events: Vec<DetectorStateEvent>,
}

impl DetectorState {
    pub fn from_runtime(dedupe: &[AggregatedDetection], incidents: &[IncidentRecord]) -> Self {
        Self {
            dedupe: dedupe
                .iter()
                .map(|entry| PersistedDedupeEntry {
                    key: entry.key.clone(),
                    kind: entry.kind,
                    count: entry.count,
                    first_seen_ms: entry.first_seen_ms,
                    last_seen_ms: entry.last_seen_ms,
                })
                .collect(),
            incidents: incidents
                .iter()
                .map(|incident| PersistedIncidentEntry {
                    incident_id: incident.incident_id.clone(),
                    vm_id: incident.vm_id.clone(),
                    status: incident.status,
                    severity: incident.severity,
                    first_seen_ms: incident.first_seen_ms,
                    last_seen_ms: incident.last_seen_ms,
                    detection_count: incident.detection_count,
                    kinds: incident.kinds.clone(),
                })
                .collect(),
        }
    }
}

pub fn load_detector_state(path: impl AsRef<Path>) -> DetectorStateLoad {
    match fs::read_to_string(path.as_ref()) {
        Ok(text) => match parse_detector_state(&text) {
            Ok(state) => DetectorStateLoad {
                state,
                sensor_events: Vec::new(),
            },
            Err(err) => DetectorStateLoad {
                state: DetectorState::default(),
                sensor_events: vec![DetectorStateEvent {
                    severity: Severity::Medium,
                    message: format!("detector state was ignored: {err}"),
                }],
            },
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => DetectorStateLoad {
            state: DetectorState::default(),
            sensor_events: Vec::new(),
        },
        Err(err) => DetectorStateLoad {
            state: DetectorState::default(),
            sensor_events: vec![DetectorStateEvent {
                severity: Severity::Medium,
                message: format!("detector state could not be read: {err}"),
            }],
        },
    }
}

pub fn save_detector_state(
    path: impl AsRef<Path>,
    state: &DetectorState,
) -> Result<(), DetectorError> {
    let path = path.as_ref();
    let mut tmp = path.to_path_buf();
    tmp.set_extension("tmp");
    fs::write(&tmp, render_detector_state(state)).map_err(|err| DetectorError::Degraded {
        detector: "detector_state".to_string(),
        detail: format!(
            "cannot write detector state temp file {}: {err}",
            tmp.display()
        ),
    })?;
    fs::rename(&tmp, path).map_err(|err| DetectorError::Degraded {
        detector: "detector_state".to_string(),
        detail: format!(
            "cannot replace detector state file {}: {err}",
            path.display()
        ),
    })
}

pub fn render_detector_state(state: &DetectorState) -> String {
    let mut out = String::from(DETECTOR_STATE_VERSION);
    out.push('\n');
    for entry in &state.dedupe {
        out.push_str("dedupe|");
        out.push_str(&join_fields(&[
            &entry.key.detector_id,
            &encode_opt(entry.key.vm_id.as_deref()),
            &encode_opt(entry.key.entity.as_deref()),
            &encode_opt_u64(entry.key.range_start),
            &encode_opt_u64(entry.key.range_end),
            &encode_opt(entry.key.symbol.as_deref()),
            entry.kind.as_str(),
            &entry.count.to_string(),
            &entry.first_seen_ms.to_string(),
            &entry.last_seen_ms.to_string(),
        ]));
        out.push('\n');
    }
    for incident in &state.incidents {
        let kinds = incident
            .kinds
            .iter()
            .map(|kind| kind.as_str())
            .collect::<Vec<_>>()
            .join(",");
        out.push_str("incident|");
        out.push_str(&join_fields(&[
            &incident.incident_id,
            &incident.vm_id,
            incident.status.as_str(),
            incident.severity.as_str(),
            &incident.first_seen_ms.to_string(),
            &incident.last_seen_ms.to_string(),
            &incident.detection_count.to_string(),
            &kinds,
        ]));
        out.push('\n');
    }
    out
}

pub fn parse_detector_state(text: &str) -> Result<DetectorState, DetectorError> {
    let mut lines = text.lines().enumerate().filter_map(|(idx, line)| {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some((idx + 1, trimmed))
        }
    });
    let Some((line, header)) = lines.next() else {
        return Err(malformed("missing detector state header"));
    };
    if header != DETECTOR_STATE_VERSION {
        return Err(malformed(format!(
            "line {line}: expected {DETECTOR_STATE_VERSION}"
        )));
    }

    let mut state = DetectorState::default();
    for (line, entry) in lines {
        let parts = entry.split('|').collect::<Vec<_>>();
        match parts.first().copied() {
            Some("dedupe") => state.dedupe.push(parse_dedupe(line, &parts)?),
            Some("incident") => state.incidents.push(parse_incident(line, &parts)?),
            Some(other) => {
                return Err(malformed(format!(
                    "line {line}: unknown detector state record '{other}'"
                )))
            }
            None => {}
        }
    }
    Ok(state)
}

fn parse_dedupe(line: usize, parts: &[&str]) -> Result<PersistedDedupeEntry, DetectorError> {
    if parts.len() != 11 {
        return Err(malformed(format!(
            "line {line}: dedupe record requires 10 fields"
        )));
    }
    Ok(PersistedDedupeEntry {
        key: DetectionDedupeKey {
            detector_id: parse_text(line, "detector_id", parts[1])?,
            vm_id: parse_opt_text(line, "vm_id", parts[2])?,
            entity: parse_opt_text(line, "entity", parts[3])?,
            range_start: parse_opt_u64(line, "range_start", parts[4])?,
            range_end: parse_opt_u64(line, "range_end", parts[5])?,
            symbol: parse_opt_text(line, "symbol", parts[6])?,
        },
        kind: parse_kind(line, parts[7])?,
        count: parse_u64(line, "count", parts[8])?,
        first_seen_ms: parse_u64(line, "first_seen_ms", parts[9])?,
        last_seen_ms: parse_u64(line, "last_seen_ms", parts[10])?,
    })
}

fn parse_incident(line: usize, parts: &[&str]) -> Result<PersistedIncidentEntry, DetectorError> {
    if parts.len() != 9 {
        return Err(malformed(format!(
            "line {line}: incident record requires 8 fields"
        )));
    }
    Ok(PersistedIncidentEntry {
        incident_id: parse_text(line, "incident_id", parts[1])?,
        vm_id: parse_text(line, "vm_id", parts[2])?,
        status: match parts[3] {
            "open" => IncidentStatus::Open,
            "updated" => IncidentStatus::Updated,
            other => {
                return Err(malformed(format!(
                    "line {line}: unsupported status '{other}'"
                )))
            }
        },
        severity: severity_from_str(parts[4]).ok_or_else(|| {
            malformed(format!("line {line}: unsupported severity '{}'", parts[4]))
        })?,
        first_seen_ms: parse_u64(line, "first_seen_ms", parts[5])?,
        last_seen_ms: parse_u64(line, "last_seen_ms", parts[6])?,
        detection_count: parse_u64(line, "detection_count", parts[7])?,
        kinds: parse_kinds(line, parts[8])?,
    })
}

fn parse_kinds(line: usize, value: &str) -> Result<Vec<DetectionKind>, DetectorError> {
    if value.trim().is_empty() {
        return Err(malformed(format!(
            "line {line}: incident kinds must not be empty"
        )));
    }
    value
        .split(',')
        .map(|kind| parse_kind(line, kind))
        .collect::<Result<Vec<_>, _>>()
}

fn parse_kind(line: usize, value: &str) -> Result<DetectionKind, DetectorError> {
    DetectionKind::parse(value)
        .ok_or_else(|| malformed(format!("line {line}: unsupported detection kind '{value}'")))
}

fn parse_text(line: usize, field: &str, value: &str) -> Result<String, DetectorError> {
    if value.is_empty() || value == "-" || value.contains('\n') || value.contains('|') {
        return Err(malformed(format!(
            "line {line}: field '{field}' is invalid"
        )));
    }
    Ok(value.to_string())
}

fn parse_opt_text(line: usize, field: &str, value: &str) -> Result<Option<String>, DetectorError> {
    if value == "-" {
        Ok(None)
    } else {
        parse_text(line, field, value).map(Some)
    }
}

fn parse_opt_u64(line: usize, field: &str, value: &str) -> Result<Option<u64>, DetectorError> {
    if value == "-" {
        Ok(None)
    } else {
        parse_u64(line, field, value).map(Some)
    }
}

fn parse_u64(line: usize, field: &str, value: &str) -> Result<u64, DetectorError> {
    value.parse().map_err(|_| {
        malformed(format!(
            "line {line}: invalid integer for {field}: '{value}'"
        ))
    })
}

fn join_fields(fields: &[&str]) -> String {
    fields
        .iter()
        .map(|field| sanitize_field(field))
        .collect::<Vec<_>>()
        .join("|")
}

fn encode_opt(value: Option<&str>) -> String {
    value.unwrap_or("-").to_string()
}

fn encode_opt_u64(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn sanitize_field(value: &str) -> String {
    value.replace(['|', '\n', '\r'], "_")
}

fn malformed(detail: impl Into<String>) -> DetectorError {
    DetectorError::MalformedInput {
        detail: detail.into(),
    }
}
