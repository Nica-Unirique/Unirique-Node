use catalog::{Content, Server};
use wire::{put_u16, put_u32, put_u64, take_u16, take_u32, take_u64, take_u8};

use crate::encode::{put_contents, put_servers, put_share, take_contents, take_servers, take_share};

const CONTENTS: u8 = 1;
const SERVERS: u8 = 2;
const DONE: u8 = 3;
const FAILED: u8 = 4;
const TO_SIGN: u8 = 5;
const PROGRESS: u8 = 6;
const SHARE: u8 = 7;
const STATUS: u8 = 8;

/// Pourquoi une commande a echoue. La liste est fixe : le client peut
/// afficher un message clair pour chaque raison.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Reason {
    UnknownContent,
    NoSource,
    BadSignature,
    TooOld,
    Refused,
}

impl Reason {
    fn code(&self) -> u8 {
        match self {
            Reason::UnknownContent => return 1,
            Reason::NoSource => return 2,
            Reason::BadSignature => return 3,
            Reason::TooOld => return 4,
            Reason::Refused => return 5,
        }
    }

    fn of(code: u8) -> Option<Reason> {
        match code {
            1 => return Some(Reason::UnknownContent),
            2 => return Some(Reason::NoSource),
            3 => return Some(Reason::BadSignature),
            4 => return Some(Reason::TooOld),
            5 => return Some(Reason::Refused),
            _ => return None,
        }
    }
}

/// Ce que le node repond a une `Command`.
pub enum Answer {
    Contents { contents: Vec<Content> },
    Servers { servers: Vec<Server> },
    Done,
    Failed { reason: Reason },
    ToSign { bytes: Vec<u8> },
    Progress { done_bytes: u64, total_bytes: u64 },
    Share { share: Option<u32> },
    Status { neighbors: u32, contents_known: u32, servers_known: u32, depth: u8, port: u16 },
}

impl Answer {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        match self {
            Answer::Contents { contents } => {
                bytes.push(CONTENTS);
                put_contents(&mut bytes, contents);
            }
            Answer::Servers { servers } => {
                bytes.push(SERVERS);
                put_servers(&mut bytes, servers);
            }
            Answer::Done => bytes.push(DONE),
            Answer::Failed { reason } => {
                bytes.push(FAILED);
                bytes.push(reason.code());
            }
            Answer::ToSign { bytes: to_sign } => {
                bytes.push(TO_SIGN);
                put_u32(&mut bytes, to_sign.len() as u32);
                bytes.extend_from_slice(to_sign);
            }
            Answer::Progress { done_bytes, total_bytes } => {
                bytes.push(PROGRESS);
                put_u64(&mut bytes, *done_bytes);
                put_u64(&mut bytes, *total_bytes);
            }
            Answer::Share { share } => {
                bytes.push(SHARE);
                put_share(&mut bytes, *share);
            }
            Answer::Status { neighbors, contents_known, servers_known, depth, port } => {
                bytes.push(STATUS);
                put_u32(&mut bytes, *neighbors);
                put_u32(&mut bytes, *contents_known);
                put_u32(&mut bytes, *servers_known);
                bytes.push(*depth);
                put_u16(&mut bytes, *port);
            }
        }

        return bytes;
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Answer> {
        if bytes.is_empty() {
            return None;
        }

        let rest = &bytes[1..];
        let mut at = 0;

        match bytes[0] {
            CONTENTS => {
                let contents = take_contents(rest, &mut at);
                if contents.is_none() {
                    return None;
                }
                return Some(Answer::Contents { contents: contents.unwrap() });
            }
            SERVERS => {
                let servers = take_servers(rest, &mut at);
                if servers.is_none() {
                    return None;
                }
                return Some(Answer::Servers { servers: servers.unwrap() });
            }
            DONE => return Some(Answer::Done),
            FAILED => {
                let code = take_u8(rest, &mut at);
                if code.is_none() {
                    return None;
                }
                let reason = Reason::of(code.unwrap());
                if reason.is_none() {
                    return None;
                }
                return Some(Answer::Failed { reason: reason.unwrap() });
            }
            TO_SIGN => {
                let length = take_u32(rest, &mut at);
                if length.is_none() {
                    return None;
                }
                let length = length.unwrap() as usize;
                if at + length > rest.len() {
                    return None;
                }
                return Some(Answer::ToSign { bytes: rest[at..at + length].to_vec() });
            }
            PROGRESS => {
                let done_bytes = take_u64(rest, &mut at);
                let total_bytes = take_u64(rest, &mut at);
                if done_bytes.is_none() || total_bytes.is_none() {
                    return None;
                }
                return Some(Answer::Progress { done_bytes: done_bytes.unwrap(), total_bytes: total_bytes.unwrap() });
            }
            SHARE => {
                let share = take_share(rest, &mut at);
                if share.is_none() {
                    return None;
                }
                return Some(Answer::Share { share: share.unwrap() });
            }
            STATUS => {
                let neighbors = take_u32(rest, &mut at);
                let contents_known = take_u32(rest, &mut at);
                let servers_known = take_u32(rest, &mut at);
                let depth = take_u8(rest, &mut at);
                let port = take_u16(rest, &mut at);
                if neighbors.is_none() || contents_known.is_none() || servers_known.is_none() {
                    return None;
                }
                if depth.is_none() || port.is_none() {
                    return None;
                }
                return Some(Answer::Status {
                    neighbors: neighbors.unwrap(),
                    contents_known: contents_known.unwrap(),
                    servers_known: servers_known.unwrap(),
                    depth: depth.unwrap(),
                    port: port.unwrap(),
                });
            }
            _ => return None,
        }
    }
}
