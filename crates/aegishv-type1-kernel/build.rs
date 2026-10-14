fn main() {
    println!("cargo:rustc-check-cfg=cfg(linux_guest)");
    println!("cargo:rerun-if-env-changed=AEGISHV_LINUX");
    if std::env::var("AEGISHV_LINUX").as_deref() == Ok("1") {
        println!("cargo:rustc-cfg=linux_guest");
    }
    println!("cargo:rerun-if-changed=../../boot/linker/x86_64-type1.ld");
    println!("cargo:rerun-if-changed=../../boot/linker/arm64-type1.ld");
    println!("cargo:rerun-if-env-changed=AEGISHV_GUEST_ELF");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("guest.elf");
    let bytes = match std::env::var_os("AEGISHV_GUEST_ELF") {
        Some(path) => {
            let path = std::path::PathBuf::from(path)
                .canonicalize()
                .expect("guest ELF path");
            println!("cargo:rerun-if-changed={}", path.display());
            let bytes = std::fs::read(path).expect("read guest ELF");
            assert!(bytes.len() <= 1024 * 1024, "guest ELF exceeds 1 MiB");
            bytes
        }
        None => Vec::new(),
    };
    std::fs::write(output, bytes).expect("write guest ELF");
}
