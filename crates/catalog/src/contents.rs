use std::fs;
use std::path::PathBuf;

use wire::key_to_hex;

use crate::criteria::Criteria;
use crate::content::Content;
use crate::shelves::{content_positions, on_shelf};
use crate::shares::{escape_csv, parse_share, read_default_share, share_to_text, version_to_text, SHARES_FILE};

pub const CONTENTS_FOLDER: &str = "Contents";

pub struct Contents {
    pub values: Vec<Content>,
    pub folder: PathBuf,

    pub ram_weight: u64,
    pub disk_weight: u64,
}

impl Contents {
    pub fn new() -> Self {
        let mut contents = Contents {
            values: Vec::new(),
            folder: PathBuf::from(CONTENTS_FOLDER),
            ram_weight: 0,
            disk_weight: 0,
        };

        contents.load_folder();
        contents.load_shares();

        contents
    }

    fn load_folder(&mut self) {
        let entries = fs::read_dir(&self.folder);
        if entries.is_err() {
            eprintln!("Failed to read contents folder: {}", CONTENTS_FOLDER);
            return;
        }

        for entry in entries.unwrap() {
            if entry.is_err() {
                continue;
            }

            self.add_local(&entry.unwrap().path());
        }
    }

    pub fn add_local(&mut self, content_folder: &PathBuf) {
        let content = Content::new(content_folder);
        if content.is_none() {
            return;
        }
        let content = content.unwrap();
        let ram_weight = content.ram_weight;
        let disk_weight = content.disk_weight;
        self.values.push(content);
        self.ram_weight += ram_weight;
        self.disk_weight += disk_weight;
    }

    pub fn add_received(&mut self, content_new: Content) {
        if !content_new.is_signed() {
            return;
        }

        for content in self.values.iter() {
            if content.name == content_new.name && content.version == content_new.version && content.autor_key == content_new.autor_key {
                return;
            }
        }

        self.ram_weight += content_new.ram_weight;
        self.values.push(content_new);
    }

    pub fn get_all(&self) -> Vec<Content> {
        return self.values.clone();
    }

    pub fn get_by_criteria(&self, criteria: &Criteria) -> Vec<Content> {
        let mut content_found = Vec::new();

        for content in self.values.iter() {
            if criteria.matches(content) && content.is_signed() {
                content_found.push(content.clone());
            }
        }

        return content_found;
    }

    /// Oublie les fiches recues qui ne sont plus sur mon etagere. Les jeux
    /// telecharges restent : ils sont a moi.
    pub fn forget_outside(&mut self, id: u64, depth: u8) {
        let mut index = self.values.len();

        while index > 0 {
            index -= 1;

            let content = &self.values[index];
            if content.downloaded || on_shelf(&content_positions(content), id, depth) {
                continue;
            }

            self.ram_weight -= content.ram_weight;
            self.values.remove(index);
        }
    }

    pub fn needing_torrent(&self) -> Vec<PathBuf> {
        let mut folders = Vec::new();

        for content in self.values.iter() {
            if content.downloaded && content.needs_torrent() {
                folders.push(content.folder.clone());
            }
        }

        return folders;
    }

    pub fn set_infohash(&mut self, folder: &PathBuf, infohash: &str) {
        for content in self.values.iter_mut() {
            if &content.folder == folder {
                content.infohash = infohash.to_string();
            }
        }
    }

    pub fn load_shares(&mut self) {
        let default = read_default_share();

        for content in self.values.iter_mut() {
            content.share = default;
        }

        let text = fs::read_to_string(SHARES_FILE);
        if text.is_err() {
            return;
        }

        for line in text.unwrap().lines().skip(1) {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() != 5 {
                continue;
            }

            for content in self.values.iter_mut() {
                if escape_csv(&content.name) == parts[0]
                    && version_to_text(content.version) == parts[1]
                    && key_to_hex(&content.autor_key) == parts[2]
                {
                    content.share = parse_share(parts[3]);
                    content.sent = parts[4].parse::<u64>().unwrap_or(0);
                }
            }
        }
    }

    pub fn save_shares(&self) {
        let mut text = String::from("name,version,autor_key,share,sent\n");

        for content in self.values.iter() {
            if !content.downloaded {
                continue;
            }

            text += &format!(
                "{},{},{},{},{}\n",
                escape_csv(&content.name),
                version_to_text(content.version),
                key_to_hex(&content.autor_key),
                share_to_text(content.share),
                content.sent
            );
        }

        if fs::write(SHARES_FILE, text).is_err() {
            eprintln!("Failed to save shares: {}", SHARES_FILE);
        }
    }

    pub fn is_holding(&self, infohash: &str) -> bool {
        for content in self.values.iter() {
            if content.infohash == infohash && content.downloaded && !content.is_exhausted() && content.is_signed() {
                return true;
            }
        }

        return false;
    }

    pub fn get_signed(&self) -> Vec<Content> {
        let mut signed = Vec::new();

        for content in self.values.iter() {
            if content.is_signed() {
                signed.push(content.clone());
            }
        }

        return signed;
    }

    /// Le quota d'un contenu installe, ou rien s'il n'est pas installe.
    pub fn get_share(&self, infohash: &str) -> Option<Option<u32>> {
        let content = self.find(infohash);
        if content.is_none() {
            return None;
        }

        return Some(content.unwrap().share);
    }

    /// Retire un contenu installe de la liste, et rend son dossier.
    pub fn remove(&mut self, infohash: &str) -> Option<PathBuf> {
        let mut index = self.values.len();

        while index > 0 {
            index -= 1;

            if self.values[index].infohash == infohash && self.values[index].downloaded {
                let content = self.values.remove(index);
                self.ram_weight -= content.ram_weight;
                self.disk_weight -= content.disk_weight;
                self.save_shares();

                return Some(content.folder);
            }
        }

        return None;
    }

    pub fn find(&self, infohash: &str) -> Option<Content> {
        for content in self.values.iter() {
            if content.infohash == infohash && content.downloaded {
                return Some(content.clone());
            }
        }

        return None;
    }

    pub fn set_signature(&mut self, infohash: &str, signature: [u8; 64]) -> Option<Content> {
        for content in self.values.iter_mut() {
            if content.infohash != infohash || !content.downloaded {
                continue;
            }

            let before = content.signature;
            content.signature = signature;

            if !content.is_signed() {
                content.signature = before;
                return None;
            }

            if !content.write_manifest(&content.folder) {
                return None;
            }

            return Some(content.clone());
        }

        return None;
    }

    pub fn get_installed(&self) -> Vec<Content> {
        let mut installed = Vec::new();

        for content in self.values.iter() {
            if content.downloaded {
                installed.push(content.clone());
            }
        }

        return installed;
    }

    pub fn set_share(&mut self, infohash: &str, share: Option<u32>) -> bool {
        if share == Some(0) {
            return false;
        }

        let mut found = false;

        for content in self.values.iter_mut() {
            if content.infohash == infohash && content.downloaded {
                content.share = share;
                found = true;
            }
        }

        if found {
            self.save_shares();
        }

        return found;
    }
}
