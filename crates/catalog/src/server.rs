use std::mem::size_of;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use neighbors::Address;
use crate::signature::{server_bytes, verify};

const SEEN_WITHIN: Duration = Duration::from_secs(5 * 60);

#[derive(Clone)]
pub struct Server {
    pub name: String,
    pub address: Address,
    pub host_key: [u8; 32],

    pub game_name: String,
    pub game_version: [u32; 3],
    pub game_autor_key: [u8; 32],

    pub signed_at: u64,
    pub signature: [u8; 64],

    pub ram_weight: u64,
    pub seen: Instant,
}

impl Server {
    pub fn new(
        name: String,
        address: Address,
        host_key: [u8; 32],
        game_name: String,
        game_version: [u32; 3],
        game_autor_key: [u8; 32],
        signed_at: u64,
        signature: [u8; 64],
    ) -> Server {
        let mut server = Server {
            name,
            address,
            host_key,
            game_name,
            game_version,
            game_autor_key,
            signed_at,
            signature,
            ram_weight: 0,
            seen: Instant::now(),
        };

        server.ram_weight = (size_of::<Server>()
            + server.name.capacity()
            + server.game_name.capacity()) as u64;

        return server;
    }

    pub fn is_dead(&self) -> bool {
        return self.seen.elapsed() >= SEEN_WITHIN;
    }

    pub fn is_same(&self, other: &Server) -> bool {
        return self.host_key == other.host_key && self.address == other.address;
    }

    pub fn is_valid(&self) -> bool {
        let now = now_seconds();
        let fresh_for = SEEN_WITHIN.as_secs();

        if self.signed_at + fresh_for < now || self.signed_at > now + fresh_for {
            return false;
        }

        return verify(&self.host_key, &server_bytes(self), &self.signature);
    }
}

pub fn now_seconds() -> u64 {
    let now = SystemTime::now().duration_since(UNIX_EPOCH);
    if now.is_err() {
        return 0;
    }

    return now.unwrap().as_secs();
}