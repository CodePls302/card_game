//! The game itself.
//!
//! The approach is deliberately simple: the `Game` resource holds the real
//! game state, and whenever something changes we throw away every card on
//! screen and draw the board again from scratch (see `render.rs`). Input
//! (`input.rs`) never moves cards itself; it asks the rules to apply an
//! `Action`, and the redraw shows the result. When a turn ends, `playback.rs`
//! replays what happened one step at a time.

mod animate;
mod input;
mod layout;
mod network;
mod opponent;
mod playback;
mod render;

use bevy::prelude::*;

use super::AppState;
use super::session::{ClientLink, ConnectionLost, Seat};
use super::sound::{PlaySound, Sound};
use crate::game::{Action, CardId, GameState, Phase, PlayerId};

/// How long a player has to finish their turn.
const TURN_SECONDS: f32 = 120.0;
/// The timer ticks every second once this few seconds are left.
const TICK_FROM_SECONDS: u32 = 10;
/// How long the graveyard stays open.
const GRAVEYARD_SECONDS: f32 = 5.0;

/// Bevy stores shared data in "resources". This one wraps our rules so the
/// systems below can read and change the game.
#[derive(Resource)]
pub struct Game(pub GameState);

impl Game {
    pub fn new(my_deck: Vec<CardId>, their_deck: Vec<CardId>) -> Self {
        Game(GameState::new(my_deck, their_deck, &mut rand::rng()))
    }
}

/// Counts down the local player's turn; End Turn is pressed automatically
/// when it runs out.
#[derive(Resource)]
pub struct TurnTimer(pub Timer);

impl Default for TurnTimer {
    fn default() -> Self {
        TurnTimer(Timer::from_seconds(TURN_SECONDS, TimerMode::Once))
    }
}

/// Whether the graveyard overlay is showing, and since when (in seconds
/// since the app started).
#[derive(Resource, Default)]
pub struct GraveyardView {
    pub open: bool,
    pub opened_at: f32,
}

/// The card shown in the details popup, if it's open.
#[derive(Resource, Default)]
pub struct CardPopup(pub Option<render::CardDetails>);

pub struct BoardPlugin;

impl Plugin for BoardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<input::Drag>()
            .init_resource::<playback::Playback>()
            .init_resource::<GraveyardView>()
            .init_resource::<CardPopup>()
            .init_resource::<TurnTimer>()
            .init_resource::<ConnectionLost>()
            .insert_resource(Seat { me: PlayerId::One })
            .add_systems(OnEnter(AppState::InGame), (reset_for_new_game, render::spawn_board))
            .add_systems(OnExit(AppState::InGame), network::close_connection)
            .add_systems(
                Update,
                (
                    network::client_receive,
                    input::end_turn_button,
                    input::graveyard_button,
                    input::start_drag,
                    input::move_drag,
                    input::drop_card,
                    input::leave_game,
                    opponent::take_turn.run_if(network::playing_against_ai),
                    network::host_receive,
                    tick_turn_timer,
                    // Must come before `collect_steps`, which takes the log
                    // the other player needs for their replay.
                    network::host_send,
                    playback::collect_steps,
                    playback::advance,
                    close_graveyard,
                    render::redraw.run_if(needs_redraw),
                    render::update_timer_text,
                    animate::animate,
                )
                    // Run in this order every frame, so the redraw sees the
                    // effects of this frame's input.
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

/// Clear anything left over from a previous game.
fn reset_for_new_game(mut commands: Commands) {
    commands.insert_resource(input::Drag::default());
    commands.insert_resource(playback::Playback::default());
    commands.insert_resource(GraveyardView::default());
    commands.insert_resource(CardPopup::default());
    commands.insert_resource(TurnTimer::default());
}

/// A run condition: redraw when the game, the replay, the graveyard view,
/// the card popup or the connection changed since the last redraw.
fn needs_redraw(
    game: Option<Res<Game>>,
    playback: Res<playback::Playback>,
    graveyard: Res<GraveyardView>,
    popup: Res<CardPopup>,
    lost: Res<ConnectionLost>,
) -> bool {
    game.is_some_and(|game| game.is_changed())
        || playback.is_changed()
        || graveyard.is_changed()
        || popup.is_changed()
        || lost.is_changed()
}

/// Count down while the local player can act; at zero, end their turn.
fn tick_turn_timer(
    time: Res<Time>,
    playback: Res<playback::Playback>,
    seat: Res<Seat>,
    client: Option<Res<ClientLink>>,
    mut timer: ResMut<TurnTimer>,
    mut game: ResMut<Game>,
    mut sounds: MessageWriter<PlaySound>,
    // The whole seconds left last frame, to notice when a new second starts.
    mut last_seconds: Local<u32>,
) {
    let waiting = game.0.phase() != Phase::Playing || game.0.player(seat.me).ended_turn || playback.is_playing();
    if waiting {
        return;
    }
    timer.0.tick(time.delta());
    let seconds = timer.0.remaining_secs().ceil() as u32;
    if seconds != *last_seconds && seconds <= TICK_FROM_SECONDS && seconds > 0 {
        sounds.write(PlaySound(Sound::TimerTick));
    }
    *last_seconds = seconds;
    // `just_finished` is only true on the frame the timer runs out, so the
    // turn is ended once even if the host takes a moment to answer.
    if timer.0.just_finished() {
        info!("Time's up: ending the turn");
        network::submit(&mut game, *seat, client.as_deref(), Action::EndTurn);
        sounds.write(PlaySound(Sound::EndTurn));
    }
}

fn close_graveyard(time: Res<Time>, mut view: ResMut<GraveyardView>) {
    if view.open && time.elapsed_secs() - view.opened_at > GRAVEYARD_SECONDS {
        view.open = false;
    }
}
