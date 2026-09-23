pub const PREFIX_BITS: u32 = 32;

struct Id {
    value: u64,
}

impl Id {
    pub fn new(name: String) -> Self {
        Self { value: Id::make_value(name) }
    }

    fn make_value(name: String) -> u64 {
        let prefix = Id::make_prefix(name);
        let suffix = Id::make_suffix();
        (prefix << PREFIX_BITS) | suffix
    }

    fn make_prefix(name: String) -> u32 {
        let mut prefix: u32 = 0x811c9dc5;

        for byte in name.as_bytes() {
            prefix ^= *byte as u32;
            prefix = prefix.wrapping_mul(0x01000193);
        }

        return prefix;
    }

    fn make_suffix() -> u32 {
        return rand::random();
    }

    pub fn get_prefix(&self) -> u32 {
        return (self.value >> PREFIX_BITS) as u32;
    }

    pub fn get_suffix(&self) -> u32 {
        return (self.value & ((1 << PREFIX_BITS) - 1)) as u32;
    }
}