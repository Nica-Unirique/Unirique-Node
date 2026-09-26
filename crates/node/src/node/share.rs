use std::path::PathBuf;

use catalog::{write_infohash, Game, GAMES_FOLDER};

use super::Node;

impl Node {
    /// Fabrique le torrent des jeux qui n'en ont pas encore : un jeu que
    /// l'auteur vient de poser, ou qu'on vient de telecharger.
    pub(super) fn complete_torrents(&self) {
        let folders = self.games.lock().unwrap().needing_torrent();

        for folder in folders.iter() {
            let infohash = self.torrents.make(folder);
            if infohash.is_none() {
                continue;
            }

            let infohash = infohash.unwrap();
            write_infohash(folder, &infohash);
            self.games.lock().unwrap().set_infohash(folder, &infohash);
        }
    }

    pub(super) fn seed_games(&self) {
        let games = self.games.lock().unwrap().get_all();

        for game in games.iter() {
            if game.downloaded && !game.is_exhausted() && game.is_signed() {
                self.seed(game);
            }
        }
    }

    pub(super) fn seed(&self, game: &Game) {
        if !self.torrents.seed(&game.folder) {
            eprintln!("Failed to share {}", game.name);
        }
    }

    pub(super) fn download(&self, game: &Game) -> bool {
        let holders = self.find_holders(&game.infohash);
        if holders.is_empty() {
            eprintln!("No one shares {}", game.name);
            return false;
        }

        let folder = PathBuf::from(GAMES_FOLDER).join(game.folder_name());
        if !self.torrents.fetch(&game.infohash, &folder, holders) {
            eprintln!("Failed to download {}", game.name);
            return false;
        }

        if !game.write_manifest(&folder) {
            eprintln!("Failed to write the manifest of {}", game.name);
            return false;
        }

        let mut games = self.games.lock().unwrap();
        games.add_local(&folder);
        games.load_shares();
        drop(games);

        self.complete_torrents();

        return true;
    }

    pub(super) fn set_share(&self, infohash: &str, share: Option<u32>) -> bool {
        if !self.games.lock().unwrap().set_share(infohash, share) {
            return false;
        }

        let game = self.games.lock().unwrap().find(infohash);
        if game.is_none() {
            return true;
        }

        let game = game.unwrap();
        if game.is_exhausted() || !game.is_signed() {
            return true;
        }

        if !self.torrents.resume(infohash) {
            self.seed(&game);
        }

        return true;
    }

    pub(super) fn apply_quotas(&self) {
        let mut to_pause = Vec::new();

        let mut games = self.games.lock().unwrap();

        for game in games.values.iter_mut() {
            if !game.downloaded || game.is_exhausted() {
                continue;
            }

            let uploaded = self.torrents.uploaded(&game.infohash);
            if uploaded.is_none() {
                continue;
            }
            let uploaded = uploaded.unwrap();

            game.sent += uploaded.saturating_sub(game.session_sent);
            game.session_sent = uploaded;

            if game.is_exhausted() {
                to_pause.push(game.infohash.clone());
            }
        }

        games.save_shares();
        drop(games);

        for infohash in to_pause.iter() {
            self.torrents.pause(infohash);
        }
    }
}
