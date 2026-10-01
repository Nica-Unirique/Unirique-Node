//! Le partage : torrents, telechargements, desinstallation, quotas.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use catalog::{content_bytes, write_infohash, Content, CONTENTS_FOLDER};
use main_address::sign_bytes;
use protocol::{Answer, Reason};

use super::local::failed;
use super::{Node, COVERS_FOLDER, COVER_WITHIN};

/// Ou en est un telechargement lance par `Download`.
#[derive(Clone, Copy)]
pub(super) enum Download {
    /// On cherche qui partage ce contenu.
    Searching,
    /// Le torrent recoit les fichiers.
    Fetching,
    Finished,
    Failed(Reason),
}

impl Node {
    /// Fabrique le torrent des contenus qui n'en ont pas encore : un contenu
    /// que l'auteur vient de poser, ou qu'on vient de telecharger.
    pub(super) fn complete_torrents(&self) {
        let folders = self.contents.lock().unwrap().needing_torrent();

        for folder in folders.iter() {
            let infohash = self.torrents.make(folder);
            if infohash.is_none() {
                continue;
            }

            let infohash = infohash.unwrap();
            write_infohash(folder, &infohash);
            self.contents.lock().unwrap().set_infohash(folder, &infohash);
        }
    }

    /// Publier, c'est poser un dossier dans `Contents/` : un `manifest.json`
    /// (nom, version, tags, description, jaquette) et `content/`. Le node le
    /// voit au tour suivant, fabrique son torrent et le partage s'il est
    /// signe. Le node principal signe lui-meme les contenus officiels.
    pub(super) fn publish_dropped(&self) {
        let added = self.contents.lock().unwrap().add_new_folders();
        if added.is_empty() {
            return;
        }

        self.complete_torrents();

        for folder in added.iter() {
            let content = self.contents.lock().unwrap().find_folder(folder);
            if content.is_none() {
                continue;
            }
            let mut content = content.unwrap();

            if !content.is_signed() {
                let signed = self.sign_official(&content);
                if signed.is_none() {
                    eprintln!("Waiting for its author's signature: {}", content.name);
                    continue;
                }
                content = signed.unwrap();
            }

            eprintln!("Published: {} {}.{}.{}", content.name, content.version[0], content.version[1], content.version[2]);
            self.share_signed(&content);
        }
    }

    /// Au demarrage : le node principal signe les contenus officiels poses
    /// pendant qu'il etait arrete.
    pub(super) fn sign_official_contents(&self) {
        let contents = self.contents.lock().unwrap().get_all();

        for content in contents.iter() {
            if content.downloaded && !content.is_signed() {
                let _ = self.sign_official(content);
            }
        }
    }

    /// Signe un contenu officiel avec la cle du node principal : un contenu
    /// pose sans auteur, ou dont l'auteur est deja le node principal. Rien sur
    /// un autre node, ni pour le contenu d'un autre auteur.
    fn sign_official(&self, content: &Content) -> Option<Content> {
        if self.main_key.is_none() || content.infohash.is_empty() {
            return None;
        }

        let key = self.main_key.as_ref().unwrap();
        let main = key.verifying_key().to_bytes();
        if content.autor_key != [0; 32] && content.autor_key != main {
            return None;
        }

        let mut contents = self.contents.lock().unwrap();
        contents.set_autor_key(&content.folder, main);

        let mut official = content.clone();
        official.autor_key = main;
        let signature = sign_bytes(key, &content_bytes(&official));

        return contents.set_signature(&content.infohash, signature);
    }

    /// Partage un contenu signe et le fait connaitre.
    fn share_signed(&self, content: &Content) {
        if content.is_exhausted() {
            return;
        }

        self.seed(content);
        self.announce_holding(content);
        self.store_content(content);
    }

    pub(super) fn seed_contents(&self) {
        let contents = self.contents.lock().unwrap().get_all();

        for content in contents.iter() {
            if content.downloaded && !content.is_exhausted() && content.is_signed() {
                self.seed(content);
            }
        }
    }

    pub(super) fn seed(&self, content: &Content) {
        if !self.torrents.seed(&content.folder) {
            eprintln!("Failed to share {}", content.name);
        }
    }

    /// Repond tout de suite ; le telechargement continue dans un thread, et
    /// le client suit son avancement avec `Progress`.
    pub(super) fn start_download(self: &Arc<Self>, content: Content) -> Answer {
        if !content.is_signed() {
            return failed(Reason::BadSignature);
        }

        if self.contents.lock().unwrap().find(&content.infohash).is_some() {
            return Answer::Done;
        }

        let mut downloads = self.downloads.lock().unwrap();
        let running = downloads.get(&content.infohash);
        if running.is_some() && !matches!(running.unwrap(), Download::Failed(_)) {
            return Answer::Done;
        }
        downloads.insert(content.infohash.clone(), Download::Searching);
        drop(downloads);

        let node = self.clone();
        thread::spawn(move || {
            let result = node.download(&content);

            let state;
            if result.is_err() {
                state = Download::Failed(result.err().unwrap());
            } else {
                state = Download::Finished;
            }

            node.downloads.lock().unwrap().insert(content.infohash.clone(), state);
        });

        return Answer::Done;
    }

