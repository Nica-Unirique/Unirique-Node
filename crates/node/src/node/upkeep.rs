use std::sync::atomic::Ordering;
use std::thread;

use neighbors::{Address, Neighbor};
use protocol::Message;

use super::{Node, ANNOUNCE_EVERY, DEPTH_MAX, ENTER_TRIES, MAIN_NODE, UPKEEP_DELAY, WEIGHT_HIGH, WEIGHT_LOW};

impl Node {
    pub(super) fn upkeep(&self) {
        if !self.enter_network() {
            eprintln!("Failed to enter the network.");
            return;
        }

        let mut tour: u32 = 0;

        loop {
            if tour % ANNOUNCE_EVERY == 0 {
                self.announce_holdings();
                self.store_contents();
            }
            tour += 1;

            self.holders.lock().unwrap().forget_old();
            self.servers.lock().unwrap().forget_dead();
            self.find_new_neighbors();
            self.apply_quotas();
            self.adjust_depth();
            self.neighbors.lock().unwrap().save();
            thread::sleep(UPKEEP_DELAY);
        }
    }

    fn enter_network(&self) -> bool {
        if self.ismain || !self.neighbors.lock().unwrap().is_empty() {
            return true;
        }

        let main = Address::from_text(MAIN_NODE).unwrap();

        for _ in 0..ENTER_TRIES {
            self.greet(main);

            if !self.neighbors.lock().unwrap().is_empty() {
                return true;
            }

            eprintln!("No neighbor yet, trying again...");
            thread::sleep(UPKEEP_DELAY);
        }
        eprintln!("Failed to enter the network after multiple tries.");
        return false;
    }

    fn greet(&self, address: Address) {
        let ping = Message::Ping {
            port: self.address.port,
            id: self.id,
            depth: self.depth.load(Ordering::Relaxed),
        };

        let answer = self.ask(address, ping);

        match answer {
            Some(Message::Pong { id, depth, .. }) => {
                self.neighbors.lock().unwrap().add(self.id, Neighbor::new(address, id, depth));
            }
            _ => {}
        }
    }

    fn find_new_neighbors(&self) {
        let contact = self.neighbors.lock().unwrap().random_contact();
        if contact.is_none() {
            return;
        }

        let (address, _id, _depth) = contact.unwrap();
        let answer = self.ask(address, Message::GetNeighbors { target: self.id });

        match answer {
            Some(Message::SendNeighbors { neighbors }) => self.receive_neighbors(address, neighbors),
            _ => self.neighbors.lock().unwrap().failed(address),
        }
    }

    fn receive_neighbors(&self, from: Address, neighbors: Vec<Neighbor>) {
        let mut my_neighbors = self.neighbors.lock().unwrap();
        my_neighbors.answered(from);

        for neighbor in neighbors {
            my_neighbors.add(self.id, neighbor);
        }
    }

    fn adjust_depth(&self) {
        let servers_weight = self.servers.lock().unwrap().ram_weight;
        let contents = self.contents.lock().unwrap();
        let weight = servers_weight + contents.ram_weight;
        drop(contents);

        let depth = self.depth.load(Ordering::Relaxed);

        if weight >= WEIGHT_HIGH && depth < DEPTH_MAX {
            self.depth.store(depth + 1, Ordering::Relaxed);
            self.contents.lock().unwrap().forget_outside(self.id, depth + 1);
            self.servers.lock().unwrap().forget_outside(self.id, depth + 1);
        }

        if weight < WEIGHT_LOW && depth > 0 {
            self.depth.store(depth - 1, Ordering::Relaxed);
        }
    }
}
