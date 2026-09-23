use std::path::PathBuf;

use crate::address::Address;
use crate::game::Game;
use crate::neighbor::Neighbor;
use crate::criteria::Criteria;
use crate::server::Server;

const PING: u8 = 1;
const PONG: u8 = 2;
const GET_NEIGHBORS: u8 = 3;
const SEND_NEIGHBORS: u8 = 4;
const GET_GAMES: u8 = 5;
const SEND_GAMES: u8 = 6;
const GET_HOLDER: u8 = 7;
const HOLDING: u8 = 8;
const ANNOUNCE_SERVER: u8 = 9;

pub enum Message {
    Ping { port: u16, id: u64, depth: u8 },
    Pong { port: u16, id: u64, depth: u8 },
    GetNeighbors { target: u64 },
    SendNeighbors { neighbors: Vec<Neighbor> },
    GetGames { criteria: Criteria },
    SendGames { games: Vec<Game> },
    GetHolder { infohash: String },
    Holding { torrent_port: u16 },
    AnnounceServer { server: Server },
}

impl Message {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Message::Ping { port, id, depth } => return write_hello(PING, *port, *id, *depth),
            Message::Pong { port, id, depth } => return write_hello(PONG, *port, *id, *depth),
            Message::GetNeighbors { target } => return write_get_neighbors(*target),
            Message::SendNeighbors { neighbors } => return write_send_neighbors(neighbors),
            Message::GetGames { criteria } => return write_get_games(criteria),
            Message::SendGames { games } => return write_send_games(games),
            Message::GetHolder { infohash } => return write_get_holder(infohash),
            Message::Holding { torrent_port } => return write_holding(*torrent_port),
            Message::AnnounceServer { server } => return write_announce_server(server),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Message> {
        if bytes.is_empty() {
            return None;
        }

        let rest = &bytes[1..];

        match bytes[0] {
            PING => return read_ping(rest),
            PONG => return read_pong(rest),
            GET_NEIGHBORS => return read_get_neighbors(rest),
            SEND_NEIGHBORS => return read_send_neighbors(rest),
            GET_GAMES => return read_get_games(rest),
            SEND_GAMES => return read_send_games(rest),
            GET_HOLDER => return read_get_holder(rest),
            HOLDING => return read_holding(rest),
            ANNOUNCE_SERVER => return read_announce_server(rest),
            _ => return None,
        }
    }
}

// ---------- Ecriture ----------

fn write_hello(code: u8, port: u16, id: u64, depth: u8) -> Vec<u8> {
    let mut bytes = vec![code];
    put_u16(&mut bytes, port);
    put_u64(&mut bytes, id);
    bytes.push(depth);

    return bytes;
}

fn write_get_neighbors(target: u64) -> Vec<u8> {
    let mut bytes = vec![GET_NEIGHBORS];
    put_u64(&mut bytes, target);

    return bytes;
}

fn write_send_neighbors(neighbors: &Vec<Neighbor>) -> Vec<u8> {
    let mut bytes = vec![SEND_NEIGHBORS];
    put_u16(&mut bytes, neighbors.len() as u16);

    for neighbor in neighbors.iter() {
        put_neighbor(&mut bytes, neighbor);
    }

    return bytes;
}

fn write_get_games(criteria: &Criteria) -> Vec<u8> {
    let mut bytes = vec![GET_GAMES];
    put_criteria(&mut bytes, criteria);

    return bytes;
}

pub fn put_criteria(bytes: &mut Vec<u8>, criteria: &Criteria) {
    if criteria.name.is_some() {
        bytes.push(1);
        put_text(bytes, criteria.name.as_ref().unwrap());
    } else {
        bytes.push(0);
    }

    put_tags(bytes, &criteria.tags);

    if criteria.autor_key.is_some() {
        bytes.push(1);
        bytes.extend_from_slice(&criteria.autor_key.unwrap());
    } else {
        bytes.push(0);
    }
}

pub fn put_tags(bytes: &mut Vec<u8>, tags: &Vec<u64>) {
    put_u16(bytes, tags.len() as u16);

    for tag in tags.iter() {
        put_u64(bytes, *tag);
    }
}

