use std::path::PathBuf;

use catalog::{Criteria, Content, Server, ServerCriteria};
use neighbors::{Address, Neighbor};
use wire::{
    put_optional_key, put_optional_text, put_tags, put_text, put_u16, put_u32, put_u64, take_key,
    take_optional_key, take_optional_text, take_optional_version, take_signature, take_tags, take_text,
    take_u16, take_u32, take_u64, take_u8, take_version,
};

// ---------- Voisins ----------

pub fn put_neighbor(bytes: &mut Vec<u8>, neighbor: &Neighbor) {
    put_text(bytes, &neighbor.address.to_text());
    put_u64(bytes, neighbor.id);
    bytes.push(neighbor.depth);
}

pub fn take_neighbor(bytes: &[u8], at: &mut usize) -> Option<Neighbor> {
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

// ---------- Jeux ----------

pub fn put_content(bytes: &mut Vec<u8>, content: &Content) {
    put_text(bytes, &content.name);

    put_u32(bytes, content.version[0]);
    put_u32(bytes, content.version[1]);
    put_u32(bytes, content.version[2]);

    bytes.extend_from_slice(&content.autor_key);

    put_tags(bytes, &content.tags);

    put_text(bytes, &content.description);
    put_u64(bytes, content.disk_weight);
    put_text(bytes, &content.infohash);
    put_text(bytes, &content.cover);

    bytes.push(content.downloadable as u8);
    bytes.extend_from_slice(&content.signature);
}

pub fn take_content(bytes: &[u8], at: &mut usize) -> Option<Content> {
    let name = take_text(bytes, at);
    let version = take_version(bytes, at);
    let autor_key = take_key(bytes, at);
    let tags = take_tags(bytes, at);
    let description = take_text(bytes, at);
    let disk_weight = take_u64(bytes, at);
    let infohash = take_text(bytes, at);
    let cover = take_text(bytes, at);
    let downloadable = take_u8(bytes, at);
    let signature = take_signature(bytes, at);

    if name.is_none() || version.is_none() || autor_key.is_none() || tags.is_none() {
        return None;
    }

    if description.is_none() || disk_weight.is_none() || downloadable.is_none() || infohash.is_none() || cover.is_none() || signature.is_none() {
        return None;
    }

    let mut content = Content {
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
        cover: cover.unwrap(),
        share: None,
        folder: PathBuf::new(),
        sent: 0,
        session_sent: 0,
        signature: signature.unwrap(),
    };

    content.compute_ram_weight();

    return Some(content);
}

pub fn put_contents(bytes: &mut Vec<u8>, contents: &Vec<Content>) {
    put_u16(bytes, contents.len() as u16);

    for content in contents.iter() {
        put_content(bytes, content);
    }
}

pub fn take_contents(bytes: &[u8], at: &mut usize) -> Option<Vec<Content>> {
    let count = take_u16(bytes, at);
    if count.is_none() {
        return None;
    }

    let mut contents = Vec::new();

    for _ in 0..count.unwrap() {
        let content = take_content(bytes, at);
        if content.is_none() {
            return None;
        }

        contents.push(content.unwrap());
    }

    return Some(contents);
}

pub fn put_criteria(bytes: &mut Vec<u8>, criteria: &Criteria) {
    put_optional_text(bytes, &criteria.name);
    put_tags(bytes, &criteria.tags);
    put_optional_key(bytes, &criteria.autor_key);
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

// ---------- Serveurs ----------

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

pub fn put_servers(bytes: &mut Vec<u8>, servers: &Vec<Server>) {
    put_u16(bytes, servers.len() as u16);

    for server in servers.iter() {
        put_server(bytes, server);
    }
}

pub fn take_servers(bytes: &[u8], at: &mut usize) -> Option<Vec<Server>> {
    let count = take_u16(bytes, at);
    if count.is_none() {
        return None;
    }

    let mut servers = Vec::new();

    for _ in 0..count.unwrap() {
        let server = take_server(bytes, at);
        if server.is_none() {
            return None;
        }

        servers.push(server.unwrap());
    }

    return Some(servers);
}

pub fn put_server_criteria(bytes: &mut Vec<u8>, criteria: &ServerCriteria) {
    put_optional_text(bytes, &criteria.name);
    put_optional_text(bytes, &criteria.game_name);

    if criteria.game_version.is_some() {
        let version = criteria.game_version.unwrap();
        bytes.push(1);
        put_u32(bytes, version[0]);
        put_u32(bytes, version[1]);
        put_u32(bytes, version[2]);
    } else {
        bytes.push(0);
    }

    put_optional_key(bytes, &criteria.game_autor_key);
    put_optional_key(bytes, &criteria.host_key);
}

pub fn take_server_criteria(bytes: &[u8], at: &mut usize) -> Option<ServerCriteria> {
    let name = take_optional_text(bytes, at);
    let game_name = take_optional_text(bytes, at);
    let game_version = take_optional_version(bytes, at);
    let game_autor_key = take_optional_key(bytes, at);
    let host_key = take_optional_key(bytes, at);

    if name.is_none() || game_name.is_none() || game_version.is_none() {
        return None;
    }

    if game_autor_key.is_none() || host_key.is_none() {
        return None;
    }

    return Some(ServerCriteria {
        name: name.unwrap(),
        game_name: game_name.unwrap(),
        game_version: game_version.unwrap(),
        game_autor_key: game_autor_key.unwrap(),
        host_key: host_key.unwrap(),
    });
}

// ---------- Quota de partage ----------

/// `None` veut dire « a l'infini ».
pub fn put_share(bytes: &mut Vec<u8>, share: Option<u32>) {
    if share.is_none() {
        bytes.push(0);
        return;
    }

    bytes.push(1);
    put_u32(bytes, share.unwrap());
}

pub fn take_share(bytes: &[u8], at: &mut usize) -> Option<Option<u32>> {
    let present = take_u8(bytes, at);
    if present.is_none() {
        return None;
    }

    if present.unwrap() == 0 {
        return Some(None);
    }

    let share = take_u32(bytes, at);
    if share.is_none() {
        return None;
    }

    return Some(Some(share.unwrap()));
}
