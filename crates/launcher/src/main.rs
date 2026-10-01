//! Le lanceur du node.
//!
//! Lance a la place du node, avec les memes arguments. Il regarde d'abord si
//! GitHub a une version plus recente du node pour cette plateforme, la
//! telecharge et verifie qu'elle est signee par le node principal, puis
//! lance le node avec tous ses arguments et rend son code de sortie.
//!
//! Sans reseau, sans signature valide, ou si le node tourne deja, il lance
//! le node qu'il a.
//!
//! `launcher --sign <version> <dossier>` signe les exe d'une release avec
//! `data/main_key.txt` : a faire la ou se trouve la cle.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

use main_address::SigningKey;
use release::{fetch_signed, latest_version, read_version, sign, version_text, PLATFORMS, PROGRAMS};
use wire::key_from_hex;

const LOCAL_VERSION: &str = "version.txt";
const MAIN_KEY_FILE: &str = "data/main_key.txt";

#[cfg(windows)]
const NODE_FILE: &str = "node.exe";
#[cfg(not(windows))]
const NODE_FILE: &str = "node";

fn main() {
    let arguments: Vec<String> = env::args().skip(1).collect();

    if arguments.len() > 0 && arguments[0] == "--sign" {
        sign_release(&arguments[1..]);
        return;
    }

    let folder = own_folder();

    update(&folder);

    exit(run_node(&folder, &arguments));
}

/// Le dossier du lanceur : le node, sa version et ses donnees y sont.
fn own_folder() -> PathBuf {
    let exe = env::current_exe();
    if exe.is_ok() && exe.as_ref().unwrap().parent().is_some() {
        return exe.unwrap().parent().unwrap().to_path_buf();
    }

    return PathBuf::from(".");
}

/// Installe la derniere version publiee, si elle est plus recente que la
/// notre. Une version plus ancienne n'est jamais installee : on ne doit pas
/// pouvoir nous faire revenir a une version qui avait un defaut.
fn update(folder: &Path) {
    let node = folder.join(NODE_FILE);
    let mut local = [0u32; 3];

    let text = fs::read_to_string(folder.join(LOCAL_VERSION));
    if text.is_ok() && node.is_file() {
        let read = read_version(&text.unwrap());
        if read.is_some() {
            local = read.unwrap();
        }
    }

    let latest = latest_version();
    if latest.is_none() {
        eprintln!("No published version found: the node stays as it is");
        return;
    }
    let latest = latest.unwrap();

    if latest <= local {
        return;
    }

    eprintln!("New node version: {} -> {}", version_text(local), version_text(latest));

    let bytes = fetch_signed("node", latest);
    if bytes.is_none() {
        return;
    }

    if !replace(&node, &bytes.unwrap()) {
        eprintln!("The node is in use: it will be updated at the next start");
        return;
    }

    let _ = fs::write(folder.join(LOCAL_VERSION), version_text(latest));
    eprintln!("Node updated to {}", version_text(latest));
}

/// Remplace le node par sa nouvelle version. Faux si l'ancien ne peut pas
/// etre retire (il tourne deja).
fn replace(node: &Path, bytes: &[u8]) -> bool {
    let fresh = node.with_extension("new");
    if fs::write(&fresh, bytes).is_err() {
        return false;
    }

    if node.is_file() && fs::remove_file(node).is_err() {
        let _ = fs::remove_file(&fresh);
        return false;
    }

    if fs::rename(&fresh, node).is_err() {
        return false;
    }

    allow_running(node);

    return true;
}

#[cfg(unix)]
fn allow_running(file: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let _ = fs::set_permissions(file, fs::Permissions::from_mode(0o755));
}

#[cfg(not(unix))]
fn allow_running(_file: &Path) {}

/// Lance le node avec nos arguments, depuis notre dossier, et rend son code.
fn run_node(folder: &Path, arguments: &[String]) -> i32 {
    let node = folder.join(NODE_FILE);
    if !node.is_file() {
        eprintln!("No node in {}", folder.display());
        return 1;
    }

    let status = Command::new(&node).args(arguments).current_dir(folder).status();
    if status.is_err() {
        eprintln!("Failed to start the node: {:?}", status.err());
        return 1;
    }

    let code = status.unwrap().code();
    if code.is_none() {
        return 1;
    }

    return code.unwrap();
}

/// `--sign <version> <dossier>` : ecrit `<fichier>.sig` a cote de chaque exe
/// publie trouve dans le dossier.
fn sign_release(arguments: &[String]) {
    if arguments.len() != 2 {
        eprintln!("Usage: launcher --sign <version> <folder>");
        exit(2);
    }

    let version = read_version(&arguments[0]);
    if version.is_none() {
        eprintln!("Unreadable version: {}", arguments[0]);
        exit(2);
    }
    let version = version.unwrap();

    let key = read_key();
    if key.is_none() {
        eprintln!("No key in {}", MAIN_KEY_FILE);
        exit(2);
    }
    let key = key.unwrap();

    let folder = Path::new(&arguments[1]);
    let mut signed = 0;

    for program in PROGRAMS.iter() {
        for platform in PLATFORMS.iter() {
            let name = format!("{}-{}", program, platform);
            let bytes = fs::read(folder.join(&name));
            if bytes.is_err() {
                eprintln!("Missing: {}", name);
                continue;
            }

            let signature = sign(&key, version, &name, &bytes.unwrap());
            if fs::write(folder.join(format!("{}.sig", name)), signature).is_err() {
                eprintln!("Failed to write {}.sig", name);
                continue;
            }

            signed += 1;
        }
    }

    eprintln!("{} file(s) signed for version {}", signed, version_text(version));
}

fn read_key() -> Option<SigningKey> {
    let text = fs::read_to_string(MAIN_KEY_FILE);
    if text.is_err() {
        return None;
    }

    let secret = key_from_hex(text.unwrap().trim());
    if secret.is_none() {
        return None;
    }

    return Some(main_address::key_from_secret(&secret.unwrap()));
}