fn write_send_games(games: &Vec<Game>) -> Vec<u8> {
    let mut bytes = vec![SEND_GAMES];
    put_u16(&mut bytes, games.len() as u16);

    for game in games.iter() {
        put_game(&mut bytes, game);
    }

    return bytes;
}

fn put_neighbor(bytes: &mut Vec<u8>, neighbor: &Neighbor) {
    put_text(bytes, &neighbor.address.to_text());
    put_u64(bytes, neighbor.id);
    bytes.push(neighbor.depth);
}

pub fn put_game(bytes: &mut Vec<u8>, game: &Game) {
    put_text(bytes, &game.name);

    put_u32(bytes, game.version[0]);
    put_u32(bytes, game.version[1]);
    put_u32(bytes, game.version[2]);

    bytes.extend_from_slice(&game.autor_key);

    put_tags(bytes, &game.tags);

    put_text(bytes, &game.description);
    put_u64(bytes, game.disk_weight);
    put_text(bytes, &game.infohash);

    bytes.push(game.downloadable as u8);
    bytes.extend_from_slice(&game.signature);
}

pub fn put_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub fn put_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub fn put_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub fn put_text(bytes: &mut Vec<u8>, text: &str) {
    put_u16(bytes, text.len() as u16);
    bytes.extend_from_slice(text.as_bytes());
}

// ---------- Lecture ----------

fn read_ping(bytes: &[u8]) -> Option<Message> {
    let hello = read_hello(bytes);
    if hello.is_none() {
        return None;
    }

    let (port, id, depth) = hello.unwrap();

    return Some(Message::Ping { port, id, depth });
}

fn read_pong(bytes: &[u8]) -> Option<Message> {
    let hello = read_hello(bytes);
    if hello.is_none() {
        return None;
    }

    let (port, id, depth) = hello.unwrap();

    return Some(Message::Pong { port, id, depth });
}

fn read_hello(bytes: &[u8]) -> Option<(u16, u64, u8)> {
    let mut at = 0;

    let port = take_u16(bytes, &mut at);
    let id = take_u64(bytes, &mut at);
    let depth = take_u8(bytes, &mut at);

    if port.is_none() || id.is_none() || depth.is_none() {
        return None;
    }

    return Some((port.unwrap(), id.unwrap(), depth.unwrap()));
}

fn read_get_neighbors(bytes: &[u8]) -> Option<Message> {
    let mut at = 0;

    let target = take_u64(bytes, &mut at);
    if target.is_none() {
        return None;
    }

    return Some(Message::GetNeighbors { target: target.unwrap() });
}

fn read_send_neighbors(bytes: &[u8]) -> Option<Message> {
    let mut at = 0;

    let count = take_u16(bytes, &mut at);
    if count.is_none() {
        return None;
    }

    let mut neighbors = Vec::new();

    for _ in 0..count.unwrap() {
        let neighbor = take_neighbor(bytes, &mut at);
        if neighbor.is_none() {
            return None;
        }

        neighbors.push(neighbor.unwrap());
    }

    return Some(Message::SendNeighbors { neighbors });
}

fn read_get_games(bytes: &[u8]) -> Option<Message> {
    let mut at = 0;

    let criteria = take_criteria(bytes, &mut at);
    if criteria.is_none() {
        return None;
    }

    return Some(Message::GetGames { criteria: criteria.unwrap() });
}

pub fn take_criteria(bytes: &[u8], at: &mut usize) -> Option<Criteria> {
    let name = take_optional_text(bytes, at);
    let tags = take_tags(bytes, at);
    let autor_key = take_optional_key(bytes, at);

    if name.is_none() || tags.is_none() || autor_key.is_none() {
        return None;
    }

    return Some(Criteria {
        name: name.unwrap(),
        tags: tags.unwrap(),
        autor_key: autor_key.unwrap(),
    });
}

fn take_optional_text(bytes: &[u8], at: &mut usize) -> Option<Option<String>> {
    let present = take_u8(bytes, at);
    if present.is_none() {
        return None;
    }

    if present.unwrap() == 0 {
        return Some(None);
    }

    let text = take_text(bytes, at);
    if text.is_none() {
        return None;
    }

    return Some(Some(text.unwrap()));
}

