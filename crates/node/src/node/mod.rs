//! Le node : il relie les autres crates entre eux.
//!
//! - `network.rs` : la porte reseau, ce que les autres nodes nous demandent ;
//! - `local.rs`   : la porte locale, ce que le client du joueur nous demande ;
//! - `upkeep.rs`  : l'entretien, toutes les 10 secondes ;
//! - `lookup.rs`  : la recherche des sources d'un jeu, par vagues ;
//! - `share.rs`   : le partage et le telechargement des jeux.

mod local;
mod lookup;
mod network;
mod proxy;
mod share;
mod shelves;
mod upkeep;

use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::atomic::AtomicU8;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use catalog::{Contents, Holders, Servers, CONTENTS_FOLDER};
use neighbors::{Address, Neighbors};
use torrents::Torrents;

use crate::settings::Settings;
use share::Download;

const MAIN_NODE: &str = "127.0.0.1:8735";
const ENTER_TRIES: u32 = 10;
/// En mode passif, un voisin qui ne nous a pas contactes depuis ce temps est
/// oublie : on ne peut pas l'appeler pour savoir s'il vit.
const SILENT_MAX: Duration = Duration::from_secs(10 * 60);

const UPKEEP_DELAY: Duration = Duration::from_secs(10);
const ANSWER_WITHIN: Duration = Duration::from_secs(5);
const BUSY_MAX: usize = 64;
const DEPTH_MAX: u8 = 63;
const WEIGHT_HIGH: u64 = 10 * 1024 * 1024;
const WEIGHT_LOW: u64 = 5 * 1024 * 1024;
const NEIGHBORS_ASKED: usize = 128;

const TORRENT_PORT_SHIFT: u16 = 10000;
const LOCAL_PORT_SHIFT: u16 = 20000;

const LOOKUP_WIDTH: usize = 3;
const LOOKUP_WAVES: usize = 20;
const CLOSER_SENT: usize = 16;
const HOLDERS_WANTED: usize = 8;
const ANNOUNCE_TO: usize = 8;
const ANNOUNCE_EVERY: u32 = 60;
const SHELF_COPIES: usize = 4;
const COVERS_FOLDER: &str = "Covers";
const COVER_WITHIN: Duration = Duration::from_secs(60);

pub struct Node {
    ismain: bool,
    id: u64,
    depth: AtomicU8,
    address: Address,
    passive: bool,
    proxy_protocol: bool,
    /// Le port que les autres joignent : celui du tunnel s'il y en a un.
    public_port: u16,
    /// Le port torrent que les autres joignent.
    public_torrent_port: u16,

    neighbors: Mutex<Neighbors>,
    contents: Mutex<Contents>,
    servers: Mutex<Servers>,
    holders: Mutex<Holders>,
    downloads: Mutex<HashMap<String, Download>>,

    torrents: Torrents,
}

impl Node {
    pub fn new(settings: Settings) -> Option<Node> {
        let torrent_port = settings.address.port + TORRENT_PORT_SHIFT;
        let torrents = Torrents::new(CONTENTS_FOLDER, torrent_port, settings.passive);
        if torrents.is_none() {
            return None;
        }

        let public_port = settings.public_port.unwrap_or(settings.address.port);
        let public_torrent_port = settings.public_torrent_port.unwrap_or(torrent_port);

        return Some(Node {
            ismain: settings.ismain,
            address: settings.address,
            passive: settings.passive,
            proxy_protocol: settings.proxy_protocol,
            public_port,
            public_torrent_port,
            id: settings.id,
            depth: AtomicU8::new(0),
            neighbors: Mutex::new(Neighbors::new()),
            contents: Mutex::new(Contents::new()),
            servers: Mutex::new(Servers::new()),
            holders: Mutex::new(Holders::new()),
            downloads: Mutex::new(HashMap::new()),
            torrents: torrents.unwrap(),
        });
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

        self.complete_torrents();
        self.seed_contents();
        self.listen(listener.unwrap());
        self.listen_local(local.unwrap());
        self.upkeep();
    }
}
