//! Ranger les fiches sur les bonnes etageres, et y chercher.

use catalog::{
    criteria_position, content_positions, name_rank, server_criteria_position, server_positions, Criteria, Content, Server,
    ServerCriteria,
};
use neighbors::{common_bits, Address, Neighbor};
use protocol::Message;

use super::lookup::next_wave;
use super::{Node, CLOSER_SENT, LOOKUP_WAVES, SHELF_COPIES};

impl Node {
    /// Les nodes les plus proches d'une position, trouves par vagues.
    fn closest_nodes(&self, key: u64) -> Vec<Neighbor> {
        let mut candidates = self.neighbors.lock().unwrap().closest(key, CLOSER_SENT);
        let mut asked: Vec<Address> = Vec::new();
        let mut answered: Vec<Neighbor> = Vec::new();

        for _ in 0..LOOKUP_WAVES {
            let wave = next_wave(&candidates, &asked);
            if wave.is_empty() {
                break;
            }

            for neighbor in wave {
                asked.push(neighbor.address);

                let answer = self.ask(neighbor.address, Message::GetNeighbors { target: key });

                match answer {
                    Some(Message::SendNeighbors { neighbors }) => {
                        self.add_candidates(&mut candidates, neighbors);
                        answered.push(neighbor);
                    }
                    _ => self.neighbors.lock().unwrap().failed(neighbor.address),
                }
            }

            candidates.sort_by_key(|neighbor| common_bits(key, neighbor.id));
            candidates.reverse();

            // Les plus proches connus ont tous ete interroges : on ne se
            // rapprochera plus.
            let mut all_asked = true;
            for candidate in candidates.iter().take(SHELF_COPIES) {
                if !asked.contains(&candidate.address) {
                    all_asked = false;
                }
            }

            if all_asked {
                break;
            }
        }

        answered.sort_by_key(|neighbor| common_bits(key, neighbor.id));
        answered.reverse();
        answered.truncate(SHELF_COPIES);

        return answered;
    }

    /// Les nodes a qui parler pour ces positions, chacun une seule fois.
    fn shelves_of(&self, positions: &Vec<u64>) -> Vec<Address> {
        let mut addresses: Vec<Address> = Vec::new();

        for position in positions.iter() {
            for neighbor in self.closest_nodes(*position) {
                if !addresses.contains(&neighbor.address) {
                    addresses.push(neighbor.address);
                }
            }
        }

        return addresses;
    }

    pub(super) fn store_content(&self, content: &Content) {
        for address in self.shelves_of(&content_positions(content)) {
            self.ask(address, Message::SendContents { contents: vec![content.clone()] });
        }
    }

    pub(super) fn store_contents(&self) {
        let contents = self.contents.lock().unwrap().get_signed();

        for content in contents.iter() {
            if content.downloaded {
                self.store_content(content);
            }
        }
    }

    pub(super) fn store_server(&self, server: &Server) {
        for address in self.shelves_of(&server_positions(server)) {
            self.ask(address, Message::AnnounceServer { server: server.clone() });
        }
    }

    /// Les nodes de l'etagere a interroger.
    fn places_to_ask(&self, position: u64) -> Vec<Address> {
        let mut addresses = Vec::new();

        for neighbor in self.closest_nodes(position) {
            addresses.push(neighbor.address);
        }

        return addresses;
    }

    /// Des criteres sans etagere (aucun, ou seulement un morceau de nom de
    /// moins de 3 lettres) : la recherche est refusee. Un tel morceau reste
    /// utilisable comme filtre, a cote d'un tag ou d'un auteur.
    pub(super) fn search_contents_on_shelves(&self, criteria: &Criteria) -> Vec<Content> {
        let position = criteria_position(criteria);
        if position.is_none() {
            return Vec::new();
        }

        let mut found = self.contents.lock().unwrap().get_by_criteria(criteria);

        for address in self.places_to_ask(position.unwrap()) {
            let answer = self.ask(address, Message::GetContents { criteria: criteria.clone() });

            match answer {
                Some(Message::SendContents { contents }) => {
                    for content in contents {
                        if content.is_signed() && criteria.matches(&content) && !contains_content(&found, &content) {
                            found.push(content);
                        }
                    }
                }
                _ => {}
            }
        }

        if criteria.name.is_some() {
            let fragment = criteria.name.clone().unwrap();
            found.sort_by_key(|content| name_rank(&content.name, &fragment));
        }

        return found;
    }

    pub(super) fn search_servers_on_shelves(&self, criteria: &ServerCriteria) -> Vec<Server> {
        let position = server_criteria_position(criteria);
        if position.is_none() {
            return Vec::new();
        }

        let mut found = self.servers.lock().unwrap().get_by_criteria(criteria);

        for address in self.places_to_ask(position.unwrap()) {
            let answer = self.ask(address, Message::GetServers { criteria: criteria.clone() });

            match answer {
                Some(Message::SendServers { servers }) => {
                    for server in servers {
                        if server.is_valid() && criteria.matches(&server) && !contains_server(&found, &server) {
                            found.push(server);
                        }
                    }
                }
                _ => {}
            }
        }

        if criteria.name.is_some() {
            let fragment = criteria.name.clone().unwrap();
            found.sort_by_key(|server| name_rank(&server.name, &fragment));
        }

        return found;
    }
}

fn contains_content(contents: &Vec<Content>, content: &Content) -> bool {
    for known in contents.iter() {
        if known.name == content.name && known.version == content.version && known.autor_key == content.autor_key {
            return true;
        }
    }

    return false;
}

fn contains_server(servers: &Vec<Server>, server: &Server) -> bool {
    for known in servers.iter() {
        if known.is_same(server) {
            return true;
        }
    }

    return false;
}