fn take_optional_key(bytes: &[u8], at: &mut usize) -> Option<Option<[u8; 32]>> {
    let present = take_u8(bytes, at);
    if present.is_none() {
        return None;
    }

    if present.unwrap() == 0 {
        return Some(None);
    }

    let key = take_key(bytes, at);
    if key.is_none() {
        return None;
    }

    return Some(Some(key.unwrap()));
}

fn read_send_games(bytes: &[u8]) -> Option<Message> {
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

    return Some(Message::SendGames { games });
}

fn take_neighbor(bytes: &[u8], at: &mut usize) -> Option<Neighbor> {
    let address = take_text(bytes, at);
    let id = take_u64(bytes, at);
    let depth = take_u8(bytes, at);

    if address.is_none() || id.is_none() || depth.is_none() {
        return None;
    }

    let address = Address::from_text(&address.unwrap());
    if address.is_none() {
        return None;
    }

    return Some(Neighbor::new(address.unwrap(), id.unwrap(), depth.unwrap()));
}

pub fn take_game(bytes: &[u8], at: &mut usize) -> Option<Game> {
    let name = take_text(bytes, at);
    let version = take_version(bytes, at);
    let autor_key = take_key(bytes, at);
    let tags = take_tags(bytes, at);
    let description = take_text(bytes, at);
    let disk_weight = take_u64(bytes, at);
    let infohash = take_text(bytes, at);
    let downloadable = take_u8(bytes, at);
    let signature = take_signature(bytes, at);

    if name.is_none() || version.is_none() || autor_key.is_none() || tags.is_none() {
        return None;
    }

    if description.is_none() || disk_weight.is_none() || downloadable.is_none() || infohash.is_none() || signature.is_none() {
        return None;
    }

    let mut game = Game {
        name: name.unwrap(),
        version: version.unwrap(),
        autor_key: autor_key.unwrap(),
        tags: tags.unwrap(),
        description: description.unwrap(),
        ram_weight: 0,
        downloaded: false,
        downloadable: downloadable.unwrap() == 1,
        disk_weight: disk_weight.unwrap(),
        infohash: infohash.unwrap(),
        share: None,
        folder: PathBuf::new(),
        sent: 0,
        session_sent: 0,
        signature: signature.unwrap(),
    };

    game.compute_ram_weight();

    return Some(game);
}

fn take_version(bytes: &[u8], at: &mut usize) -> Option<[u32; 3]> {
    let major = take_u32(bytes, at);
    let minor = take_u32(bytes, at);
    let patch = take_u32(bytes, at);

    if major.is_none() || minor.is_none() || patch.is_none() {
        return None;
    }

    return Some([major.unwrap(), minor.unwrap(), patch.unwrap()]);
}

fn take_tags(bytes: &[u8], at: &mut usize) -> Option<Vec<u64>> {
    let count = take_u16(bytes, at);
    if count.is_none() {
        return None;
    }

    let mut tags = Vec::new();

    for _ in 0..count.unwrap() {
        let tag = take_u64(bytes, at);
        if tag.is_none() {
            return None;
        }

        tags.push(tag.unwrap());
    }

    return Some(tags);
}

pub fn take_u8(bytes: &[u8], at: &mut usize) -> Option<u8> {
    if *at + 1 > bytes.len() {
        return None;
    }

    let value = bytes[*at];
    *at += 1;

    return Some(value);
}

pub fn take_u16(bytes: &[u8], at: &mut usize) -> Option<u16> {
    if *at + 2 > bytes.len() {
        return None;
    }

    let mut value = [0u8; 2];
    value.copy_from_slice(&bytes[*at..*at + 2]);
    *at += 2;

    return Some(u16::from_le_bytes(value));
}

pub fn take_u32(bytes: &[u8], at: &mut usize) -> Option<u32> {
    if *at + 4 > bytes.len() {
        return None;
    }

    let mut value = [0u8; 4];
    value.copy_from_slice(&bytes[*at..*at + 4]);
    *at += 4;

    return Some(u32::from_le_bytes(value));
}

