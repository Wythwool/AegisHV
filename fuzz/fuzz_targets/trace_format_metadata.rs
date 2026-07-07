#![no_main]

use aegishv::trace_format::parse_tracepoint_format;
use libfuzzer_sys::fuzz_target;

const MAX_FORMAT_BYTES: usize = 16 * 1024;

fuzz_target!(|data: &[u8]| {
    let data = &data[..data.len().min(MAX_FORMAT_BYTES)];
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    if let Ok(format) = parse_tracepoint_format("kvm", "kvm_exit", text) {
        let _ = format.has_field("vcpu_id");
        let _ = format.has_field("exit_reason");
        let _ = format.has_field("guest_rip");
    }
});
