use crate::detectors::{
    score_detection, AttributionQuality, DetectionKind, DetectionRecord, DetectionSource,
    DetectorError, ProfileConfidence, ScoreFactors, SourceReliability,
};
use crate::event::{Category, Event, IdentityConfidence};
use crate::util::parse_hex_u64;

const DETECTOR_ID: &str = "wx_correlation";

pub fn wx_detection_from_event(ev: &Event) -> Result<Option<DetectionRecord>, DetectorError> {
    if ev.category != Category::Wx {
        return Ok(None);
    }
    let Some(wx) = &ev.wx else {
        return Err(DetectorError::MalformedInput {
            detail: "W^X event is missing wx payload".to_string(),
        });
    };
    let source = DetectionSource::new(
        "tracefs_wx_correlation",
        SourceReliability::Tracefs,
        ProfileConfidence::None,
    );
    let attribution = if ev.guest_symbol.is_some() {
        AttributionQuality::GuestSymbol
    } else if ev.guest_process.is_some() {
        AttributionQuality::GuestProcess
    } else if ev
        .addr
        .as_ref()
        .and_then(|addr| addr.gpa.as_ref())
        .is_some()
    {
        AttributionQuality::GuestAddress
    } else {
        AttributionQuality::HostOnly
    };
    let identity = ev
        .identity
        .as_ref()
        .map(|identity| identity.confidence)
        .unwrap_or(IdentityConfidence::Low);
    let score = score_detection(ScoreFactors {
        base_severity: ev.severity,
        source: source.reliability,
        attribution,
        profile: source.profile,
        identity,
        data_loss: ev.data_loss,
        policy_match: ev.rule_id.is_some(),
    });
    let mut record = DetectionRecord::new(
        DETECTOR_ID,
        DetectionKind::WxCorrelation,
        "write then execute on same guest page",
        format!("W^X correlation delta was {} ms", wx.delta_ms),
        source,
        score,
    )
    .with_tag("wx")
    .with_tag("tracefs");
    if let Some(vm_id) = &ev.vm_id {
        record = record.with_vm_id(vm_id);
    }
    if let Some(process) = &ev.guest_process {
        record = record.with_entity(process);
    }
    if let Some(symbol) = ev.guest_symbol.as_ref().or(ev.guest_module.as_ref()) {
        record = record.with_symbol(symbol);
    }
    if let Some(gpa) = ev.addr.as_ref().and_then(|addr| addr.gpa.as_ref()) {
        if let Some(start) = parse_hex_u64(gpa) {
            let end = start.saturating_add(wx.page_size.unwrap_or(4096));
            record = record.with_range(start, end);
        }
    }
    Ok(Some(record))
}

pub fn unsupported_wx_vmi_attribution(reason: impl Into<String>) -> DetectorError {
    DetectorError::Unsupported {
        detector: DETECTOR_ID.to_string(),
        detail: reason.into(),
    }
}

pub fn wx_base_source() -> DetectionSource {
    DetectionSource::new(
        "tracefs_wx_correlation",
        SourceReliability::Tracefs,
        ProfileConfidence::None,
    )
}
