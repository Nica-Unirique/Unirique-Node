use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::address::Address;
use crate::criteria::Criteria;
use crate::game::Game;
use crate::games::Games;
use crate::message::Message;
use crate::neighbor::Neighbor;
use crate::neighbors::Neighbors;
use crate::servers::Servers;
use crate::settings::Settings;
use crate::busy::Busy;

const MAIN_NODE: &str = "127.0.0.1:8735";
const ENTER_TRIES: u32 = 10;

const UPKEEP_DELAY: Duration = Duration::from_secs(10);
const ANSWER_WITHIN: Duration = Duration::from_secs(5);
const MESSAGE_MAX: u32 = 64 * 1024;
const BUSY_MAX: usize = 64;
const DEPTH_MAX: u8 = 63;
const WEIGHT_HIGH: u64 = 10 * 1024 * 1024;
const WEIGHT_LOW: u64 = 5 * 1024 * 1024;
const NEIGHBORS_ASKED: usize = 128;

pub struct Node {
    ismain: bool,
    id: u64,
    depth: AtomicU8,
    address: Address,

    neighbors: Mutex<Neighbors>,
    games: Mutex<Games>,
    servers: Mutex<Servers>,
}

impl Node {
    pub fn new(settings: Settings) -> Node {
        return Node {
            ismain: settings.ismain,
            address: settings.address,
            id: settings.id,
            depth: AtomicU8::new(0),
            neighbors: Mutex::new(Neighbors::new()),
            games: Mutex::new(Games::new()),
            servers: Mutex::new(Servers::new()),
        };
    }

    pub fn run(self: &Arc<Self>) {
        let listener = TcpListener::bind(self.address.socket());
        if listener.is_err() {
            eprintln!("Failed to listen on {}", self.address.to_text());
            return;
        }

        self.listen(listener.unwrap());
        self.upkeep();
    }

    fn listen(self: &Arc<Self>, listener: TcpListener) {
        let node = self.clone();

        thread::spawn(move || {
            let busy = Arc::new(AtomicUsize::new(0));

            for coming in listener.incoming() {
                while busy.load(Ordering::Relaxed) >= BUSY_MAX {
                    thread::sleep(Duration::from_millis(10));
                }

                if coming.is_err() {
                    continue;
                }

                busy.fetch_add(1, Ordering::Relaxed);

                let serving = node.clone();
                let counted = Busy(busy.clone());
                let stream = coming.unwrap();

                thread::spawn(move || {
                    let _counted = counted;
                    serving.serve(stream);
                });
            }
        });
    }

    fn serve(&self, mut stream: TcpStream) {
        let _ = stream.set_read_timeout(Some(ANSWER_WITHIN));
        let _ = stream.set_write_timeout(Some(ANSWER_WITHIN));

        let from = stream.peer_addr();
        if from.is_err() {
            return;
        }

        let message = read_message(&mut stream);
        if message.is_none() {
            return;
        }

        let answer = self.answer(&message.unwrap(), from.unwrap());
        if answer.is_none() {
            return;
        }

        write_message(&mut stream, &answer.unwrap());
    }

    fn answer(&self, message: &Message, from: SocketAddr) -> Option<Message> {
        match message {
            Message::Ping { port, id, depth } => return Some(self.answer_ping(*port, *id, *depth, from)),
            Message::GetNeighbors { target } => return Some(self.answer_get_neighbors(*target)),
            Message::GetGames { criteria } => return Some(self.answer_get_games(criteria)),
            Message::SendGames { games } => {
                self.receive_games(games);
                return None;
            }
            Message::Pong { .. } => return None,
            Message::SendNeighbors { .. } => return None,
        }
    }

    fn answer_ping(&self, port: u16, id: u64, depth: u8, from: SocketAddr) -> Message {
        let address = Address::new(from.ip(), port);

        let mut neighbors = self.neighbors.lock().unwrap();
        neighbors.add(self.id, Neighbor::new(address, id, depth));
        neighbors.answered(address);
        drop(neighbors);

        return Message::Pong {
            port: self.address.port,
            id: self.id,
            depth: self.depth.load(Ordering::Relaxed),
        };
    }

    fn answer_get_neighbors(&self, target: u64) -> Message {
        let neighbors = self.neighbors.lock().unwrap().closest(target, NEIGHBORS_ASKED);

        return Message::SendNeighbors { neighbors };
    }

    fn answer_get_games(&self, criteria: &Criteria) -> Message {
        let games = self.games.lock().unwrap().get_by_criteria(criteria);

        return Message::SendGames { games };
    }

    fn receive_games(&self, games: &Vec<Game>) {
        let mut my_games = self.games.lock().unwrap();

        for game in games.iter() {
            my_games.add_received(game.clone());
        }
    }

    fn upkeep(&self) {
        if !self.enter_network() {
            eprintln!("Failed to enter the network.");
            return;
        }

        self.publish_games();

        loop {
            self.servers.lock().unwrap().forget_dead();
            self.find_new_neighbors();
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

    fn publish_games(&self) {
        let games = self.games.lock().unwrap().get_all();
        let contacts = self.neighbors.lock().unwrap().contacts();

        for address in contacts {
            self.ask(address, Message::SendGames { games: games.clone() });
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
        let games = self.games.lock().unwrap();
        let weight = servers_weight + games.ram_weight + games.disk_weight;
        drop(games);

        let depth = self.depth.load(Ordering::Relaxed);

        if weight >= WEIGHT_HIGH && depth < DEPTH_MAX {
            self.depth.store(depth + 1, Ordering::Relaxed);
        }

        if weight < WEIGHT_LOW && depth > 0 {
            self.depth.store(depth - 1, Ordering::Relaxed);
        }
    }

    fn ask(&self, address: Address, message: Message) -> Option<Message> {
        let stream = TcpStream::connect_timeout(&address.socket(), ANSWER_WITHIN);
        if stream.is_err() {
            return None;
        }

        let mut stream = stream.unwrap();
        let _ = stream.set_read_timeout(Some(ANSWER_WITHIN));
        let _ = stream.set_write_timeout(Some(ANSWER_WITHIN));

        if !write_message(&mut stream, &message) {
            return None;
        }

        return read_message(&mut stream);
    }
}

fn read_message(stream: &mut TcpStream) -> Option<Message> {
    let mut head = [0u8; 4];
    if stream.read_exact(&mut head).is_err() {
        return None;
    }

    let length = u32::from_le_bytes(head);
    if length == 0 || length > MESSAGE_MAX {
        return None;
    }

    let mut bytes = vec![0u8; length as usize];
    if stream.read_exact(&mut bytes).is_err() {
        return None;
    }

    return Message::from_bytes(&bytes);
}

fn write_message(stream: &mut TcpStream, message: &Message) -> bool {
    let bytes = message.to_bytes();
    if bytes.len() > MESSAGE_MAX as usize {
        return false;
    }

    if stream.write_all(&(bytes.len() as u32).to_le_bytes()).is_err() {
        return false;
    }

    if stream.write_all(&bytes).is_err() {
        return false;
    }

    return true;
}