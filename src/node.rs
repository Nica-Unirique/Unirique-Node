use std::fs;
use std::path::PathBuf;
use tokio::runtime::Runtime;
use librqbit::{AddTorrent, AddTorrentOptions, ListenerMode, ListenerOptions, Session, SessionOptions};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::str::FromStr;
use librqbit::api::TorrentIdOrHash;
use librqbit::dht::Id20;
use librqbit::ManagedTorrent;

use crate::games::GAMES_FOLDER;
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
use crate::command::Command;

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
const LOCAL_PORT_SHIFT: u16 = 20000;

pub struct Node {
    ismain: bool,
    id: u64,
    depth: AtomicU8,
    address: Address,

    neighbors: Mutex<Neighbors>,
    games: Mutex<Games>,
    servers: Mutex<Servers>,

    runtime: Runtime,
    torrents: Arc<Session>,
}

impl Node {
    pub fn new(settings: Settings) -> Option<Node> {
        let runtime = Runtime::new();
        if runtime.is_err() {
            eprintln!("Failed to start tokio");
            return None;
        }
        let runtime = runtime.unwrap();

        let port = settings.address.port + TORRENT_PORT_SHIFT;
        let torrents = runtime.block_on(Session::new_with_opts(PathBuf::from(GAMES_FOLDER), torrent_options(port)));
        if torrents.is_err() {
            eprintln!("Failed to start torrents on port {}", port);
            return None;
        }

        return Some(Node {
            ismain: settings.ismain,
            address: settings.address,
            id: settings.id,
            depth: AtomicU8::new(0),
            neighbors: Mutex::new(Neighbors::new()),
            games: Mutex::new(Games::new()),
            servers: Mutex::new(Servers::new()),
            runtime,
            torrents: torrents.unwrap(),
        });
    }

    fn seed_games(&self) {
        let games = self.games.lock().unwrap().get_all();

        for game in games.iter() {
            if game.downloaded && !game.is_exhausted() {
                self.seed(game);
            }
        }
    }

    fn seed(&self, game: &Game) {
        let torrent = fs::read(game.folder.join("game.torrent"));
        if torrent.is_err() {
            return;
        }

        let mut options = AddTorrentOptions::default();
        options.output_folder = Some(game.folder.join("content").to_string_lossy().to_string());
        options.overwrite = true;
        options.disable_trackers = true;

        let added = self.runtime.block_on(self.torrents.add_torrent(AddTorrent::from_bytes(torrent.unwrap()), Some(options)));
        if added.is_err() {
            eprintln!("Failed to share {}", game.name);
        }
    }

    pub fn run(self: &Arc<Self>) {
        let listener = TcpListener::bind(self.address.socket());
        if listener.is_err() {
            eprintln!("Failed to listen on {}", self.address.to_text());
            return;
        }

        let local = TcpListener::bind(("127.0.0.1", self.address.port + LOCAL_PORT_SHIFT));
        if local.is_err() {
            eprintln!("Failed to open the local door on port {}", self.address.port + LOCAL_PORT_SHIFT);
            return;
        }

        self.seed_games();
        self.listen(listener.unwrap());
        self.listen_local(local.unwrap());
        self.upkeep();
    }

    fn listen_local(self: &Arc<Self>, listener: TcpListener) {
        let node = self.clone();

        thread::spawn(move || {
            for coming in listener.incoming() {
                if coming.is_err() {
                    continue;
                }

                let serving = node.clone();
                let stream = coming.unwrap();

                thread::spawn(move || serving.serve_local(stream));
            }
        });
    }

    fn serve_local(&self, mut stream: TcpStream) {
        let bytes = read_frame(&mut stream);
        if bytes.is_none() {
            return;
        }

        let command = Command::from_bytes(&bytes.unwrap());
        if command.is_none() {
            return;
        }

        let answer = self.obey(command.unwrap());
        if answer.is_none() {
            return;
        }

        write_frame(&mut stream, &answer.unwrap().to_bytes());
    }

    fn obey(&self, command: Command) -> Option<Command> {
        match command {
            Command::Search { criteria } => return Some(Command::Games { games: self.search(&criteria) }),
            Command::Download { game } => return Some(Command::Done { ok: self.download(&game) }),
            Command::Installed => return Some(Command::Games { games: self.games.lock().unwrap().get_installed() }),
            Command::SetShare { infohash, share } => {
                return Some(Command::Done { ok: self.games.lock().unwrap().set_share(&infohash, share) })
            }
            Command::Games { .. } => return None,
            Command::Done { .. } => return None,
        }
    }

