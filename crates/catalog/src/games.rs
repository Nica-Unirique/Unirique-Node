use std::fs;
use std::path::PathBuf;

use wire::key_to_hex;

use crate::criteria::Criteria;
use crate::game::Game;
use crate::shelves::{game_positions, on_shelf};
use crate::shares::{escape_csv, parse_share, read_default_share, share_to_text, version_to_text, SHARES_FILE};

pub const GAMES_FOLDER: &str = "Games";

pub struct Games {
    pub values: Vec<Game>,
    pub folder: PathBuf,

    pub ram_weight: u64,
    pub disk_weight: u64,
}

impl Games {
    pub fn new() -> Self {
        let mut games = Games {
            values: Vec::new(),
            folder: PathBuf::from(GAMES_FOLDER),
            ram_weight: 0,
            disk_weight: 0,
        };

        games.load_folder();
        games.load_shares();

        games
    }

    fn load_folder(&mut self) {
        let entries = fs::read_dir(&self.folder);
        if entries.is_err() {
            eprintln!("Failed to read games folder: {}", GAMES_FOLDER);
            return;
        }

        for entry in entries.unwrap() {
            if entry.is_err() {
                continue;
            }

            self.add_local(&entry.unwrap().path());
        }
    }

    pub fn add_local(&mut self, game_folder: &PathBuf) {
        let game = Game::new(game_folder);
        if game.is_none() {
            return;
        }
        let game = game.unwrap();
        let ram_weight = game.ram_weight;
        let disk_weight = game.disk_weight;
        self.values.push(game);
        self.ram_weight += ram_weight;
        self.disk_weight += disk_weight;
    }

    pub fn add_received(&mut self, game_new: Game) {
        if !game_new.is_signed() {
            return;
        }

        for game in self.values.iter() {
            if game.name == game_new.name && game.version == game_new.version && game.autor_key == game_new.autor_key {
                return;
            }
        }

        self.ram_weight += game_new.ram_weight;
        self.values.push(game_new);
    }

    pub fn get_all(&self) -> Vec<Game> {
        return self.values.clone();
    }

    pub fn get_by_criteria(&self, criteria: &Criteria) -> Vec<Game> {
        let mut game_found = Vec::new();

        for game in self.values.iter() {
            if criteria.matches(game) && game.is_signed() {
                game_found.push(game.clone());
            }
        }

        return game_found;
    }

    /// Oublie les fiches recues qui ne sont plus sur mon etagere. Les jeux
    /// telecharges restent : ils sont a moi.
    pub fn forget_outside(&mut self, id: u64, depth: u8) {
        let mut index = self.values.len();

        while index > 0 {
            index -= 1;

            let game = &self.values[index];
            if game.downloaded || on_shelf(&game_positions(game), id, depth) {
                continue;
            }

            self.ram_weight -= game.ram_weight;
            self.values.remove(index);
        }
    }

    pub fn needing_torrent(&self) -> Vec<PathBuf> {
        let mut folders = Vec::new();

        for game in self.values.iter() {
            if game.downloaded && game.needs_torrent() {
                folders.push(game.folder.clone());
            }
        }

        return folders;
    }

    pub fn set_infohash(&mut self, folder: &PathBuf, infohash: &str) {
        for game in self.values.iter_mut() {
            if &game.folder == folder {
                game.infohash = infohash.to_string();
            }
        }
    }

    pub fn load_shares(&mut self) {
        let default = read_default_share();

        for game in self.values.iter_mut() {
            game.share = default;
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

            for game in self.values.iter_mut() {
                if escape_csv(&game.name) == parts[0]
                    && version_to_text(game.version) == parts[1]
                    && key_to_hex(&game.autor_key) == parts[2]
                {
                    game.share = parse_share(parts[3]);
                    game.sent = parts[4].parse::<u64>().unwrap_or(0);
                }
            }
        }
    }

    pub fn save_shares(&self) {
        let mut text = String::from("name,version,autor_key,share,sent\n");

        for game in self.values.iter() {
            if !game.downloaded {
                continue;
            }

            text += &format!(
                "{},{},{},{},{}\n",
                escape_csv(&game.name),
                version_to_text(game.version),
                key_to_hex(&game.autor_key),
                share_to_text(game.share),
                game.sent
            );
        }

        if fs::write(SHARES_FILE, text).is_err() {
            eprintln!("Failed to save shares: {}", SHARES_FILE);
        }
    }

    pub fn is_holding(&self, infohash: &str) -> bool {
        for game in self.values.iter() {
            if game.infohash == infohash && game.downloaded && !game.is_exhausted() && game.is_signed() {
                return true;
            }
        }

        return false;
    }

    pub fn get_signed(&self) -> Vec<Game> {
        let mut signed = Vec::new();

        for game in self.values.iter() {
            if game.is_signed() {
                signed.push(game.clone());
            }
        }

        return signed;
    }

    pub fn find(&self, infohash: &str) -> Option<Game> {
        for game in self.values.iter() {
            if game.infohash == infohash && game.downloaded {
                return Some(game.clone());
            }
        }

        return None;
    }

    pub fn set_signature(&mut self, infohash: &str, signature: [u8; 64]) -> Option<Game> {
        for game in self.values.iter_mut() {
            if game.infohash != infohash || !game.downloaded {
                continue;
            }

            let before = game.signature;
            game.signature = signature;

            if !game.is_signed() {
                game.signature = before;
                return None;
            }

            if !game.write_manifest(&game.folder) {
                return None;
            }

            return Some(game.clone());
        }

        return None;
    }

    pub fn get_installed(&self) -> Vec<Game> {
        let mut installed = Vec::new();

        for game in self.values.iter() {
            if game.downloaded {
                installed.push(game.clone());
            }
        }

        return installed;
    }

    pub fn set_share(&mut self, infohash: &str, share: Option<u32>) -> bool {
        if share == Some(0) {
            return false;
        }

        let mut found = false;

        for game in self.values.iter_mut() {
            if game.infohash == infohash && game.downloaded {
                game.share = share;
                found = true;
            }
        }

        if found {
            self.save_shares();
        }

        return found;
    }
}
