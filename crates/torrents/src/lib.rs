use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use librqbit::api::TorrentIdOrHash;
use librqbit::dht::Id20;
use librqbit::spawn_utils::BlockingSpawner;
use librqbit::{
    create_torrent, AddTorrent, AddTorrentOptions, CreateTorrentOptions, ListenerMode, ListenerOptions,
    ManagedTorrent, Session, SessionOptions,
};
use tokio::runtime::Runtime;

/// Tout ce qui touche `librqbit` : fabriquer un torrent, partager un jeu,
/// en telecharger un, compter ce qui a ete envoye.
pub struct Torrents {
    runtime: Runtime,
    session: Arc<Session>,
}

impl Torrents {
    pub fn new(folder: &str, port: u16) -> Option<Torrents> {
        let runtime = Runtime::new();
        if runtime.is_err() {
            eprintln!("Failed to start tokio");
            return None;
        }
        let runtime = runtime.unwrap();

        let session = runtime.block_on(Session::new_with_opts(PathBuf::from(folder), session_options(port)));
        if session.is_err() {
            eprintln!("Failed to start torrents on port {}", port);
            return None;
        }

        return Some(Torrents { runtime, session: session.unwrap() });
    }

    /// Fabrique `content.torrent` a partir de `content/`, et rend l'infohash.
    pub fn make(&self, folder: &PathBuf) -> Option<String> {
        let content = folder.join("content");
        let torrent = self.runtime.block_on(async {
            let spawner = BlockingSpawner::new(1);
            return create_torrent(&content, CreateTorrentOptions::default(), &spawner).await;
        });
        if torrent.is_err() {
            eprintln!("Failed to create the torrent: {:?}", torrent.err());
            return None;
        }

        let torrent = torrent.unwrap();

        let bytes = torrent.as_bytes();
        if bytes.is_err() {
            eprintln!("Failed to encode the torrent");
            return None;
        }

        if fs::write(folder.join("content.torrent"), bytes.unwrap()).is_err() {
            eprintln!("Failed to write content.torrent");
            return None;
        }

        return Some(torrent.info_hash().as_string());
    }

    pub fn seed(&self, folder: &PathBuf) -> bool {
        let torrent = fs::read(folder.join("content.torrent"));
        if torrent.is_err() {
            return false;
        }

        let mut options = AddTorrentOptions::default();
        options.output_folder = Some(folder.join("content").to_string_lossy().to_string());
        options.overwrite = true;
        options.disable_trackers = true;

        let added = self.runtime.block_on(self.session.add_torrent(AddTorrent::from_bytes(torrent.unwrap()), Some(options)));

        return added.is_ok();
    }

    pub fn fetch(&self, infohash: &str, folder: &PathBuf, holders: Vec<SocketAddr>) -> bool {
        let mut options = AddTorrentOptions::default();
        options.output_folder = Some(folder.join("content").to_string_lossy().to_string());
        options.disable_trackers = true;
        options.initial_peers = Some(holders);

        let magnet = format!("magnet:?xt=urn:btih:{}", infohash);

        return self.runtime.block_on(async {
            let added = self.session.add_torrent(AddTorrent::from_url(magnet), Some(options)).await;
            if added.is_err() {
                return false;
            }

            let handle = added.unwrap().into_handle();
            if handle.is_none() {
                return false;
            }

            return handle.unwrap().wait_until_completed().await.is_ok();
        });
    }

    /// Telecharge UN fichier d'un torrent dans `folder`, par exemple une
    /// jaquette, puis retire le torrent de la session : il ne doit ni gener
    /// un telechargement complet plus tard, ni partager un contenu incomplet.
    pub fn fetch_file(&self, infohash: &str, folder: &PathBuf, holders: Vec<SocketAddr>, file: &str, within: Duration) -> bool {
        let mut options = AddTorrentOptions::default();
        options.output_folder = Some(folder.to_string_lossy().to_string());
        options.disable_trackers = true;
        options.initial_peers = Some(holders);
        options.only_files_regex = Some(format!("^{}$", regex_escaped(file)));

        let magnet = format!("magnet:?xt=urn:btih:{}", infohash);

        let done = self.runtime.block_on(async {
            let added = self.session.add_torrent(AddTorrent::from_url(magnet), Some(options)).await;
            if added.is_err() {
                return false;
            }

            let handle = added.unwrap().into_handle();
            if handle.is_none() {
                return false;
            }

            let finished = tokio::time::timeout(within, handle.unwrap().wait_until_completed()).await;

            return finished.is_ok() && finished.unwrap().is_ok();
        });

        self.remove(infohash);

        return done;
    }

    /// Ce que ce torrent a envoye depuis le demarrage de la session.
    pub fn uploaded(&self, infohash: &str) -> Option<u64> {
        let torrent = self.find(infohash);
        if torrent.is_none() {
            return None;
        }

        return Some(torrent.unwrap().stats().uploaded_bytes);
    }

    pub fn pause(&self, infohash: &str) {
        let torrent = self.find(infohash);
        if torrent.is_none() {
            return;
        }

        let _ = self.runtime.block_on(self.session.pause(&torrent.unwrap()));
    }

    /// Reprend l'envoi d'un torrent en pause. Rend faux si ce torrent n'est
    /// pas dans la session (il faut alors le partager avec `seed`).
    pub fn resume(&self, infohash: &str) -> bool {
        let torrent = self.find(infohash);
        if torrent.is_none() {
            return false;
        }

        let _ = self.runtime.block_on(self.session.unpause(&torrent.unwrap()));

        return true;
    }

    /// Ce qui est deja telecharge, et la taille totale.
    pub fn progress(&self, infohash: &str) -> Option<(u64, u64)> {
        let torrent = self.find(infohash);
        if torrent.is_none() {
            return None;
        }

        let stats = torrent.unwrap().stats();

        return Some((stats.progress_bytes, stats.total_bytes));
    }

    /// Retire un torrent de la session (les fichiers sont laisses : c'est au
    /// node de supprimer le dossier).
    pub fn remove(&self, infohash: &str) {
        let id = Id20::from_str(infohash);
        if id.is_err() {
            return;
        }

        let _ = self.runtime.block_on(self.session.delete(TorrentIdOrHash::Hash(id.unwrap()), false));
    }

    fn find(&self, infohash: &str) -> Option<Arc<ManagedTorrent>> {
        let id = Id20::from_str(infohash);
        if id.is_err() {
            return None;
        }

        return self.session.get(TorrentIdOrHash::Hash(id.unwrap()));
    }
}

fn session_options(port: u16) -> SessionOptions {
    let mut listen = ListenerOptions::default();
    listen.mode = ListenerMode::TcpOnly;
    listen.listen_addr = SocketAddr::from(([0, 0, 0, 0], port));

    let mut options = SessionOptions::default();
    options.dht = None;
    options.disable_trackers = true;
    options.disable_local_service_discovery = true;
    options.listen = Some(listen);

    return options;
}

fn regex_escaped(text: &str) -> String {
    let mut escaped = String::new();

    for letter in text.chars() {
        if ".+*?()[]{}|^$\\".contains(letter) {
            escaped.push('\\');
        }
        escaped.push(letter);
    }

    return escaped;
}
