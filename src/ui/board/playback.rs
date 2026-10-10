//! Replays the end of a turn one step at a time, so players can follow what
//! each card did. The rules finish the turn instantly and log every step
//! with a snapshot of the board; while those steps are queued here, the
//! screen shows the snapshots instead of the live game.

use std::collections::VecDeque;

use bevy::prelude::*;

use super::{Game, TurnTimer};
use crate::game::{Effect, GameEvent, GameState, Outcome, PlayerId, Step, card};
use crate::ui::session::Seat;
use crate::ui::sound::{PlaySound, Sound};

#[derive(Resource, Default)]
pub struct Playback {
    queue: VecDeque<Step>,
    /// The step on screen right now.
    current: Option<Step>,
    /// The board as it was just before the current step, used to show where
    /// a destroyed card used to be.
    previous: Option<GameState>,
}

impl Playback {
    pub fn is_playing(&self) -> bool {
        self.current.is_some()
    }

    pub fn current(&self) -> Option<&Step> {
        self.current.as_ref()
    }

    pub fn previous(&self) -> Option<&GameState> {
        self.previous.as_ref()
    }

    /// Add steps to replay (used when they arrive from the host).
    pub fn enqueue(&mut self, steps: Vec<Step>) {
        self.queue.extend(steps);
    }
}

/// How long each kind of step stays on screen, in seconds.
fn duration(event: &GameEvent) -> f32 {
    match event {
        GameEvent::TurnEnded => 0.6,
        GameEvent::NewTurn { .. } => 0.9,
        GameEvent::GameEnded => 0.9,
        _ => 1.2,
    }
}

/// The sound for each kind of step, if any.
fn sound(event: &GameEvent) -> Option<Sound> {
    match event {
        GameEvent::TurnEnded | GameEvent::GameEnded => None,
        GameEvent::Revealed { .. } => Some(Sound::CardReveal),
        GameEvent::Destroyed { .. } => Some(Sound::CardDestroyed),
        GameEvent::Copied { .. } => Some(Sound::CardCopied),
        GameEvent::Moved { .. } => Some(Sound::CardMove),
        GameEvent::SecretFlipped { .. } => Some(Sound::SecretFlipped),
        GameEvent::MissionCompleted { .. } => Some(Sound::MissionComplete),
        GameEvent::NewTurn { .. } => Some(Sound::TurnStart),
    }
}

/// Move the rules' log into the playback queue.
pub fn collect_steps(mut game: ResMut<Game>, mut playback: ResMut<Playback>) {
    // Check first: only take the log (which counts as a change) if there is one.
    if game.0.has_log() {
        let steps = game.0.take_log();
        playback.queue.extend(steps);
    }
}

/// Show the next step when the current one's time is up, or straight away
/// if the player clicks.
pub fn advance(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut playback: ResMut<Playback>,
    mut turn_timer: ResMut<TurnTimer>,
    game: Res<Game>,
    seat: Res<Seat>,
    mut sounds: MessageWriter<PlaySound>,
    // A `Local` belongs to this system alone and keeps its value between
    // frames. Keeping the step timer here means ticking it doesn't count as
    // a change to `Playback`, which would redraw the board every frame.
    mut step_timer: Local<Timer>,
) {
    if playback.current.is_some() {
        step_timer.tick(time.delta());
        let skip = mouse.just_pressed(MouseButton::Left);
        if !step_timer.is_finished() && !skip {
            return;
        }
    } else if playback.queue.is_empty() {
        return;
    }

    let previous = playback.current.take().map(|step| step.after);
    match playback.queue.pop_front() {
        Some(step) => {
            *step_timer = Timer::from_seconds(duration(&step.event), TimerMode::Once);
            if let Some(sound) = sound(&step.event) {
                sounds.write(PlaySound(sound));
            }
            playback.previous = previous;
            playback.current = Some(step);
        }
        None => {
            // The replay is over: the new turn starts now, or the result shows.
            playback.previous = None;
            turn_timer.0.reset();
            match game.0.outcome() {
                Some(Outcome::Winner(player)) if player == seat.me => {
                    sounds.write(PlaySound(Sound::Victory));
                }
                Some(Outcome::Winner(_)) => {
                    sounds.write(PlaySound(Sound::Defeat));
                }
                Some(Outcome::Draw) | None => {}
            }
        }
    }
}

/// "You" or "The opponent", for captions.
fn who(player: PlayerId, seat: Seat) -> &'static str {
    if player == seat.me { "You" } else { "The opponent" }
}

fn whose(player: PlayerId, seat: Seat) -> &'static str {
    if player == seat.me { "your" } else { "the opponent's" }
}

/// A sentence describing a step, shown at the top of the screen.
pub fn caption(event: &GameEvent, shown: &GameState, seat: Seat) -> String {
    match *event {
        GameEvent::TurnEnded => "Revealing cards...".to_string(),
        GameEvent::Revealed { player, zone, card: id, .. } => {
            format!("{} revealed {} at Zone {}", who(player, seat), card(id).name, zone.name())
        }
        GameEvent::Destroyed { player, card: id, by, .. } => {
            format!("{} destroyed {} {}", card(by).name, whose(player, seat), card(id).name)
        }
        GameEvent::Copied { zone, card: id, .. } => format!("{} copied itself at Zone {}", card(id).name, zone.name()),
        GameEvent::Moved { card: id, from, to, .. } => {
            format!("{} wandered from Zone {} to Zone {}", card(id).name, from.name(), to.name())
        }
        GameEvent::SecretFlipped { player, card: id, .. } => {
            let secret = format!("{} secret {} flips", capitalised(whose(player, seat)), card(id).name);
            if card(id).effect == Effect::RevealHand {
                let viewer = if player == seat.me { "you can" } else { "they can" };
                format!("{secret}: {viewer} see the other hand this turn")
            } else {
                format!("{secret}!")
            }
        }
        GameEvent::MissionCompleted { player, zone } => format!(
            "{} completed the mission at Zone {}: {}",
            who(player, seat),
            zone.name(),
            shown.zone_mission(zone).description()
        ),
        GameEvent::NewTurn { turn } => format!("Turn {turn}"),
        GameEvent::GameEnded => "The final turn is over".to_string(),
    }
}

fn capitalised(text: &str) -> String {
    let mut characters = text.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}
