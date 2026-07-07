use std::path::Path;

const KVM_EXIT_REQUIRED_FIELD_GROUPS: &[ExpectedFieldGroup] = &[
    ExpectedFieldGroup {
        label: "vcpu_id",
        aliases: &["vcpu_id"],
    },
    ExpectedFieldGroup {
        label: "exit_reason",
        aliases: &["exit_reason", "reason"],
    },
    ExpectedFieldGroup {
        label: "instruction_pointer",
        aliases: &["guest_rip", "rip", "pc", "elr"],
    },
];

struct ExpectedFieldGroup {
    label: &'static str,
    aliases: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceField {
    pub name: String,
    pub ty: String,
    pub offset: usize,
    pub size: usize,
    pub signed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TracepointFormat {
    pub system: String,
    pub name: String,
    pub id: Option<u32>,
    pub fields: Vec<TraceField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TracepointDiagnosticStatus {
    Ok,
    Missing,
    Unreadable,
    Malformed,
    MissingFields,
}

impl TracepointDiagnosticStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Missing => "missing",
            Self::Unreadable => "unreadable",
            Self::Malformed => "malformed",
            Self::MissingFields => "missing_fields",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TracepointDiagnostic {
    pub system: String,
    pub name: String,
    pub status: TracepointDiagnosticStatus,
    pub missing_fields: Vec<String>,
    pub message: String,
}

impl TracepointDiagnostic {
    pub fn is_ok(&self) -> bool {
        self.status == TracepointDiagnosticStatus::Ok
    }
}

impl TracepointFormat {
    pub fn has_field(&self, name: &str) -> bool {
        self.fields.iter().any(|f| f.name == name)
    }
}

pub fn diagnose_kvm_tracepoints(root: &Path) -> Vec<TracepointDiagnostic> {
    vec![diagnose_tracepoint_format(
        root,
        "kvm",
        "kvm_exit",
        KVM_EXIT_REQUIRED_FIELD_GROUPS,
    )]
}

fn diagnose_tracepoint_format(
    root: &Path,
    system: &str,
    name: &str,
    required: &[ExpectedFieldGroup],
) -> TracepointDiagnostic {
    let path = root.join("events").join(system).join(name).join("format");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return TracepointDiagnostic {
                system: system.to_string(),
                name: name.to_string(),
                status: TracepointDiagnosticStatus::Missing,
                missing_fields: required
                    .iter()
                    .map(|field| field.label.to_string())
                    .collect(),
                message: format!(
                    "tracepoint {system}/{name} format metadata is missing under {}",
                    root.display()
                ),
            }
        }
        Err(err) => {
            return TracepointDiagnostic {
                system: system.to_string(),
                name: name.to_string(),
                status: TracepointDiagnosticStatus::Unreadable,
                missing_fields: required
                    .iter()
                    .map(|field| field.label.to_string())
                    .collect(),
                message: format!("read tracepoint {system}/{name} format metadata: {err}"),
            }
        }
    };
    let format = match parse_tracepoint_format(system, name, &text) {
        Ok(format) => format,
        Err(err) => {
            return TracepointDiagnostic {
                system: system.to_string(),
                name: name.to_string(),
                status: TracepointDiagnosticStatus::Malformed,
                missing_fields: required
                    .iter()
                    .map(|field| field.label.to_string())
                    .collect(),
                message: err,
            }
        }
    };
    let missing_fields = required
        .iter()
        .filter(|group| !group.aliases.iter().any(|alias| format.has_field(alias)))
        .map(|group| group.label.to_string())
        .collect::<Vec<_>>();
    if missing_fields.is_empty() {
        TracepointDiagnostic {
            system: system.to_string(),
            name: name.to_string(),
            status: TracepointDiagnosticStatus::Ok,
            missing_fields,
            message: format!(
                "tracepoint {system}/{name} format metadata contains expected parser fields"
            ),
        }
    } else {
        TracepointDiagnostic {
            system: system.to_string(),
            name: name.to_string(),
            status: TracepointDiagnosticStatus::MissingFields,
            message: format!(
                "tracepoint {system}/{name} format metadata is missing expected field groups: {}",
                missing_fields.join(",")
            ),
            missing_fields,
        }
    }
}

pub fn read_tracepoint_format(
    root: &Path,
    system: &str,
    name: &str,
) -> Result<TracepointFormat, String> {
    let path = root.join("events").join(system).join(name).join("format");
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    parse_tracepoint_format(system, name, &text)
}

pub fn parse_tracepoint_format(
    system: &str,
    name: &str,
    text: &str,
) -> Result<TracepointFormat, String> {
    let mut id = None;
    let mut fields = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("ID:") {
            id = v.trim().parse::<u32>().ok();
        } else if let Some(v) = line.strip_prefix("field:") {
            if let Some(field) = parse_field_line(v) {
                fields.push(field);
            }
        }
    }
    if fields.is_empty() {
        return Err(format!("tracepoint {system}/{name} has no fields"));
    }
    Ok(TracepointFormat {
        system: system.to_string(),
        name: name.to_string(),
        id,
        fields,
    })
}

