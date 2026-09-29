//! Ce que fait le node principal pour etre trouve : il lit les adresses de
//! ses tunnels (`data/tunnels.txt`, ecrit par le script des tunnels), les
//! signe, et ecrit `data/ip.txt`, que le script publie sur GitHub.

use std::fs;
use std::sync::atomic::Ordering;

use catalog::now_seconds;
use main_address::{key_from_secret, public_hex, MainAddress, SigningKey};
use wire::{key_from_hex, key_to_hex};

use super::{resolve, Node};

const MAIN_KEY_FILE: &str = "data/main_key.txt";
const TUNNELS_FILE: &str = "data/tunnels.txt";
const PUBLISHED_FILE: &str = "data/ip.txt";

impl Node {
    /// Quand les tunnels ont change : les nouveaux ports publics, et la
    /// nouvelle adresse signee a publier.
    pub(super) fn publish_address(&self) {
        if self.main_key.is_none() {
            return;
        }

        let text = fs::read_to_string(TUNNELS_FILE);
        if text.is_err() {
            return;
        }
        let text = text.unwrap();

        let mut seen = self.tunnels_seen.lock().unwrap();
        if *seen == text {
            return;
        }
        *seen = text.clone();
        drop(seen);

        let tunnels = read_tunnels(&text);
        if tunnels.is_none() {
            eprintln!("Unreadable {}", TUNNELS_FILE);
            return;
        }

        let mut address = tunnels.unwrap();

        let node = resolve(&address.node);
        let torrent = resolve(&address.torrent);
        if node.is_none() || torrent.is_none() {
            eprintln!("Failed to find the tunnels of {}", TUNNELS_FILE);
            return;
        }

        self.public_port.store(node.unwrap().port, Ordering::Relaxed);
        *self.public_torrent.lock().unwrap() = torrent.unwrap();

        address.time = now_seconds();
        address.sign(self.main_key.as_ref().unwrap());

        if fs::write(PUBLISHED_FILE, address.to_text()).is_err() {
            eprintln!("Failed to write {}", PUBLISHED_FILE);
            return;
        }

        eprintln!("Published address: node {}, torrent {}", address.node, address.torrent);
    }
}

/// La cle du node principal : lue dans `data/main_key.txt`, ou creee la
/// premiere fois. Sa cle publique est affichee : c'est elle qu'il faut
/// ecrire dans `MAIN_KEY`.
pub(super) fn load_main_key() -> Option<SigningKey> {
    let text = fs::read_to_string(MAIN_KEY_FILE);
    if text.is_ok() {
        let secret = key_from_hex(text.unwrap().trim());
        if secret.is_none() {
            eprintln!("Unreadable {}", MAIN_KEY_FILE);
            return None;
        }

        let key = key_from_secret(&secret.unwrap());
        eprintln!("Main key (MAIN_KEY): {}", public_hex(&key));
        return Some(key);
    }

    let secret: [u8; 32] = rand::random();
    let _ = fs::create_dir_all("data");
    if fs::write(MAIN_KEY_FILE, key_to_hex(&secret)).is_err() {
        eprintln!("Failed to write {}", MAIN_KEY_FILE);
        return None;
    }

    let key = key_from_secret(&secret);
    eprintln!("New main key (MAIN_KEY): {}", public_hex(&key));
    return Some(key);
}

/// `node=`, `torrent=` et `ucompany=`, une par ligne.
fn read_tunnels(text: &str) -> Option<MainAddress> {
    let mut node = None;
    let mut torrent = None;
    let mut ucompany = None;

    for line in text.lines() {
        let split = line.trim().split_once('=');
        if split.is_none() {
            continue;
        }

        let (name, value) = split.unwrap();
        let value = value.trim();
        if value.is_empty() {
            continue;
        }

        match name.trim() {
            "node" => node = Some(value),
            "torrent" => torrent = Some(value),
            "ucompany" => ucompany = Some(value),
            _ => {}
        }
    }

    if node.is_none() || torrent.is_none() || ucompany.is_none() {
        return None;
    }

    return Some(MainAddress::new(node.unwrap(), torrent.unwrap(), ucompany.unwrap(), 0));
}
