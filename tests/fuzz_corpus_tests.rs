use std::path::Path;

use aegishv::config::Config;

#[test]
fn config_minimal_seed_is_valid_aegishv_config() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fuzz/corpus/config_input/minimal.toml");
    Config::load(Some(&path)).unwrap_or_else(|err| {
        panic!(
            "fuzz config minimal seed must be accepted by Config::load: {}",
            err
        )
    });
}
