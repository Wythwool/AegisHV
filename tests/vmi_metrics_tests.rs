use aegishv::metrics::{Metrics, VmiMetricArchitecture, VmiMetricTranslationMode};
use aegishv::vmi::VmiErrorKind;

#[test]
fn vmi_memory_read_metrics_record_attempts_successes_and_typed_failures() {
    let metrics = Metrics::new().unwrap();

    metrics.record_vmi_memory_read_attempt();
    metrics.record_vmi_memory_read_success();
    metrics.record_vmi_memory_read_failure(VmiErrorKind::MissingMemory);
    metrics.record_vmi_memory_read_failure(VmiErrorKind::PermissionDenied);

    let text = metrics.encode();

    assert!(text.contains("aegishv_vmi_memory_read_attempts_total 1"));
    assert!(text.contains("aegishv_vmi_memory_read_successes_total 1"));
    assert!(text.contains("aegishv_vmi_memory_read_failures_total{kind=\"missing_memory\"} 1"));
    assert!(text.contains("aegishv_vmi_memory_read_failures_total{kind=\"permission_denied\"} 1"));
}

#[test]
fn vmi_translation_metrics_record_bounded_arch_mode_and_error_kind() {
    let metrics = Metrics::new().unwrap();

    metrics.record_vmi_translation_attempt(
        VmiMetricArchitecture::X86_64,
        VmiMetricTranslationMode::X86_64FourLevel,
    );
    metrics.record_vmi_translation_success(
        VmiMetricArchitecture::X86_64,
        VmiMetricTranslationMode::X86_64FourLevel,
    );
    metrics.record_vmi_translation_attempt(
        VmiMetricArchitecture::Arm64,
        VmiMetricTranslationMode::Arm64Stage1Size4K,
    );
    metrics.record_vmi_translation_failure(
        VmiMetricArchitecture::Arm64,
        VmiMetricTranslationMode::Arm64Stage1Size4K,
        VmiErrorKind::InvalidAddress,
    );

    let text = metrics.encode();

    assert!(text.contains(
        "aegishv_vmi_translation_attempts_total{architecture=\"x86_64\",mode=\"x86_64-4level\"} 1"
    ));
    assert!(text.contains(
        "aegishv_vmi_translation_successes_total{architecture=\"x86_64\",mode=\"x86_64-4level\"} 1"
    ));
    assert!(text.contains(
        "aegishv_vmi_translation_attempts_total{architecture=\"arm64\",mode=\"arm64-stage1-4k\"} 1"
    ));
    assert!(text.contains(
        "aegishv_vmi_translation_failures_total{architecture=\"arm64\",mode=\"arm64-stage1-4k\",kind=\"invalid_address\"} 1"
    ));
}

#[test]
fn vmi_register_profile_fixture_and_unsupported_metrics_render_typed_counters() {
    let metrics = Metrics::new().unwrap();

    metrics.record_vmi_register_access_attempt();
    metrics.record_vmi_register_access_failure(VmiErrorKind::InvalidInput);
    metrics.record_vmi_profile_lookup_attempt();
    metrics.record_vmi_profile_lookup_miss();
    metrics.record_vmi_profile_lookup_failure(VmiErrorKind::MissingProfile);
    metrics.record_vmi_fixture_load_attempt();
    metrics.record_vmi_fixture_load_failure(VmiErrorKind::Malformed);
    metrics.record_vmi_unsupported_backend_call();

    let text = metrics.encode();

    assert!(text.contains("aegishv_vmi_register_access_attempts_total 1"));
    assert!(text.contains("aegishv_vmi_register_access_failures_total{kind=\"invalid_input\"} 1"));
    assert!(text.contains("aegishv_vmi_profile_lookup_attempts_total 1"));
    assert!(text.contains("aegishv_vmi_profile_lookup_misses_total 1"));
    assert!(text.contains("aegishv_vmi_profile_lookup_failures_total{kind=\"missing_profile\"} 1"));
    assert!(text.contains("aegishv_vmi_fixture_load_attempts_total 1"));
    assert!(text.contains("aegishv_vmi_fixture_load_failures_total{kind=\"malformed\"} 1"));
    assert!(text.contains("aegishv_vmi_unsupported_backend_calls_total 1"));
}

#[test]
fn vmi_metrics_do_not_render_addresses_paths_or_error_details() {
    let metrics = Metrics::new().unwrap();

    metrics.record_vmi_memory_read_failure(VmiErrorKind::Unmapped);
    metrics.record_vmi_translation_failure(
        VmiMetricArchitecture::X86_64,
        VmiMetricTranslationMode::X86_64La57,
        VmiErrorKind::TranslationFailure,
    );
    metrics.record_vmi_fixture_load_failure(VmiErrorKind::TemporarilyUnavailable);

    let text = metrics.encode();

    for forbidden in [
        "0x1000",
        "0xffff800000001000",
        "C:\\Users\\User",
        "/sys/kernel",
        "tests/fixtures/vmi",
        "snapshot.map",
        "bad descriptor detail",
        "kernel_or_build",
    ] {
        assert!(
            !text.contains(forbidden),
            "VMI metrics leaked forbidden text: {forbidden}"
        );
    }
}
