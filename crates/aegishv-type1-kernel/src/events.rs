use core::fmt::{self, Write};

#[derive(Clone, Copy)]
pub enum Backend {
    Vmx,
    Svm,
    El2,
}

#[derive(Clone, Copy)]
pub enum Access {
    Read,
    Write,
    Execute,
}

#[derive(Clone, Copy)]
pub enum Action {
    Allow,
    Deny,
    Emulate,
}

impl Action {
    fn name(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Emulate => "emulate",
        }
    }
}

impl Access {
    fn name(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Execute => "execute",
        }
    }
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
    vcpu: usize,
}

impl<W: Write> Events<W> {
    pub const fn new(writer: W, backend: Backend) -> Self {
        Self {
            writer,
            backend,
            sequence: 0,
            vcpu: 0,
        }
    }

    pub fn denied(&mut self, rip: u64, address: u64, access: Access) -> fmt::Result {
        self.page(rip, address, access, Action::Deny)
    }

    pub fn cpu(&mut self, vcpu: usize) {
        self.vcpu = vcpu;
    }

    fn prefix(&mut self, kind: &str) -> fmt::Result {
        self.sequence += 1;
        write!(self.writer,
            "{{\"source\":\"aegishv-type1\",\"backend\":\"{}\",\"sequence\":{},\"vcpu\":{},\"kind\":\"{}\"",
            self.backend.name(), self.sequence, self.vcpu, kind)
    }

    pub fn page(&mut self, rip: u64, address: u64, access: Access, action: Action) -> fmt::Result {
        self.prefix("page_access")?;
        writeln!(
            self.writer,
            ",\"rip\":{},\"gpa\":{},\"access\":\"{}\",\"action\":\"{}\"}}",
            rip,
            address,
            access.name(),
            action.name()
        )
    }

    pub fn syscall(&mut self, rip: u64, register: u32, target: u64, allowed: bool) -> fmt::Result {
        self.prefix("syscall_entry")?;
        writeln!(
            self.writer,
            ",\"rip\":{},\"register\":{},\"target\":{},\"action\":\"{}\"}}",
            rip,
            register,
            target,
            if allowed { "allow" } else { "deny" }
        )
    }

    pub fn pmu_cycles(&mut self, cycles: u64) -> fmt::Result {
        self.prefix("pmu_sample")?;
        writeln!(self.writer, ",\"counter\":\"cycles\",\"value\":{cycles}}}")
    }

    pub fn pmu_status(&mut self, available: bool) -> fmt::Result {
        self.prefix("pmu_status")?;
        writeln!(self.writer, ",\"available\":{available}}}")
    }

    pub fn vectors(
        &mut self,
        rip: u64,
        target: u64,
        address: u64,
        valid: bool,
        changed: bool,
    ) -> fmt::Result {
        self.prefix("vector_sample")?;
        writeln!(self.writer,
            ",\"rip\":{rip},\"target\":{target},\"gpa\":{address},\"valid\":{valid},\"changed\":{changed}}}")
    }

    pub fn stopped(&mut self, rip: u64, result: u64, budget: bool) -> fmt::Result {
        self.prefix(if budget { "vcpu_budget" } else { "guest_halt" })?;
        writeln!(self.writer, ",\"rip\":{rip},\"result\":{result}}}")
    }

    pub fn exception(&mut self, rip: u64, vector: u64, address: u64) -> fmt::Result {
        self.prefix("guest_exception")?;
        writeln!(
            self.writer,
            ",\"rip\":{rip},\"vector\":{vector},\"address\":{address}}}"
        )
    }
}
