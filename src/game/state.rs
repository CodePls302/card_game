use std::fmt;

use rand::rngs::StdRng;
use rand::seq::{IndexedRandom, SliceRandom};
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use super::card::{CardId, Effect};
use super::catalogue::card;
use super::event::{GameEvent, Step};
use super::mission::{ALL_MISSIONS, Mission};

pub const STARTING_HAND: usize = 8;
pub const MAX_TURNS: u32 = 8;
/// How many cards one player can have at a single zone.
pub const ZONE_CAPACITY: usize = 5;
/// How many cards fit in each player's secret zone.
pub const SECRET_CAPACITY: usize = 3;

/// Energy available on a turn: 2 on turn 1, up to 9 on turn 8. Unused
/// energy is lost at the end of the turn.
pub fn energy_for_turn(turn: u32) -> u32 {
    turn + 1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlayerId {
    One,
    Two,
}

impl PlayerId {
    pub const BOTH: [PlayerId; 2] = [PlayerId::One, PlayerId::Two];

    /// Position of this player in `GameState::players`.
    pub fn index(self) -> usize {
        match self {
            PlayerId::One => 0,
            PlayerId::Two => 1,
        }
    }

    pub fn opponent(self) -> PlayerId {
        match self {
            PlayerId::One => PlayerId::Two,
            PlayerId::Two => PlayerId::One,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Zone {
    A,
    B,
    C,
}

impl Zone {
    pub const ALL: [Zone; 3] = [Zone::A, Zone::B, Zone::C];

    pub fn index(self) -> usize {
        match self {
            Zone::A => 0,
            Zone::B => 1,
            Zone::C => 2,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Zone::A => "A",
            Zone::B => "B",
            Zone::C => "C",
        }
    }
}

/// Where a card from the hand can be played. An enum variant can carry data:
/// `Target::Zone(Zone::B)` means "zone B".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    Zone(Zone),
    /// The player's own secret zone.
    Secret,
}

/// A card on the board. Each one gets its own `uid` (unique id) when it
/// arrives, so two copies of the same design can be told apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacedCard {
    pub uid: u32,
    pub card: CardId,
    /// Face-up. Cards in zones flip when the turn ends; Secret cards only
    /// when their condition is met.
    pub revealed: bool,
    /// Played from the hand this turn, so it can still be taken back.
    pub played_this_turn: bool,
    /// Made by an effect rather than played from the hand.
    pub is_copy: bool,
    pub moved_this_turn: bool,
}

impl PlacedCard {
    fn new(uid: u32, card: CardId, revealed: bool) -> Self {
        PlacedCard { uid, card, revealed, played_this_turn: false, is_copy: false, moved_this_turn: false }
    }
}

/// Everything belonging to one player.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerState {
    /// The top of the deck is the *end* of the Vec, so drawing is `pop()`.
    pub deck: Vec<CardId>,
    pub hand: Vec<CardId>,
    /// This player's side of each zone, indexed by `Zone::index()`.
    pub zones: [Vec<PlacedCard>; 3],
    /// Face-down Secret cards. They add no power to any zone.
    pub secrets: Vec<PlacedCard>,
    /// This player's cards that were destroyed.
    pub graveyard: Vec<CardId>,
    /// Energy left to spend this turn.
    pub energy: u32,
    /// The turn on which this player can see the opponent's hand, if any.
    pub sees_opponent_hand_on: Option<u32>,
    pub ended_turn: bool,
}

impl PlayerState {
    fn draw(&mut self) {
        // `pop` returns `None` on an empty deck, so drawing from an empty
        // deck simply does nothing.
        if let Some(card) = self.deck.pop() {
            self.hand.push(card);
        }
    }

    pub fn cards_at(&self, zone: Zone) -> &[PlacedCard] {
        &self.zones[zone.index()]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Playing,
    GameOver,
}

/// Something a player asks to do. All changes to the game go through
/// `GameState::apply`, which makes this the message the network layer will
/// send later.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    PlayCard {
        card: CardId,
        to: Target,
    },
    /// Take back a card played this turn.
    ReturnCard {
        uid: u32,
    },
    /// Move a card already on the board (e.g. Cloud Whale) to another zone.
    MoveCard {
        uid: u32,
        to: Zone,
    },
    EndTurn,
}

/// Why an action was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionError {
    GameOver,
    TurnAlreadyEnded,
    CardNotInHand,
    ZoneFull,
    SecretZoneOnly,
    NotASecret,
    CannotMove,
    CannotReturn,
    NotEnoughEnergy,
}

// Implementing `Display` lets an error be printed with `{}` in messages.
impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let message = match self {
            ActionError::GameOver => "the game is over",
            ActionError::TurnAlreadyEnded => "you already ended your turn",
            ActionError::CardNotInHand => "that card is not in your hand",
            ActionError::ZoneFull => "that zone is full",
            ActionError::SecretZoneOnly => "secret cards can only go in your secret zone",
            ActionError::NotASecret => "only secret cards can go in the secret zone",
            ActionError::CannotMove => "that card can't move there right now",
            ActionError::CannotReturn => "only cards played this turn can go back to your hand",
            ActionError::NotEnoughEnergy => "not enough energy",
        };
        f.write_str(message)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Winner(PlayerId),
    Draw,
}