    fn search(&self, criteria: &Criteria) -> Vec<Game> {
        let contacts = self.neighbors.lock().unwrap().contacts();

        for address in contacts {
            let answer = self.ask(address, Message::GetGames { criteria: criteria.clone() });

            match answer {
                Some(Message::SendGames { games }) => self.receive_games(&games),
                _ => {}
            }
        }

        return self.games.lock().unwrap().get_by_criteria(criteria);
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
            Message::GetHolder { infohash } => return self.answer_get_holder(infohash),
            Message::Holding { .. } => return None,
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

    fn answer_get_holder(&self, infohash: &str) -> Option<Message> {
        if !self.games.lock().unwrap().is_holding(infohash) {
            return None;
        }

        return Some(Message::Holding { torrent_port: self.address.port + TORRENT_PORT_SHIFT });
    }

    fn find_holders(&self, infohash: &str) -> Vec<SocketAddr> {
        let contacts = self.neighbors.lock().unwrap().contacts();
        let mut holders = Vec::new();

        for address in contacts {
            let answer = self.ask(address, Message::GetHolder { infohash: infohash.to_string() });

            match answer {
                Some(Message::Holding { torrent_port }) => holders.push(SocketAddr::new(address.ip, torrent_port)),
                _ => {}
            }
        }

        return holders;
    }

    pub fn download(&self, game: &Game) -> bool {
        let holders = self.find_holders(&game.infohash);
        if holders.is_empty() {
            eprintln!("No one shares {}", game.name);
            return false;
        }

        let folder = PathBuf::from(GAMES_FOLDER).join(game.folder_name());
        if !self.fetch(&game.infohash, &folder, holders) {
            eprintln!("Failed to download {}", game.name);
            return false;
        }

        if !game.write_manifest(&folder) {
            eprintln!("Failed to write the manifest of {}", game.name);
            return false;
        }

        let mut games = self.games.lock().unwrap();
        games.add_local(&folder);
        games.load_shares();

        return true;
    }

    fn fetch(&self, infohash: &str, folder: &PathBuf, holders: Vec<SocketAddr>) -> bool {
        let mut options = AddTorrentOptions::default();
        options.output_folder = Some(folder.join("content").to_string_lossy().to_string());
        options.disable_trackers = true;
        options.initial_peers = Some(holders);

        let magnet = format!("magnet:?xt=urn:btih:{}", infohash);

        return self.runtime.block_on(async {
            let added = self.torrents.add_torrent(AddTorrent::from_url(magnet), Some(options)).await;
            if added.is_err() {
                return false;
            }

            let handle = added.unwrap().into_handle();
            if handle.is_none() {
                return false;
            }

            return handle.unwrap().wait_until_completed().await.is_ok();
        });
    }

    fn apply_quotas(&self) {
        let mut to_pause = Vec::new();

        let mut games = self.games.lock().unwrap();

        for game in games.values.iter_mut() {
            if !game.downloaded || game.is_exhausted() {
                continue;
            }

            let torrent = self.find_torrent(&game.infohash);
            if torrent.is_none() {
                continue;
            }
            let torrent = torrent.unwrap();

            let uploaded = torrent.stats().uploaded_bytes;
            game.sent += uploaded.saturating_sub(game.session_sent);
            game.session_sent = uploaded;

            if game.is_exhausted() {
                to_pause.push(torrent);
            }
        }

        games.save_shares();
        drop(games);

        for torrent in to_pause.iter() {
            let _ = self.runtime.block_on(self.torrents.pause(torrent));
        }
    }

    fn find_torrent(&self, infohash: &str) -> Option<Arc<ManagedTorrent>> {
        let id = Id20::from_str(infohash);
        if id.is_err() {
            return None;
        }

        return self.torrents.get(TorrentIdOrHash::Hash(id.unwrap()));
    }
}

fn read_message(stream: &mut TcpStream) -> Option<Message> {
    let bytes = read_frame(stream);
    if bytes.is_none() {
        return None;
    }

    return Message::from_bytes(&bytes.unwrap());
}

fn write_message(stream: &mut TcpStream, message: &Message) -> bool {
    return write_frame(stream, &message.to_bytes());
}

fn read_frame(stream: &mut TcpStream) -> Option<Vec<u8>> {
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

    return Some(bytes);
}

fn write_frame(stream: &mut TcpStream, bytes: &[u8]) -> bool {
    if bytes.len() > MESSAGE_MAX as usize {
        return false;
    }

    if stream.write_all(&(bytes.len() as u32).to_le_bytes()).is_err() {
        return false;
    }

    if stream.write_all(bytes).is_err() {
        return false;
    }

    return true;
}

const TORRENT_PORT_SHIFT: u16 = 10000;

fn torrent_options(port: u16) -> SessionOptions {
    let mut listen = ListenerOptions::default();
    listen.mode = ListenerMode::TcpOnly;
    listen.listen_addr = SocketAddr::from(([0, 0, 0, 0], port));

    let mut options = SessionOptions::default();
    options.dht = None;
    options.disable_trackers = true;
    options.disable_local_service_discovery = true;
    options.listen = Some(listen);

    return options;
}