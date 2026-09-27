//! La porte locale : ce que le client (ou le serveur) du joueur demande au
//! node. Une `Command` recoit toujours une `Answer`.

use std::net::{TcpListener, TcpStream};
use std::process;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;

use catalog::{content_bytes, read_default_share, server_bytes, write_default_share, Server};
use protocol::{Answer, Command, Reason, Subject};
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

    fn serve_local(self: &Arc<Self>, mut stream: TcpStream) {
        let bytes = read_frame(&mut stream);
        if bytes.is_none() {
            return;
        }

        let command = Command::from_bytes(&bytes.unwrap());
        if command.is_none() {
            return;
        }

        let command = command.unwrap();
        let stopping = matches!(command, Command::Stop);

        let answer = self.obey(command);

        for piece in answer.split() {
            if !write_frame(&mut stream, &piece.to_bytes()) {
                break;
            }
        }

        if stopping {
            self.stop();
        }
    }

    fn obey(self: &Arc<Self>, command: Command) -> Answer {
        match command {
            Command::ContentSearch { criteria } => {
                return Answer::Contents { contents: self.search_contents_on_shelves(&criteria) };
            }
            Command::Installed => return Answer::Contents { contents: self.contents.lock().unwrap().get_installed() },
            Command::Download { content } => return self.start_download(content),
            Command::Progress { infohash } => return self.progress(&infohash),
            Command::Uninstall { infohash } => return self.uninstall(&infohash),
            Command::GetShare { infohash } => return self.get_share(&infohash),
            Command::SetShare { infohash, share } => return self.set_share(&infohash, share),
            Command::GetDefaultShare => return Answer::Share { share: read_default_share() },
            Command::SetDefaultShare { share } => return set_default_share(share),
            Command::ServersSearch { criteria } => {
                return Answer::Servers { servers: self.search_servers_on_shelves(&criteria) };
            }
            Command::Challenge { what } => return self.challenge(what),
            Command::Signed { what, signature } => return self.signed(what, signature),
            Command::Stop => return Answer::Done,
            Command::Status => return self.status(),
        }
    }

    fn challenge(&self, what: Subject) -> Answer {
        match what {
            Subject::Content { infohash } => {
                let content = self.contents.lock().unwrap().find(&infohash);
                if content.is_none() {
                    return failed(Reason::UnknownContent);
                }

                return Answer::ToSign { bytes: content_bytes(&content.unwrap()) };
            }
            Subject::Server { server } => return Answer::ToSign { bytes: server_bytes(&server) },
        }
    }

    fn signed(&self, what: Subject, signature: [u8; 64]) -> Answer {
        match what {
            Subject::Content { infohash } => return self.sign_content(&infohash, signature),
            Subject::Server { mut server } => {
                server.signature = signature;
                return self.declare_server(server);
            }
        }
    }

    fn sign_content(&self, infohash: &str, signature: [u8; 64]) -> Answer {
        if self.contents.lock().unwrap().find(infohash).is_none() {
            return failed(Reason::UnknownContent);
        }

        let content = self.contents.lock().unwrap().set_signature(infohash, signature);
        if content.is_none() {
            return failed(Reason::BadSignature);
        }

        let content = content.unwrap();
        self.seed(&content);
        self.announce_holding(&content);
        self.store_content(&content);

        return Answer::Done;
    }

    fn declare_server(&self, server: Server) -> Answer {
        if !server.is_fresh() {
            return failed(Reason::TooOld);
        }

        if !server.is_signed() {
            return failed(Reason::BadSignature);
        }

        self.servers.lock().unwrap().add(server.clone());
        self.store_server(&server);

        return Answer::Done;
    }

    fn status(&self) -> Answer {
        return Answer::Status {
            neighbors: self.neighbors.lock().unwrap().count() as u32,
            contents_known: self.contents.lock().unwrap().values.len() as u32,
            servers_known: self.servers.lock().unwrap().values.len() as u32,
            depth: self.depth.load(Ordering::Relaxed),
            port: self.address.port,
        };
    }

    /// Sauvegarde, puis arrete le programme.
    fn stop(&self) {
        self.neighbors.lock().unwrap().save();
        self.contents.lock().unwrap().save_shares();

        process::exit(0);
    }
}

fn set_default_share(share: Option<u32>) -> Answer {
    if share == Some(0) {
        return failed(Reason::Refused);
    }

    if !write_default_share(share) {
        return failed(Reason::Refused);
    }

    return Answer::Done;
}

pub(super) fn failed(reason: Reason) -> Answer {
    return Answer::Failed { reason };
}
