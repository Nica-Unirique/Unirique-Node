//! L'adresse du node principal, publiee dans `ip.txt` sur GitHub.
//!
//! Le node principal est derriere des tunnels gratuits dont l'adresse change
//! toutes les heures : elle n'est donc pas ecrite dans le code. Le node
//! principal l'ecrit, la signe, et un script la publie ; les nodes et les
//! clients la lisent et verifient la signature avec `MAIN_KEY`.
//!
//! Le fichier, une valeur par ligne :
//!
//! ```text
//! node=xxx.a.free.pinggy.link:41234
//! torrent=yyy.a.free.pinggy.link:41235
//! ucompany=https://zzz.a.free.pinggy.link
//! time=1790000000
//! signature=<128 chiffres hexadecimaux>
//! ```

use std::io::Read;
use std::time::Duration;

use ed25519_dalek::{Signature, Signer, VerifyingKey};

pub use ed25519_dalek::SigningKey;

use wire::{key_to_hex, put_text, put_u64, signature_from_hex, signature_to_hex};

/// Ou le fichier est publie.
pub const ADDRESS_URL: &str = "https://raw.githubusercontent.com/Nica-Unirique/Unirique-IP/main/ip.txt";

/// La cle publique du node principal. A remplacer par celle qu'affiche le
/// node principal a son premier lancement (`--main`).
pub const MAIN_KEY: [u8; 32] = [0; 32];

const FETCH_WITHIN: Duration = Duration::from_secs(10);
const FILE_MAX: u64 = 4 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct MainAddress {
    /// Ou joindre le node principal : `hote:port`.
    pub node: String,
    /// Ou joindre son partage torrent : `hote:port`.
    pub torrent: String,
    /// L'adresse de UCompany : `https://...`.
    pub ucompany: String,
    /// Quand l'adresse a ete signee, en secondes depuis 1970.
    pub time: u64,
    pub signature: [u8; 64],
}

impl MainAddress {
    pub fn new(node: &str, torrent: &str, ucompany: &str, time: u64) -> MainAddress {
        return MainAddress {
            node: String::from(node),
            torrent: String::from(torrent),
            ucompany: String::from(ucompany),
            time,
            signature: [0; 64],
        };
    }

    pub fn sign(&mut self, key: &SigningKey) {
        self.signature = key.sign(&self.signed_bytes()).to_bytes();
    }

    pub fn is_signed_by(&self, key: &[u8; 32]) -> bool {
        let key = VerifyingKey::from_bytes(key);
        if key.is_err() {
            return false;
        }

        let signature = Signature::from_bytes(&self.signature);

        return key.unwrap().verify_strict(&self.signed_bytes(), &signature).is_ok();
    }

    fn signed_bytes(&self) -> Vec<u8> {
        let mut bytes = b"unirique-main".to_vec();

        put_text(&mut bytes, &self.node);
        put_text(&mut bytes, &self.torrent);
        put_text(&mut bytes, &self.ucompany);
        put_u64(&mut bytes, self.time);

        return bytes;
    }

    pub fn to_text(&self) -> String {
        let mut text = String::new();

        text += &format!("node={}\n", self.node);
        text += &format!("torrent={}\n", self.torrent);
        text += &format!("ucompany={}\n", self.ucompany);
        text += &format!("time={}\n", self.time);
        text += &format!("signature={}\n", signature_to_hex(&self.signature));

        return text;
    }

    /// Lit le fichier. Rien s'il manque une valeur : un fichier a moitie
    /// ecrit ne doit pas envoyer les joueurs nulle part.
    pub fn from_text(text: &str) -> Option<MainAddress> {
        let values = read_values(text);

        let node = value_of(&values, "node");
        let torrent = value_of(&values, "torrent");
        let ucompany = value_of(&values, "ucompany");
        let time = value_of(&values, "time");
        let signature = value_of(&values, "signature");

        if node.is_none() || torrent.is_none() || ucompany.is_none() || time.is_none() || signature.is_none() {
            return None;
        }

        let time = time.unwrap().parse::<u64>();
        let signature = signature_from_hex(&signature.unwrap());
        if time.is_err() || signature.is_none() {
            return None;
        }

        return Some(MainAddress {
            node: node.unwrap(),
            torrent: torrent.unwrap(),
            ucompany: ucompany.unwrap(),
            time: time.unwrap(),
            signature: signature.unwrap(),
        });
    }
}

