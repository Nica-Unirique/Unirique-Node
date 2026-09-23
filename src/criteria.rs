use crate::game::Game;

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