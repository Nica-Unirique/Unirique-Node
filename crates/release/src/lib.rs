//! Les versions publiees du node et du lanceur, sur GitHub.
//!
//! Chaque release du depot `Unirique-Node` contient `version.txt`, et pour
//! chacune des 4 plateformes un `node-<plateforme>` et un
//! `launcher-<plateforme>`, chacun avec sa signature `<fichier>.sig`.
//!
//! Ce qui est signe, avec la cle du node principal (`MAIN_KEY`) :
//! `unirique-release` + la version + le nom du fichier + l'empreinte SHA-256
//! du fichier. Un fichier signe pour une plateforme ne passe donc pas pour
//! une autre, et une version ne passe pas pour une autre.

use std::io::Read;
use std::time::Duration;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

use main_address::MAIN_KEY;
use wire::{put_text, signature_from_hex, signature_to_hex};

/// Ou les fichiers de la derniere release se telechargent.
pub const RELEASES: &str = "https://github.com/Nica-Unirique/Unirique-Node/releases/latest/download/";

/// Le fichier de la release qui donne sa version.
pub const VERSION_FILE: &str = "version.txt";

/// Les 4 plateformes publiees.
pub const PLATFORMS: [&str; 4] = ["windows-x86_64.exe", "windows-aarch64.exe", "linux-x86_64", "linux-aarch64"];

/// Les programmes publies pour chaque plateforme.
pub const PROGRAMS: [&str; 2] = ["node", "launcher"];

const TEXT_WITHIN: Duration = Duration::from_secs(10);
const FILE_WITHIN: Duration = Duration::from_secs(300);
const TEXT_MAX: u64 = 4 * 1024;
const FILE_MAX: u64 = 200 * 1024 * 1024;

/// La plateforme de ce programme, telle qu'elle est nommee dans les releases.
pub fn platform() -> &'static str {
    if cfg!(all(windows, target_arch = "aarch64")) {
        return PLATFORMS[1];
    }

    if cfg!(windows) {
        return PLATFORMS[0];
    }

    if cfg!(target_arch = "aarch64") {
        return PLATFORMS[3];
    }

    return PLATFORMS[2];
}

/// Le nom publie d'un programme pour cette plateforme : `node-linux-aarch64`.
pub fn published_name(program: &str) -> String {
    return format!("{}-{}", program, platform());
}

/// `X.Y.Z`, avec ou sans `v` devant.
pub fn read_version(text: &str) -> Option<[u32; 3]> {
    let text = text.trim().trim_start_matches('v');
    let parts: Vec<&str> = text.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    let mut version = [0u32; 3];
    for seat in 0..3 {
        let read = parts[seat].parse::<u32>();
        if read.is_err() {
            return None;
        }
        version[seat] = read.unwrap();
    }

    return Some(version);
}

pub fn version_text(version: [u32; 3]) -> String {
    return format!("{}.{}.{}", version[0], version[1], version[2]);
}

pub fn sign(key: &SigningKey, version: [u32; 3], name: &str, bytes: &[u8]) -> String {
    let signature = key.sign(&signed_bytes(version, name, bytes));

    return signature_to_hex(&signature.to_bytes());
}

/// Signe par le node principal ?
pub fn is_signed(version: [u32; 3], name: &str, bytes: &[u8], signature_hex: &str) -> bool {
    let signature = signature_from_hex(signature_hex.trim());
    if signature.is_none() {
        return false;
    }

    let key = VerifyingKey::from_bytes(&MAIN_KEY);
    if key.is_err() {
        return false;
    }

    let signature = Signature::from_bytes(&signature.unwrap());

    return key.unwrap().verify_strict(&signed_bytes(version, name, bytes), &signature).is_ok();
}

fn signed_bytes(version: [u32; 3], name: &str, bytes: &[u8]) -> Vec<u8> {
    let mut signed = b"unirique-release".to_vec();

    put_text(&mut signed, &version_text(version));
    put_text(&mut signed, name);
    signed.extend_from_slice(&Sha256::digest(bytes));

    return signed;
}