pub fn take_u64(bytes: &[u8], at: &mut usize) -> Option<u64> {
    if *at + 8 > bytes.len() {
        return None;
    }

    let mut value = [0u8; 8];
    value.copy_from_slice(&bytes[*at..*at + 8]);
    *at += 8;

    return Some(u64::from_le_bytes(value));
}

fn take_key(bytes: &[u8], at: &mut usize) -> Option<[u8; 32]> {
    if *at + 32 > bytes.len() {
        return None;
    }

    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes[*at..*at + 32]);
    *at += 32;

    return Some(key);
}

pub fn take_text(bytes: &[u8], at: &mut usize) -> Option<String> {
    let length = take_u16(bytes, at);
    if length.is_none() {
        return None;
    }

    let length = length.unwrap() as usize;
    if *at + length > bytes.len() {
        return None;
    }

    let text = String::from_utf8(bytes[*at..*at + length].to_vec());
    if text.is_err() {
        return None;
    }

    *at += length;

    return Some(text.unwrap());
}

fn write_get_holder(infohash: &String) -> Vec<u8> {
    let mut bytes = vec![GET_HOLDER];
    put_text(&mut bytes, infohash);

    return bytes;
}

fn write_holding(torrent_port: u16) -> Vec<u8> {
    let mut bytes = vec![HOLDING];
    put_u16(&mut bytes, torrent_port);

    return bytes;
}

fn read_get_holder(bytes: &[u8]) -> Option<Message> {
    let mut at = 0;

    let infohash = take_text(bytes, &mut at);
    if infohash.is_none() {
        return None;
    }

    return Some(Message::GetHolder { infohash: infohash.unwrap() });
}

fn read_holding(bytes: &[u8]) -> Option<Message> {
    let mut at = 0;

    let torrent_port = take_u16(bytes, &mut at);
    if torrent_port.is_none() {
        return None;
    }

    return Some(Message::Holding { torrent_port: torrent_port.unwrap() });
}

fn write_announce_server(server: &Server) -> Vec<u8> {
    let mut bytes = vec![ANNOUNCE_SERVER];
    put_server(&mut bytes, server);

    return bytes;
}

fn read_announce_server(bytes: &[u8]) -> Option<Message> {
    let mut at = 0;

    let server = take_server(bytes, &mut at);
    if server.is_none() {
        return None;
    }

    return Some(Message::AnnounceServer { server: server.unwrap() });
}

pub fn put_server(bytes: &mut Vec<u8>, server: &Server) {
    put_text(bytes, &server.name);
    put_text(bytes, &server.address.to_text());
    bytes.extend_from_slice(&server.host_key);
    put_text(bytes, &server.game_name);
    put_u32(bytes, server.game_version[0]);
    put_u32(bytes, server.game_version[1]);
    put_u32(bytes, server.game_version[2]);
    bytes.extend_from_slice(&server.game_autor_key);
    put_u64(bytes, server.signed_at);
    bytes.extend_from_slice(&server.signature);
}

pub fn take_server(bytes: &[u8], at: &mut usize) -> Option<Server> {
    let name = take_text(bytes, at);
    let address = take_text(bytes, at);
    let host_key = take_key(bytes, at);
    let game_name = take_text(bytes, at);
    let game_version = take_version(bytes, at);
    let game_autor_key = take_key(bytes, at);
    let signed_at = take_u64(bytes, at);
    let signature = take_signature(bytes, at);

    if name.is_none() || address.is_none() || host_key.is_none() || game_name.is_none() {
        return None;
    }

    if game_version.is_none() || game_autor_key.is_none() || signed_at.is_none() || signature.is_none() {
        return None;
    }

    let address = Address::from_text(&address.unwrap());
    if address.is_none() {
        return None;
    }

    return Some(Server::new(
        name.unwrap(),
        address.unwrap(),
        host_key.unwrap(),
        game_name.unwrap(),
        game_version.unwrap(),
        game_autor_key.unwrap(),
        signed_at.unwrap(),
        signature.unwrap(),
    ));
}

pub fn take_signature(bytes: &[u8], at: &mut usize) -> Option<[u8; 64]> {
    if *at + 64 > bytes.len() {
        return None;
    }

    let mut signature = [0u8; 64];
    signature.copy_from_slice(&bytes[*at..*at + 64]);
    *at += 64;

    return Some(signature);
}