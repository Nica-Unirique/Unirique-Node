use std::fs;
use std::mem::size_of;
use std::path::PathBuf;

use wire::{key_from_hex, key_to_hex, signature_from_hex, signature_to_hex};

use crate::signature::{content_bytes, verify};

#[derive(Clone)]
pub struct Content {
    pub name: String,
    pub version: [u32; 3],
    pub autor_key: [u8; 32], // clef ed25519
    pub tags: Vec<u64>,
    pub description: String,
    /// Le chemin de la jaquette dans `content/`, ou rien.
    pub cover: String,

    pub ram_weight: u64,

    pub downloaded: bool,
    pub downloadable: bool,
    pub disk_weight: u64,

    pub share: Option<u32>,
    pub infohash: String,

    pub folder: PathBuf,

    pub sent: u64,
    pub session_sent: u64,
    pub signature: [u8; 64],
}

impl Content {
    pub fn new(folder: &PathBuf) -> Option<Self> {
        let mut content = Content {
            name: String::new(),
            version: [0, 0, 0],
            autor_key: [0; 32],
            tags: Vec::new(),
            description: String::new(),
            cover: String::new(),
            ram_weight: 0,
            downloaded: false,
            downloadable: false,
            disk_weight: 0,
            share: Some(4),
            infohash: String::new(),
            folder: folder.clone(),
            sent: 0,
            session_sent: 0,
            signature: [0; 64],
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
        content.name = name.unwrap();

        let version = json["version"].as_array().map(|arr| {
            [
                arr.get(0).and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                arr.get(1).and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                arr.get(2).and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            ]});
        if version.is_none() {
            return None;
        }
        content.version = version.unwrap();

        // Sans auteur, le contenu attend une cle : celle du node principal,
        // qui signe les contenus officiels qu'on pose dans son dossier.
        let autor = json["autor_key"].as_str();
        if autor.is_some() && !autor.unwrap().is_empty() {
            let autor_key = key_from_hex(autor.unwrap());
            if autor_key.is_none() {
                return None;
            }
            content.autor_key = autor_key.unwrap();
        }

        let tags = json["tags"].as_array().map(|arr| {
            arr.iter().filter_map(|v| v.as_u64()).collect::<Vec<u64>>()});
        if tags.is_some() {
            content.tags = tags.unwrap();
        }

        let description = json["description"].as_str().map(String::from);
        if description.is_some() {
            content.description = description.unwrap();
        }

        let cover = json["cover"].as_str();
        if cover.is_some() {
            content.cover = cover.unwrap().to_string();
        }

        let signature = json["signature"].as_str();
        if signature.is_some() {
            let read = signature_from_hex(signature.unwrap());
            if read.is_some() {
                content.signature = read.unwrap();
            }
        }

        content.compute_ram_weight();

        // Sans content.torrent, l'infohash reste vide : le node fabriquera le
        // torrent (crate `torrents`), puis appellera `Contents::set_infohash`.
        let infohash = json["infohash"].as_str();
        if infohash.is_some() && folder.join("content.torrent").is_file() {
            content.infohash = infohash.unwrap().to_string();
        }

        let files = folder.join("content");
        content.downloaded = files.is_dir();
        content.disk_weight = folder_size(&files);

        let mut downloadable = false;
        if content.downloaded {
            downloadable = json["downloadable"].as_bool().unwrap_or(false);
        }
        content.downloadable = downloadable;

        Some(content)
    }

    pub fn compute_ram_weight(&mut self) {
        let weight = size_of::<Content>()
            + self.name.capacity()
            + self.tags.capacity() * size_of::<u64>()
            + self.description.capacity()
            + self.cover.capacity();

        self.ram_weight = weight as u64;
    }

    pub fn needs_torrent(&self) -> bool {
        return self.infohash.is_empty();
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
            "cover": self.cover,
            "infohash": self.infohash,
            "signature": signature_to_hex(&self.signature),
        });

        let text = serde_json::to_string_pretty(&manifest);
        if text.is_err() {
            return false;
        }

        return fs::write(folder.join("manifest.json"), text.unwrap()).is_ok();
    }

    /// Une jaquette annoncee, dont le chemin reste dans `content/` : pas de
    /// `..`, pas de chemin absolu, pas de lettre de lecteur.
    pub fn cover_is_safe(&self) -> bool {
        if self.cover.is_empty() || self.cover.starts_with('/') || self.cover.starts_with('\\') {
            return false;
        }

        if self.cover.contains(':') || self.cover.contains('\\') {
            return false;
        }

        for part in self.cover.split('/') {
            if part.is_empty() || part == "." || part == ".." {
                return false;
            }
        }

        return true;
    }

    pub fn is_signed(&self) -> bool {
        return verify(&self.autor_key, &content_bytes(self), &self.signature);
    }

    pub fn is_exhausted(&self) -> bool {
        if self.share.is_none() {
            return false;
        }

        return self.sent >= self.share.unwrap() as u64 * self.disk_weight;
    }
}

pub fn write_infohash(folder: &PathBuf, infohash: &str) {
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
