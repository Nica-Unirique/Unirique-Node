//! L'en-tete PROXY qu'un tunnel (playit, HAProxy...) envoie au debut de
//! chaque connexion : il donne la vraie adresse de celui qui se connecte.
//!
//! Version 1 : une ligne de texte, `PROXY TCP4 <source> <dest> <port source> <port dest>\r\n`.
//! Version 2 : 12 octets de signature, puis les adresses en binaire.

use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};

const V2_SIGNATURE: [u8; 12] = [0x0D, 0x0A, 0x0D, 0x0A, 0x00, 0x0D, 0x0A, 0x51, 0x55, 0x49, 0x54, 0x0A];
const V1_MAX: usize = 107;
const V2_ADDRESSES_MAX: usize = 512;

/// La vraie adresse de celui qui se connecte. `peer` (le tunnel) quand
/// l'en-tete ne la donne pas ; rien si l'en-tete manque ou est illisible :
/// la connexion est alors refusee.
pub(super) fn read_proxy_header(stream: &mut TcpStream, peer: SocketAddr) -> Option<SocketAddr> {
    let mut start = [0u8; 12];
    if stream.read_exact(&mut start).is_err() {
        return None;
    }

    if start == V2_SIGNATURE {
        return read_v2(stream, peer);
    }

    if &start[0..6] == b"PROXY " {
        return read_v1(stream, &start, peer);
    }

    return None;
}

fn read_v1(stream: &mut TcpStream, start: &[u8], peer: SocketAddr) -> Option<SocketAddr> {
    let mut line = start.to_vec();

    while !line.ends_with(b"\r\n") {
        if line.len() >= V1_MAX {
            return None;
        }

        let mut byte = [0u8; 1];
        if stream.read_exact(&mut byte).is_err() {
            return None;
        }
        line.push(byte[0]);
    }

    let text = String::from_utf8(line);
    if text.is_err() {
        return None;
    }

    return parse_v1(text.unwrap().trim_end(), peer);
}

fn parse_v1(line: &str, peer: SocketAddr) -> Option<SocketAddr> {
    let parts: Vec<&str> = line.split(' ').collect();

    if parts.len() >= 2 && parts[1] == "UNKNOWN" {
        return Some(peer);
    }

    if parts.len() != 6 || (parts[1] != "TCP4" && parts[1] != "TCP6") {
        return None;
    }

    let ip = parts[2].parse::<IpAddr>();
    let port = parts[4].parse::<u16>();
    if ip.is_err() || port.is_err() {
        return None;
    }

    return Some(SocketAddr::new(ip.unwrap(), port.unwrap()));
}

fn read_v2(stream: &mut TcpStream, peer: SocketAddr) -> Option<SocketAddr> {
    let mut head = [0u8; 4];
    if stream.read_exact(&mut head).is_err() {
        return None;
    }

    let version_command = head[0];
    let family = head[1];
    let length = u16::from_be_bytes([head[2], head[3]]) as usize;

    if version_command >> 4 != 2 || length > V2_ADDRESSES_MAX {
        return None;
    }

    let mut addresses = vec![0u8; length];
    if stream.read_exact(&mut addresses).is_err() {
        return None;
    }

    return parse_v2(version_command & 0x0F, family, &addresses, peer);
}

fn parse_v2(command: u8, family: u8, addresses: &[u8], peer: SocketAddr) -> Option<SocketAddr> {
    // LOCAL : le tunnel parle pour lui-meme (verification de sante).
    if command == 0 {
        return Some(peer);
    }

    if command != 1 {
        return None;
    }

    // TCP sur IPv4 : source (4), destination (4), port source (2), port destination (2).
    if family == 0x11 && addresses.len() >= 12 {
        let ip = Ipv4Addr::new(addresses[0], addresses[1], addresses[2], addresses[3]);
        let port = u16::from_be_bytes([addresses[8], addresses[9]]);
        return Some(SocketAddr::new(IpAddr::V4(ip), port));
    }

    // TCP sur IPv6 : source (16), destination (16), port source (2), port destination (2).
    if family == 0x21 && addresses.len() >= 36 {
        let mut octets = [0u8; 16];
        octets.copy_from_slice(&addresses[0..16]);
        let port = u16::from_be_bytes([addresses[32], addresses[33]]);
        return Some(SocketAddr::new(IpAddr::V6(Ipv6Addr::from(octets)), port));
    }

    // Autre famille (UNSPEC, UDP, unix) : l'adresse n'est pas une IP utile.
    return Some(peer);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tunnel() -> SocketAddr {
        return "127.0.0.1:5000".parse().unwrap();
    }

    #[test]
    fn a_v1_line_gives_the_real_address() {
        let found = parse_v1("PROXY TCP4 82.14.3.7 192.168.1.10 51234 8735", tunnel());
        assert_eq!(found, Some("82.14.3.7:51234".parse().unwrap()));
    }

    #[test]
    fn a_v1_unknown_line_keeps_the_tunnel() {
        assert_eq!(parse_v1("PROXY UNKNOWN", tunnel()), Some(tunnel()));
    }

    #[test]
    fn a_broken_v1_line_is_refused() {
        assert_eq!(parse_v1("PROXY TCP4 82.14.3.7", tunnel()), None);
        assert_eq!(parse_v1("PROXY TCP4 pas.une.ip 1.2.3.4 1 2", tunnel()), None);
    }

    #[test]
    fn a_v2_ipv4_header_gives_the_real_address() {
        let addresses = [82, 14, 3, 7, 10, 0, 0, 1, 0xC8, 0x22, 0x22, 0x1F];
        let found = parse_v2(1, 0x11, &addresses, tunnel());
        assert_eq!(found, Some("82.14.3.7:51234".parse().unwrap()));
    }

    #[test]
    fn a_v2_local_header_keeps_the_tunnel() {
        assert_eq!(parse_v2(0, 0x00, &[], tunnel()), Some(tunnel()));
    }

    #[test]
    fn a_short_v2_ipv4_header_keeps_the_tunnel() {
        assert_eq!(parse_v2(1, 0x11, &[1, 2, 3], tunnel()), Some(tunnel()));
    }
}
