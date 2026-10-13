use aegishv::linux_symbols::{parse_kallsyms_text, parse_system_map_text};
use aegishv::vmi::VmiErrorKind;

#[test]
fn parses_kallsyms_addresses_types_names_and_modules() {
    let table = parse_kallsyms_text(
        r#"
ffffffff81000000 T _stext
ffffffff81000100 t entry_SYSCALL_64
ffffffffc0010000 t hooked_entry [sample_module]
"#,
    )
    .expect("parse kallsyms");

    assert_eq!(table.symbols().len(), 3);
    assert_eq!(table.unique_by_name("_stext").unwrap().kind, 'T');
    assert_eq!(
        table
            .unique_by_name("hooked_entry")
            .unwrap()
            .module
            .as_deref(),
        Some("sample_module")
    );
}

#[test]
fn duplicate_symbol_names_are_kept_but_unique_lookup_is_refused() {
    let table = parse_kallsyms_text(
        r#"
ffffffff81000000 T duplicate_name
ffffffff81001000 t duplicate_name
"#,
    )
    .expect("parse kallsyms with duplicates");

    assert_eq!(table.by_name("duplicate_name").len(), 2);
    let err = table
        .unique_by_name("duplicate_name")
        .expect_err("ambiguous symbol lookup must fail");

    assert_eq!(err.kind(), VmiErrorKind::InconsistentSnapshot);
    assert!(err.to_string().contains("ambiguous"));
}

#[test]
fn rejects_malformed_symbol_lines() {
    let err = parse_kallsyms_text("not-hex T _stext\n").expect_err("bad address must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("invalid symbol address"));

    let err =
        parse_kallsyms_text("ffffffff81000000 text _stext\n").expect_err("bad type must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("invalid symbol type"));

    let err = parse_system_map_text("ffffffffc0010000 t handler [mod]\n")
        .expect_err("System.map module suffix must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("System.map"));
}

#[test]
fn rejects_empty_symbol_maps() {
    let err = parse_system_map_text("# empty\n\n").expect_err("empty map must fail");

    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("does not contain any symbols"));
}
