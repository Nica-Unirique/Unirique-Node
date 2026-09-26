use catalog::{Criteria, Game, Server, ServerCriteria};
use wire::{put_text, take_signature, take_text, take_u8};

use crate::encode::{
    put_criteria, put_game, put_games, put_server, put_server_criteria, put_servers, take_criteria, take_game,
    take_games, take_server, take_server_criteria, take_servers,
};

const SEARCH: u8 = 1;
const DOWNLOAD: u8 = 2;
const INSTALLED: u8 = 3;
const SET_SHARE: u8 = 4;
const GAMES: u8 = 5;
const DONE: u8 = 6;
const CHALLENGE: u8 = 7;
const TO_SIGN: u8 = 8;
const SIGNED: u8 = 9;
const SEARCH_SERVERS: u8 = 10;
const SERVERS: u8 = 11;

pub enum Subject {
    Game { infohash: String },
    Server { server: Server },
}

/// Ce que le client (ou le serveur) du joueur dit a son node, par la porte locale.
pub enum Command {
    Search { criteria: Criteria },
    Download { game: Game },
    Installed,
    SetShare { infohash: String, share: Option<u32> },
    Games { games: Vec<Game> },
    Done { ok: bool },
    Challenge { what: Subject },
    ToSign { bytes: Vec<u8> },
    Signed { what: Subject, signature: [u8; 64] },
    SearchServers { criteria: ServerCriteria },
    Servers { servers: Vec<Server> },
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
            Command::Challenge { what } => return write_challenge(what),
            Command::ToSign { bytes } => return write_to_sign(bytes),
            Command::Signed { what, signature } => return write_signed(what, signature),
            Command::SearchServers { criteria } => return write_search_servers(criteria),
            Command::Servers { servers } => return write_servers(servers),
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
            CHALLENGE => return read_challenge(rest),
            TO_SIGN => return read_to_sign(rest),
            SIGNED => return read_signed(rest),
            SEARCH_SERVERS => return read_search_servers(rest),
            SERVERS => return read_servers(rest),
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
    put_games(&mut bytes, games);

    return bytes;
}

fn write_challenge(what: &Subject) -> Vec<u8> {
    let mut bytes = vec![CHALLENGE];
    put_subject(&mut bytes, what);

    return bytes;
}

fn write_to_sign(to_sign: &Vec<u8>) -> Vec<u8> {
    let mut bytes = vec![TO_SIGN];
    bytes.extend_from_slice(&(to_sign.len() as u32).to_le_bytes());
    bytes.extend_from_slice(to_sign);

    return bytes;
}

fn write_signed(what: &Subject, signature: &[u8; 64]) -> Vec<u8> {
    let mut bytes = vec![SIGNED];
    put_subject(&mut bytes, what);
    bytes.extend_from_slice(signature);

    return bytes;
}

fn write_search_servers(criteria: &ServerCriteria) -> Vec<u8> {
    let mut bytes = vec![SEARCH_SERVERS];
    put_server_criteria(&mut bytes, criteria);

    return bytes;
}

fn write_servers(servers: &Vec<Server>) -> Vec<u8> {
    let mut bytes = vec![SERVERS];
    put_servers(&mut bytes, servers);

    return bytes;
}

fn put_subject(bytes: &mut Vec<u8>, what: &Subject) {
    match what {
        Subject::Game { infohash } => {
            bytes.push(1);
            put_text(bytes, infohash);
        }
        Subject::Server { server } => {
            bytes.push(2);
            put_server(bytes, server);
        }
    }
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

    let games = take_games(bytes, &mut at);
    if games.is_none() {
        return None;
    }

    return Some(Command::Games { games: games.unwrap() });
}

fn read_done(bytes: &[u8]) -> Option<Command> {
    let mut at = 0;

    let ok = take_u8(bytes, &mut at);
    if ok.is_none() {
        return None;
    }

    return Some(Command::Done { ok: ok.unwrap() == 1 });
}

fn read_challenge(bytes: &[u8]) -> Option<Command> {
    let mut at = 0;

    let what = take_subject(bytes, &mut at);
    if what.is_none() {
        return None;
    }

    return Some(Command::Challenge { what: what.unwrap() });
}

fn read_to_sign(bytes: &[u8]) -> Option<Command> {
    if bytes.len() < 4 {
        return None;
    }

    let mut length = [0u8; 4];
    length.copy_from_slice(&bytes[0..4]);
    let length = u32::from_le_bytes(length) as usize;

    if 4 + length > bytes.len() {
        return None;
    }

    return Some(Command::ToSign { bytes: bytes[4..4 + length].to_vec() });
}

fn read_signed(bytes: &[u8]) -> Option<Command> {
    let mut at = 0;

    let what = take_subject(bytes, &mut at);
    let signature = take_signature(bytes, &mut at);

    if what.is_none() || signature.is_none() {
        return None;
    }

    return Some(Command::Signed { what: what.unwrap(), signature: signature.unwrap() });
}

fn read_search_servers(bytes: &[u8]) -> Option<Command> {
    let mut at = 0;

    let criteria = take_server_criteria(bytes, &mut at);
    if criteria.is_none() {
        return None;
    }

    return Some(Command::SearchServers { criteria: criteria.unwrap() });
}

fn read_servers(bytes: &[u8]) -> Option<Command> {
    let mut at = 0;

    let servers = take_servers(bytes, &mut at);
    if servers.is_none() {
        return None;
    }

    return Some(Command::Servers { servers: servers.unwrap() });
}

fn take_subject(bytes: &[u8], at: &mut usize) -> Option<Subject> {
    let kind = take_u8(bytes, at);
    if kind.is_none() {
        return None;
    }

    if kind.unwrap() == 1 {
        let infohash = take_text(bytes, at);
        if infohash.is_none() {
            return None;
        }

        return Some(Subject::Game { infohash: infohash.unwrap() });
    }

    if kind.unwrap() == 2 {
        let server = take_server(bytes, at);
        if server.is_none() {
            return None;
        }

        return Some(Subject::Server { server: server.unwrap() });
    }

    return None;
}
