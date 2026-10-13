use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn host_text_gate_rejects_fpu_simd_and_extended_state_instructions() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/tmp")
        .join(format!("type1-host-text-{}", std::process::id()));
    let fixture = directory.join("disassembly.txt");
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create host-text fixture directory");

    fs::write(
        &fixture,
        "ffffffff80200000: 48 89 c3\tmovq %rax, %rbx\nffffffff80200003: e8 00 00 00 00\tcallq 0 <xmm0>\nffffffff80200008: f4\thlt\n",
    )
    .expect("write safe disassembly");
    let safe = Command::new("bash")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .arg("scripts/check-type1-host-text.sh")
        .arg(&fixture)
        .output()
        .expect("run host-text check");
    assert!(
        safe.status.success(),
        "safe disassembly was rejected: {}",
        String::from_utf8_lossy(&safe.stderr)
    );

    for (case, instruction) in [
        ("x87", "fnop"),
        ("XMM", "movdqa %xmm0, %xmm0"),
        ("MMX", "emms"),
        ("AVX", "vzeroupper"),
        ("AVX MXCSR", "vldmxcsr (%rax)"),
        ("AVX MXCSR store", "vstmxcsr (%rax)"),
        ("XSAVE", "xsave64 (%rax)"),
        ("XCR read", "xgetbv"),
        ("XCR", "xsetbv"),
        ("PKRU read", "rdpkru"),
        ("PKRU", "wrpkru"),
        ("MPX", "bndmov %bnd0, %bnd1"),
        ("AMX", "tilezero %tmm0"),
        ("AMX config", "ldtilecfg (%rax)"),
    ] {
        fs::write(&fixture, format!("ffffffff80200000: 90\t{instruction}\n"))
            .expect("write forbidden disassembly");
        let output = Command::new("bash")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .arg("scripts/check-type1-host-text.sh")
            .arg(&fixture)
            .output()
            .expect("run host-text refusal check");
        assert_eq!(output.status.code(), Some(70), "case: {case}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("FPU/SIMD state instruction found"),
            "unexpected stderr for {case}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fs::write(&fixture, "Disassembly of section .text:\n").expect("write empty disassembly");
    let empty = Command::new("bash")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .arg("scripts/check-type1-host-text.sh")
        .arg(&fixture)
        .output()
        .expect("run empty host-text refusal check");
    assert_eq!(empty.status.code(), Some(70));
    assert!(String::from_utf8_lossy(&empty.stderr).contains("contains no instructions"));

    fs::remove_dir_all(directory).expect("remove host-text fixture directory");
}
