use std::mem::size_of;
use std::path::PathBuf;
use std::fs;
use serde_json;
use librqbit::spawn_utils::BlockingSpawner;
use librqbit::{create_torrent, CreateTorrentOptions};

#[derive(Clone)]
pub struct Game {
    pub name: String,
    pub version: [u32; 3],
    pub autor_key: [u8; 32], // clef ed25519
    pub tags: Vec<u64>,
    pub description: String,

    pub ram_weight: u64,

    pub downloaded: bool,
    pub downloadable: bool,
    pub disk_weight: u64,
    
    pub share: Option<u32>,
    pub infohash: String,

    pub folder: PathBuf,

    pub sent: u64,
    pub session_sent: u64,
}

impl Game {
    pub fn new(folder: &PathBuf) -> Option<Self> {
        let mut game = Game {
            name: String::new(),
            version: [0, 0, 0],
            autor_key: [0; 32],
            tags: Vec::new(),
            description: String::new(),
            ram_weight: 0,
            downloaded: false,
            downloadable: false,
            disk_weight: 0,
            share: Some(4),
            infohash: String::new(),
            folder: folder.clone(),
            sent: 0,
            session_sent: 0,
        };

        let manifest = folder.join("manifest.json");
        if !manifest.is_file() {
            return None;
        }
        let text = fs::read_to_string(&manifest);
        if text.is_err() {
            return None;
        }
        let text = text.unwrap();
        let json = serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}'));
        if json.is_err() {
            return None;
        }
        let json = json.unwrap();

        let name = json["name"].as_str().map(String::from);
        if name.is_none() {
            return None;
        }
        game.name = name.unwrap();
        
        let version = json["version"].as_array().map(|arr| {
            [
                arr.get(0).and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                arr.get(1).and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                arr.get(2).and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            ]});
        if version.is_none() {
            return None;
        }
        game.version = version.unwrap();

        let autor = json["autor_key"].as_str();
        if autor.is_none() {
            return None;
        }

        let autor_key = key_from_hex(autor.unwrap());
        if autor_key.is_none() {
            return None;
        }
        game.autor_key = autor_key.unwrap();

        let tags = json["tags"].as_array().map(|arr| {
            arr.iter().filter_map(|v| v.as_u64()).collect::<Vec<u64>>()});
        if tags.is_none() {
            return None;
        }
        game.tags = tags.unwrap();

        let description = json["description"].as_str().map(String::from);
        if description.is_none() {
            return None;
        }
        game.description = description.unwrap();
        game.compute_ram_weight();

        let infohash = ensure_torrent(folder, &json);
        if infohash.is_none() {
            return None;
        }
        game.infohash = infohash.unwrap();

        let content = folder.join("content");
        game.downloaded = content.is_dir();
        game.disk_weight = folder_size(&content);

        let mut downloadable = false;
        if game.downloaded {
            downloadable = json["downloadable"].as_bool().unwrap_or(false);
        }
        game.downloadable = downloadable;
        
        Some(game)
    }

    pub fn compute_ram_weight(&mut self) {
        let weight = size_of::<Game>()
            + self.name.capacity()
            + self.tags.capacity() * size_of::<u64>()
            + self.description.capacity();

        self.ram_weight = weight as u64;
    }

    pub fn folder_name(&self) -> String {
        let mut name = String::new();

        for letter in self.name.chars() {
            if letter.is_alphanumeric() || letter == '-' || letter == '_' {
                name.push(letter);
            } else {
                name.push('_');
            }
        }

        return format!("{}-{}.{}.{}", name, self.version[0], self.version[1], self.version[2]);
    }

    pub fn write_manifest(&self, folder: &PathBuf) -> bool {
        let manifest = serde_json::json!({
            "name": self.name,
            "version": self.version,
            "autor_key": key_to_hex(&self.autor_key),
            "tags": self.tags,
            "description": self.description,
            "infohash": self.infohash,
        });

        let text = serde_json::to_string_pretty(&manifest);
        if text.is_err() {
            return false;
        }

        return fs::write(folder.join("manifest.json"), text.unwrap()).is_ok();
    }

    pub fn is_exhausted(&self) -> bool {
        if self.share.is_none() {
            return false;
        }

        return self.sent >= self.share.unwrap() as u64 * self.disk_weight;
    }
}

pub fn key_from_hex(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }

    let mut key = [0u8; 32];
    for place in 0..32 {
        let pair = &text[place * 2..place * 2 + 2];
        let byte = u8::from_str_radix(pair, 16);
        if byte.is_err() {
            return None;
        }

        key[place] = byte.unwrap();
    }
    
    return Some(key);
}

pub fn key_to_hex(key: &[u8; 32]) -> String {
    let mut text = String::new();

    for byte in key.iter() {
        text += &format!("{:02x}", byte);
    }

    return text;
}

fn ensure_torrent(folder: &PathBuf, json: &serde_json::Value) -> Option<String> {
    let known = json["infohash"].as_str();
    if known.is_some() && folder.join("game.torrent").is_file() {
        return Some(known.unwrap().to_string());
    }

    let made = make_torrent(folder);
    if made.is_none() {
        return None;
    }

    let infohash = made.unwrap();
    write_infohash(folder, &infohash);

    return Some(infohash);
}

fn folder_size(folder: &PathBuf) -> u64 {
    let entries = fs::read_dir(folder);
    if entries.is_err() {
        return 0;
    }

    let mut size = 0;

    for entry in entries.unwrap() {
        if entry.is_err() {
            continue;
        }

        let path = entry.unwrap().path();

        if path.is_dir() {
            size += folder_size(&path);
            continue;
        }

        let metadata = fs::metadata(&path);
        if metadata.is_ok() {
            size += metadata.unwrap().len();
        }
    }

    return size;
}

fn make_torrent(folder: &PathBuf) -> Option<String> {
    let runtime = tokio::runtime::Runtime::new();
    if runtime.is_err() {
        eprintln!("Failed to start tokio");
        return None;
    }

    let content = folder.join("content");
    let torrent = runtime.unwrap().block_on(async {
        let spawner = BlockingSpawner::new(1);
        return create_torrent(&content, CreateTorrentOptions::default(), &spawner).await;
    });
    if torrent.is_err() {
        eprintln!("Failed to create the torrent: {:?}", torrent.err());
        return None;
    }

    let torrent = torrent.unwrap();

    let bytes = torrent.as_bytes();
    if bytes.is_err() {
        eprintln!("Failed to encode the torrent");
        return None;
    }

    if fs::write(folder.join("game.torrent"), bytes.unwrap()).is_err() {
        eprintln!("Failed to write game.torrent");
        return None;
    }

    return Some(torrent.info_hash().as_string());
}

fn write_infohash(folder: &PathBuf, infohash: &str) {
    let path = folder.join("manifest.json");

    let text = fs::read_to_string(&path);
    if text.is_err() {
        eprintln!("Failed to read manifest.json");
        return;
    }

    let text = text.unwrap();
    let json = serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}'));
    if json.is_err() {
        eprintln!("manifest.json is not valid json");
        return;
    }

    let mut json = json.unwrap();
    json["infohash"] = serde_json::Value::from(infohash);

    let object = json.as_object_mut();
    if object.is_some() {
        object.unwrap().remove("files");
    }

    let written = serde_json::to_string_pretty(&json);
    if written.is_err() || fs::write(&path, written.unwrap()).is_err() {
        eprintln!("Failed to write manifest.json");
        return;
    }

    println!("infohash: {}", infohash);
}

