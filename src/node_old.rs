use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const UPKEEP_DELAY: Duration = Duration::from_secs(10);

use crate::settings::Settings;
use crate::neighbors::Neighbors;
use crate::games::Games;
use crate::servers::Servers;
use crate::address::Address;
use crate::message::Message;

pub struct Node {
    ismain: bool,
    id: u64,
    depth: AtomicU8,

    //settings: Settings,
    address: Address,

    neighbors: Mutex<Neighbors>,
    games: Mutex<Games>,
    servers: Mutex<Servers>,
}

impl Node {
    pub fn new(settings: Settings) -> Self {
        Self {
            ismain: settings.ismain,
            id: settings.id,
            depth: AtomicU8::new(0),
            address: settings.address,
            neighbors: Mutex::new(settings.neighbors),
            games: Mutex::new(settings.games),
            servers: Mutex::new(settings.servers),
        }
    }

    pub fn run(self: &Arc<Self>) {
        let listener = TcpListener::bind(("0.0.0.0", self.address.port));
        if listener.is_err() {
            eprintln!("Failed to bind to address: {}", self.address.port);
            return;
        }
        self.listen(listener.unwrap());
        self.upkeep();
    }

    fn listen(self: &Arc<Self>, listener: TcpListener) {
        let node = self.clone();
        thread::spawn(move || node.listen_thread(listener));
    }

    fn listen_thread(self: Arc<Self>, listener: TcpListener) {
        //let listener = TcpListener::bind(("0.0.0.0", self.address.port));
        //if listener.is_err() {
        //    eprintln!("Failed to bind to address: {}", self.address.port);
        //    return;
        //}
        //let listener = listener.unwrap();
        let busy = Arc::new(AtomicUsize::new(0));

        for coming in listener.incoming() {
            while busy.load(Ordering::Relaxed) >= 64 {
                thread::sleep(std::time::Duration::from_millis(10));
            }
            if coming.is_err() {
                continue;
            }
            busy.fetch_add(1, Ordering::Relaxed);

            let node = self.clone();
            let counted = Busy(busy.clone());
            let stream = coming.unwrap();
            thread::spawn(move || {
                let _counted = counted;
                node.listen_thread_stream(stream);
            });
        }
    }

    fn listen_thread_stream(&self,  mut stream: TcpStream) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
        let from = stream.peer_addr();
        if from.is_err() {
            return;
        }

        let mut head = [0u8; 4];
        if stream.read_exact(&mut head).is_err() {
            return;
        }

        let long = u32::from_le_bytes(head);
        if long == 0 || long > 64 * 1024 { // 64 KB
            return;
        }

        let mut asked = vec![0u8; long as usize];
        if stream.read_exact(&mut asked).is_err() {
            return;
        }