/// The whole game. Both players take their turn at the same time; a turn
/// ends once both have pressed End Turn.
/// `Serialize`/`Deserialize` let the host send the whole game to the other
/// player over the network.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameState {
    turn: u32,
    phase: Phase,
    players: [PlayerState; 2],
    /// The mission at each zone, picked at random when the game starts.
    zone_missions: [Mission; 3],
    /// `missions_done[player][zone]`: has that player completed it?
    missions_done: [[bool; 3]; 2],
    /// `destroyed_enemy_at[player][zone]`: has that player destroyed an
    /// opponent's card there? Used by the "destroy an enemy card" mission.
    destroyed_enemy_at: [[bool; 3]; 2],
    /// The last uid handed out to a `PlacedCard`.
    last_uid: u32,
    /// Randomness for effects like Dune Runner. Kept inside the game, seeded
    /// once, so the same seed always plays out the same way. It isn't sent
    /// over the network: only the host runs effects, so the other player's
    /// copy gets a fresh one it never uses.
    #[serde(skip, default = "StdRng::from_os_rng")]
    rng: StdRng,
    /// What happened at the end of the last turn, for the screen to replay.
    log: Vec<Step>,
}

impl GameState {
    /// Start a game: shuffle each deck, pick the zone missions, and deal the
    /// opening hands. The random number generator is passed in so tests can
    /// use a fixed seed.
    pub fn new(deck_one: Vec<CardId>, deck_two: Vec<CardId>, rng: &mut impl Rng) -> Self {
        let mut players = [deck_one, deck_two].map(|mut deck| {
            deck.shuffle(rng);
            PlayerState { deck, ..Default::default() }
        });
        for player in &mut players {
            for _ in 0..STARTING_HAND {
                player.draw();
            }
            player.energy = energy_for_turn(1);
        }
        let mut zone_missions = ALL_MISSIONS;
        zone_missions.shuffle(rng);

        GameState {
            turn: 1,
            phase: Phase::Playing,
            players,
            zone_missions,
            missions_done: [[false; 3]; 2],
            destroyed_enemy_at: [[false; 3]; 2],
            last_uid: 0,
            rng: StdRng::from_rng(rng),
            log: Vec::new(),
        }
    }

