//! A stand-in opponent so the game can be played alone until networking
//! exists. It plays random legal cards until it runs out of energy or room.

use rand::Rng;
use rand::seq::IndexedRandom;

use super::state::{Action, GameState, PlayerId, Target, Zone};

/// Pick one random legal card to play, or `None` when nothing can be played.
pub fn choose_play(game: &GameState, player: PlayerId, rng: &mut impl Rng) -> Option<Action> {
    let targets = [Target::Zone(Zone::A), Target::Zone(Zone::B), Target::Zone(Zone::C), Target::Secret];

    // Every (card, target) pair the rules would allow right now.
    let mut legal_plays = Vec::new();
    for &card in &game.player(player).hand {
        for to in targets {
            if game.check_play(player, card, to).is_ok() {
                legal_plays.push(Action::PlayCard { card, to });
            }
        }
    }
    legal_plays.choose(rng).copied()
}
