use regex::Regex;

use crate::{EptInfo, Event};

#[derive(Debug, Clone)]
pub struct ParseOut {
    pub vm: String,
    pub vcpu: i32,
    pub arch: String,
    pub reason: String,
    pub rip: Option<String>,
    pub gpa: Option<String>,
    pub ept: Option<EptInfo>,
}

fn parse_hex(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(n) = s.strip_prefix("0x") {
        u64::from_str_radix(n, 16).ok()
    } else {
        s.parse::<u64>().ok()
    }
}

fn decode_ept_qual(q: u64) -> EptInfo {
    // Intel SDM: bits 0 read, 1 write, 2 exec.
    let r = (q & 1) != 0;
    let w = (q & 2) != 0;
    let x = (q & 4) != 0;
    EptInfo {
        read: r,
        write: w,
        exec: x,
        qual: format!("0x{:x}", q),
    }
}

pub fn parse_trace_line(line: &str) -> Option<ParseOut> {
    if !line.contains("kvm_exit") { return None; }

    let arch = if line.contains("qemu-system-x86") { "x86_64" } else if line.contains("aarch64") { "aarch64" } else { "unknown" };

    let re = Regex::new(r":\s*kvm_exit:\s*reason\s+([A-Z0-9_]+).*").unwrap();
    let reason = re.captures(line).and_then(|c| c.get(1)).map(|m| m.as_str().to_string()).unwrap_or_else(|| "UNKNOWN".into());

    let rip = Regex::new(r"(rip|pc)\s+(0x[0-9a-fA-F]+)").ok()
        .and_then(|re| re.captures(line)).and_then(|c| c.get(2)).map(|m| m.as_str().to_string());

    let gpa = Regex::new(r"(gpa|far)\s+(0x[0-9a-fA-F]+)").ok()
        .and_then(|re| re.captures(line)).and_then(|c| c.get(2)).map(|m| m.as_str().to_string());

    let qual = Regex::new(r"(error_code|info|qual)\s+(0x[0-9a-fA-F]+)").ok()
        .and_then(|re| re.captures(line)).and_then(|c| c.get(2)).and_then(|m| parse_hex(m.as_str()));

    let vcpu = Regex::new(r"vcpu\s+(\d+)").ok().and_then(|re| re.captures(line)).and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<i32>().ok()).unwrap_or(0);

    let vm = {
        let parts: Vec<&str> = line.split_whitespace().collect();
        parts.get(0).map(|s| s.to_string()).unwrap_or_else(|| "unknown".into())
    };

    let ept = if reason.contains("EPT") || reason.contains("NPT") {
        qual.map(decode_ept_qual)
    } else {
        None
    };

    Some(ParseOut { vm, vcpu, arch: arch.into(), reason, rip, gpa, ept })
}

pub fn to_event(p: ParseOut) -> Event {
    let (severity, message) = classify(&p);
    Event {
        version: 1,
        ts: time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap(),
        arch: p.arch,
        vm: p.vm,
        vcpu: p.vcpu,
        reason: p.reason,
        rip: p.rip,
        gpa: p.gpa,
        ept: p.ept,
        severity,
        message,
    }
}

fn classify(p: &ParseOut) -> (String, String) {
    if p.reason == "EPT_VIOLATION" || p.reason == "NPT_VIOLATION" {
        if let Some(e) = &p.ept {
            if e.exec && !e.write {
                return ("high".into(), format!("exec violation at {} gpa={:?}", p.rip.clone().unwrap_or_default(), p.gpa));
            }
            if e.exec && e.write {
                return ("critical".into(), format!("write+exec violation at gpa={:?}", p.gpa));
            }
            if e.write {
                return ("medium".into(), format!("write violation gpa={:?}", p.gpa));
            }
            return ("low".into(), "read violation".into());
        }
    }
    if p.reason.contains("EPT") && p.reason.contains("MISCONFIG") {
        return ("high".into(), "EPT misconfig".into());
    }
    ("info".into(), p.reason.clone())
}
