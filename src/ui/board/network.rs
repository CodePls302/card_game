//! The board's side of LAN play.
//!
//! - Hosting: the other player's actions arrive here and are applied to the
//!   real game, and every change to the game is sent back to them.
//! - Joined: the host's copy of the game arrives here and replaces ours. Our
//!   own actions are sent to the host instead of being applied (`submit`).

use bevy::prelude::*;

use super::Game;
use super::playback::Playback;
use crate::game::{Action, ActionError};
use crate::net::{ClientMessage, HostMessage, NetEvent};
use crate::ui::session::{ClientLink, ConnectionLost, HostLink, Seat};
use crate::ui::sound::{PlaySound, Sound};

/// What became of an action the local player took.
pub enum Submitted {
    /// Applied to the game here.
    Applied,
    /// The rules refused it.
    Rejected(ActionError),
    /// Sent to the host, who will answer with the new game state (or a
    /// rejection).
    Sent,
}

/// Hand the local player's action to the rules: directly, or via the host
/// when playing in someone else's game.
///
/// `game` is a `&mut ResMut<Game>` rather than a `&mut Game` so that, when
/// the action is only sent, the game isn't marked as changed (which would
/// redraw the board before the host has answered).
pub fn submit(game: &mut ResMut<Game>, seat: Seat, client: Option<&ClientLink>, action: Action) -> Submitted {
    match client {
        Some(link) => {
            link.0.send(ClientMessage::Action(action));
            Submitted::Sent
        }
        None => match game.0.apply(seat.me, action) {
            Ok(()) => Submitted::Applied,
            Err(error) => Submitted::Rejected(error),
        },
    }
}

/// A run condition: true when playing against the AI (no LAN connection).
pub fn playing_against_ai(host: Option<Res<HostLink>>, client: Option<Res<ClientLink>>) -> bool {
    host.is_none() && client.is_none()
}

/// Hosting: apply the other player's actions as they arrive.
pub fn host_receive(
    host: Option<Res<HostLink>>,
    seat: Res<Seat>,
    mut game: ResMut<Game>,
    mut lost: ResMut<ConnectionLost>,
) {
    let Some(host) = host else { return };
    for event in host.0.receive() {
        match event {
            NetEvent::Message(ClientMessage::Action(action)) => {
                if let Err(error) = game.0.apply(seat.opponent(), action) {
                    host.0.send(HostMessage::Rejected(error));
                }
            }
            // Only expected once, in the lobby.
            NetEvent::Message(ClientMessage::Hello { .. }) => {}
            NetEvent::Disconnected => lost.0 = true,
        }
    }
}

/// Hosting: whenever the game changed, send the whole of it to the other
/// player. This runs before the replay takes the log out of the game, so
/// the other player gets the log too and can replay the same steps.
pub fn host_send(host: Option<Res<HostLink>>, game: Res<Game>) {
    if let Some(host) = host {
        if game.is_changed() {
            host.0.send(HostMessage::State(Box::new(game.0.clone())));
        }
    }
}

/// Joined: take in the host's game state, and any rejections.
pub fn client_receive(
    client: Option<Res<ClientLink>>,
    mut game: ResMut<Game>,
    mut playback: ResMut<Playback>,
    mut lost: ResMut<ConnectionLost>,
    mut sounds: MessageWriter<PlaySound>,
) {
    let Some(client) = client else { return };
    for event in client.0.receive() {
        match event {
            NetEvent::Message(HostMessage::State(state)) => {
                // `*state` takes the `GameState` out of its `Box`.
                let mut state = *state;
                // Queue its log for the replay straight away: if several
                // states arrive in one frame, a later one would otherwise
                // replace this one before its log was played.
                let steps = state.take_log();
                if !steps.is_empty() {
                    playback.enqueue(steps);
                }
                game.0 = state;
            }
            NetEvent::Message(HostMessage::Rejected(error)) => {
                info!("The host refused that: {error}");
                sounds.write(PlaySound(Sound::Invalid));
                // Redraw, which puts the card back where it was.
                game.set_changed();
            }
            NetEvent::Message(HostMessage::Refused(reason)) => {
                warn!("The host ended the game: {reason}");
                lost.0 = true;
            }
            NetEvent::Disconnected => lost.0 = true,
        }
    }
}

/// Leaving the game screen closes any LAN connection (dropping it closes
/// the socket, which the other side sees as a disconnection).
pub fn close_connection(mut commands: Commands) {
    commands.remove_resource::<HostLink>();
    commands.remove_resource::<ClientLink>();
    commands.insert_resource(ConnectionLost::default());
}
