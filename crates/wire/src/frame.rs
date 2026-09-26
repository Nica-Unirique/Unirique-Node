use std::io::{Read, Write};
use std::net::TcpStream;

pub const MESSAGE_MAX: u32 = 64 * 1024;

pub fn read_frame(stream: &mut TcpStream) -> Option<Vec<u8>> {
    let mut head = [0u8; 4];
    if stream.read_exact(&mut head).is_err() {
        return None;
    }

    let length = u32::from_le_bytes(head);
    if length == 0 || length > MESSAGE_MAX {
        return None;
    }

    let mut bytes = vec![0u8; length as usize];
    if stream.read_exact(&mut bytes).is_err() {
        return None;
    }

    return Some(bytes);
}

pub fn write_frame(stream: &mut TcpStream, bytes: &[u8]) -> bool {
    if bytes.len() > MESSAGE_MAX as usize {
        return false;
    }

    if stream.write_all(&(bytes.len() as u32).to_le_bytes()).is_err() {
        return false;
    }

    if stream.write_all(bytes).is_err() {
        return false;
    }

    return true;
}
