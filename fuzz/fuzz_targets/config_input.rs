#![no_main]

use aegishv::config::Config;
use libfuzzer_sys::fuzz_target;
use std::io::Write;

const MAX_CONFIG_BYTES: usize = 16 * 1024;

fn fnv64(data: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in data {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fuzz_target!(|data: &[u8]| {
    let data = &data[..data.len().min(MAX_CONFIG_BYTES)];
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let path = std::env::temp_dir().join(format!(
        "aegishv-fuzz-config-{}-{:016x}.toml",
        std::process::id(),
        fnv64(data)
    ));

    if let Ok(mut file) = std::fs::File::create(&path) {
        let _ = file.write_all(text.as_bytes());
        let _ = Config::load(Some(&path));
    }
    let _ = std::fs::remove_file(path);
});
