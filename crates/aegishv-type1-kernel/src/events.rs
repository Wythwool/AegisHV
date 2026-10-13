use core::fmt::{self, Write};

#[derive(Clone, Copy)]
pub enum Backend {
    Vmx,
    Svm,
    El2,
}

impl Backend {
    fn name(self) -> &'static str {
        match self {
            Self::Vmx => "vmx",
            Self::Svm => "svm",
            Self::El2 => "el2",
        }
    }
}

pub struct Events<W> {
    writer: W,
    backend: Backend,
    sequence: u64,
}

impl<W: Write> Events<W> {
    pub const fn new(writer: W, backend: Backend) -> Self {
        Self {
            writer,
            backend,
            sequence: 0,
        }
    }

    pub fn denied(&mut self, rip: u64, address: u64, write: bool) -> fmt::Result {
        self.sequence += 1;
        writeln!(self.writer,
            "{{\"source\":\"aegishv-type1\",\"backend\":\"{}\",\"sequence\":{},\"vcpu\":0,\"kind\":\"page_access\",\"rip\":{},\"gpa\":{},\"access\":\"{}\",\"action\":\"deny\"}}",
            self.backend.name(), self.sequence, rip, address, if write { "write" } else { "execute" })
    }

    pub fn syscall(&mut self, rip: u64, register: u32, target: u64, allowed: bool) -> fmt::Result {
        self.sequence += 1;
        writeln!(self.writer,
            "{{\"source\":\"aegishv-type1\",\"backend\":\"{}\",\"sequence\":{},\"vcpu\":0,\"kind\":\"syscall_entry\",\"rip\":{},\"register\":{},\"target\":{},\"action\":\"{}\"}}",
            self.backend.name(), self.sequence, rip, register, target, if allowed { "allow" } else { "deny" })
    }

    pub fn pmu_cycles(&mut self, cycles: u64) -> fmt::Result {
        self.sequence += 1;
        writeln!(self.writer,
            "{{\"source\":\"aegishv-type1\",\"backend\":\"{}\",\"sequence\":{},\"vcpu\":0,\"kind\":\"pmu_sample\",\"counter\":\"cycles\",\"value\":{}}}",
            self.backend.name(), self.sequence, cycles)
    }
}
