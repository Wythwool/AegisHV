use crate::event::{
    AddrInfo, Category, Event, IdentityConfidence, IdentityInfo, Severity, ViolationBits,
};
use crate::identity::{IDENTITY_SOURCE_FALLBACK_PID, IDENTITY_SOURCE_TRACE_COMM};
use crate::util::{format_hex, now_rfc3339, parse_comm_pid, parse_hex_u64};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedExit {
    pub vm: String,
    pub vm_id: Option<String>,
    pub raw_comm: String,
    pub host_pid: Option<i32>,
    pub host_cpu: Option<i32>,
    pub vcpu_id: Option<i32>,
    pub arch: String,
    pub reason: String,
    pub rip: Option<String>,
    pub gva: Option<String>,
    pub gpa: Option<String>,
    pub qual: Option<String>,
    pub cr3: Option<String>,
    pub asid: Option<String>,
    pub vmid: Option<String>,
    pub vpid: Option<String>,
    pub bits: Option<ViolationBits>,
    pub parse_source: String,
}

// ParsedExit stays inline because parsed kvm_exit lines immediately feed
// classification and correlation. Boxing would allocate on the parser hot path.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseOutcome {
    Parsed(ParsedExit),
    Unsupported {
        kind: UnsupportedKind,
        detail: String,
    },
    MalformedKvmExit {
        detail: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedKind {
    UnrelatedTracepoint,
    UnsupportedTracepoint,
}

impl UnsupportedKind {
    pub fn as_metric_reason(self) -> &'static str {
        match self {
            Self::UnrelatedTracepoint => "unrelated_tracepoint",
            Self::UnsupportedTracepoint => "unsupported_line",
        }
    }
}

pub fn decode_x86_ept_qual(qual: u64) -> ViolationBits {
    ViolationBits {
        read: (qual & 0x1) != 0,
        write: (qual & 0x2) != 0,
        exec: (qual & 0x4) != 0,
    }
}

pub fn decode_amd_npf_error(info: u64) -> ViolationBits {
    let write = (info & (1 << 1)) != 0;
    let exec = (info & (1 << 4)) != 0;
    let read = !write && !exec;
    ViolationBits { read, write, exec }
}

pub fn decode_arm_esr(esr: u64) -> Option<ViolationBits> {
    let ec = (esr >> 26) & 0x3f;
    match ec {
        0x20 | 0x21 => Some(ViolationBits {
            read: false,
            write: false,
            exec: true,
        }),
        0x24 | 0x25 => {
            let write = (esr & (1 << 6)) != 0;
            Some(ViolationBits {
                read: !write,
                write,
                exec: false,
            })
        }
        _ => None,
    }
}

fn trap_type(reason: &str, arch: &str) -> Option<String> {
    let upper = reason.to_ascii_uppercase();
    if upper.contains("EPT") && upper.contains("VIOLATION") {
        Some("ept_violation".to_string())
    } else if upper.contains("EPT") && upper.contains("MISCONFIG") {
        Some("ept_misconfig".to_string())
    } else if upper.contains("NPT") || upper.contains("NPF") {
        Some("npt_fault".to_string())
    } else if arch == "aarch64" && (upper.contains("STAGE2") || upper.contains("S2")) {
        Some("stage2_fault".to_string())
    } else {
        None
    }
}

pub fn parse_line(line: &str) -> ParseOutcome {
    if !line.contains("kvm_exit:") {
        if line.contains("kvm_") {
            return ParseOutcome::Unsupported {
                kind: UnsupportedKind::UnsupportedTracepoint,
                detail: "non-kvm_exit tracepoint".to_string(),
            };
        }
        return ParseOutcome::Unsupported {
            kind: UnsupportedKind::UnrelatedTracepoint,
            detail: "unrelated line".to_string(),
        };
    }
    match parse_kvm_exit_line_result(line) {
        Ok(p) => ParseOutcome::Parsed(p),
        Err(e) => ParseOutcome::MalformedKvmExit { detail: e },
    }
}

pub fn parse_kvm_exit_line(line: &str) -> Option<ParsedExit> {
    match parse_line(line) {
        ParseOutcome::Parsed(p) => Some(p),
        _ => None,
    }
}

