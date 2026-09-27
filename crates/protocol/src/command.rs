use catalog::{Content, Criteria, Server, ServerCriteria};
use wire::{put_text, take_signature, take_text, take_u8};

use crate::encode::{
    put_content, put_criteria, put_server, put_server_criteria, put_share, take_content, take_criteria, take_server,
    take_server_criteria, take_share,
};

const CONTENT_SEARCH: u8 = 1;
const INSTALLED: u8 = 2;
const DOWNLOAD: u8 = 3;
const PROGRESS: u8 = 4;
const UNINSTALL: u8 = 5;
const GET_SHARE: u8 = 6;
const SET_SHARE: u8 = 7;
const GET_DEFAULT_SHARE: u8 = 8;
const SET_DEFAULT_SHARE: u8 = 9;
const SERVERS_SEARCH: u8 = 10;
const CHALLENGE: u8 = 11;
const SIGNED: u8 = 12;
const STOP: u8 = 13;
const STATUS: u8 = 14;

pub enum Subject {
    Content { infohash: String },
    Server { server: Server },
}

/// Ce que le client (ou le serveur) du joueur demande a son node, par la
/// porte locale. Chaque commande recoit une `Answer`.
pub enum Command {
    ContentSearch { criteria: Criteria },
    Installed,
    Download { content: Content },
    Progress { infohash: String },
    Uninstall { infohash: String },
    GetShare { infohash: String },
    SetShare { infohash: String, share: Option<u32> },
    GetDefaultShare,
    SetDefaultShare { share: Option<u32> },
    ServersSearch { criteria: ServerCriteria },
    Challenge { what: Subject },
    Signed { what: Subject, signature: [u8; 64] },
    Stop,
    Status,
}

impl Command {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        match self {
            Command::ContentSearch { criteria } => {
                bytes.push(CONTENT_SEARCH);
                put_criteria(&mut bytes, criteria);
            }
            Command::Installed => bytes.push(INSTALLED),
            Command::Download { content } => {
                bytes.push(DOWNLOAD);
                put_content(&mut bytes, content);
            }
            Command::Progress { infohash } => {
                bytes.push(PROGRESS);
                put_text(&mut bytes, infohash);
            }
            Command::Uninstall { infohash } => {
                bytes.push(UNINSTALL);
                put_text(&mut bytes, infohash);
            }
            Command::GetShare { infohash } => {
                bytes.push(GET_SHARE);
                put_text(&mut bytes, infohash);
            }
            Command::SetShare { infohash, share } => {
                bytes.push(SET_SHARE);
                put_text(&mut bytes, infohash);
                put_share(&mut bytes, *share);
            }
            Command::GetDefaultShare => bytes.push(GET_DEFAULT_SHARE),
            Command::SetDefaultShare { share } => {
                bytes.push(SET_DEFAULT_SHARE);
                put_share(&mut bytes, *share);
            }
            Command::ServersSearch { criteria } => {
                bytes.push(SERVERS_SEARCH);
                put_server_criteria(&mut bytes, criteria);
            }
            Command::Challenge { what } => {
                bytes.push(CHALLENGE);
                put_subject(&mut bytes, what);
            }
            Command::Signed { what, signature } => {
                bytes.push(SIGNED);
                put_subject(&mut bytes, what);
                bytes.extend_from_slice(signature);
            }
            Command::Stop => bytes.push(STOP),
            Command::Status => bytes.push(STATUS),
        }

        return bytes;
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Command> {
        if bytes.is_empty() {
            return None;
        }

        let rest = &bytes[1..];
        let mut at = 0;

        match bytes[0] {
            CONTENT_SEARCH => {
                let criteria = take_criteria(rest, &mut at);
                if criteria.is_none() {
                    return None;
                }
                return Some(Command::ContentSearch { criteria: criteria.unwrap() });
            }
            INSTALLED => return Some(Command::Installed),
            DOWNLOAD => {
                let content = take_content(rest, &mut at);
                if content.is_none() {
                    return None;
                }
                return Some(Command::Download { content: content.unwrap() });
            }
            PROGRESS => {
                let infohash = take_text(rest, &mut at);
                if infohash.is_none() {
                    return None;
                }
                return Some(Command::Progress { infohash: infohash.unwrap() });
            }
            UNINSTALL => {
                let infohash = take_text(rest, &mut at);
                if infohash.is_none() {
                    return None;
                }
                return Some(Command::Uninstall { infohash: infohash.unwrap() });
            }
            GET_SHARE => {
                let infohash = take_text(rest, &mut at);
                if infohash.is_none() {
                    return None;
                }
                return Some(Command::GetShare { infohash: infohash.unwrap() });
            }
            SET_SHARE => {
                let infohash = take_text(rest, &mut at);
                let share = take_share(rest, &mut at);
                if infohash.is_none() || share.is_none() {
                    return None;
                }
                return Some(Command::SetShare { infohash: infohash.unwrap(), share: share.unwrap() });
            }
            GET_DEFAULT_SHARE => return Some(Command::GetDefaultShare),
            SET_DEFAULT_SHARE => {
                let share = take_share(rest, &mut at);
                if share.is_none() {
                    return None;
                }
                return Some(Command::SetDefaultShare { share: share.unwrap() });
            }
            SERVERS_SEARCH => {
                let criteria = take_server_criteria(rest, &mut at);
                if criteria.is_none() {
                    return None;
                }
                return Some(Command::ServersSearch { criteria: criteria.unwrap() });
            }
            CHALLENGE => {
                let what = take_subject(rest, &mut at);
                if what.is_none() {
                    return None;
                }
                return Some(Command::Challenge { what: what.unwrap() });
            }
            SIGNED => {
                let what = take_subject(rest, &mut at);
                let signature = take_signature(rest, &mut at);
                if what.is_none() || signature.is_none() {
                    return None;
                }
                return Some(Command::Signed { what: what.unwrap(), signature: signature.unwrap() });
            }
            STOP => return Some(Command::Stop),
            STATUS => return Some(Command::Status),
            _ => return None,
        }
    }
}

fn put_subject(bytes: &mut Vec<u8>, what: &Subject) {
    match what {
        Subject::Content { infohash } => {
            bytes.push(1);
            put_text(bytes, infohash);
        }
        Subject::Server { server } => {
            bytes.push(2);
            put_server(bytes, server);
        }
    }
}

fn take_subject(bytes: &[u8], at: &mut usize) -> Option<Subject> {
    let kind = take_u8(bytes, at);
    if kind.is_none() {
        return None;
    }

    if kind.unwrap() == 1 {
        let infohash = take_text(bytes, at);
        if infohash.is_none() {
            return None;
        }

        return Some(Subject::Content { infohash: infohash.unwrap() });
    }

    if kind.unwrap() == 2 {
        let server = take_server(bytes, at);
        if server.is_none() {
            return None;
        }

        return Some(Subject::Server { server: server.unwrap() });
    }

    return None;
}
