use std::fs;
use std::path::PathBuf;
use crate::criteria::Criteria;

use crate::game::Game;

const GAMES_FOLDER: &str = "Games";

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

        let entries = fs::read_dir(&games.folder);
        if entries.is_err() {
            eprintln!("Failed to read games folder: {:?}", entries.err().unwrap());
            return games;
        }

        for entry in entries.unwrap() {
            if entry.is_err() {
                continue;
            }
            let entry = entry.unwrap();
            
            games.add_local(&entry.path());
        }
        games
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
            if criteria.matches(game) {
                game_found.push(game.clone());
            }
        }

        return game_found;
    }
}