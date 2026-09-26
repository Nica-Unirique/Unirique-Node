use catalog::{Criteria, Game, Server, ServerCriteria};
use neighbors::{Address, Neighbor};
use wire::{put_text, put_u16, put_u64, take_text, take_u16, take_u64, take_u8};

use crate::encode::{
    put_criteria, put_games, put_neighbor, put_server, put_server_criteria, put_servers, take_criteria, take_games,
    take_neighbor, take_server, take_server_criteria, take_servers,
};

const PING: u8 = 1;
const PONG: u8 = 2;
const GET_NEIGHBORS: u8 = 3;
const SEND_NEIGHBORS: u8 = 4;
const GET_GAMES: u8 = 5;
const SEND_GAMES: u8 = 6;
const GET_HOLDER: u8 = 7;
const SEND_HOLDERS: u8 = 8;
const ANNOUNCE_SERVER: u8 = 9;
const ANNOUNCE_HOLDER: u8 = 10;
const GET_SERVERS: u8 = 11;
const SEND_SERVERS: u8 = 12;

/// Ce que deux nodes se disent, par la porte reseau.
pub enum Message {
    Ping { port: u16, id: u64, depth: u8 },
    Pong { port: u16, id: u64, depth: u8 },
    GetNeighbors { target: u64 },
    SendNeighbors { neighbors: Vec<Neighbor> },
    GetGames { criteria: Criteria },
    SendGames { games: Vec<Game> },
    GetHolder { infohash: String },
    SendHolders { holders: Vec<Address>, neighbors: Vec<Neighbor> },
    AnnounceHolder { infohash: String, torrent_port: u16 },
    GetServers { criteria: ServerCriteria },
    SendServers { servers: Vec<Server> },
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
            Message::SendHolders { holders, neighbors } => return write_send_holders(holders, neighbors),
            Message::AnnounceHolder { infohash, torrent_port } => return write_announce_holder(infohash, *torrent_port),
            Message::GetServers { criteria } => return write_get_servers(criteria),
            Message::SendServers { servers } => return write_send_servers(servers),
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
            SEND_HOLDERS => return read_send_holders(rest),
            ANNOUNCE_HOLDER => return read_announce_holder(rest),
            GET_SERVERS => return read_get_servers(rest),
            SEND_SERVERS => return read_send_servers(rest),
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

fn write_send_games(games: &Vec<Game>) -> Vec<u8> {
    let mut bytes = vec![SEND_GAMES];
    put_games(&mut bytes, games);

    return bytes;
}

fn write_get_holder(infohash: &String) -> Vec<u8> {
    let mut bytes = vec![GET_HOLDER];
    put_text(&mut bytes, infohash);

    return bytes;
}

fn write_send_holders(holders: &Vec<Address>, neighbors: &Vec<Neighbor>) -> Vec<u8> {
    let mut bytes = vec![SEND_HOLDERS];

    put_u16(&mut bytes, holders.len() as u16);
    for holder in holders.iter() {
        put_text(&mut bytes, &holder.to_text());
    }

    put_u16(&mut bytes, neighbors.len() as u16);
    for neighbor in neighbors.iter() {
        put_neighbor(&mut bytes, neighbor);
    }

    return bytes;
}

fn write_announce_holder(infohash: &String, torrent_port: u16) -> Vec<u8> {
    let mut bytes = vec![ANNOUNCE_HOLDER];
    put_text(&mut bytes, infohash);
    put_u16(&mut bytes, torrent_port);

    return bytes;
}

fn write_get_servers(criteria: &ServerCriteria) -> Vec<u8> {
    let mut bytes = vec![GET_SERVERS];
    put_server_criteria(&mut bytes, criteria);

    return bytes;
}

fn write_send_servers(servers: &Vec<Server>) -> Vec<u8> {
    let mut bytes = vec![SEND_SERVERS];
    put_servers(&mut bytes, servers);

    return bytes;
}

fn write_announce_server(server: &Server) -> Vec<u8> {
    let mut bytes = vec![ANNOUNCE_SERVER];
    put_server(&mut bytes, server);

    return bytes;
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

fn read_send_games(bytes: &[u8]) -> Option<Message> {
    let mut at = 0;

    let games = take_games(bytes, &mut at);
    if games.is_none() {
        return None;
    }

    return Some(Message::SendGames { games: games.unwrap() });
}

fn read_get_holder(bytes: &[u8]) -> Option<Message> {
    let mut at = 0;

    let infohash = take_text(bytes, &mut at);
    if infohash.is_none() {
        return None;
    }

    return Some(Message::GetHolder { infohash: infohash.unwrap() });
}

fn read_send_holders(bytes: &[u8]) -> Option<Message> {
    let mut at = 0;

    let count = take_u16(bytes, &mut at);
    if count.is_none() {
        return None;
    }

    let mut holders = Vec::new();

    for _ in 0..count.unwrap() {
        let text = take_text(bytes, &mut at);
        if text.is_none() {
            return None;
        }

        let holder = Address::from_text(&text.unwrap());
        if holder.is_none() {
            return None;
        }

        holders.push(holder.unwrap());
    }

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

    return Some(Message::SendHolders { holders, neighbors });
}

fn read_announce_holder(bytes: &[u8]) -> Option<Message> {
    let mut at = 0;

    let infohash = take_text(bytes, &mut at);
    let torrent_port = take_u16(bytes, &mut at);

    if infohash.is_none() || torrent_port.is_none() {
        return None;
    }

    return Some(Message::AnnounceHolder { infohash: infohash.unwrap(), torrent_port: torrent_port.unwrap() });
}

fn read_get_servers(bytes: &[u8]) -> Option<Message> {
    let mut at = 0;

    let criteria = take_server_criteria(bytes, &mut at);
    if criteria.is_none() {
        return None;
    }

    return Some(Message::GetServers { criteria: criteria.unwrap() });
}

fn read_send_servers(bytes: &[u8]) -> Option<Message> {
    let mut at = 0;

    let servers = take_servers(bytes, &mut at);
    if servers.is_none() {
        return None;
    }

    return Some(Message::SendServers { servers: servers.unwrap() });
}

fn read_announce_server(bytes: &[u8]) -> Option<Message> {
    let mut at = 0;

    let server = take_server(bytes, &mut at);
    if server.is_none() {
        return None;
    }

    return Some(Message::AnnounceServer { server: server.unwrap() });
}
