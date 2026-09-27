use ed25519_dalek::{Signature, VerifyingKey};

use wire::{put_tags, put_text, put_u32, put_u64};

use crate::content::Content;
use crate::server::Server;

pub fn content_bytes(content: &Content) -> Vec<u8> {
    let mut bytes = b"unirique-content".to_vec();

    put_text(&mut bytes, &content.name);
    put_u32(&mut bytes, content.version[0]);
    put_u32(&mut bytes, content.version[1]);
    put_u32(&mut bytes, content.version[2]);
    bytes.extend_from_slice(&content.autor_key);
    put_tags(&mut bytes, &content.tags);
    put_text(&mut bytes, &content.description);
    put_text(&mut bytes, &content.infohash);
    put_text(&mut bytes, &content.cover);

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
