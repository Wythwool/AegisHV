struct Bank {
    enabled: u32,
    pending: u32,
    active: u32,
    priority: [u8; 32],
    targets: [u8; 32],
}

impl Bank {
    const fn new() -> Self {
        Self {
            enabled: 0,
            pending: 0,
            active: 0,
            priority: [0xa0; 32],
            targets: [1; 32],
        }
    }

    fn level(&mut self, id: u32, asserted: bool) {
        if asserted {
            self.pending |= 1 << id;
        } else {
            self.pending &= !(1 << id);
        }
    }
}

pub struct Distributor {
    enabled: bool,
    cpus: u8,
    shared: Bank,
}

impl Distributor {
    pub const fn new(cpus: u8) -> Self {
        assert!(cpus > 0 && cpus <= 8);
        Self {
            enabled: false,
            cpus,
            shared: Bank::new(),
        }
    }

    pub fn level(&mut self, id: u32, asserted: bool) {
        assert!((32..64).contains(&id));
        self.shared.level(id - 32, asserted);
    }

    pub fn targets(&self, id: u32) -> u8 {
        assert!((32..64).contains(&id));
        self.shared.targets[(id - 32) as usize]
    }
}

pub struct Cpu {
    enabled: bool,
    mask: u32,
    local: Bank,
    sources: [u8; 16],
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub const fn new() -> Self {
        Self {
            enabled: false,
            mask: 0,
            local: Bank::new(),
            sources: [0; 16],
        }
    }

    pub fn level(&mut self, id: u32, asserted: bool) {
        assert!((16..32).contains(&id));
        self.local.level(id, asserted);
    }

    pub fn sgi(&mut self, id: u32, source: usize) {
        assert!(id < 16 && source < 8);
        self.sources[id as usize] |= 1 << source;
        self.local.pending |= 1 << id;
    }

    pub fn next(&self, distributor: &Distributor, cpu: usize) -> u32 {
        assert!(cpu < usize::from(distributor.cpus));
        if !distributor.enabled || !self.enabled {
            return 1023;
        }
        (0..64)
            .filter(|id| {
                let bank = if *id < 32 {
                    &self.local
                } else {
                    &distributor.shared
                };
                let bit = id % 32;
                bank.pending & bank.enabled & !bank.active & (1 << bit) != 0
                    && u32::from(bank.priority[bit]) < self.mask
                    && (*id < 32 || bank.targets[bit] & (1 << cpu) != 0)
            })
            .min_by_key(|id| {
                if *id < 32 {
                    self.local.priority[*id]
                } else {
                    distributor.shared.priority[id - 32]
                }
            })
            .map_or(1023, |id| id as u32)
    }

