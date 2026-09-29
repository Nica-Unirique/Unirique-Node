use std::net::SocketAddr;

use catalog::Content;
use neighbors::{common_bits, Address, Neighbor};
use protocol::Message;

use super::{
    Node, ANNOUNCE_TO, CLOSER_SENT, HOLDERS_WANTED, LOOKUP_WAVES, LOOKUP_WIDTH,
};

impl Node {
    pub(super) fn answer_get_holder(&self, infohash: &str) -> Message {
        let mut holders = self.holders.lock().unwrap().of(infohash);

        if self.contents.lock().unwrap().is_holding(infohash) {
            holders.push(*self.public_torrent.lock().unwrap());
        }

        let neighbors = self.neighbors.lock().unwrap().closest(key_of(infohash), CLOSER_SENT);

        return Message::SendHolders { holders, neighbors };
    }

    pub(super) fn find_holders(&self, infohash: &str) -> Vec<SocketAddr> {
        let (holders, _) = self.lookup(infohash);
        let mut sockets = Vec::new();

        for holder in holders.iter() {
            sockets.push(holder.socket());
        }

        return sockets;
    }

    fn lookup(&self, infohash: &str) -> (Vec<Address>, Vec<Neighbor>) {
        let key = key_of(infohash);

        let mut candidates = self.neighbors.lock().unwrap().closest(key, CLOSER_SENT);
        let mut asked: Vec<Address> = Vec::new();
        let mut answered: Vec<Neighbor> = Vec::new();
        let mut holders: Vec<Address> = Vec::new();

        for _ in 0..LOOKUP_WAVES {
            let wave = next_wave(&candidates, &asked);
            if wave.is_empty() {
                break;
            }

            for neighbor in wave {
                asked.push(neighbor.address);

                let answer = self.ask(neighbor.address, Message::GetHolder { infohash: infohash.to_string() });

                match answer {
                    Some(Message::SendHolders { holders: found, neighbors }) => {
                        add_holders(&mut holders, found, neighbor.address);
                        self.add_candidates(&mut candidates, neighbors);
                        answered.push(neighbor);
                    }
                    _ => self.neighbors.lock().unwrap().failed(neighbor.address),
                }
            }

            if holders.len() >= HOLDERS_WANTED {
                break;
            }

            candidates.sort_by_key(|neighbor| common_bits(key, neighbor.id));
            candidates.reverse();
        }

        answered.sort_by_key(|neighbor| common_bits(key, neighbor.id));
        answered.reverse();

        return (holders, answered);
    }

    pub(super) fn add_candidates(&self, candidates: &mut Vec<Neighbor>, neighbors: Vec<Neighbor>) {
        for neighbor in neighbors {
            if neighbor.id == self.id {
                continue;
            }

            let mut known = false;
            for candidate in candidates.iter() {
                if candidate.address == neighbor.address {
                    known = true;
                }
            }

            if !known {
                candidates.push(neighbor);
            }
        }
    }

    pub(super) fn announce_holdings(&self) {
        let contents = self.contents.lock().unwrap().get_signed();

        for content in contents.iter() {
            if content.downloaded && !content.is_exhausted() {
                self.announce_holding(content);
            }
        }
    }

    pub(super) fn announce_holding(&self, content: &Content) {
        let (_, closest) = self.lookup(&content.infohash);
        let torrent_port = self.public_torrent.lock().unwrap().port;

        for neighbor in closest.iter().take(ANNOUNCE_TO) {
            self.ask(neighbor.address, Message::AnnounceHolder { infohash: content.infohash.clone(), torrent_port });
        }
    }
}

fn key_of(infohash: &str) -> u64 {
    if infohash.len() < 16 {
        return 0;
    }

    return u64::from_str_radix(&infohash[..16], 16).unwrap_or(0);
}

pub(super) fn next_wave(candidates: &Vec<Neighbor>, asked: &Vec<Address>) -> Vec<Neighbor> {
    let mut wave = Vec::new();

    for candidate in candidates.iter() {
        if wave.len() >= LOOKUP_WIDTH {
            break;
        }

        if !asked.contains(&candidate.address) {
            wave.push(candidate.clone());
        }
    }

    return wave;
}

fn add_holders(holders: &mut Vec<Address>, found: Vec<Address>, from: Address) {
    for holder in found {
        let mut holder = holder;

        if holder.ip.is_unspecified() {
            holder = Address::new(from.ip, holder.port);
        }

        if !holders.contains(&holder) {
            holders.push(holder);
        }
    }
}