/// Remplace `RELEASES` quand elle est donnee : essais avec des fichiers servis
/// en local.
const RELEASES_OVERRIDE: &str = "UNIRIQUE_RELEASES";

fn releases() -> String {
    let chosen = std::env::var(RELEASES_OVERRIDE);
    if chosen.is_ok() {
        return chosen.unwrap();
    }

    return String::from(RELEASES);
}

/// La version de la derniere release.
pub fn latest_version() -> Option<[u32; 3]> {
    let text = download(&format!("{}{}", releases(), VERSION_FILE), TEXT_WITHIN, TEXT_MAX);
    if text.is_none() {
        return None;
    }

    let text = String::from_utf8(text.unwrap());
    if text.is_err() {
        return None;
    }

    return read_version(&text.unwrap());
}

/// Un programme de la derniere release pour cette plateforme, seulement si
/// le node principal l'a signe pour cette version.
pub fn fetch_signed(program: &str, version: [u32; 3]) -> Option<Vec<u8>> {
    let name = published_name(program);

    let bytes = download(&format!("{}{}", releases(), name), FILE_WITHIN, FILE_MAX);
    if bytes.is_none() {
        eprintln!("Failed to download {}", name);
        return None;
    }

    let signature = download(&format!("{}{}.sig", releases(), name), TEXT_WITHIN, TEXT_MAX);
    if signature.is_none() {
        eprintln!("Failed to download {}.sig", name);
        return None;
    }

    let signature = String::from_utf8(signature.unwrap());
    if signature.is_err() {
        return None;
    }

    let bytes = bytes.unwrap();
    if !is_signed(version, &name, &bytes, &signature.unwrap()) {
        eprintln!("{} is not signed by the main node: refused", name);
        return None;
    }

    return Some(bytes);
}

fn download(url: &str, within: Duration, most: u64) -> Option<Vec<u8>> {
    let config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(within))
        .build();

    let answer = ureq::Agent::new_with_config(config).get(url).call();
    if answer.is_err() {
        return None;
    }
    let mut answer = answer.unwrap();

    if answer.status() != 200 {
        return None;
    }

    let mut bytes = Vec::new();
    let read = answer.body_mut().with_config().limit(most).reader().read_to_end(&mut bytes);
    if read.is_err() {
        return None;
    }

    return Some(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_reads_with_or_without_its_v() {
        assert_eq!(read_version("0.2.13"), Some([0, 2, 13]));
        assert_eq!(read_version(" v1.0.0\n"), Some([1, 0, 0]));
        assert_eq!(read_version("1.0"), None);
        assert_eq!(read_version("1.x.0"), None);
    }

    #[test]
    fn versions_compare_number_by_number() {
        assert!([0, 10, 0] > [0, 9, 9]);
        assert!([1, 0, 0] > [0, 99, 99]);
    }

    #[test]
    fn a_signature_holds_only_for_its_version_its_name_and_its_bytes() {
        let key = SigningKey::from_bytes(&[3; 32]);
        let public = key.verifying_key().to_bytes();
        let signature = sign(&key, [0, 2, 0], "node-linux-aarch64", b"programme");

        let check = |version: [u32; 3], name: &str, bytes: &[u8]| {
            let signature = Signature::from_bytes(&signature_from_hex(&signature).unwrap());
            VerifyingKey::from_bytes(&public).unwrap().verify_strict(&signed_bytes(version, name, bytes), &signature).is_ok()
        };

        assert!(check([0, 2, 0], "node-linux-aarch64", b"programme"));
        assert!(!check([0, 3, 0], "node-linux-aarch64", b"programme"));
        assert!(!check([0, 2, 0], "node-windows-x86_64.exe", b"programme"));
        assert!(!check([0, 2, 0], "node-linux-aarch64", b"autre programme"));
    }

    #[test]
    fn this_build_has_a_published_name() {
        assert!(PLATFORMS.contains(&platform()));
        assert!(published_name("node").starts_with("node-"));
    }
}
