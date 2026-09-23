use crate::server::Server;

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