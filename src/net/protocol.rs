//! The messages the host and the joining player send each other.

use serde::{Deserialize, Serialize};

use super::connection::Connection;
use crate::game::{Action, ActionError, CardId, GameState};

/// The TCP port the host listens on. Change it here if something else on
/// your computer already uses it.
pub const GAME_PORT: u16 = 7777;

/// Bump this whenever the messages or rules change, so two different builds
/// of the game refuse to play together instead of misbehaving.
pub const PROTOCOL_VERSION: u32 = 1;

/// Sent by the joining player.
#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    /// The first message: who is joining, and with which deck.
    Hello { version: u32, name: String, deck: Vec<CardId> },
    /// Something the joining player wants to do.
    Action(Action),
}

/// Sent by the host.
#[derive(Debug, Serialize, Deserialize)]
pub enum HostMessage {
    /// The whole game, including the log of what just happened. `Box` keeps
    /// this enum small, since a `GameState` is large.
    State(Box<GameState>),
    /// The joining player's last action wasn't allowed.
    Rejected(ActionError),
    /// The host won't play with this player (wrong version, invalid deck).
    Refused(String),
}

/// The host's end of the connection: sends `HostMessage`s, receives
/// `ClientMessage`s.
pub type HostConnection = Connection<HostMessage, ClientMessage>;
/// The joining player's end: the other way round.
pub type ClientConnection = Connection<ClientMessage, HostMessage>;

/// A name to show other players: the computer's name, if it can be found.
pub fn player_name() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Player".to_string())
}