    pub fn access(
        &mut self,
        distributor: &mut Distributor,
        cpu: usize,
        address: u64,
        value: Option<u32>,
        bytes: usize,
    ) -> Option<u32> {
        if !matches!(bytes, 1 | 2 | 4)
            || address as usize % bytes != 0
            || cpu >= usize::from(distributor.cpus)
        {
            return None;
        }
        let offset = address & 0xffff;
        if address & !0xffff == 0x0801_0000 {
            if bytes != 4 {
                return None;
            }
            return Some(match (offset, value) {
                (0, Some(value)) => {
                    self.enabled = value & 1 != 0;
                    0
                }
                (0, None) => u32::from(self.enabled),
                (4, Some(value)) => {
                    self.mask = value & 255;
                    0
                }
                (4, None) => self.mask,
                (0xc, None) => {
                    let id = self.next(distributor, cpu);
                    if id == 1023 {
                        return Some(id);
                    }
                    let bank = if id < 32 {
                        &mut self.local
                    } else {
                        &mut distributor.shared
                    };
                    bank.pending &= !(1 << (id % 32));
                    bank.active |= 1 << (id % 32);
                    if id < 16 && self.sources[id as usize] != 0 {
                        let source = self.sources[id as usize].trailing_zeros();
                        self.sources[id as usize] &= !(1 << source);
                        if self.sources[id as usize] != 0 {
                            bank.pending |= 1 << id;
                        }
                        id | (source << 10)
                    } else {
                        id
                    }
                }
                (0x10 | 0x1000, Some(value)) => {
                    let id = value & 1023;
                    if id < 64 {
                        let bank = if id < 32 {
                            &mut self.local
                        } else {
                            &mut distributor.shared
                        };
                        bank.active &= !(1 << (id % 32));
                    }
                    0
                }
                (0xfc, None) => 0x0202_043b,
                _ => 0,
            });
        }
        if address & !0xffff != 0x0800_0000 {
            return None;
        }
        if matches!(offset, 0x400..=0x43f | 0x800..=0x83f) {
            let start = (offset & 31) as usize;
            if start + bytes > 32 {
                return None;
            }
            let local = offset & 32 == 0;
            if local && offset >= 0x800 {
                return Some((0..bytes).fold(0, |word, byte| word | (1 << cpu) << (byte * 8)));
            }
            let bank = if local {
                &mut self.local
            } else {
                &mut distributor.shared
            };
            let array = if offset < 0x800 {
                &mut bank.priority
            } else {
                &mut bank.targets
            };
            let mut result = 0;
            for index in 0..bytes {
                if let Some(value) = value {
                    array[start + index] = (value >> (index * 8)) as u8;
                }
                result |= u32::from(array[start + index]) << (index * 8);
            }
            return Some(result);
        }
        if bytes != 4 {
            return None;
        }
        let bank = if offset & 4 == 0 {
            &mut self.local
        } else {
            &mut distributor.shared
        };
        let register = offset & !4;
        if matches!(register, 0x100 | 0x180 | 0x200 | 0x280 | 0x300 | 0x380) {
            let bits = match register {
                0x100 | 0x180 => &mut bank.enabled,
                0x200 | 0x280 => &mut bank.pending,
                _ => &mut bank.active,
            };
            if let Some(value) = value {
                if register & 0x80 != 0 {
                    *bits &= !value;
                } else {
                    *bits |= value;
                }
            }
            return Some(*bits);
        }
        Some(match (offset, value) {
            (0, Some(value)) => {
                distributor.enabled = value & 1 != 0;
                0
            }
            (0, None) => u32::from(distributor.enabled),
            (4, None) => 1 | ((u32::from(distributor.cpus) - 1) << 5),
            (8, None) => 0x0102_043b,
            (0xfe8, None) => 0x2b,
            _ => 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_irq_moves_between_cpus_and_has_one_active_owner() {
        let mut d = Distributor::new(2);
        let (mut a, mut b) = (Cpu::new(), Cpu::new());
        d.enabled = true;
        a.enabled = true;
        b.enabled = true;
        a.mask = 255;
        b.mask = 255;
        a.access(&mut d, 0, 0x0800_0104, Some(2), 4).unwrap();
        b.access(&mut d, 1, 0x0800_0821, Some(2), 1).unwrap();
        d.level(33, true);
        assert_eq!(a.next(&d, 0), 1023);
        assert_eq!(b.next(&d, 1), 33);
        assert_eq!(b.access(&mut d, 1, 0x0801_000c, None, 4), Some(33));
        d.level(33, true);
        a.access(&mut d, 0, 0x0800_0821, Some(1), 1).unwrap();
        assert_eq!(a.next(&d, 0), 1023);
        b.access(&mut d, 1, 0x0801_0010, Some(33), 4).unwrap();
        assert_eq!(a.next(&d, 0), 33);
    }

    #[test]
    fn sgi_keeps_both_senders_until_acknowledged() {
        let mut d = Distributor::new(4);
        let mut cpu = Cpu::new();
        d.enabled = true;
        cpu.enabled = true;
        cpu.mask = 255;
        cpu.local.enabled = 0xffff;
        cpu.sgi(2, 0);
        cpu.sgi(2, 3);
        assert_eq!(cpu.access(&mut d, 1, 0x0801_000c, None, 4), Some(2));
        assert_eq!(cpu.next(&d, 1), 1023);
        cpu.access(&mut d, 1, 0x0801_0010, Some(2), 4).unwrap();
        assert_eq!(
            cpu.access(&mut d, 1, 0x0801_000c, None, 4),
            Some(2 | (3 << 10))
        );
        cpu.access(&mut d, 1, 0x0801_0010, Some(2 | (3 << 10)), 4)
            .unwrap();
        assert_eq!(cpu.next(&d, 1), 1023);
    }
}