fn parse_kvm_exit_line_result(line: &str) -> Result<ParsedExit, String> {
    let idx = line
        .find("kvm_exit:")
        .ok_or_else(|| "missing kvm_exit tracepoint".to_string())?;
    let header = line[..idx].trim_end_matches(':').trim();
    let rest = line[idx + "kvm_exit:".len()..].trim();
    let mut head_parts = header.split_whitespace();
    let raw_comm = head_parts
        .next()
        .ok_or_else(|| "missing trace comm".to_string())?
        .to_string();
    let host_cpu = parse_host_cpu(header);
    let (vm, host_pid) = parse_comm_pid(&raw_comm);
    let vm_id = host_pid.map(|pid| format!("host-pid:{pid}"));

    let pairs = parse_rest(rest);
    let reason = pairs
        .get("reason")
        .cloned()
        .or_else(|| pairs.get("exit_reason").cloned())
        .unwrap_or_else(|| "UNKNOWN".to_string());
    if reason == "UNKNOWN" && rest.is_empty() {
        return Err("empty kvm_exit payload".to_string());
    }

    let mut rip = pairs
        .get("rip")
        .cloned()
        .or_else(|| pairs.get("pc").cloned());
    let gpa = pairs
        .get("gpa")
        .cloned()
        .or_else(|| pairs.get("ipa").cloned())
        .or_else(|| pairs.get("far").cloned());
    let gva = pairs
        .get("gva")
        .cloned()
        .or_else(|| pairs.get("linear").cloned());
    let mut qual = access_qualification_value(&pairs)
        .cloned()
        .or_else(|| pairs.get("info").cloned())
        .or_else(|| pairs.get("esr").cloned());
    let cr3 = pairs.get("cr3").cloned();
    let asid = pairs.get("asid").cloned();
    let vmid = pairs.get("vmid").cloned();
    let vpid = pairs.get("vpid").cloned();
    let vcpu_id = pairs
        .get("vcpu")
        .or_else(|| pairs.get("vcpu_id"))
        .and_then(|v| v.parse::<i32>().ok());

    let upper_reason = reason.to_ascii_uppercase();
    let arch = if pairs.contains_key("esr")
        || pairs.contains_key("hpfar")
        || upper_reason.contains("STAGE2")
        || upper_reason.contains("HVC")
    {
        "aarch64".to_string()
    } else {
        "x86_64".to_string()
    };

    if rip.is_none() && arch == "aarch64" {
        rip = pairs.get("elr").cloned();
    }

    let bits = if arch == "aarch64" {
        pairs
            .get("esr")
            .and_then(|v| parse_hex_u64(v))
            .and_then(decode_arm_esr)
    } else if upper_reason.contains("NPF") || upper_reason.contains("NPT") {
        pairs
            .get("info")
            .or_else(|| pairs.get("error_code"))
            .and_then(|v| parse_hex_u64(v))
            .map(decode_amd_npf_error)
    } else if upper_reason.contains("EPT") || upper_reason.contains("VIOLATION") {
        access_qualification_value(&pairs)
            .or_else(|| pairs.get("info"))
            .and_then(|v| parse_hex_u64(v))
            .map(decode_x86_ept_qual)
    } else {
        None
    };

    if qual.is_none() {
        qual = pairs.get("error").cloned();
    }

    Ok(ParsedExit {
        vm,
        vm_id,
        raw_comm,
        host_pid,
        host_cpu,
        vcpu_id,
        arch,
        reason,
        rip,
        gva,
        gpa,
        qual,
        cr3,
        asid,
        vmid,
        vpid,
        bits,
        parse_source: "trace_pipe_text".to_string(),
    })
}

fn parse_host_cpu(header: &str) -> Option<i32> {
    let lb = header.find('[')?;
    let rb = header[lb + 1..].find(']')? + lb + 1;
    header[lb + 1..rb].trim().parse::<i32>().ok()
}

fn parse_rest(rest: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    let mut i = 0usize;
    while i < tokens.len() {
        let tok = tokens[i].trim_end_matches(',');
        if let Some((k, v)) = tok.split_once('=') {
            map.insert(normalize_key(k), normalize_value(v));
            i += 1;
            continue;
        }
        let key = normalize_key(tok);
        if is_known_key(&key) {
            if let Some(v) = tokens.get(i + 1) {
                map.insert(key, normalize_value(v));
                i += 2;
                continue;
            }
        } else if i == 0 && !tok.is_empty() {
            // Some tracepoints format the exit reason as the first payload token.
            map.entry("reason".to_string())
                .or_insert_with(|| tok.to_string());
        }
        i += 1;
    }
    map
}

fn normalize_key(k: &str) -> String {
    k.trim()
        .trim_end_matches(':')
        .trim_start_matches("exit_")
        .to_ascii_lowercase()
}

fn normalize_value(v: &str) -> String {
    v.trim().trim_matches(',').trim_matches('"').to_string()
}

fn is_known_key(k: &str) -> bool {
    matches!(
        k,
        "reason"
            | "exit_reason"
            | "rip"
            | "pc"
            | "elr"
            | "gpa"
            | "ipa"
            | "far"
            | "hpfar"
            | "gva"
            | "linear"
            | "qual"
            | "qualification"
            | "exit_qualification"
            | "error_code"
            | "info"
            | "esr"
            | "cr3"
            | "asid"
            | "vmid"
            | "vpid"
            | "vcpu"
            | "vcpu_id"
    )
}

