
use aegishv::parser::parse_trace_line;

#[test]
fn parse_ept_bits() {
    let line = "qemu-system-x86-1234 [001] ...: kvm_exit: reason EPT_VIOLATION rip 0x7f gpa 0x1000 error_code 0x5";
    let p = parse_trace_line(line).unwrap();
    let e = p.ept.unwrap();
    assert!(e.exec && e.read && !e.write);
}
