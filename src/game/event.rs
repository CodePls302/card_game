//! What happened during the end of a turn, step by step, so the screen can
//! replay it slowly for human players.

use serde::{Deserialize, Serialize};

use super::card::CardId;
use super::state::{GameState, PlayerId, Zone};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameEvent {
    /// Both players ended the turn; cards are about to be revealed.
    TurnEnded,
    /// A Clockwork Ant added a copy of itself.
    Copied {
        player: PlayerId,
        zone: Zone,
        uid: u32,
        card: CardId,
    },
    /// A card moved by its own effect (e.g. Dune Runner).
    Moved {
        player: PlayerId,
        uid: u32,
        card: CardId,
        from: Zone,
        to: Zone,
    },
    Revealed {
        player: PlayerId,
        zone: Zone,
        uid: u32,
        card: CardId,
    },
    /// `player` owned the destroyed card; `by` is the card that did it.
    Destroyed {
        player: PlayerId,
        zone: Zone,
        uid: u32,
        card: CardId,
        by: CardId,
        by_uid: u32,
    },
    SecretFlipped {
        player: PlayerId,
        uid: u32,
        card: CardId,
    },
    MissionCompleted {
        player: PlayerId,
        zone: Zone,
    },
    NewTurn {
        turn: u32,
    },
    GameEnded,
}

/// One event, plus a snapshot of the whole game right after it happened.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Step {
    pub event: GameEvent,
    pub after: GameState,
}