fn parse_field_line(line: &str) -> Option<TraceField> {
    let mut parts = line.split(';').map(str::trim);
    let decl = parts.next()?.trim();
    let offset = find_attr(line, "offset:")?.parse().ok()?;
    let size = find_attr(line, "size:")?.parse().ok()?;
    let signed = find_attr(line, "signed:")
        .map(|v| v == "1")
        .unwrap_or(false);
    let mut tokens = decl.split_whitespace().collect::<Vec<_>>();
    let name = tokens.pop()?.trim_start_matches('*').to_string();
    let ty = tokens.join(" ");
    Some(TraceField {
        name,
        ty,
        offset,
        size,
        signed,
    })
}

fn find_attr<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let idx = line.find(key)? + key.len();
    let tail = &line[idx..];
    Some(tail.split(';').next()?.trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_tracefs(label: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "aegishv-trace-format-{label}-{}-{}",
            std::process::id(),
            crate::util::next_sequence()
        ));
        std::fs::create_dir_all(path.join("events/kvm/kvm_exit")).expect("create tracefs fixture");
        path
    }

    fn write_kvm_exit_format(root: &Path, text: &str) {
        let path = root.join("events/kvm/kvm_exit/format");
        let mut file = std::fs::File::create(path).expect("create format fixture");
        write!(file, "{text}").expect("write format fixture");
    }

    #[test]
    fn parses_format_file() {
        let text = "name: kvm_exit\nID: 123\nformat:\n\tfield:unsigned short common_type;\toffset:0;\tsize:2;\tsigned:0;\n\tfield:u32 vcpu_id;\toffset:8;\tsize:4;\tsigned:0;\n";
        let f = parse_tracepoint_format("kvm", "kvm_exit", text).unwrap();
        assert_eq!(f.id, Some(123));
        assert!(f.has_field("vcpu_id"));
    }

    #[test]
    fn diagnoses_kvm_exit_format_with_required_fields() {
        let root = temp_tracefs("ok");
        write_kvm_exit_format(
            &root,
            "name: kvm_exit\nID: 123\nformat:\n\tfield:u32 vcpu_id;\toffset:8;\tsize:4;\tsigned:0;\n\tfield:u32 exit_reason;\toffset:12;\tsize:4;\tsigned:0;\n\tfield:unsigned long guest_rip;\toffset:16;\tsize:8;\tsigned:0;\n",
        );

        let diagnostics = diagnose_kvm_tracepoints(&root);

        assert_eq!(diagnostics[0].status, TracepointDiagnosticStatus::Ok);
        assert!(diagnostics[0].missing_fields.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn diagnoses_kvm_exit_format_missing_expected_fields() {
        let root = temp_tracefs("missing-fields");
        write_kvm_exit_format(
            &root,
            "name: kvm_exit\nID: 123\nformat:\n\tfield:u32 vcpu_id;\toffset:8;\tsize:4;\tsigned:0;\n",
        );

        let diagnostics = diagnose_kvm_tracepoints(&root);

        assert_eq!(
            diagnostics[0].status,
            TracepointDiagnosticStatus::MissingFields
        );
        assert_eq!(
            diagnostics[0].missing_fields,
            ["exit_reason", "instruction_pointer"]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn diagnoses_missing_kvm_exit_format() {
        let root = temp_tracefs("missing");
        let _ = std::fs::remove_file(root.join("events/kvm/kvm_exit/format"));

        let diagnostics = diagnose_kvm_tracepoints(&root);

        assert_eq!(diagnostics[0].status, TracepointDiagnosticStatus::Missing);
        assert!(diagnostics[0]
            .message
            .contains("format metadata is missing"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn diagnoses_malformed_kvm_exit_format() {
        let root = temp_tracefs("malformed");
        write_kvm_exit_format(&root, "name: kvm_exit\nformat:\n\tfield:bad field\n");

        let diagnostics = diagnose_kvm_tracepoints(&root);

        assert_eq!(diagnostics[0].status, TracepointDiagnosticStatus::Malformed);
        assert!(diagnostics[0].message.contains("has no fields"));
        let _ = std::fs::remove_dir_all(root);
    }
}
