use aegishv::event::{Category, Event, Severity, TrapInfo, ViolationBits};

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

#[test]
fn emitted_event_can_include_trap_contract_fields() {
    let mut ev = Event::base(
        Category::Wx,
        Severity::Critical,
        "2026-01-01T00:00:00Z".to_string(),
        "vm-a".to_string(),
    );
    ev.trap = Some(TrapInfo {
        trap_id: "trap:vm-a:as0:0x2000:execute".to_string(),
        trap_kind: "execute".to_string(),
        backend: "synthetic".to_string(),
        page: "0x2000".to_string(),
        permissions_before: Some(ViolationBits {
            read: true,
            write: false,
            exec: false,
        }),
        permissions_after: Some(ViolationBits {
            read: true,
            write: false,
            exec: true,
        }),
        decision: "allow_step".to_string(),
        invalidation_status: "recorded".to_string(),
    });

    let json = ev.to_json();

    assert!(json.contains("\"trap\":{"));
    assert!(json.contains("\"trap_id\":\"trap:vm-a:as0:0x2000:execute\""));
    assert!(json.contains("\"permissions_before\":{\"read\":true,\"write\":false,\"exec\":false}"));
    assert!(json.contains("\"decision\":\"allow_step\""));
}
