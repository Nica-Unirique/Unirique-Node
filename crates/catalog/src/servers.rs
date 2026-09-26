use crate::criteria::ServerCriteria;
use crate::server::Server;
use crate::shelves::{on_shelf, server_positions};

pub struct Servers {
    pub values: Vec<Server>,
    pub ram_weight: u64,
}

impl Servers {
    pub fn new() -> Servers {
        return Servers {
            values: Vec::new(),
            ram_weight: 0,
        };
    }

    pub fn add(&mut self, server: Server) {
        for known in self.values.iter_mut() {
            if known.is_same(&server) {
                self.ram_weight -= known.ram_weight;
                self.ram_weight += server.ram_weight;
                *known = server;
                return;
            }
        }

        self.ram_weight += server.ram_weight;
        self.values.push(server);
    }

    pub fn get_by_criteria(&self, criteria: &ServerCriteria) -> Vec<Server> {
        let mut found = Vec::new();

        for server in self.values.iter() {
            if !server.is_dead() && server.is_valid() && criteria.matches(server) {
                found.push(server.clone());
            }
        }

        return found;
    }

    pub fn forget_outside(&mut self, id: u64, depth: u8) {
        let mut index = self.values.len();

        while index > 0 {
            index -= 1;

            if !on_shelf(&server_positions(&self.values[index]), id, depth) {
                self.ram_weight -= self.values[index].ram_weight;
                self.values.remove(index);
            }
        }
    }

    pub fn forget_dead(&mut self) {
        let mut index = self.values.len();

        while index > 0 {
            index -= 1;

            if self.values[index].is_dead() {
                self.ram_weight -= self.values[index].ram_weight;
                self.values.remove(index);
            }
        }
    }
}