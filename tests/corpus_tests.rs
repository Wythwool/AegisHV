use aegishv::config::{Config, WxAllowEntry};
use aegishv::parser::{classify_exit, parse_kvm_exit_line};
use aegishv::wx::WxEngine;

#[test]
fn malicious_corpus_produces_wx_alert() {
    let cfg = Config::default();
    let wx = WxEngine::new(&cfg);
    let sample = include_str!("../corpus/malicious/wx_same_vm_same_as.log");

    let mut alerts = 0;
    for line in sample.lines().filter(|l| !l.trim().is_empty()) {
        let parsed = parse_kvm_exit_line(line).expect("malicious corpus line parses");
        let ev = classify_exit(&parsed);
        if wx.on_exit_event(&ev).is_some() {
            alerts += 1;
        }
    }

    assert!(alerts >= 1, "expected W^X alert from malicious corpus");
}

#[test]
fn benign_corpus_does_not_alert() {
    let mut cfg = Config::default();
    cfg.wx_allow.entries.push(WxAllowEntry {
        vm: "qemu-system-x86".to_string(),
        gpa_prefix: "0x7f".to_string(),
    });
    let wx = WxEngine::new(&cfg);
    let sample = include_str!("../corpus/benign/jit_allowed.log");

    for line in sample.lines().filter(|l| !l.trim().is_empty()) {
        let parsed = parse_kvm_exit_line(line).expect("benign corpus line parses");
        let ev = classify_exit(&parsed);
        assert!(
            wx.on_exit_event(&ev).is_none(),
            "benign corpus should not alert"
        );
    }
}
