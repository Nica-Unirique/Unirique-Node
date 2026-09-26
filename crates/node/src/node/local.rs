use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

use catalog::{game_bytes, server_bytes, Criteria, Game, Server, ServerCriteria};
use protocol::{Command, Subject};
use wire::{read_frame, write_frame};

use super::Node;

impl Node {
    pub(super) fn listen_local(self: &Arc<Self>, listener: TcpListener) {
        let node = self.clone();

        thread::spawn(move || {
            for coming in listener.incoming() {
                if coming.is_err() {
                    continue;
                }

                let serving = node.clone();
                let stream = coming.unwrap();

                thread::spawn(move || serving.serve_local(stream));
            }
        });
    }

    fn serve_local(&self, mut stream: TcpStream) {
        let bytes = read_frame(&mut stream);
        if bytes.is_none() {
            return;
        }

        let command = Command::from_bytes(&bytes.unwrap());
        if command.is_none() {
            return;
        }

        let answer = self.obey(command.unwrap());
        if answer.is_none() {
            return;
        }

        for piece in answer.unwrap().split() {
            if !write_frame(&mut stream, &piece.to_bytes()) {
                return;
            }
        }
    }

    fn obey(&self, command: Command) -> Option<Command> {
        match command {
            Command::Search { criteria } => return Some(Command::Games { games: self.search(&criteria) }),
            Command::Download { game } => return Some(Command::Done { ok: self.download(&game) }),
            Command::Installed => return Some(Command::Games { games: self.games.lock().unwrap().get_installed() }),
            Command::SetShare { infohash, share } => return Some(Command::Done { ok: self.set_share(&infohash, share) }),
            Command::Challenge { what } => return Some(self.challenge(what)),
            Command::Signed { what, signature } => return Some(Command::Done { ok: self.signed(what, signature) }),
            Command::SearchServers { criteria } => return Some(Command::Servers { servers: self.search_servers(&criteria) }),
            Command::Games { .. } => return None,
            Command::Done { .. } => return None,
            Command::ToSign { .. } => return None,
            Command::Servers { .. } => return None,
        }
    }

    fn challenge(&self, what: Subject) -> Command {
        match what {
            Subject::Game { infohash } => {
                let game = self.games.lock().unwrap().find(&infohash);
                if game.is_none() {
                    return Command::Done { ok: false };
                }

                return Command::ToSign { bytes: game_bytes(&game.unwrap()) };
            }
            Subject::Server { server } => return Command::ToSign { bytes: server_bytes(&server) },
        }
    }

    fn signed(&self, what: Subject, signature: [u8; 64]) -> bool {
        match what {
            Subject::Game { infohash } => return self.sign_game(&infohash, signature),
            Subject::Server { mut server } => {
                server.signature = signature;
                return self.declare_server(server);
            }
        }
    }

    fn sign_game(&self, infohash: &str, signature: [u8; 64]) -> bool {
        let game = self.games.lock().unwrap().set_signature(infohash, signature);
        if game.is_none() {
            return false;
        }

        let game = game.unwrap();
        self.seed(&game);
        self.announce_holding(&game);
        self.store_game(&game);

        return true;
    }

    fn declare_server(&self, server: Server) -> bool {
        if !server.is_valid() {
            return false;
        }

        self.servers.lock().unwrap().add(server.clone());
        self.store_server(&server);

        return true;
    }

    fn search(&self, criteria: &Criteria) -> Vec<Game> {
        return self.search_games_on_shelves(criteria);
    }

    fn search_servers(&self, criteria: &ServerCriteria) -> Vec<Server> {
        return self.search_servers_on_shelves(criteria);
    }
}
