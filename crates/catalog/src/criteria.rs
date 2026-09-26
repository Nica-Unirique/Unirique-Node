use crate::game::Game;
use crate::server::Server;

#[derive(Clone)]
pub struct Criteria {
    pub name: Option<String>,
    pub tags: Vec<u64>,
    pub autor_key: Option<[u8; 32]>,
}

impl Criteria {
    pub fn matches(&self, game: &Game) -> bool {
        if self.name.is_some() {
            let name = self.name.as_ref().unwrap().to_lowercase();
            if !game.name.to_lowercase().contains(&name) {
                return false;
            }
        }

        for tag in self.tags.iter() {
            if !game.tags.contains(tag) {
                return false;
            }
        }

        if self.autor_key.is_some() && self.autor_key.unwrap() != game.autor_key {
            return false;
        }

        return true;
    }
}
#[derive(Clone)]
pub struct ServerCriteria {
    pub name: Option<String>,
    pub game_name: Option<String>,
    pub game_version: Option<[u32; 3]>,
    pub game_autor_key: Option<[u8; 32]>,
    pub host_key: Option<[u8; 32]>,
}

impl ServerCriteria {
    pub fn matches(&self, server: &Server) -> bool {
        if self.name.is_some() {
            let name = self.name.as_ref().unwrap().to_lowercase();
            if !server.name.to_lowercase().contains(&name) {
                return false;
            }
        }

        if self.game_name.is_some() {
            let game_name = self.game_name.as_ref().unwrap().to_lowercase();
            if !server.game_name.to_lowercase().contains(&game_name) {
                return false;
            }
        }

        if self.game_version.is_some() && self.game_version.unwrap() != server.game_version {
            return false;
        }

        if self.game_autor_key.is_some() && self.game_autor_key.unwrap() != server.game_autor_key {
            return false;
        }

        if self.host_key.is_some() && self.host_key.unwrap() != server.host_key {
            return false;
        }

        return true;
    }
}