fn access_qualification_value(
    pairs: &std::collections::HashMap<String, String>,
) -> Option<&String> {
    pairs
        .get("qual")
        .or_else(|| pairs.get("exit_qualification"))
        .or_else(|| pairs.get("qualification"))
        .or_else(|| pairs.get("error_code"))
}

pub fn classify_exit(p: &ParsedExit) -> Event {
    let severity = classify_severity(p);
    let mut ev = Event::base(Category::Exit, severity, now_rfc3339(), p.vm.clone());
    ev.vm_id = p.vm_id.clone();
    ev.raw_comm = Some(p.raw_comm.clone());
    ev.host_pid = p.host_pid;
    let mut identity_sources = vec![IDENTITY_SOURCE_TRACE_COMM.to_string()];
    if p.host_pid.is_some() {
        identity_sources.push(IDENTITY_SOURCE_FALLBACK_PID.to_string());
    }
    ev.identity = Some(IdentityInfo {
        sources: identity_sources,
        confidence: IdentityConfidence::Low,
        start_time_verified: false,
        ambiguous: false,
    });
    ev.host_cpu = p.host_cpu;
    ev.vcpu_id = p.vcpu_id;
    ev.vcpu = p.vcpu_id;
    ev.arch = Some(p.arch.clone());
    ev.reason = Some(p.reason.clone());
    ev.trap_type = trap_type(&p.reason, &p.arch);
    ev.cr3 = p.cr3.clone();
    ev.asid = p.asid.clone();
    ev.vmid = p.vmid.clone();
    ev.vpid = p.vpid.clone();
    ev.tags.push(format!("source:{}", p.parse_source));
    if p.bits.is_some() {
        ev.tags.push("memory-access".to_string());
    }
    ev.addr = Some(AddrInfo {
        rip: p.rip.clone(),
        gva: p.gva.clone(),
        gpa: p.gpa.clone(),
        qual: p.qual.clone(),
    });
    ev.violation = p.bits;
    ev
}

pub fn is_parser_degraded(p: &ParsedExit) -> bool {
    p.bits.is_none() && expects_access_bits(&p.reason, &p.arch)
}

fn expects_access_bits(reason: &str, arch: &str) -> bool {
    let upper = reason.to_ascii_uppercase();
    if arch == "aarch64" {
        upper.contains("STAGE2") || upper.contains("S2")
    } else {
        upper.contains("NPF")
            || upper.contains("NPT")
            || upper.contains("EPT") && upper.contains("VIOLATION")
    }
}

fn classify_severity(p: &ParsedExit) -> Severity {
    let upper = p.reason.to_ascii_uppercase();
    if upper.contains("MISCONFIG") {
        Severity::High
    } else if let Some(bits) = p.bits {
        if bits.exec {
            Severity::Medium
        } else {
            Severity::Info
        }
    } else if upper.contains("VIOLATION") || upper.contains("NPF") || upper.contains("STAGE2") {
        Severity::Low
    } else {
        Severity::Info
    }
}

pub fn parsed_gpa_page(p: &ParsedExit, page_size: u64) -> Option<String> {
    let gpa = p.gpa.as_ref().and_then(|v| parse_hex_u64(v))?;
    Some(format_hex(crate::util::page_align(gpa, page_size)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_unrelated_without_parse_error() {
        assert!(matches!(
            parse_line("sched_switch: x"),
            ParseOutcome::Unsupported {
                kind: UnsupportedKind::UnrelatedTracepoint,
                ..
            }
        ));
    }

    #[test]
    fn detects_malformed_kvm_exit() {
        assert!(matches!(
            parse_line("qemu [000]: kvm_exit:"),
            ParseOutcome::MalformedKvmExit { .. }
        ));
    }

    #[test]
    fn trace_comm_identity_metadata_is_low_confidence() {
        let parsed = ParsedExit {
            vm: "qemu-system-x86".to_string(),
            vm_id: Some("host-pid:1234".to_string()),
            raw_comm: "qemu-system-x86-1234".to_string(),
            host_pid: Some(1234),
            host_cpu: Some(0),
            vcpu_id: None,
            arch: "x86_64".to_string(),
            reason: "EPT_VIOLATION".to_string(),
            rip: None,
            gva: None,
            gpa: Some("0x1000".to_string()),
            qual: Some("0x5".to_string()),
            cr3: None,
            asid: None,
            vmid: None,
            vpid: None,
            bits: None,
            parse_source: "trace_pipe_text".to_string(),
        };

        let ev = classify_exit(&parsed);
        let identity = ev.identity.as_ref().unwrap();

        assert_eq!(identity.confidence, IdentityConfidence::Low);
        assert_eq!(
            identity.sources,
            vec![
                IDENTITY_SOURCE_TRACE_COMM.to_string(),
                IDENTITY_SOURCE_FALLBACK_PID.to_string()
            ]
        );
    }
}
