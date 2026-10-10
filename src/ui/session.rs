//! Who is playing on this screen, and how: against the AI, hosting a LAN
//! game, or joined to someone else's.

use bevy::prelude::*;

use crate::game::{CardId, PlayerId};
use crate::net::{ClientConnection, HostConnection};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayMode {
    #[default]
    VsAi,
    Host,
    Join,
}

/// The mode picked on the main menu, used by the deck select screen.
#[derive(Resource, Default)]
pub struct ChosenMode(pub PlayMode);

/// The deck picked for a LAN game, kept until the game starts.
#[derive(Resource)]
pub struct ChosenDeck(pub Vec<CardId>);

/// Which player sits at this screen. The host (and a game against the AI)
/// is Player One; a player who joins a LAN game is Player Two.
#[derive(Resource, Clone, Copy)]
pub struct Seat {
    pub me: PlayerId,
}

impl Seat {
    pub fn opponent(&self) -> PlayerId {
        self.me.opponent()
    }
}

/// Present while hosting a LAN game: the connection to the other player.
#[derive(Resource)]
pub struct HostLink(pub HostConnection);

/// Present while playing in someone else's LAN game: the connection to
/// the host.
#[derive(Resource)]
pub struct ClientLink(pub ClientConnection);

/// Set when the other player disconnects during a LAN game.
#[derive(Resource, Default)]
pub struct ConnectionLost(pub bool);
