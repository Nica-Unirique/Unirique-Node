use std::fs;
use std::path::Path;

use neighbors::Address;

pub struct Settings {
    pub ismain: bool,
    pub id: u64,
    pub address: Address,
    /// Ne fait que repondre : n'ouvre jamais de connexion vers un autre node
    /// ni vers un pair torrent. Pour un node dont l'IP doit rester cachee
    /// derriere un tunnel.
    pub passive: bool,
    /// Le port que les autres joignent, si un tunnel le change.
    pub public_port: Option<u16>,
    /// Le port torrent que les autres joignent, si un tunnel le change.
    pub public_torrent_port: Option<u16>,
    /// Chaque connexion commence par l'en-tete PROXY du tunnel, qui donne la
    /// vraie adresse de celui qui se connecte.
    pub proxy_protocol: bool,
}

impl Settings {
    pub fn new() -> Self {
        Self {
            ismain: false,
            id: rand::random(),
            address: Address::here(8735),
            passive: false,
            public_port: None,
            public_torrent_port: None,
            proxy_protocol: false,
        }
    }

    pub fn from_arguments(arguments: impl Iterator<Item = String>) -> Self {
        let mut new = Settings::new();

        let mut i = 0;
        let arguments: Vec<String> = arguments.collect();
        while i < arguments.len() {
            let argument = &arguments[i];
            i += 1;

            if argument == "--main" {
                new.ismain = true;
                continue;
            }
            else if argument == "--passive" {
                new.passive = true;
                continue;
            }
            else if argument == "--proxy-protocol" {
                new.proxy_protocol = true;
                continue;
            }
            else if !argument.contains('=') {
                continue;
            }

            let split = argument.split_once('=');
            let (name, value) = split.unwrap();

            if name == "--port" {
                let read = value.parse::<u16>();
                if read.is_err() {
                    eprintln!("unreadable port: {}", value);
                    continue;
                }
                new.address.port = read.unwrap();
                continue;
            }

            if name == "--public-port" || name == "--public-torrent" {
                let read = value.parse::<u16>();
                if read.is_err() {
                    eprintln!("unreadable port: {}", value);
                    continue;
                }

                if name == "--public-port" {
                    new.public_port = Some(read.unwrap());
                }
                else {
                    new.public_torrent_port = Some(read.unwrap());
                }
                continue;
            }
        }

        if Path::new("data/id.txt").is_file() {
            let read = fs::read_to_string("data/id.txt");
            if read.is_ok() {
                let id = read.unwrap().trim().parse::<u64>();
                if id.is_ok() {
                    new.id = id.unwrap();
                }
                else {
                    eprintln!("Failed to parse id file");
                }
            }
            else {
                eprintln!("Failed to read id file");
            }
        }
        else {
            let _ = fs::create_dir_all("data");
            let _ = fs::write("data/id.txt", new.id.to_string());
        }

        return new;
    }
}