use ed25519_dalek::{Signature, VerifyingKey};

use crate::game::Game;
use crate::message::{put_tags, put_text, put_u32, put_u64};
use crate::server::Server;

pub fn game_bytes(game: &Game) -> Vec<u8> {
    let mut bytes = b"unirique-game".to_vec();

    put_text(&mut bytes, &game.name);
    put_u32(&mut bytes, game.version[0]);
    put_u32(&mut bytes, game.version[1]);
    put_u32(&mut bytes, game.version[2]);
    bytes.extend_from_slice(&game.autor_key);
    put_tags(&mut bytes, &game.tags);
    put_text(&mut bytes, &game.description);
    put_text(&mut bytes, &game.infohash);

    return bytes;
}

pub fn server_bytes(server: &Server) -> Vec<u8> {
    let mut bytes = b"unirique-server".to_vec();

    put_text(&mut bytes, &server.name);
    put_text(&mut bytes, &server.address.to_text());
    bytes.extend_from_slice(&server.host_key);
    put_text(&mut bytes, &server.game_name);
    put_u32(&mut bytes, server.game_version[0]);
    put_u32(&mut bytes, server.game_version[1]);
    put_u32(&mut bytes, server.game_version[2]);
    bytes.extend_from_slice(&server.game_autor_key);
    put_u64(&mut bytes, server.signed_at);

    return bytes;
}

pub fn verify(key: &[u8; 32], bytes: &[u8], signature: &[u8; 64]) -> bool {
    let key = VerifyingKey::from_bytes(key);
    if key.is_err() {
        return false;
    }

    let signature = Signature::from_bytes(signature);

    return key.unwrap().verify_strict(bytes, &signature).is_ok();
}

pub fn signature_from_hex(text: &str) -> Option<[u8; 64]> {
    if text.len() != 128 {
        return None;
    }

    let mut signature = [0u8; 64];

    for place in 0..64 {
        let byte = u8::from_str_radix(&text[place * 2..place * 2 + 2], 16);
        if byte.is_err() {
            return None;
        }

        signature[place] = byte.unwrap();
    }

    return Some(signature);
}

pub fn signature_to_hex(signature: &[u8; 64]) -> String {
    let mut text = String::new();

    for byte in signature.iter() {
        text += &format!("{:02x}", byte);
    }

    return text;
}