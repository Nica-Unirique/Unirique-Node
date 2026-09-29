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
mod publish;
mod share;
mod shelves;
mod upkeep;

use std::collections::HashMap;
use std::net::{TcpListener, ToSocketAddrs};
use std::sync::atomic::{AtomicU16, AtomicU8};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use catalog::{Contents, Holders, Servers, CONTENTS_FOLDER};
use main_address::SigningKey;
use neighbors::{Address, Neighbors};
use torrents::Torrents;

use crate::settings::Settings;
use share::Download;

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
    /// Le port que les autres joignent : celui du tunnel s'il y en a un. Il
    /// change avec les tunnels.
    public_port: AtomicU16,
    /// Ou les autres joignent notre partage torrent. Sans IP (`here`), celui
    /// qui demande met celle par laquelle il nous a joints.
    public_torrent: Mutex<Address>,
    /// Le node principal a joindre au lieu de l'adresse publiee.
    main_node: Option<String>,
    /// La cle qui signe l'adresse publiee : seulement le node principal.
    main_key: Option<SigningKey>,
    /// Le dernier `data/tunnels.txt` lu.
    tunnels_seen: Mutex<String>,

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

        let mut main_key = None;
        if settings.ismain {
            main_key = publish::load_main_key();
        }

        return Some(Node {
            ismain: settings.ismain,
            address: settings.address,
            passive: settings.passive,
            proxy_protocol: settings.proxy_protocol,
            public_port: AtomicU16::new(public_port),
            public_torrent: Mutex::new(Address::here(public_torrent_port)),
            main_node: settings.main_node,
            main_key,
            tunnels_seen: Mutex::new(String::new()),
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

/// `hote:port` en adresse IPv4 : un tunnel donne un nom, pas une IP.
fn resolve(text: &str) -> Option<Address> {
    let found = text.to_socket_addrs();
    if found.is_err() {
        return None;
    }

    for socket in found.unwrap() {
        if socket.is_ipv4() {
            return Some(Address::new(socket.ip(), socket.port()));
        }
    }

    return None;
}
