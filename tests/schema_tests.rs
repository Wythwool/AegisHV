use aegishv::event::{Category, Event, Severity};

#[test]
fn emitted_event_contains_schema_v2_contract_fields() {
    let ev = Event::base(
        Category::Sensor,
        Severity::Info,
        "2026-01-01T00:00:00Z".to_string(),
        "host".to_string(),
    );
    let json = ev.to_json();
    assert!(json.contains("\"schema_version\":2"));
    assert!(json.contains("\"sequence\":"));
    assert!(json.contains("\"monotonic_ms\":"));
    assert!(json.contains("\"data_loss\":false"));
}
