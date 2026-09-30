use std::sync::atomic::Ordering;
use std::thread;

use neighbors::{Address, Neighbor};
use protocol::Message;

use super::{resolve, Node, ANNOUNCE_EVERY, DEPTH_MAX, SILENT_MAX, UPKEEP_DELAY, WEIGHT_HIGH, WEIGHT_LOW};

impl Node {
    pub(super) fn upkeep(&self) {
        self.enter_network();

        if self.passive {
            self.upkeep_passive();
            return;
        }

        let mut tour: u32 = 0;

        loop {
            if tour % ANNOUNCE_EVERY == 0 {
                self.announce_holdings();
                self.store_contents();
            }
            tour += 1;

            // Tous nos voisins ont disparu : on rentre par le node principal.
            self.enter_network();
            self.publish_address();
            self.holders.lock().unwrap().forget_old();
            self.servers.lock().unwrap().forget_dead();
            self.find_new_neighbors();
            self.apply_quotas();
            self.adjust_depth();
            self.neighbors.lock().unwrap().save();
            thread::sleep(UPKEEP_DELAY);
        }
    }

    /// L'entretien d'un node qui n'appelle personne : ce qui se fait sans
    /// reseau, et l'oubli des voisins silencieux.
    fn upkeep_passive(&self) {
        loop {
            self.publish_address();
            self.holders.lock().unwrap().forget_old();
            self.servers.lock().unwrap().forget_dead();
            self.neighbors.lock().unwrap().forget_silent(SILENT_MAX);
            self.apply_quotas();
            self.adjust_depth();
            self.neighbors.lock().unwrap().save();
            thread::sleep(UPKEEP_DELAY);
        }
    }

    /// Tant qu'on n'a aucun voisin, on salue le node principal, sans limite :
    /// son adresse change toutes les heures, et la copie publiee met quelques
    /// minutes a suivre.
    fn enter_network(&self) {
        if self.ismain {
            return;
        }

        while self.neighbors.lock().unwrap().is_empty() {
            let main = self.main_node();
            if main.is_some() {
                self.greet(main.unwrap());
            }

            if !self.neighbors.lock().unwrap().is_empty() {
                return;
            }

            eprintln!("No neighbor yet, trying again...");
            thread::sleep(UPKEEP_DELAY);
        }
    }

    /// L'adresse du node principal : celle donnee au lancement, sinon celle
    /// publiee sur GitHub, lue a chaque essai puisqu'elle change.
    fn main_node(&self) -> Option<Address> {
        if self.main_node.is_some() {
            return resolve(self.main_node.as_ref().unwrap());
        }

        let published = main_address::fetch();
        if published.is_none() {
            return None;
        }

        return resolve(&published.unwrap().node);
    }

    fn greet(&self, address: Address) {
        let ping = Message::Ping {
            port: self.public_port.load(Ordering::Relaxed),
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
