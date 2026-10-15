pub struct Vectors {
    target: u64,
    address: u64,
    bytes: [u8; 2048],
    ready: bool,
}

impl Default for Vectors {
    fn default() -> Self {
        Self::new()
    }
}

impl Vectors {
    pub const fn new() -> Self {
        Self {
            target: 0,
            address: 0,
            bytes: [0; 2048],
            ready: false,
        }
    }

    pub fn sample(&mut self, target: u64, address: u64, bytes: &[u8; 2048]) -> bool {
        let changed = self.ready
            && (self.target != target || self.address != address || self.bytes != *bytes);
        self.target = target;
        self.address = address;
        self.bytes = *bytes;
        self.ready = true;
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_code_changes_and_remapping_at_the_same_virtual_address() {
        let mut vectors = Vectors::new();
        let mut bytes = [0; 2048];
        assert!(!vectors.sample(0xffff_0000, 0x4020_0000, &bytes));
        assert!(!vectors.sample(0xffff_0000, 0x4020_0000, &bytes));
        bytes[1024] = 0x14;
        assert!(vectors.sample(0xffff_0000, 0x4020_0000, &bytes));
        assert!(vectors.sample(0xffff_0000, 0x4030_0000, &bytes));
        assert!(vectors.sample(0xffff_0800, 0x4030_0000, &bytes));
        assert!(!vectors.sample(0xffff_0800, 0x4030_0000, &bytes));
    }
}