/// L'adresse publiee, seulement si le node principal l'a bien signee.
pub fn fetch() -> Option<MainAddress> {
    let text = download(ADDRESS_URL);
    if text.is_none() {
        eprintln!("Failed to download the main node address: {}", ADDRESS_URL);
        return None;
    }

    let address = MainAddress::from_text(&text.unwrap());
    if address.is_none() {
        eprintln!("The main node address is unreadable");
        return None;
    }

    let address = address.unwrap();
    if !address.is_signed_by(&MAIN_KEY) {
        eprintln!("The main node address is not signed by the main node");
        return None;
    }

    return Some(address);
}

/// Une cle de node principal, a partir de 32 octets secrets.
pub fn key_from_secret(secret: &[u8; 32]) -> SigningKey {
    return SigningKey::from_bytes(secret);
}

/// La cle publique a ecrire dans `MAIN_KEY`.
pub fn public_hex(key: &SigningKey) -> String {
    return key_to_hex(&key.verifying_key().to_bytes());
}

fn download(url: &str) -> Option<String> {
    let config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(FETCH_WITHIN))
        .build();

    let answer = ureq::Agent::new_with_config(config).get(url).call();
    if answer.is_err() {
        return None;
    }
    let mut answer = answer.unwrap();

    if answer.status() != 200 {
        return None;
    }

    let mut text = String::new();
    let read = answer.body_mut().as_reader().take(FILE_MAX).read_to_string(&mut text);
    if read.is_err() {
        return None;
    }

    return Some(text);
}

fn read_values(text: &str) -> Vec<(String, String)> {
    let mut values = Vec::new();

    for line in text.lines() {
        let split = line.trim().split_once('=');
        if split.is_none() {
            continue;
        }

        let (name, value) = split.unwrap();
        values.push((String::from(name.trim()), String::from(value.trim())));
    }

    return values;
}

fn value_of(values: &Vec<(String, String)>, name: &str) -> Option<String> {
    for (found, value) in values.iter() {
        if found == name && !value.is_empty() {
            return Some(value.clone());
        }
    }

    return None;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed() -> (MainAddress, SigningKey) {
        let key = key_from_secret(&[7; 32]);
        let mut address = MainAddress::new("a.free.pinggy.link:41234", "b.free.pinggy.link:41235", "https://c.free.pinggy.link", 1_790_000_000);
        address.sign(&key);

        return (address, key);
    }

    #[test]
    fn a_signed_address_reads_back_the_same_and_stays_signed() {
        let (address, key) = signed();
        let read = MainAddress::from_text(&address.to_text()).unwrap();

        assert_eq!(read, address);
        assert!(read.is_signed_by(&key.verifying_key().to_bytes()));
    }

    #[test]
    fn a_changed_address_is_no_longer_signed() {
        let (mut address, key) = signed();
        address.node = String::from("pirate.example:41234");

        assert!(!address.is_signed_by(&key.verifying_key().to_bytes()));
    }

    #[test]
    fn another_key_did_not_sign_it() {
        let (address, _) = signed();
        let other = key_from_secret(&[8; 32]);

        assert!(!address.is_signed_by(&other.verifying_key().to_bytes()));
    }

    #[test]
    fn a_file_missing_a_value_is_refused() {
        let (address, _) = signed();
        let text = address.to_text().replace("ucompany=https://c.free.pinggy.link\n", "");

        assert_eq!(MainAddress::from_text(&text), None);
    }
}
