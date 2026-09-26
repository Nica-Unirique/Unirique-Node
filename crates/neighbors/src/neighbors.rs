use std::fs;

use crate::address::Address;
use crate::neighbor::Neighbor;

const NEIGHBORS_NEAR_FILE: &str = "data/neighbors_near.csv";
const NEIGHBORS_FAR_FILE: &str = "data/neighbors_far.csv";

const NEAR_MAX: usize = 64;
const FAR_MAX: usize = 64;
const FAR_PER_DISTANCE: usize = 4;

pub struct Neighbors {
    near: Vec<Neighbor>,
    far: Vec<Neighbor>,
}

impl Neighbors {
    pub fn new() -> Neighbors {
        return Neighbors {
            near: read_list(NEIGHBORS_NEAR_FILE),
            far: read_list(NEIGHBORS_FAR_FILE),
        };
    }
    pub fn add(&mut self, id_me: u64, neighbor: Neighbor) {
        if neighbor.id == id_me {
            return;
        }

        if self.update(&neighbor) {
            return;
        }

        if self.near.len() < NEAR_MAX {
            self.near.push(neighbor);
            return;
        }

        let neighbor = self.take_near_place(id_me, neighbor);
        self.add_far(id_me, neighbor);
    }

    fn update(&mut self, neighbor_new: &Neighbor) -> bool {
        for neighbors in [&mut self.near, &mut self.far] {
            for neighbor in neighbors.iter_mut() {
                if neighbor.address == neighbor_new.address {
                    neighbor.id = neighbor_new.id;
                    neighbor.depth = neighbor_new.depth;
                    return true;
                }
            }
        }

        return false;
    }

    fn take_near_place(&mut self, id_me: u64, neighbor_new: Neighbor) -> Neighbor {
        let mut weakest = 0;
        let mut index = 1;

        while index < self.near.len() {
            if common_bits(id_me, self.near[index].id) <= common_bits(id_me, self.near[weakest].id) {
                weakest = index;
            }
            index += 1;
        }

        if common_bits(id_me, neighbor_new.id) <= common_bits(id_me, self.near[weakest].id) {
            return neighbor_new;
        }

        let leaving = self.near[weakest].clone();
        self.near[weakest] = neighbor_new;

        return leaving;
    }

    fn add_far(&mut self, id_me: u64, neighbor_new: Neighbor) {
        if self.far.len() >= FAR_MAX {
            return;
        }

        let distance = common_bits(id_me, neighbor_new.id);
        let mut same_distance = 0;

        for neighbor in self.far.iter() {
            if common_bits(id_me, neighbor.id) == distance {
                same_distance += 1;
            }
        }

        if same_distance >= FAR_PER_DISTANCE {
            return;
        }

        self.far.push(neighbor_new);
    }

    pub fn answered(&mut self, address: Address) {
        for neighbors in [&mut self.near, &mut self.far] {
            for neighbor in neighbors.iter_mut() {
                if neighbor.address == address {
                    neighbor.answered();
                    return;
                }
            }
        }
    }

    pub fn failed(&mut self, address: Address) {
        for neighbors in [&mut self.near, &mut self.far] {
            let mut index = neighbors.len();
            while index > 0 {
                index -= 1;
                if neighbors[index].address == address {
                    neighbors[index].failed();

                    if neighbors[index].is_dead() {
                        neighbors.remove(index);
                    }
                    return;
                }
            }
        }
    }

    pub fn random_contact(&self) -> Option<(Address, u64, u8)> {
        let total = self.near.len() + self.far.len();
        if total == 0 {
            return None;
        }

        let index = rand::random_range(0..total);

        let chosen;
        if index < self.near.len() {
            chosen = &self.near[index];
        } else {
            chosen = &self.far[index - self.near.len()];
        }

        return Some((chosen.address, chosen.id, chosen.depth));
    }

    pub fn closest(&self, target: u64, most: usize) -> Vec<Neighbor> {
        let mut all = Vec::new();

        for neighbors in [&self.near, &self.far] {
            for neighbor in neighbors.iter() {
                all.push(neighbor.clone());
            }
        }

        all.sort_by_key(|neighbor| common_bits(target, neighbor.id));
        all.reverse();
        all.truncate(most);

        return all;
    }

    pub fn contacts(&self) -> Vec<Address> {
        let mut addresses = Vec::new();

        for neighbors in [&self.near, &self.far] {
            for neighbor in neighbors.iter() {
                addresses.push(neighbor.address);
            }
        }

        return addresses;
    }

    pub fn is_empty(&self) -> bool {
        return self.near.is_empty() && self.far.is_empty();
    }

    pub fn save(&self) {
        write_list(NEIGHBORS_NEAR_FILE, &self.near);
        write_list(NEIGHBORS_FAR_FILE, &self.far);
    }
}

pub fn common_bits(a: u64, b: u64) -> u32 {
    return (a ^ b).leading_zeros();
}

fn read_list(path: &str) -> Vec<Neighbor> {
    let mut neighbors = Vec::new();

    let text = fs::read_to_string(path);
    if text.is_err() {
        return neighbors;
    }

    for line in text.unwrap().lines().skip(1) {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() != 3 {
            continue;
        }

        let address = Address::from_text(parts[0]);
        let id = parts[1].parse::<u64>();
        let depth = parts[2].parse::<u8>();

        if address.is_none() || id.is_err() || depth.is_err() {
            continue;
        }

        neighbors.push(Neighbor::new(address.unwrap(), id.unwrap(), depth.unwrap()));
    }

    return neighbors;
}

fn write_list(path: &str, neighbors: &Vec<Neighbor>) {
    let mut text = String::from("address,id,depth\n");

    for neighbor in neighbors.iter() {
        text += &format!("{},{},{}\n", neighbor.address.to_text(), neighbor.id, neighbor.depth);
    }

    if fs::write(path, text).is_err() {
        eprintln!("Failed to save neighbors: {}", path);
    }
}