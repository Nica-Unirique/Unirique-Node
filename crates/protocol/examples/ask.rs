//! Essai : parle a la porte locale d'un node.
//!     cargo run -p protocol --example ask -- <port local> search <nom>
//!     cargo run -p protocol --example ask -- <port local> download <nom>

use std::net::TcpStream;
use std::time::Duration;

use catalog::Criteria;
use protocol::{Answer, Command};
use wire::{read_frame, write_frame};

fn ask(port: u16, command: Command) -> Option<Answer> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
    write_frame(&mut stream, &command.to_bytes());

    let mut pieces = Vec::new();
    loop {
        let bytes = read_frame(&mut stream);
        if bytes.is_none() {
            break;
        }
        let piece = Answer::from_bytes(&bytes.unwrap());
        if piece.is_none() {
            break;
        }
        pieces.push(piece.unwrap());
    }

    return Answer::merge(pieces);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let port: u16 = args[0].parse().unwrap();
    let criteria = Criteria { name: Some(args[2].clone()), tags: Vec::new(), autor_key: None };

    let found = match ask(port, Command::ContentSearch { criteria }) {
        Some(Answer::Contents { contents }) => contents,
        other => {
            println!("recherche : {:?}", other.is_some());
            return;
        }
    };
    println!("trouve : {}", found.len());
    for content in found.iter() {
        println!("  {} {:?} signe={} auteur={}", content.name, content.version, content.is_signed(), wire::key_to_hex(&content.autor_key));
    }

    if args[1] != "download" || found.is_empty() {
        return;
    }

    let content = found[0].clone();
    let infohash = content.infohash.clone();
    println!("telechargement : {:?}", ask(port, Command::Download { content }).map(|a| matches!(a, Answer::Done)));

    for _ in 0..60 {
        std::thread::sleep(Duration::from_secs(1));
        if let Some(Answer::Progress { done_bytes, total_bytes }) = ask(port, Command::Progress { infohash: infohash.clone() }) {
            println!("progression : {}/{}", done_bytes, total_bytes);
            if total_bytes > 0 && done_bytes == total_bytes {
                return;
            }
        }
    }
}
