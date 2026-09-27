use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use catalog::{content_positions, on_shelf, server_positions, Criteria, Content, Server};
use neighbors::{Address, Neighbor};
use protocol::Message;
use wire::{read_frame, write_frame};

use super::{Node, ANSWER_WITHIN, BUSY_MAX, NEIGHBORS_ASKED};
use crate::busy::Busy;

impl Node {
    pub(super) fn listen(self: &Arc<Self>, listener: TcpListener) {
        let node = self.clone();

        thread::spawn(move || {
            let busy = Arc::new(AtomicUsize::new(0));

            for coming in listener.incoming() {
                while busy.load(Ordering::Relaxed) >= BUSY_MAX {
                    thread::sleep(Duration::from_millis(10));
                }

                if coming.is_err() {
                    continue;
                }

                busy.fetch_add(1, Ordering::Relaxed);

                let serving = node.clone();
                let counted = Busy(busy.clone());
                let stream = coming.unwrap();

                thread::spawn(move || {
                    let _counted = counted;
                    serving.serve(stream);
                });
            }
        });
    }

    fn serve(&self, mut stream: TcpStream) {
        let _ = stream.set_read_timeout(Some(ANSWER_WITHIN));
        let _ = stream.set_write_timeout(Some(ANSWER_WITHIN));

        let from = stream.peer_addr();
        if from.is_err() {
            return;
        }

        let message = read_message(&mut stream);
        if message.is_none() {
            return;
        }

        let answer = self.answer(&message.unwrap(), from.unwrap());
        if answer.is_none() {
            return;
        }

        for piece in answer.unwrap().split() {
            if !write_message(&mut stream, &piece) {
                return;
            }
        }
    }

    fn answer(&self, message: &Message, from: SocketAddr) -> Option<Message> {
        match message {
            Message::Ping { port, id, depth } => return Some(self.answer_ping(*port, *id, *depth, from)),
            Message::GetNeighbors { target } => return Some(self.answer_get_neighbors(*target)),
            Message::GetContents { criteria } => return Some(self.answer_get_contents(criteria)),
            Message::SendContents { contents } => {
                self.receive_contents(contents);
                return None;
            }
            Message::Pong { .. } => return None,
            Message::SendNeighbors { .. } => return None,
            Message::GetHolder { infohash } => return Some(self.answer_get_holder(infohash)),
            Message::SendHolders { .. } => return None,
            Message::GetServers { criteria } => {
                return Some(Message::SendServers { servers: self.servers.lock().unwrap().get_by_criteria(criteria) });
            }
            Message::SendServers { .. } => return None,
            Message::AnnounceHolder { infohash, torrent_port } => {
                self.holders.lock().unwrap().add(infohash, Address::new(from.ip(), *torrent_port));
                return None;
            }
            Message::AnnounceServer { server } => {
                self.receive_server(server);
                return None;
            }
        }
    }

    fn answer_ping(&self, port: u16, id: u64, depth: u8, from: SocketAddr) -> Message {
        let address = Address::new(from.ip(), port);

        let mut neighbors = self.neighbors.lock().unwrap();
        neighbors.add(self.id, Neighbor::new(address, id, depth));
        neighbors.answered(address);
        drop(neighbors);

        return Message::Pong {
            port: self.address.port,
            id: self.id,
            depth: self.depth.load(Ordering::Relaxed),
        };
    }

    fn answer_get_neighbors(&self, target: u64) -> Message {
        let neighbors = self.neighbors.lock().unwrap().closest(target, NEIGHBORS_ASKED);

        return Message::SendNeighbors { neighbors };
    }

    fn answer_get_contents(&self, criteria: &Criteria) -> Message {
        let contents = self.contents.lock().unwrap().get_by_criteria(criteria);

        return Message::SendContents { contents };
    }

    pub(super) fn receive_contents(&self, contents: &Vec<Content>) {
        let depth = self.depth.load(Ordering::Relaxed);
        let mut my_contents = self.contents.lock().unwrap();

        for content in contents.iter() {
            if on_shelf(&content_positions(content), self.id, depth) {
                my_contents.add_received(content.clone());
            }
        }
    }

    pub(super) fn receive_server(&self, server: &Server) {
        let depth = self.depth.load(Ordering::Relaxed);

        if server.is_valid() && on_shelf(&server_positions(server), self.id, depth) {
            self.servers.lock().unwrap().add(server.clone());
        }
    }

    pub(super) fn ask(&self, address: Address, message: Message) -> Option<Message> {
        let stream = TcpStream::connect_timeout(&address.socket(), ANSWER_WITHIN);
        if stream.is_err() {
            return None;
        }

        let mut stream = stream.unwrap();
        let _ = stream.set_read_timeout(Some(ANSWER_WITHIN));
        let _ = stream.set_write_timeout(Some(ANSWER_WITHIN));

        if !write_message(&mut stream, &message) {
            return None;
        }

        // La reponse peut arriver en plusieurs morceaux : on lit jusqu'a ce que
        // l'autre raccroche.
        let mut pieces = Vec::new();

        loop {
            let piece = read_message(&mut stream);
            if piece.is_none() {
                break;
            }

            pieces.push(piece.unwrap());
        }

        return Message::merge(pieces);
    }
}

fn read_message(stream: &mut TcpStream) -> Option<Message> {
    let bytes = read_frame(stream);
    if bytes.is_none() {
        return None;
    }

    return Message::from_bytes(&bytes.unwrap());
}

fn write_message(stream: &mut TcpStream, message: &Message) -> bool {
    return write_frame(stream, &message.to_bytes());
}
