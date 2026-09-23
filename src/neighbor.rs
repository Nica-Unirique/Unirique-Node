use crate::address::Address;

const FAILURES_MAX: u8 = 3;

#[derive(Clone)]
pub struct Neighbor {
    pub address: Address,
    pub id: u64,
    pub depth: u8,
    pub failures: u8,
}

impl Neighbor {
    pub fn new(address: Address, id: u64, depth: u8) -> Neighbor {
        return Neighbor {
            address,
            id,
            depth,
            failures: 0,
        };
    }

    pub fn answered(&mut self) {
        self.failures = 0;
    }

    pub fn failed(&mut self) {
        self.failures += 1;
    }

    pub fn is_dead(&self) -> bool {
        return self.failures >= FAILURES_MAX;
    }
}