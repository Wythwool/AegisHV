use aegishv::parser::{
    classify_exit, is_parser_degraded, parse_kvm_exit_line, parse_line, ParseOutcome,
    UnsupportedKind,
};

fn ept_line(qualification_key: &str, qualification_value: &str) -> String {
    format!(
        "qemu-system-x86-1234 [007] d..2 1.0: kvm_exit: reason EPT_VIOLATION rip 0x1 gpa 0x1000 {qualification_key} {qualification_value}"
    )
}

#[test]
fn parses_sample_lines() {
    let sample = include_str!("../examples/traces/kvm_exit_sample.log");
    let mut n = 0;
    for line in sample.lines() {
        if let Some(p) = parse_kvm_exit_line(line) {
            n += 1;
            assert!(!p.vm.is_empty());
            assert!(!p.reason.is_empty());
            let ev = classify_exit(&p);
            assert_eq!(ev.vm, p.vm);
            assert_eq!(ev.host_cpu, p.host_cpu);
            assert_eq!(ev.vcpu, p.vcpu_id);
        }
    }
    assert!(n > 0);
}

#[test]
fn trace_header_cpu_is_not_guest_vcpu() {
    let line = "qemu-system-x86-1234 [007] d..2 1.0: kvm_exit: reason EPT_VIOLATION rip 0x1 gpa 0x1000 error_code 0x4";
    let parsed = parse_kvm_exit_line(line).unwrap();
    assert_eq!(parsed.host_cpu, Some(7));
    assert_eq!(parsed.vcpu_id, None);
    let ev = classify_exit(&parsed);
    assert_eq!(ev.host_cpu, Some(7));
    assert_eq!(ev.vcpu_id, None);
    assert_eq!(ev.vcpu, None);
}

#[test]
fn explicit_vcpu_is_guest_vcpu() {
    let line = concat!(
        "qemu-system-x86-1234 [007] d..2 1.0: kvm_exit: ",
        "reason EPT_VIOLATION vcpu 3 rip 0x1 gpa 0x1000 error_code 0x4 cr3 0xbeef"
    );
    let parsed = parse_kvm_exit_line(line).unwrap();
    assert_eq!(parsed.host_cpu, Some(7));
    assert_eq!(parsed.vcpu_id, Some(3));
    let ev = classify_exit(&parsed);
    assert_eq!(ev.vcpu_id, Some(3));
    assert_eq!(ev.cr3.as_deref(), Some("0xbeef"));
}

#[test]
fn parses_amd_npf_exec_and_write() {
    let sample = include_str!("../examples/traces/amd_npf_sample.log");
    let mut saw_exec = false;
    let mut saw_write = false;
    for line in sample.lines() {
        let parsed = parse_kvm_exit_line(line).expect("parsed amd line");
        assert_eq!(parsed.arch, "x86_64");
        let bits = parsed.bits.expect("bits");
        if bits.exec {
            saw_exec = true;
        }
        if bits.write {
            saw_write = true;
        }
    }
    assert!(saw_exec);
    assert!(saw_write);
}

#[test]
fn parses_arm_stage2_abort() {
    let sample = include_str!("../examples/traces/arm_stage2_sample.log");
    let parsed: Vec<_> = sample
        .lines()
        .map(|line| parse_kvm_exit_line(line).expect("parsed arm line"))
        .collect();
    assert_eq!(parsed[0].arch, "aarch64");
    assert!(parsed[0].bits.as_ref().expect("bits").read);
    assert!(parsed[1].bits.as_ref().expect("bits").write);
}

#[test]
fn decodes_ept_access_bits_from_qualification_aliases() {
    for key in ["qual", "exit_qualification", "qualification", "error_code"] {
        let parsed = parse_kvm_exit_line(&ept_line(key, "0x6")).expect("parsed ept line");
        let bits = parsed.bits.expect("ept access bits");
        assert!(!bits.read, "{key} should not set read for 0x6");
        assert!(bits.write, "{key} should set write for 0x6");
        assert!(bits.exec, "{key} should set exec for 0x6");
        assert_eq!(parsed.qual.as_deref(), Some("0x6"));
    }
}

#[test]
fn malformed_ept_qualification_does_not_decode_access_bits() {
    let parsed =
        parse_kvm_exit_line(&ept_line("exit_qualification", "not-hex")).expect("parsed ept line");
    assert_eq!(parsed.qual.as_deref(), Some("not-hex"));
    assert!(parsed.bits.is_none());
    assert!(is_parser_degraded(&parsed));
}

#[test]
fn unsupported_outcomes_have_bounded_metric_reasons() {
    let unsupported = parse_line("qemu-system-x86 [000]: kvm_page_fault: gpa 0x1000");
    let unrelated = parse_line("sched_switch: prev_comm=qemu next_comm=worker");

    assert!(matches!(
        unsupported,
        ParseOutcome::Unsupported {
            kind: UnsupportedKind::UnsupportedTracepoint,
            ..
        }
    ));
    assert!(matches!(
        unrelated,
        ParseOutcome::Unsupported {
            kind: UnsupportedKind::UnrelatedTracepoint,
            ..
        }
    ));
    assert_eq!(
        UnsupportedKind::UnsupportedTracepoint.as_metric_reason(),
        "unsupported_line"
    );
    assert_eq!(
        UnsupportedKind::UnrelatedTracepoint.as_metric_reason(),
        "unrelated_tracepoint"
    );
}
