//! Les etageres : a quelles positions du reseau une fiche est rangee.
//!
//! Une fiche a plusieurs positions, une par critere de recherche :
//! - la cle de son auteur (ou de son hote, pour un serveur) ;
//! - chacun de ses tags ;
//! - chaque groupe de 3 lettres de son nom, pour qu'un morceau de nom,
//!   meme au milieu, mene a la bonne etagere.
//!
//! Le hash ne sert qu'a repartir les positions sur tout le reseau.

use neighbors::common_bits;

use crate::criteria::{Criteria, ServerCriteria};
use crate::content::Content;
use crate::server::Server;

const NAME: &str = "name";
const TAG: &str = "tag";
const SERVER_NAME: &str = "server-name";

pub fn content_positions(content: &Content) -> Vec<u64> {
    let mut positions = vec![key_position(&content.autor_key)];

    for tag in content.tags.iter() {
        add(&mut positions, hash_text(TAG, &tag.to_string()));
    }

    for trigram in trigrams(&content.name) {
        add(&mut positions, hash_text(NAME, &trigram));
    }

    return positions;
}

pub fn server_positions(server: &Server) -> Vec<u64> {
    let mut positions = vec![key_position(&server.host_key)];

    for trigram in trigrams(&server.name) {
        add(&mut positions, hash_text(SERVER_NAME, &trigram));
    }

    for trigram in trigrams(&server.game_name) {
        add(&mut positions, hash_text(NAME, &trigram));
    }

    return positions;
}

/// L'etagere ou chercher ces criteres, ou rien s'ils n'en designent aucune
/// (par exemple un morceau de nom de moins de 3 lettres).
pub fn criteria_position(criteria: &Criteria) -> Option<u64> {
    if criteria.autor_key.is_some() {
        return Some(key_position(&criteria.autor_key.unwrap()));
    }

    if !criteria.tags.is_empty() {
        return Some(hash_text(TAG, &criteria.tags[0].to_string()));
    }

    if criteria.name.is_some() {
        let found = trigrams(criteria.name.as_ref().unwrap());
        if !found.is_empty() {
            return Some(hash_text(NAME, &found[0]));
        }
    }

    return None;
}

pub fn server_criteria_position(criteria: &ServerCriteria) -> Option<u64> {
    if criteria.host_key.is_some() {
        return Some(key_position(&criteria.host_key.unwrap()));
    }

    if criteria.name.is_some() {
        let found = trigrams(criteria.name.as_ref().unwrap());
        if !found.is_empty() {
            return Some(hash_text(SERVER_NAME, &found[0]));
        }
    }

    if criteria.game_name.is_some() {
        let found = trigrams(criteria.game_name.as_ref().unwrap());
        if !found.is_empty() {
            return Some(hash_text(NAME, &found[0]));
        }
    }

    return None;
}

/// Une fiche est sur mon etagere si une de ses positions partage au moins
/// `depth` bits avec mon id.
pub fn on_shelf(positions: &Vec<u64>, id: u64, depth: u8) -> bool {
    for position in positions.iter() {
        if common_bits(*position, id) >= depth as u32 {
            return true;
        }
    }

    return false;
}

/// Pour trier les resultats : 0 = un mot entier du nom, 1 = le debut d'un
/// mot, 2 = ailleurs.
pub fn name_rank(name: &str, fragment: &str) -> u8 {
    let name = name.to_lowercase();
    let fragment = fragment.to_lowercase();

    for word in name.split_whitespace() {
        if word == fragment {
            return 0;
        }
    }

    for word in name.split_whitespace() {
        if word.starts_with(&fragment) {
            return 1;
        }
    }

    return 2;
}

fn trigrams(text: &str) -> Vec<String> {
    let letters: Vec<char> = text.to_lowercase().chars().collect();
    let mut found = Vec::new();

    if letters.len() < 3 {
        return found;
    }

    for start in 0..letters.len() - 2 {
        let trigram: String = letters[start..start + 3].iter().collect();

        if !found.contains(&trigram) {
            found.push(trigram);
        }
    }

    return found;
}

fn key_position(key: &[u8; 32]) -> u64 {
    let mut first = [0u8; 8];
    first.copy_from_slice(&key[0..8]);

    return u64::from_be_bytes(first);
}

/// FNV-1a sur 64 bits.
fn hash_text(kind: &str, text: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;

    for byte in kind.bytes().chain([b':']).chain(text.bytes()) {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }

    return hash;
}

fn add(positions: &mut Vec<u64>, position: u64) {
    if !positions.contains(&position) {
        positions.push(position);
    }
}
