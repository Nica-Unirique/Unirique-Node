use crate::criteria::Criteria;
use crate::game::Game;
use crate::message::{put_criteria, put_game, put_text, put_u16, take_criteria, take_game, take_text, take_u16, take_u8};

const SEARCH: u8 = 1;
const DOWNLOAD: u8 = 2;
const INSTALLED: u8 = 3;
const SET_SHARE: u8 = 4;
const GAMES: u8 = 5;
const DONE: u8 = 6;

pub enum Command {
    Search { criteria: Criteria },
    Download { game: Game },
    Installed,
    SetShare { infohash: String, share: Option<u32> },
    Games { games: Vec<Game> },
    Done { ok: bool },
}

impl Command {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Command::Search { criteria } => return write_search(criteria),
            Command::Download { game } => return write_download(game),
            Command::Installed => return vec![INSTALLED],
            Command::SetShare { infohash, share } => return write_set_share(infohash, *share),
            Command::Games { games } => return write_games(games),
            Command::Done { ok } => return vec![DONE, *ok as u8],
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Command> {
        if bytes.is_empty() {
            return None;
        }

        let rest = &bytes[1..];

        match bytes[0] {
            SEARCH => return read_search(rest),
            DOWNLOAD => return read_download(rest),
            INSTALLED => return Some(Command::Installed),
            SET_SHARE => return read_set_share(rest),
            GAMES => return read_games(rest),
            DONE => return read_done(rest),
            _ => return None,
        }
    }
}

// ---------- Ecriture ----------

fn write_search(criteria: &Criteria) -> Vec<u8> {
    let mut bytes = vec![SEARCH];
    put_criteria(&mut bytes, criteria);

    return bytes;
}

fn write_download(game: &Game) -> Vec<u8> {
    let mut bytes = vec![DOWNLOAD];
    put_game(&mut bytes, game);

    return bytes;
}

fn write_set_share(infohash: &String, share: Option<u32>) -> Vec<u8> {
    let mut bytes = vec![SET_SHARE];
    put_text(&mut bytes, infohash);

    if share.is_none() {
        bytes.push(0);
    } else {
        bytes.push(1);
        bytes.extend_from_slice(&share.unwrap().to_le_bytes());
    }

    return bytes;
}

fn write_games(games: &Vec<Game>) -> Vec<u8> {
    let mut bytes = vec![GAMES];
    put_u16(&mut bytes, games.len() as u16);

    for game in games.iter() {
        put_game(&mut bytes, game);
    }

    return bytes;
}

// ---------- Lecture ----------

fn read_search(bytes: &[u8]) -> Option<Command> {
    let mut at = 0;

    let criteria = take_criteria(bytes, &mut at);
    if criteria.is_none() {
        return None;
    }

    return Some(Command::Search { criteria: criteria.unwrap() });
}

fn read_download(bytes: &[u8]) -> Option<Command> {
    let mut at = 0;

    let game = take_game(bytes, &mut at);
    if game.is_none() {
        return None;
    }

    return Some(Command::Download { game: game.unwrap() });
}

fn read_set_share(bytes: &[u8]) -> Option<Command> {
    let mut at = 0;

    let infohash = take_text(bytes, &mut at);
    let present = take_u8(bytes, &mut at);

    if infohash.is_none() || present.is_none() {
        return None;
    }

    if present.unwrap() == 0 {
        return Some(Command::SetShare { infohash: infohash.unwrap(), share: None });
    }

    if at + 4 > bytes.len() {
        return None;
    }

    let mut value = [0u8; 4];
    value.copy_from_slice(&bytes[at..at + 4]);

    return Some(Command::SetShare { infohash: infohash.unwrap(), share: Some(u32::from_le_bytes(value)) });
}

fn read_games(bytes: &[u8]) -> Option<Command> {
    let mut at = 0;

    let count = take_u16(bytes, &mut at);
    if count.is_none() {
        return None;
    }

    let mut games = Vec::new();

    for _ in 0..count.unwrap() {
        let game = take_game(bytes, &mut at);
        if game.is_none() {
            return None;
        }

        games.push(game.unwrap());
    }

    return Some(Command::Games { games });
}

fn read_done(bytes: &[u8]) -> Option<Command> {
    let mut at = 0;

    let ok = take_u8(bytes, &mut at);
    if ok.is_none() {
        return None;
    }

    return Some(Command::Done { ok: ok.unwrap() == 1 });
}