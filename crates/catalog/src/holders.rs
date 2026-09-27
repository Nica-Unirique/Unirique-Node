use std::time::{Duration, Instant};

use neighbors::Address;

const KEPT_FOR: Duration = Duration::from_secs(30 * 60);
const PER_CONTENT_MAX: usize = 20;
const RECORDS_MAX: usize = 10_000;

struct Record {
    infohash: String,
    address: Address,
    seen: Instant,
}

pub struct Holders {
    records: Vec<Record>,
}

impl Holders {
    pub fn new() -> Holders {
        return Holders { records: Vec::new() };
    }

    pub fn add(&mut self, infohash: &str, address: Address) {
        let mut same_content = 0;

        for record in self.records.iter_mut() {
            if record.infohash != infohash {
                continue;
            }

            if record.address == address {
                record.seen = Instant::now();
                return;
            }

            same_content += 1;
        }

        if same_content >= PER_CONTENT_MAX || self.records.len() >= RECORDS_MAX {
            return;
        }

        self.records.push(Record {
            infohash: infohash.to_string(),
            address,
            seen: Instant::now(),
        });
    }

    pub fn of(&self, infohash: &str) -> Vec<Address> {
        let mut found = Vec::new();

        for record in self.records.iter() {
            if record.infohash == infohash && record.seen.elapsed() < KEPT_FOR {
                found.push(record.address);
            }
        }

        return found;
    }

    pub fn forget_old(&mut self) {
        let mut index = self.records.len();

        while index > 0 {
            index -= 1;

            if self.records[index].seen.elapsed() >= KEPT_FOR {
                self.records.remove(index);
            }
        }
    }
}