        let message = Message::from_bytes(&asked);
        if message.is_none() {
            return;
        }
        let answer = self.get_answer(&message.unwrap(), from.unwrap()); 
        if answer.is_none() {
            return;
        }
        let bytes = answer.unwrap().to_bytes();
        let _ = stream.write_all(&(bytes.len() as u32).to_le_bytes());
        let _ = stream.write_all(&bytes);
    }

    fn get_answer(&self, message: &Message, from: SocketAddr) -> Option<Message> {
        match message {
            Message::Pong {port, id, depth} => {
                self.neighbors.lock().unwrap().answered(Address::new(from.ip(), *port));
                return None; // je devrais mettre a jour le depth
            }
            Message::Ping {port, id, depth} => {
                let address = Address::new(from.ip(), *port);
                let mut neighbors = self.neighbors.lock().unwrap();
                neighbors.add(address, *id, *depth);
                neighbors.answered(address);
                drop(neighbors);
                return Some(Message::Pong {id: self.id, depth: self.depth, port: self.address.port});
            }
            Message::GetNeighbors {} => {
                //self.neighbors.lock().unwrap().answered(Address::new(from.ip(), *port));
                let neighbors = self.neighbors.lock().unwrap().some(128);
                return Some(Message::SendNeighbors { neighbors });
            }
            Message::SendNeighbors { neighbors} => {
                //self.neighbors.lock().unwrap().answered(Address::new(from.ip(), *port));
                let mut my_neighbors = self.neighbors.lock().unwrap();

                for neighbor in neighbors {
                    my_neighbors.add(neighbor.address, neighbor.id, neighbor.depth);
                }
                return None;
            }
            Message::GetGames { criteria } => {
                //self.neighbors.lock().unwrap().answered(Address::new(from.ip(), *port));
                let games = self.games.lock().unwrap();
                let answer = Message::SendGames { games: games.get_by_criteria(criteria) };
                return Some(answer);
            }
            Message::SendGames { games } => {
                let mut my_games = self.games.lock().unwrap();

                for game in games {
                    my_games.add(game.clone());
                }
                return None;
            },
        }
    }

    fn upkeep(&self) {
        if !self.enter_network() {
            eprintln!("Failed to enter network.");
            return;
        }
        self.publish_games();
        loop {
            self.forget_dead_data();
            self.find_new_neighbors();
            //self.gestion(); a supprimée ?
            self.adjust_depth();
            self.save();
            thread::sleep(UPKEEP_DELAY);
        }
    }

    fn enter_network(&self) -> bool {
        if !self.ismain {
            for _ in 0..10 {
                eprintln!("Searching for 1 or more neighbors...");
                // Implement logic to attempt joining the network if no neighbors are found
                if !self.neighbors.lock().unwrap().is_empty() {
                    return true;
                }
                thread::sleep(UPKEEP_DELAY);
            }
        }
        else {
            return true;
        }
        return false;
    }

    fn publish_games(&self) {
        let games = self.games.lock().unwrap().get_all();
        let contacts = self.neighbors.lock().unwrap().contacts();

        for address in contacts {
            let _ = self.send_message(address, Message::SendGames { games: games.clone() });
        }
    }

    fn forget_dead_data(&self) {
        self.servers.lock().unwrap().forget_dead();
        self.neighbors.lock().unwrap().forget_dead();
    }

    fn find_new_neighbors(&self) {
        let contact = self.neighbors.lock().unwrap().random_contact();
        if contact.is_none() {
            eprintln!("No contact found for finding new neighbors.");
            return;
        }
        let (address, _id, _depth) = contact.unwrap();
        let answer = self.send_message(address, Message::GetNeighbors { });
        match answer {
            Some(Message::SendNeighbors { neighbors }) => {
                let mut my_neighbors = self.neighbors.lock().unwrap();
                my_neighbors.answered(address);

                for neighbor in neighbors {
                    my_neighbors.add(neighbor.address, neighbor.id, neighbor.depth);
                }
            }
            None => {
                eprintln!("Failed to get neighbors from contact.");
                self.neighbors.lock().unwrap().failed(address);
            }
            Some(_) => {
                eprintln!("Unexpected message received when expecting neighbors.");
                self.neighbors.lock().unwrap().failed(address);
            }
        }
    }

    fn adjust_depth(&self) {
        let mo = 1024 * 1024;
        //let neighbors_weight = self.neighbors.lock().unwrap().get_weight();
        let servers_weight = self.servers.lock().unwrap().get_weight();
        let games_weight = self.games.lock().unwrap().get_weight();
        let total_weight = servers_weight + games_weight;
        let depth = self.depth.load(Ordering::Relaxed);
        if total_weight >= 10 * mo {
            if depth >= 63 {
                return;
            }
            self.depth.store(depth.saturating_add(1), Ordering::Relaxed);
        }
        else if total_weight < 5 * mo {
            self.depth.store(depth.saturating_sub(1), Ordering::Relaxed);
        }
    }

    fn save(&self) {
        self.neighbors.lock().unwrap().save();
        self.games.lock().unwrap().save();
    }

    fn send_message(&self, address: Address, message: Message) -> Option<Message> {
        let stream = TcpStream::connect_timeout(&address.socket(), Duration::from_secs(5));
        if stream.is_err() {
            return None;
        }

        let mut stream = stream.unwrap();
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

        let bytes = message.to_bytes();
        if stream.write_all(&(bytes.len() as u32).to_le_bytes()).is_err() {
            return None;
        }
        if stream.write_all(&bytes).is_err() {
            return None;
        }

        let mut head = [0u8; 4];
        if stream.read_exact(&mut head).is_err() {
            return None;
        }

        let long = u32::from_le_bytes(head);
        if long == 0 || long > 64 * 1024 { // 64 KB
            return None;
        }

        let mut answer = vec![0u8; long as usize];
        if stream.read_exact(&mut answer).is_err() {
            return None;
        }

        return Message::from_bytes(&answer);
    }
}

struct Busy(Arc<AtomicUsize>);

impl Drop for Busy {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}