    fn download(&self, content: &Content) -> Result<(), Reason> {
        let holders = self.find_holders(&content.infohash);
        if holders.is_empty() {
            eprintln!("No one shares {}", content.name);
            return Err(Reason::NoSource);
        }

        self.downloads.lock().unwrap().insert(content.infohash.clone(), Download::Fetching);

        // Une jaquette en cours de telechargement tient peut-etre ce torrent.
        self.torrents.remove(&content.infohash);

        let folder = PathBuf::from(CONTENTS_FOLDER).join(content.folder_name());
        if !self.torrents.fetch(&content.infohash, &folder, holders) {
            eprintln!("Failed to download {}", content.name);
            return Err(Reason::NoSource);
        }

        if !content.write_manifest(&folder) {
            eprintln!("Failed to write the manifest of {}", content.name);
            return Err(Reason::Refused);
        }

        let mut contents = self.contents.lock().unwrap();
        contents.add_local(&folder);
        contents.load_shares();
        drop(contents);

        self.complete_torrents();

        return Ok(());
    }

    /// La jaquette d'un contenu : celle du contenu installe, ou celle deja
    /// telechargee, ou sinon elle seule, depuis les sources du contenu.
    pub(super) fn cover(&self, content: &Content) -> Answer {
        if !content.is_signed() {
            return failed(Reason::BadSignature);
        }

        if !content.cover_is_safe() {
            return failed(Reason::Refused);
        }

        let installed = self.contents.lock().unwrap().find(&content.infohash);
        if installed.is_some() {
            let path = installed.unwrap().folder.join("content").join(&content.cover);
            if path.is_file() {
                return cover_file(&path);
            }
        }

        let folder = PathBuf::from(COVERS_FOLDER).join(&content.infohash);
        let path = folder.join(&content.cover);
        if path.is_file() {
            return cover_file(&path);
        }

        let holders = self.find_holders(&content.infohash);
        if holders.is_empty() {
            return failed(Reason::NoSource);
        }

        if !self.torrents.fetch_file(&content.infohash, &folder, holders, &content.cover, COVER_WITHIN) || !path.is_file() {
            return failed(Reason::NoSource);
        }

        return cover_file(&path);
    }

    pub(super) fn progress(&self, infohash: &str) -> Answer {
        let state = self.downloads.lock().unwrap().get(infohash).copied();

        if state.is_none() {
            let installed = self.contents.lock().unwrap().find(infohash);
            if installed.is_none() {
                return failed(Reason::UnknownContent);
            }

            let size = installed.unwrap().disk_weight;
            return Answer::Progress { done_bytes: size, total_bytes: size };
        }

        match state.unwrap() {
            Download::Failed(reason) => return failed(reason),
            Download::Searching => return Answer::Progress { done_bytes: 0, total_bytes: 0 },
            Download::Fetching | Download::Finished => {
                let progress = self.torrents.progress(infohash);
                if progress.is_none() {
                    return Answer::Progress { done_bytes: 0, total_bytes: 0 };
                }

                let (done_bytes, total_bytes) = progress.unwrap();
                return Answer::Progress { done_bytes, total_bytes };
            }
        }
    }

    /// Arrete de partager le contenu et supprime son dossier.
    pub(super) fn uninstall(&self, infohash: &str) -> Answer {
        let folder = self.contents.lock().unwrap().remove(infohash);
        if folder.is_none() {
            return failed(Reason::UnknownContent);
        }

        self.torrents.remove(infohash);
        self.downloads.lock().unwrap().remove(infohash);

        if fs::remove_dir_all(folder.unwrap()).is_err() {
            eprintln!("Failed to delete the folder of {}", infohash);
        }

        return Answer::Done;
    }

    pub(super) fn get_share(&self, infohash: &str) -> Answer {
        let share = self.contents.lock().unwrap().get_share(infohash);
        if share.is_none() {
            return failed(Reason::UnknownContent);
        }

        return Answer::Share { share: share.unwrap() };
    }

    pub(super) fn set_share(&self, infohash: &str, share: Option<u32>) -> Answer {
        if share == Some(0) {
            return failed(Reason::Refused);
        }

        if !self.contents.lock().unwrap().set_share(infohash, share) {
            return failed(Reason::UnknownContent);
        }

        let content = self.contents.lock().unwrap().find(infohash);
        if content.is_none() {
            return Answer::Done;
        }

        let content = content.unwrap();
        if content.is_exhausted() || !content.is_signed() {
            return Answer::Done;
        }

        if !self.torrents.resume(infohash) {
            self.seed(&content);
        }

        return Answer::Done;
    }

    pub(super) fn apply_quotas(&self) {
        let mut to_pause = Vec::new();

        let mut contents = self.contents.lock().unwrap();

        for content in contents.values.iter_mut() {
            if !content.downloaded || content.is_exhausted() {
                continue;
            }

            let uploaded = self.torrents.uploaded(&content.infohash);
            if uploaded.is_none() {
                continue;
            }
            let uploaded = uploaded.unwrap();

            content.sent += uploaded.saturating_sub(content.session_sent);
            content.session_sent = uploaded;

            if content.is_exhausted() {
                to_pause.push(content.infohash.clone());
            }
        }

        contents.save_shares();
        drop(contents);

        for infohash in to_pause.iter() {
            self.torrents.pause(infohash);
        }
    }
}

fn cover_file(path: &PathBuf) -> Answer {
    let full = fs::canonicalize(path);
    if full.is_err() {
        return failed(Reason::NoSource);
    }

    return Answer::CoverFile { path: full.unwrap().to_string_lossy().to_string() };
}
