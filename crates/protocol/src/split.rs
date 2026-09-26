//! Une reponse trop grosse pour un seul message (64 Ko) part en plusieurs
//! morceaux, qui sont recolles a l'arrivee.

use wire::MESSAGE_MAX;

use crate::command::Command;
use crate::message::Message;

impl Message {
    pub fn split(self) -> Vec<Message> {
        let mut pieces = Vec::new();

        match self {
            Message::SendGames { games } => {
                for part in split_list(games, |part| Message::SendGames { games: part }.to_bytes().len()) {
                    pieces.push(Message::SendGames { games: part });
                }
            }
            Message::SendServers { servers } => {
                for part in split_list(servers, |part| Message::SendServers { servers: part }.to_bytes().len()) {
                    pieces.push(Message::SendServers { servers: part });
                }
            }
            other => pieces.push(other),
        }

        return pieces;
    }

    pub fn merge(pieces: Vec<Message>) -> Option<Message> {
        let mut merged: Option<Message> = None;

        for piece in pieces {
            merged = match (merged, piece) {
                (Some(Message::SendGames { games: mut all }), Message::SendGames { games }) => {
                    all.extend(games);
                    Some(Message::SendGames { games: all })
                }
                (Some(Message::SendServers { servers: mut all }), Message::SendServers { servers }) => {
                    all.extend(servers);
                    Some(Message::SendServers { servers: all })
                }
                (None, piece) => Some(piece),
                (Some(first), _) => Some(first),
            };
        }

        return merged;
    }
}

impl Command {
    pub fn split(self) -> Vec<Command> {
        let mut pieces = Vec::new();

        match self {
            Command::Games { games } => {
                for part in split_list(games, |part| Command::Games { games: part }.to_bytes().len()) {
                    pieces.push(Command::Games { games: part });
                }
            }
            Command::Servers { servers } => {
                for part in split_list(servers, |part| Command::Servers { servers: part }.to_bytes().len()) {
                    pieces.push(Command::Servers { servers: part });
                }
            }
            other => pieces.push(other),
        }

        return pieces;
    }

    pub fn merge(pieces: Vec<Command>) -> Option<Command> {
        let mut merged: Option<Command> = None;

        for piece in pieces {
            merged = match (merged, piece) {
                (Some(Command::Games { games: mut all }), Command::Games { games }) => {
                    all.extend(games);
                    Some(Command::Games { games: all })
                }
                (Some(Command::Servers { servers: mut all }), Command::Servers { servers }) => {
                    all.extend(servers);
                    Some(Command::Servers { servers: all })
                }
                (None, piece) => Some(piece),
                (Some(first), _) => Some(first),
            };
        }

        return merged;
    }
}

/// Coupe une liste en paquets dont chacun tient dans un message.
///
/// `bytes_of` donne la taille du message qui porterait cette liste. Un
/// element trop gros pour tenir seul dans un message est abandonne.
/// Il y a toujours au moins un paquet, meme vide : une reponse vide reste
/// une reponse.
fn split_list<T: Clone>(items: Vec<T>, bytes_of: impl Fn(Vec<T>) -> usize) -> Vec<Vec<T>> {
    let most = MESSAGE_MAX as usize;
    let empty = bytes_of(Vec::new());

    let mut parts = Vec::new();
    let mut current = Vec::new();
    let mut size = empty;

    for item in items {
        let item_size = bytes_of(vec![item.clone()]) - empty;

        if empty + item_size > most {
            continue;
        }

        if size + item_size > most {
            parts.push(current);
            current = Vec::new();
            size = empty;
        }

        current.push(item);
        size += item_size;
    }

    parts.push(current);

    return parts;
}