    pub fn turn(&self) -> u32 {
        self.turn
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn player(&self, id: PlayerId) -> &PlayerState {
        &self.players[id.index()]
    }

    pub fn zone_mission(&self, zone: Zone) -> Mission {
        self.zone_missions[zone.index()]
    }

    /// Has `player` completed the mission at `zone`?
    pub fn mission_done(&self, player: PlayerId, zone: Zone) -> bool {
        self.missions_done[player.index()][zone.index()]
    }

    /// Can `player` see the opponent's hand right now?
    pub fn sees_opponent_hand(&self, player: PlayerId) -> bool {
        self.player(player).sees_opponent_hand_on == Some(self.turn)
    }

    /// Hand over everything that happened at the end of the last turn, and
    /// forget it.
    pub fn take_log(&mut self) -> Vec<Step> {
        std::mem::take(&mut self.log)
    }

    pub fn has_log(&self) -> bool {
        !self.log.is_empty()
    }

    /// The single entry point for changing the game.
    pub fn apply(&mut self, player: PlayerId, action: Action) -> Result<(), ActionError> {
        if self.phase == Phase::GameOver {
            return Err(ActionError::GameOver);
        }
        if self.player(player).ended_turn {
            return Err(ActionError::TurnAlreadyEnded);
        }
        match action {
            Action::PlayCard { card, to } => self.play_card(player, card, to),
            Action::ReturnCard { uid } => self.return_card(player, uid),
            Action::MoveCard { uid, to } => self.move_card(player, uid, to),
            Action::EndTurn => {
                self.players[player.index()].ended_turn = true;
                if self.players.iter().all(|p| p.ended_turn) {
                    self.finish_turn();
                }
                Ok(())
            }
        }
    }

    /// Would playing `id` to `to` be allowed? Changes nothing, so the AI and
    /// the UI can ask before trying.
    pub fn check_play(&self, player: PlayerId, id: CardId, to: Target) -> Result<(), ActionError> {
        let state = self.player(player);
        if !state.hand.contains(&id) {
            return Err(ActionError::CardNotInHand);
        }
        if card(id).cost > state.energy {
            return Err(ActionError::NotEnoughEnergy);
        }
        let secret = card(id).is_secret();
        // A `match` can test several things at once; `if` after a pattern is
        // called a "match guard". The first arm that fits wins.
        match to {
            Target::Secret if !secret => Err(ActionError::NotASecret),
            Target::Secret if state.secrets.len() >= SECRET_CAPACITY => Err(ActionError::ZoneFull),
            Target::Zone(_) if secret => Err(ActionError::SecretZoneOnly),
            Target::Zone(zone) if state.cards_at(zone).len() >= ZONE_CAPACITY => Err(ActionError::ZoneFull),
            _ => Ok(()),
        }
    }

    fn play_card(&mut self, player: PlayerId, id: CardId, to: Target) -> Result<(), ActionError> {
        // `?` returns early with the error if the play isn't allowed.
        self.check_play(player, id, to)?;
        let placed = PlacedCard { played_this_turn: true, ..PlacedCard::new(self.next_uid(), id, false) };

        let state = &mut self.players[player.index()];
        state.energy -= card(id).cost;
        if let Some(position) = state.hand.iter().position(|&c| c == id) {
            state.hand.remove(position);
        }
        match to {
            Target::Secret => state.secrets.push(placed),
            Target::Zone(zone) => state.zones[zone.index()].push(placed),
        }
        Ok(())
    }

    fn return_card(&mut self, player: PlayerId, uid: u32) -> Result<(), ActionError> {
        let state = &mut self.players[player.index()];
        // Look through every zone and the secret zone for this turn's card.
        let places = state.zones.iter_mut().chain(std::iter::once(&mut state.secrets));
        let mut taken = None;
        for cards in places {
            if let Some(index) = cards.iter().position(|p| p.uid == uid && p.played_this_turn) {
                taken = Some(cards.remove(index));
                break;
            }
        }
        let placed = taken.ok_or(ActionError::CannotReturn)?;
        state.hand.push(placed.card);
        state.energy += card(placed.card).cost;
        Ok(())
    }

    fn move_card(&mut self, player: PlayerId, uid: u32, to: Zone) -> Result<(), ActionError> {
        let (from, index) = self.locate(player, uid).ok_or(ActionError::CannotMove)?;
        let state = &mut self.players[player.index()];
        let placed = state.zones[from.index()][index];

        let movable = card(placed.card).effect == Effect::Movable;
        if !movable || !placed.revealed || placed.moved_this_turn || from == to {
            return Err(ActionError::CannotMove);
        }
        if state.zones[to.index()].len() >= ZONE_CAPACITY {
            return Err(ActionError::ZoneFull);
        }
        state.zones[from.index()].remove(index);
        // `..placed` copies every other field from `placed`.
        state.zones[to.index()].push(PlacedCard { moved_this_turn: true, ..placed });
        Ok(())
    }

    /// Both players have ended the turn. Run triggers, reveal what was
    /// played, check missions, then start the next turn or end the game.
    /// Every step is recorded in the log so the screen can replay it.
    fn finish_turn(&mut self) {
        self.record(GameEvent::TurnEnded);
        self.run_triggers();

        // Reveal this turn's cards one at a time, running Flip effects as
        // they go. Player One goes first on odd turns, Player Two on even
        // turns; each player's cards reveal in the order they were played.
        let first = if self.turn % 2 == 1 { PlayerId::One } else { PlayerId::Two };
        for player in [first, first.opponent()] {
            let mut to_reveal: Vec<u32> = self.players[player.index()]
                .zones
                .iter()
                .flatten()
                .filter(|placed| !placed.revealed)
                .map(|placed| placed.uid)
                .collect();
            // uids count up as cards are played, so sorting gives play order.
            to_reveal.sort();
            for uid in to_reveal {
                // An earlier reveal may have destroyed this card.
                let Some((zone, index)) = self.locate(player, uid) else { continue };
                let placed = &mut self.players[player.index()].zones[zone.index()][index];
                placed.revealed = true;
                let card = placed.card;
                self.record(GameEvent::Revealed { player, zone, uid, card });
                self.resolve_flip(player, zone, uid);
                self.after_reveal(player, uid);
                self.check_missions();
            }
        }

        for player in &mut self.players {
            player.ended_turn = false;
            for placed in player.zones.iter_mut().flatten().chain(player.secrets.iter_mut()) {
                placed.moved_this_turn = false;
                placed.played_this_turn = false;
            }
        }

        if self.turn >= MAX_TURNS {
            self.phase = Phase::GameOver;
            self.record(GameEvent::GameEnded);
            return;
        }
        self.turn += 1;
        for player in &mut self.players {
            player.draw();
            player.energy = energy_for_turn(self.turn);
        }
        self.record(GameEvent::NewTurn { turn: self.turn });
        self.flip_start_of_turn_secrets();
    }

    /// Trigger effects of cards that were already face-up this turn.
    fn run_triggers(&mut self) {
        for player in PlayerId::BOTH {
            // Each original Clockwork Ant adds one copy of itself.
            for zone in Zone::ALL {
                let copiers: Vec<CardId> = self
                    .player(player)
                    .cards_at(zone)
                    .iter()
                    .filter(|p| p.revealed && !p.is_copy && card(p.card).effect == Effect::CopyEachTurn)
                    .map(|p| p.card)
                    .collect();
                for id in copiers {
                    if self.player(player).cards_at(zone).len() < ZONE_CAPACITY {
                        let uid = self.next_uid();
                        let copy = PlacedCard { is_copy: true, ..PlacedCard::new(uid, id, true) };
                        self.players[player.index()].zones[zone.index()].push(copy);
                        self.record(GameEvent::Copied { player, zone, uid, card: id });
                    }
                }
            }

            // Each Dune Runner moves to a random other zone with room.
            let wanderers: Vec<u32> = self.players[player.index()]
                .zones
                .iter()
                .flatten()
                .filter(|p| p.revealed && card(p.card).effect == Effect::Wander)
                .map(|p| p.uid)
                .collect();
            for uid in wanderers {
                let Some((from, index)) = self.locate(player, uid) else { continue };
                let open: Vec<Zone> = Zone::ALL
                    .into_iter()
                    .filter(|&zone| zone != from && self.player(player).cards_at(zone).len() < ZONE_CAPACITY)
                    .collect();
                if let Some(&to) = open.choose(&mut self.rng) {
                    let zones = &mut self.players[player.index()].zones;
                    let placed = zones[from.index()].remove(index);
                    zones[to.index()].push(placed);
                    self.record(GameEvent::Moved { player, uid, card: placed.card, from, to });
                }
            }
        }
        self.check_missions();
    }

    /// Run a card's Flip effect, if it has one, as it turns face-up.
    fn resolve_flip(&mut self, player: PlayerId, zone: Zone, uid: u32) {
        let Some((_, index)) = self.locate(player, uid) else { return };
        let placed = self.player(player).cards_at(zone)[index];

        match card(placed.card).effect {
            Effect::DestroyWeaker => {
                let power = self.card_power(player, zone, index);
                // Pick every target before destroying any: destroying a card
                // can change the power of the others (e.g. Hollow King).
                let mut doomed = Vec::new();
                for owner in PlayerId::BOTH {
                    for (i, other) in self.player(owner).cards_at(zone).iter().enumerate() {
                        if other.uid != uid && other.revealed && self.card_power(owner, zone, i) < power {
                            doomed.push(other.uid);
                        }
                    }
                }
                for target in doomed {
                    self.destroy(target, player, placed);
                }
            }
            // Other effects do nothing at the moment the card flips.
            _ => {}
        }
    }

    /// A card just turned face-up. Give the opponent's secrets a chance to
    /// react to it.
    fn after_reveal(&mut self, owner: PlayerId, uid: u32) {
        let Some((zone, index)) = self.locate(owner, uid) else { return };
        let power = self.card_power(owner, zone, index);
        let enemy = owner.opponent();

        let enemy_secrets = &mut self.players[enemy.index()].secrets;
        // `matches!` checks a value against a pattern and gives a bool.
        let ambush = enemy_secrets.iter_mut().find(|secret| {
            !secret.revealed
                && matches!(card(secret.card).effect, Effect::AmbushStrong(threshold) if power >= threshold)
        });
        if let Some(secret) = ambush {
            secret.revealed = true;
            let secret = *secret;
            self.record(GameEvent::SecretFlipped { player: enemy, uid: secret.uid, card: secret.card });
            self.destroy(uid, enemy, secret);
        }
    }

    /// Secrets whose condition is "your next turn starts" (Night Owl).
    fn flip_start_of_turn_secrets(&mut self) {
        for player in PlayerId::BOTH {
            let turn = self.turn;
            let state = &mut self.players[player.index()];
            let mut flipped = Vec::new();
            for secret in &mut state.secrets {
                if !secret.revealed && card(secret.card).effect == Effect::RevealHand {
                    secret.revealed = true;
                    state.sees_opponent_hand_on = Some(turn);
                    flipped.push(*secret);
                }
            }
            for secret in flipped {
                self.record(GameEvent::SecretFlipped { player, uid: secret.uid, card: secret.card });
            }
        }
    }

    /// Mark any newly completed missions.
    fn check_missions(&mut self) {
        for player in PlayerId::BOTH {
            for zone in Zone::ALL {
                if self.mission_done(player, zone) {
                    continue;
                }
                let done = match self.zone_mission(zone) {
                    Mission::FillZone => self.player(player).cards_at(zone).len() >= ZONE_CAPACITY,
                    Mission::DestroyEnemyHere => self.destroyed_enemy_at[player.index()][zone.index()],
                    Mission::ReachPower(target) => self.zone_power(player, zone) >= target,
                };
                if done {
                    self.missions_done[player.index()][zone.index()] = true;
                    self.record(GameEvent::MissionCompleted { player, zone });
                }
            }
        }
    }

    /// Remove a card from the board and put it in its owner's graveyard.
    /// `by_player` and `by` say whose card did it.
    fn destroy(&mut self, uid: u32, by_player: PlayerId, by: PlacedCard) {
        for owner in PlayerId::BOTH {
            let Some((zone, index)) = self.locate(owner, uid) else { continue };
            let destroyed = self.players[owner.index()].zones[zone.index()].remove(index);
            self.players[owner.index()].graveyard.push(destroyed.card);
            if owner != by_player {
                self.destroyed_enemy_at[by_player.index()][zone.index()] = true;
            }
            let card = destroyed.card;
            self.record(GameEvent::Destroyed { player: owner, zone, uid, card, by: by.card, by_uid: by.uid });
            return;
        }
    }

    /// Add an event to the log, with a snapshot of the game as it is now.
    fn record(&mut self, event: GameEvent) {
        // Take the log out first so the snapshot doesn't copy it too.
        let mut log = std::mem::take(&mut self.log);
        log.push(Step { event, after: self.clone() });
        self.log = log;
    }

    fn next_uid(&mut self) -> u32 {
        self.last_uid += 1;
        self.last_uid
    }

    /// Find a player's card in the zones: which zone, and where in it.
    pub fn locate(&self, player: PlayerId, uid: u32) -> Option<(Zone, usize)> {
        Zone::ALL.into_iter().find_map(|zone| {
            let index = self.player(player).cards_at(zone).iter().position(|p| p.uid == uid)?;
            Some((zone, index))
        })
    }

    /// The power of one card in a zone, including bonuses from Passive
    /// effects of the owner's other face-up cards there.
    pub fn card_power(&self, owner: PlayerId, zone: Zone, index: usize) -> i32 {
        let cards = self.player(owner).cards_at(zone);
        let mut power = card(cards[index].card).power;
        for (i, other) in cards.iter().enumerate() {
            if i != index && other.revealed {
                if let Effect::EmpowerZone(bonus) = card(other.card).effect {
                    power += bonus;
                }
            }
        }
        power
    }

    /// A player's total power at a zone. Only revealed cards count, so the
    /// score never gives away cards the opponent has not seen yet.
    pub fn zone_power(&self, player: PlayerId, zone: Zone) -> i32 {
        self.player(player)
            .cards_at(zone)
            .iter()
            .enumerate()
            .filter(|(_, placed)| placed.revealed)
            .map(|(index, _)| self.card_power(player, zone, index))
            .sum()
    }

    /// Who is winning a zone right now, if anyone. A player who has
    /// completed their mission there beats one who hasn't, whatever the
    /// power. Otherwise (both or neither complete), higher power wins.
    pub fn zone_winner(&self, zone: Zone) -> Option<PlayerId> {
        let one = PlayerId::One;
        let two = PlayerId::Two;
        match (self.mission_done(one, zone), self.mission_done(two, zone)) {
            (true, false) => Some(one),
            (false, true) => Some(two),
            _ => match self.zone_power(one, zone).cmp(&self.zone_power(two, zone)) {
                std::cmp::Ordering::Greater => Some(one),
                std::cmp::Ordering::Less => Some(two),
                std::cmp::Ordering::Equal => None,
            },
        }
    }

    /// Who won, or `None` while the game is still going. Whoever wins more
    /// zones wins; if that is even, total power across all zones decides.
    pub fn outcome(&self) -> Option<Outcome> {
        if self.phase != Phase::GameOver {
            return None;
        }
        let (mut zones_one, mut zones_two, mut total_one, mut total_two) = (0, 0, 0, 0);
        for zone in Zone::ALL {
            total_one += self.zone_power(PlayerId::One, zone);
            total_two += self.zone_power(PlayerId::Two, zone);
            match self.zone_winner(zone) {
                Some(PlayerId::One) => zones_one += 1,
                Some(PlayerId::Two) => zones_two += 1,
                None => {}
            }
        }
        // Compare as tuples: zones first, total power as the tie-breaker.
        Some(match (zones_one, total_one).cmp(&(zones_two, total_two)) {
            std::cmp::Ordering::Greater => Outcome::Winner(PlayerId::One),
            std::cmp::Ordering::Less => Outcome::Winner(PlayerId::Two),
            std::cmp::Ordering::Equal => Outcome::Draw,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::all_cards;

    const EMBER_SPRITE: CardId = CardId(0); // cost 1, power 1
    const STONE_GOLEM: CardId = CardId(1); // cost 4, power 5
    const NIGHT_OWL: CardId = CardId(3); // secret: see the enemy hand
    const MOSS_TROLL: CardId = CardId(6); // power 6, destroys weaker cards
    const DUNE_RUNNER: CardId = CardId(8); // moves every turn
    const CORAL_GIANT: CardId = CardId(12); // power 6
    const CLOCKWORK_ANT: CardId = CardId(15);
    const LANTERN_GHOST: CardId = CardId(16); // secret: ambushes 5+ power
    const GRANITE_BEAR: CardId = CardId(18); // power 10
    const CLOUD_WHALE: CardId = CardId(20); // movable
    const HOLLOW_KING: CardId = CardId(23); // others +2

    const ONE: PlayerId = PlayerId::One;
    const TWO: PlayerId = PlayerId::Two;

    fn new_game() -> GameState {
        let mut rng = StdRng::seed_from_u64(7);
        GameState::new(all_cards(), all_cards(), &mut rng)
    }

    /// Replace a player's hand, so a test knows exactly what they hold, and
    /// give them plenty of energy so cost doesn't get in the way.
    fn give(game: &mut GameState, player: PlayerId, cards: &[CardId]) {
        game.players[player.index()].hand = cards.to_vec();
        game.players[player.index()].energy = 99;
    }

    fn play(game: &mut GameState, player: PlayerId, card: CardId, to: Target) -> Result<(), ActionError> {
        game.apply(player, Action::PlayCard { card, to })
    }

    fn end_turn_for_both(game: &mut GameState) {
        game.apply(ONE, Action::EndTurn).unwrap();
        game.apply(TWO, Action::EndTurn).unwrap();
    }

    fn names_at(game: &GameState, player: PlayerId, zone: Zone) -> Vec<&'static str> {
        game.player(player).cards_at(zone).iter().map(|p| card(p.card).name).collect()
    }

    #[test]
    fn players_start_with_eight_cards() {
        let game = new_game();
        for id in PlayerId::BOTH {
            assert_eq!(game.player(id).hand.len(), 8);
            assert_eq!(game.player(id).deck.len(), 17);
        }
    }

    #[test]
    fn each_new_turn_draws_one_card() {
        let mut game = new_game();
        end_turn_for_both(&mut game);
        assert_eq!(game.turn(), 2);
        assert_eq!(game.player(ONE).hand.len(), 9);
    }

    #[test]
    fn turn_waits_for_both_players() {
        let mut game = new_game();
        game.apply(ONE, Action::EndTurn).unwrap();
        assert_eq!(game.turn(), 1);
        assert_eq!(game.apply(ONE, Action::EndTurn), Err(ActionError::TurnAlreadyEnded));
    }

    #[test]
    fn game_ends_after_eight_turns() {
        let mut game = new_game();
        for _ in 0..MAX_TURNS {
            assert_eq!(game.phase(), Phase::Playing);
            end_turn_for_both(&mut game);
        }
        assert_eq!(game.phase(), Phase::GameOver);
        assert_eq!(game.apply(ONE, Action::EndTurn), Err(ActionError::GameOver));
    }

    #[test]
    fn every_zone_gets_a_different_mission() {
        let game = new_game();
        let missions: Vec<Mission> = Zone::ALL.into_iter().map(|zone| game.zone_mission(zone)).collect();
        for mission in ALL_MISSIONS {
            assert!(missions.contains(&mission), "{mission:?} missing");
        }
    }

    #[test]
    fn energy_is_turn_plus_one_and_does_not_carry_over() {
        let mut game = new_game();
        game.players[0].hand = vec![EMBER_SPRITE, STONE_GOLEM];
        assert_eq!(game.player(ONE).energy, 2);
        assert_eq!(play(&mut game, ONE, STONE_GOLEM, Target::Zone(Zone::A)), Err(ActionError::NotEnoughEnergy));
        play(&mut game, ONE, EMBER_SPRITE, Target::Zone(Zone::A)).unwrap();
        assert_eq!(game.player(ONE).energy, 1);

        end_turn_for_both(&mut game);
        assert_eq!(game.player(ONE).energy, 3, "turn 2 gives 3, leftover is lost");
        end_turn_for_both(&mut game);
        play(&mut game, ONE, STONE_GOLEM, Target::Zone(Zone::A)).unwrap();
        assert_eq!(game.player(ONE).energy, 0);
    }

    #[test]
    fn cards_can_be_taken_back_until_the_turn_ends() {
        let mut game = new_game();
        give(&mut game, ONE, &[STONE_GOLEM, NIGHT_OWL]);
        play(&mut game, ONE, STONE_GOLEM, Target::Zone(Zone::A)).unwrap();
        play(&mut game, ONE, NIGHT_OWL, Target::Secret).unwrap();
        let golem = game.player(ONE).cards_at(Zone::A)[0].uid;
        let owl = game.player(ONE).secrets[0].uid;

        game.apply(ONE, Action::ReturnCard { uid: owl }).unwrap();
        assert_eq!(game.player(ONE).hand, [NIGHT_OWL]);
        assert_eq!(game.player(ONE).energy, 99 - 4, "the owl's cost is refunded");

        end_turn_for_both(&mut game);
        assert_eq!(game.apply(ONE, Action::ReturnCard { uid: golem }), Err(ActionError::CannotReturn));
    }

    #[test]
    fn played_cards_score_once_revealed() {
        let mut game = new_game();
        give(&mut game, ONE, &[STONE_GOLEM]);
        play(&mut game, ONE, STONE_GOLEM, Target::Zone(Zone::B)).unwrap();
        assert!(game.player(ONE).hand.is_empty());
        assert_eq!(game.zone_power(ONE, Zone::B), 0, "hidden until the turn ends");

        end_turn_for_both(&mut game);
        assert_eq!(game.zone_power(ONE, Zone::B), 5);
    }

    #[test]
    fn cannot_play_a_card_twice() {
        let mut game = new_game();
        give(&mut game, ONE, &[STONE_GOLEM]);
        play(&mut game, ONE, STONE_GOLEM, Target::Zone(Zone::A)).unwrap();
        assert_eq!(play(&mut game, ONE, STONE_GOLEM, Target::Zone(Zone::A)), Err(ActionError::CardNotInHand));
    }

    #[test]
    fn zones_hold_five_cards() {
        let mut game = new_game();
        let hand: Vec<CardId> = (0..6).map(CardId).filter(|&id| id != NIGHT_OWL).collect();
        give(&mut game, ONE, &hand);
        for &card in &hand[..5] {
            play(&mut game, ONE, card, Target::Zone(Zone::C)).unwrap();
        }
        assert_eq!(play(&mut game, ONE, CORAL_GIANT, Target::Zone(Zone::C)), Err(ActionError::CardNotInHand));
        give(&mut game, ONE, &[CORAL_GIANT]);
        assert_eq!(play(&mut game, ONE, CORAL_GIANT, Target::Zone(Zone::C)), Err(ActionError::ZoneFull));
    }

    #[test]
    fn empower_zone_boosts_other_cards() {
        let mut game = new_game();
        game.players[0].zones[0] = vec![
            PlacedCard::new(1, HOLLOW_KING, true),
            PlacedCard::new(2, STONE_GOLEM, true),
            PlacedCard::new(3, EMBER_SPRITE, true),
        ];
        assert_eq!(game.card_power(ONE, Zone::A, 0), 0, "the king doesn't boost itself");
        assert_eq!(game.card_power(ONE, Zone::A, 1), 7);
        assert_eq!(game.zone_power(ONE, Zone::A), 0 + 7 + 3);
    }

    #[test]
    fn moss_troll_flips_at_end_of_turn_and_destroys_weaker_cards_on_both_sides() {
        let mut game = new_game();
        game.players[0].zones[0] = vec![PlacedCard::new(101, EMBER_SPRITE, true)];
        game.players[1].zones[0] = vec![
            PlacedCard::new(102, STONE_GOLEM, true),
            PlacedCard::new(103, CORAL_GIANT, true), // equal power: survives
        ];
        give(&mut game, ONE, &[MOSS_TROLL]);
        play(&mut game, ONE, MOSS_TROLL, Target::Zone(Zone::A)).unwrap();
        assert!(!game.player(ONE).cards_at(Zone::A)[1].revealed, "stays face-down until the turn ends");
        assert_eq!(names_at(&game, TWO, Zone::A), ["Stone Golem", "Coral Giant"], "nothing happens yet");

        end_turn_for_both(&mut game);
        assert_eq!(names_at(&game, ONE, Zone::A), ["Moss Troll"]);
        assert_eq!(names_at(&game, TWO, Zone::A), ["Coral Giant"]);
        assert_eq!(game.player(ONE).graveyard, [EMBER_SPRITE]);
        assert_eq!(game.player(TWO).graveyard, [STONE_GOLEM]);
    }

    #[test]
    fn reveal_order_alternates_each_turn() {
        // Turn 1 is odd, so Player One reveals first: their Ember Sprite is
        // face-up by the time Player Two's Moss Troll flips, and gets destroyed.
        let mut game = new_game();
        give(&mut game, ONE, &[EMBER_SPRITE]);
        give(&mut game, TWO, &[MOSS_TROLL]);
        play(&mut game, ONE, EMBER_SPRITE, Target::Zone(Zone::A)).unwrap();
        play(&mut game, TWO, MOSS_TROLL, Target::Zone(Zone::A)).unwrap();
        end_turn_for_both(&mut game);
        assert!(names_at(&game, ONE, Zone::A).is_empty());

        // Turn 2 is even, so Player Two reveals first: the troll flips while
        // the sprite is still face-down, and the sprite survives.
        let mut game = new_game();
        end_turn_for_both(&mut game);
        give(&mut game, ONE, &[EMBER_SPRITE]);
        give(&mut game, TWO, &[MOSS_TROLL]);
        play(&mut game, ONE, EMBER_SPRITE, Target::Zone(Zone::A)).unwrap();
        play(&mut game, TWO, MOSS_TROLL, Target::Zone(Zone::A)).unwrap();
        end_turn_for_both(&mut game);
        assert_eq!(names_at(&game, ONE, Zone::A), ["Ember Sprite"]);
    }

    #[test]
    fn clockwork_ant_adds_one_copy_per_turn() {
        let mut game = new_game();
        give(&mut game, ONE, &[CLOCKWORK_ANT]);
        play(&mut game, ONE, CLOCKWORK_ANT, Target::Zone(Zone::B)).unwrap();
        end_turn_for_both(&mut game);
        assert_eq!(game.player(ONE).cards_at(Zone::B).len(), 1, "no copy on the turn it is revealed");

        for expected in [2, 3, 4, 5, 5] {
            end_turn_for_both(&mut game);
            assert_eq!(game.player(ONE).cards_at(Zone::B).len(), expected);
        }
        let copies = game.player(ONE).cards_at(Zone::B).iter().filter(|p| p.is_copy).count();
        assert_eq!(copies, 4);
    }

    #[test]
    fn dune_runner_moves_to_another_zone_every_turn() {
        let mut game = new_game();
        give(&mut game, ONE, &[DUNE_RUNNER]);
        play(&mut game, ONE, DUNE_RUNNER, Target::Zone(Zone::A)).unwrap();
        let uid = game.player(ONE).cards_at(Zone::A)[0].uid;
        end_turn_for_both(&mut game);
        assert_eq!(game.locate(ONE, uid).unwrap().0, Zone::A, "doesn't move on the turn it is revealed");

        let mut zone = Zone::A;
        for _ in 0..3 {
            end_turn_for_both(&mut game);
            let new_zone = game.locate(ONE, uid).unwrap().0;
            assert_ne!(new_zone, zone);
            zone = new_zone;
        }
    }

    #[test]
    fn cloud_whale_moves_once_per_turn() {
        let mut game = new_game();
        give(&mut game, ONE, &[CLOUD_WHALE, STONE_GOLEM]);
        play(&mut game, ONE, CLOUD_WHALE, Target::Zone(Zone::A)).unwrap();
        play(&mut game, ONE, STONE_GOLEM, Target::Zone(Zone::A)).unwrap();
        let whale = game.player(ONE).cards_at(Zone::A)[0].uid;
        let golem = game.player(ONE).cards_at(Zone::A)[1].uid;
        let move_to = |uid, to| Action::MoveCard { uid, to };

        assert_eq!(game.apply(ONE, move_to(whale, Zone::B)), Err(ActionError::CannotMove), "not revealed yet");
        end_turn_for_both(&mut game);

        game.apply(ONE, move_to(whale, Zone::B)).unwrap();
        assert_eq!(names_at(&game, ONE, Zone::B), ["Cloud Whale"]);
        assert_eq!(game.apply(ONE, move_to(whale, Zone::C)), Err(ActionError::CannotMove), "already moved");
        assert_eq!(game.apply(ONE, move_to(golem, Zone::C)), Err(ActionError::CannotMove), "golem can't move");

        end_turn_for_both(&mut game);
        game.apply(ONE, move_to(whale, Zone::C)).unwrap();
        assert_eq!(names_at(&game, ONE, Zone::C), ["Cloud Whale"]);
    }

    #[test]
    fn secret_zone_takes_only_secret_cards() {
        let mut game = new_game();
        give(&mut game, ONE, &[LANTERN_GHOST, LANTERN_GHOST, LANTERN_GHOST, LANTERN_GHOST, STONE_GOLEM]);
        assert_eq!(play(&mut game, ONE, LANTERN_GHOST, Target::Zone(Zone::A)), Err(ActionError::SecretZoneOnly));
        assert_eq!(play(&mut game, ONE, STONE_GOLEM, Target::Secret), Err(ActionError::NotASecret));
        for _ in 0..SECRET_CAPACITY {
            play(&mut game, ONE, LANTERN_GHOST, Target::Secret).unwrap();
        }
        assert_eq!(play(&mut game, ONE, LANTERN_GHOST, Target::Secret), Err(ActionError::ZoneFull));

        end_turn_for_both(&mut game);
        assert!(game.player(ONE).secrets.iter().all(|s| !s.revealed), "secrets stay face-down");
    }

    #[test]
    fn lantern_ghost_ambushes_a_strong_card() {
        let mut game = new_game();
        give(&mut game, ONE, &[EMBER_SPRITE, STONE_GOLEM]);
        give(&mut game, TWO, &[LANTERN_GHOST]);
        play(&mut game, TWO, LANTERN_GHOST, Target::Secret).unwrap();
        play(&mut game, ONE, EMBER_SPRITE, Target::Zone(Zone::A)).unwrap();
        play(&mut game, ONE, STONE_GOLEM, Target::Zone(Zone::B)).unwrap();
        end_turn_for_both(&mut game);

        assert_eq!(names_at(&game, ONE, Zone::A), ["Ember Sprite"], "too weak to trigger it");
        assert!(names_at(&game, ONE, Zone::B).is_empty(), "the golem was destroyed");
        assert!(game.player(TWO).secrets[0].revealed);
    }

    #[test]
    fn night_owl_shows_the_enemy_hand_for_one_turn() {
        let mut game = new_game();
        give(&mut game, ONE, &[NIGHT_OWL]);
        play(&mut game, ONE, NIGHT_OWL, Target::Secret).unwrap();
        assert!(!game.sees_opponent_hand(ONE));

        end_turn_for_both(&mut game);
        assert!(game.player(ONE).secrets[0].revealed);
        assert!(game.sees_opponent_hand(ONE), "during turn 2");
        assert!(!game.sees_opponent_hand(TWO));

        end_turn_for_both(&mut game);
        assert!(!game.sees_opponent_hand(ONE), "only for one turn");
    }

    #[test]
    fn missions_complete_when_their_condition_is_met() {
        let mut game = new_game();
        game.zone_missions = [Mission::FillZone, Mission::DestroyEnemyHere, Mission::ReachPower(20)];

        // Fill zone A.
        let fillers: Vec<CardId> = [0, 1, 2, 4, 5].map(CardId).to_vec();
        give(&mut game, ONE, &fillers);
        for &id in &fillers {
            play(&mut game, ONE, id, Target::Zone(Zone::A)).unwrap();
        }
        // Destroy an enemy card at B with a Moss Troll.
        game.players[1].zones[1] = vec![PlacedCard::new(500, EMBER_SPRITE, true)];
        give(&mut game, ONE, &[MOSS_TROLL]);
        play(&mut game, ONE, MOSS_TROLL, Target::Zone(Zone::B)).unwrap();
        // Player Two reaches 20 power at C.
        give(&mut game, TWO, &[GRANITE_BEAR, CORAL_GIANT, STONE_GOLEM]);
        for id in [GRANITE_BEAR, CORAL_GIANT, STONE_GOLEM] {
            play(&mut game, TWO, id, Target::Zone(Zone::C)).unwrap();
        }
        end_turn_for_both(&mut game);

        assert!(game.mission_done(ONE, Zone::A));
        assert!(game.mission_done(ONE, Zone::B));
        assert!(!game.mission_done(ONE, Zone::C));
        assert!(game.mission_done(TWO, Zone::C), "10 + 6 + 5 = 21 power");
        assert!(!game.mission_done(TWO, Zone::A));
    }

    #[test]
    fn a_completed_mission_beats_power() {
        let mut game = new_game();
        game.players[0].zones[0] = vec![PlacedCard::new(1, STONE_GOLEM, true)];
        game.players[1].zones[0] = vec![PlacedCard::new(2, EMBER_SPRITE, true)];
        assert_eq!(game.zone_winner(Zone::A), Some(ONE), "no missions: power decides");
        assert_eq!(game.zone_winner(Zone::B), None, "empty zone: nobody");

        game.missions_done[TWO.index()][0] = true;
        assert_eq!(game.zone_winner(Zone::A), Some(TWO), "only Two completed it");

        game.missions_done[ONE.index()][0] = true;
        assert_eq!(game.zone_winner(Zone::A), Some(ONE), "both completed: power decides");
    }

    #[test]
    fn more_zones_wins_and_total_power_breaks_ties() {
        let mut game = new_game();
        game.players[0].zones[0] = vec![PlacedCard::new(1, STONE_GOLEM, true)];
        game.players[1].zones[1] = vec![PlacedCard::new(2, EMBER_SPRITE, true)];
        game.phase = Phase::GameOver;
        assert_eq!(game.outcome(), Some(Outcome::Winner(ONE)), "one zone each, One has more power");

        game.missions_done[TWO.index()][2] = true;
        assert_eq!(game.outcome(), Some(Outcome::Winner(TWO)), "Two's mission wins them empty zone C");
    }

    #[test]
    fn a_game_survives_being_sent_as_json() {
        let mut game = new_game();
        give(&mut game, ONE, &[STONE_GOLEM]);
        play(&mut game, ONE, STONE_GOLEM, Target::Zone(Zone::A)).unwrap();
        end_turn_for_both(&mut game);

        let json = serde_json::to_string(&game).unwrap();
        let mut copy: GameState = serde_json::from_str(&json).unwrap();
        assert_eq!(copy.turn(), 2);
        assert_eq!(names_at(&copy, ONE, Zone::A), ["Stone Golem"]);
        assert_eq!(copy.take_log().len(), game.take_log().len(), "the replay log comes along");
    }

    #[test]
    fn the_end_of_a_turn_is_logged_step_by_step() {
        let mut game = new_game();
        give(&mut game, ONE, &[EMBER_SPRITE, STONE_GOLEM]);
        play(&mut game, ONE, EMBER_SPRITE, Target::Zone(Zone::A)).unwrap();
        play(&mut game, ONE, STONE_GOLEM, Target::Zone(Zone::B)).unwrap();
        assert!(!game.has_log());
        end_turn_for_both(&mut game);

        let events: Vec<GameEvent> = game.take_log().into_iter().map(|step| step.event).collect();
        assert_eq!(events.first(), Some(&GameEvent::TurnEnded));
        let revealed: Vec<CardId> = events
            .iter()
            .filter_map(|event| match event {
                GameEvent::Revealed { card, .. } => Some(*card),
                _ => None,
            })
            .collect();
        assert_eq!(revealed, [EMBER_SPRITE, STONE_GOLEM], "in the order they were played");
        assert_eq!(events.last(), Some(&GameEvent::NewTurn { turn: 2 }));
        assert!(!game.has_log(), "taking the log empties it");
    }
}
