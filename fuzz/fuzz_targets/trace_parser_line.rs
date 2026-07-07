#![no_main]

use aegishv::parser::{
    classify_exit, is_parser_degraded, parse_line, parsed_gpa_page, ParseOutcome,
};
use libfuzzer_sys::fuzz_target;

const MAX_LINE_BYTES: usize = 4096;

fuzz_target!(|data: &[u8]| {
    let data = &data[..data.len().min(MAX_LINE_BYTES)];
    let Ok(line) = std::str::from_utf8(data) else {
        return;
    };

    if let ParseOutcome::Parsed(parsed) = parse_line(line) {
        let event = classify_exit(&parsed);
        let _ = event.to_json();
        let _ = is_parser_degraded(&parsed);
        let _ = parsed_gpa_page(&parsed, 4096);
    }
});
