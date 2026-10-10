use bevy::prelude::*;

use super::Game;
use crate::game::{Action, Phase, ai};
use crate::ui::session::Seat;

/// Lets the stand-in AI play whenever it hasn't finished its turn yet. Only
/// runs in games against the AI; in LAN games the other player's actions
/// arrive through `network.rs` instead.
pub fn take_turn(mut game: ResMut<Game>, seat: Res<Seat>) {
    let opponent = seat.opponent();
    if game.0.phase() != Phase::Playing || game.0.player(opponent).ended_turn {
        return;
    }
    let mut rng = rand::rng();
    // Keep playing random cards until nothing else fits, then end the turn.
    while let Some(action) = ai::choose_play(&game.0, opponent, &mut rng) {
        if let Err(error) = game.0.apply(opponent, action) {
            warn!("Opponent tried {action:?} but {error}");
            break;
        }
    }
    let _ = game.0.apply(opponent, Action::EndTurn);
}
