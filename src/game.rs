use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::fs;
use serde_json;
use sha2::{Digest, Sha256};
use std::io::Read;

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
        };

        let manifest = folder.join("manifest.json");
        if !manifest.is_file() {
            return None;
        }
        let text = fs::read_to_string(&manifest);
        if text.is_err() {
            return None;
        }
        let json = serde_json::from_str::<serde_json::Value>(&text.unwrap());
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

        let (downloaded, disk_weight) = Game::is_download(folder, &json);
        game.downloaded = downloaded;
        game.disk_weight = disk_weight;

        let mut downloadable = false;
        if downloaded {
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

    pub fn is_download(folder: &PathBuf, json: &serde_json::Value) -> (bool, u64) {
        let files = json["files"].as_array();
        if files.is_none() {
            return (false, 0);
        }
        let mut disk_weight = 0;
        let files = files.unwrap();
        for file in files {
            let relative = file["path"].as_str();
            let size = file["size"].as_u64();
            let hash = file["hash"].as_str();

            if relative.is_none() || size.is_none() || hash.is_none() {
                return (false, 0);
            }

            let path = folder.join(relative.unwrap());
            if !is_inside(folder, &path) {
                return (false, 0);
            }

            let metadata = fs::metadata(&path);
            if metadata.is_err() {
                return (false, 0);
            }
            let metadata = metadata.unwrap();
            if !metadata.is_file() || metadata.len() != size.unwrap() {
                return (false, 0);
            }
            disk_weight += metadata.len();

            let found = hash_of(&path);
            if found.is_none() || found.unwrap() != hash.unwrap() {
                return (false, 0);
            }
        }
        return (true, disk_weight);
    }
}

/// Le fichier `path` est-il vraiment dans le dossier `folder` ?
pub fn is_inside(folder: &Path, path: &Path) -> bool {
    // Le vrai emplacement sur le disque : les `..` et les liens sont suivis.
    let real_folder = fs::canonicalize(folder);
    if real_folder.is_err() {
        return false;
    }

    let real_path = fs::canonicalize(path);
    if real_path.is_err() {
        return false;
    }

    let real_folder = real_folder.unwrap();
    let real_path = real_path.unwrap();
    return real_path.starts_with(real_folder);
}

/// Le SHA-256 d'un fichier, en hexadecimal minuscule.
pub fn hash_of(path: &Path) -> Option<String> {
    let file = fs::File::open(path);
    if file.is_err() {
        return None;
    }

    let mut file = file.unwrap();
    let mut hasher = Sha256::new();
    let mut piece = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut piece);
        if read.is_err() {
            return None;
        }

        let bytes_read = read.unwrap();
        if bytes_read == 0 {
            break;
        }

        hasher.update(&piece[..bytes_read]);
    }

    let hash = hasher.finalize();
    return Some(format!("{:x}", hash));
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