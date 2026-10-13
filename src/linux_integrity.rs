use std::collections::BTreeMap;

use crate::linux_vmi::{LinuxTextRange, LinuxVirtualMemoryReader, LinuxVmiError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxTextHashStatus {
    Match,
    Mismatch,
    UnknownBaseline,
}

impl LinuxTextHashStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Match => "match",
            Self::Mismatch => "mismatch",
            Self::UnknownBaseline => "unknown_baseline",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxTextHashResult {
    pub owner: String,
    pub start: u64,
    pub end: u64,
    pub sha256: String,
    pub expected_sha256: Option<String>,
    pub status: LinuxTextHashStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxIntegrityReport {
    pub ok: bool,
    pub results: Vec<LinuxTextHashResult>,
    pub findings: Vec<String>,
}

pub fn check_linux_kernel_text_hash(
    memory: &dyn LinuxVirtualMemoryReader,
    ranges: &[LinuxTextRange],
    baselines: &BTreeMap<String, String>,
) -> Result<LinuxIntegrityReport, LinuxVmiError> {
    check_text_hashes(
        memory,
        &ranges
            .iter()
            .filter(|range| range.owner == "vmlinux")
            .cloned()
            .collect::<Vec<_>>(),
        baselines,
    )
}

pub fn check_linux_module_text_hashes(
    memory: &dyn LinuxVirtualMemoryReader,
    ranges: &[LinuxTextRange],
    baselines: &BTreeMap<String, String>,
) -> Result<LinuxIntegrityReport, LinuxVmiError> {
    check_text_hashes(
        memory,
        &ranges
            .iter()
            .filter(|range| range.owner != "vmlinux")
            .cloned()
            .collect::<Vec<_>>(),
        baselines,
    )
}

pub fn check_text_hashes(
    memory: &dyn LinuxVirtualMemoryReader,
    ranges: &[LinuxTextRange],
    baselines: &BTreeMap<String, String>,
) -> Result<LinuxIntegrityReport, LinuxVmiError> {
    let mut results = Vec::new();
    let mut findings = Vec::new();

    for range in ranges {
        if range.end <= range.start {
            return Err(LinuxVmiError::InconsistentSnapshot {
                detail: format!("text range '{}' is empty or inverted", range.owner),
            });
        }
        let sha256 = hash_virtual_range(memory, range.start, range.end)?;
        let expected = baselines.get(&range.owner).cloned();
        let status = match expected.as_deref() {
            Some(value) if value.eq_ignore_ascii_case(&sha256) => LinuxTextHashStatus::Match,
            Some(_) => LinuxTextHashStatus::Mismatch,
            None => LinuxTextHashStatus::UnknownBaseline,
        };
        if status != LinuxTextHashStatus::Match {
            findings.push(format!(
                "text range '{}' hash status is {}",
                range.owner,
                status.as_str()
            ));
        }
        results.push(LinuxTextHashResult {
            owner: range.owner.clone(),
            start: range.start,
            end: range.end,
            sha256,
            expected_sha256: expected,
            status,
        });
    }

    Ok(LinuxIntegrityReport {
        ok: findings.is_empty(),
        results,
        findings,
    })
}

fn hash_virtual_range(
    memory: &dyn LinuxVirtualMemoryReader,
    start: u64,
    end: u64,
) -> Result<String, LinuxVmiError> {
    let mut hasher = Sha256::new();
    let mut cursor = start;
    let mut buf = [0u8; 4096];
    while cursor < end {
        let remaining = usize::try_from((end - cursor).min(buf.len() as u64)).map_err(|_| {
            LinuxVmiError::Malformed {
                detail: "text range chunk length does not fit this target".to_string(),
            }
        })?;
        memory.read_virtual(cursor, &mut buf[..remaining])?;
        hasher.update(&buf[..remaining]);
        cursor = cursor
            .checked_add(u64::try_from(remaining).expect("chunk length fits u64"))
            .ok_or_else(|| LinuxVmiError::InconsistentSnapshot {
                detail: "text range cursor overflowed".to_string(),
            })?;
    }
    Ok(hex(&hasher.finish()))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex(&hasher.finish())
}

#[derive(Debug, Clone)]
struct Sha256 {
    state: [u32; 8],
    bit_len: u64,
    buffer: [u8; 64],
    buffer_len: usize,
}

impl Sha256 {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            bit_len: 0,
            buffer: [0u8; 64],
            buffer_len: 0,
        }
    }

    fn update(&mut self, mut input: &[u8]) {
        self.bit_len = self
            .bit_len
            .wrapping_add(u64::try_from(input.len()).expect("slice length fits u64") * 8);
        if self.buffer_len > 0 {
            let needed = 64 - self.buffer_len;
            let take = needed.min(input.len());
            self.buffer[self.buffer_len..self.buffer_len + take].copy_from_slice(&input[..take]);
            self.buffer_len += take;
            input = &input[take..];
            if self.buffer_len == 64 {
                let block = self.buffer;
                self.compress(&block);
                self.buffer_len = 0;
            }
        }
        while input.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&input[..64]);
            self.compress(&block);
            input = &input[64..];
        }
        if !input.is_empty() {
            self.buffer[..input.len()].copy_from_slice(input);
            self.buffer_len = input.len();
        }
    }

    fn finish(mut self) -> [u8; 32] {
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;
        if self.buffer_len > 56 {
            self.buffer[self.buffer_len..].fill(0);
            let block = self.buffer;
            self.compress(&block);
            self.buffer_len = 0;
        }
        self.buffer[self.buffer_len..56].fill(0);
        self.buffer[56..64].copy_from_slice(&self.bit_len.to_be_bytes());
        let block = self.buffer;
        self.compress(&block);

        let mut out = [0u8; 32];
        for (idx, value) in self.state.iter().enumerate() {
            out[idx * 4..idx * 4 + 4].copy_from_slice(&value.to_be_bytes());
        }
        out
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for (idx, chunk) in block.chunks_exact(4).enumerate().take(16) {
            w[idx] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        for idx in 16..64 {
            let s0 =
                w[idx - 15].rotate_right(7) ^ w[idx - 15].rotate_right(18) ^ (w[idx - 15] >> 3);
            let s1 = w[idx - 2].rotate_right(17) ^ w[idx - 2].rotate_right(19) ^ (w[idx - 2] >> 10);
            w[idx] = w[idx - 16]
                .wrapping_add(s0)
                .wrapping_add(w[idx - 7])
                .wrapping_add(s1);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        for idx in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[idx])
                .wrapping_add(w[idx]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];
