//! The rules of the game, as plain Rust.
//!
//! Nothing in this module knows about Bevy, windows, or the mouse. That keeps
//! the rules easy to unit-test, and later a network server can run this exact
//! code to decide what is legal, while each player's client only draws it.

pub mod ai;
mod card;
mod catalogue;
mod deck;
mod event;
mod mission;
mod state;

// Re-export what the rest of the program uses, so it can write
// `crate::game::Zone` instead of `crate::game::state::Zone`.
pub use card::{CardId, Effect, EffectKind};
pub use catalogue::{all_cards, card, card_by_name};
pub use deck::{DECK_SIZE, Deck, default_decks};
pub use event::{GameEvent, Step};
pub use state::{
    Action, ActionError, GameState, MAX_TURNS, Outcome, Phase, PlacedCard, PlayerId, Target, Zone, energy_for_turn,
};
