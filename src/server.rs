use std::mem::size_of;
use std::time::{Duration, Instant};

use crate::address::Address;

const SEEN_WITHIN: Duration = Duration::from_secs(5 * 60);

pub struct Server {
    pub name: String,
    pub address: Address,
    pub host_key: [u8; 32],

    pub game_name: String,
    pub game_version: [u32; 3],
    pub game_autor_key: [u8; 32],

    pub ram_weight: u64,
    pub seen: Instant,
}

impl Server {
    pub fn new(name: String, address: Address, host_key: [u8; 32], game_name: String, game_version: [u32; 3], game_autor_key: [u8; 32],) -> Server {
        let mut server = Server {
            name,
            address,
            host_key,
            game_name,
            game_version,
            game_autor_key,
            ram_weight: 0,
            seen: Instant::now(),
        };

        server.ram_weight = (size_of::<Server>()
            + server.name.capacity()
            + server.game_name.capacity()) as u64;

        return server;
    }

    pub fn seen_again(&mut self) {
        self.seen = Instant::now();
    }

    pub fn is_dead(&self) -> bool {
        return self.seen.elapsed() >= SEEN_WITHIN;
    }

    pub fn is_same(&self, other: &Server) -> bool {
        return self.host_key == other.host_key && self.address == other.address;
    }
}