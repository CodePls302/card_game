use bevy::prelude::*;

use super::layout::{
    END_TURN_POS, END_TURN_SIZE, GRAVEYARD_BUTTON_POS, GRAVEYARD_BUTTON_SIZE, in_hand_area, target_at,
};
use super::network::{Submitted, submit};
use super::playback::Playback;
use super::render::{CardDetails, Draggable};
use super::{CardPopup, Game, GraveyardView};
use crate::game::{Action, Phase, Target};
use crate::ui::AppState;
use crate::ui::session::{ClientLink, ConnectionLost, Seat};
use crate::ui::sound::{PlaySound, Sound};
use crate::ui::widgets::{CARD_SIZE, Cursor, contains};

/// If the mouse moves less than this between press and release, it was a
/// click (open the details popup) rather than a drag.
const CLICK_DISTANCE: f32 = 8.0;

/// What the mouse is doing with cards.
#[derive(Resource, Default)]
pub struct Drag {
    /// The card the mouse was pressed on, and where.
    pressed: Option<Pressed>,
    /// The card being dragged, if the pressed card can be dragged.
    held: Option<Held>,
}

struct Pressed {
    entity: Entity,
    at: Vec2,
}

struct Held {
    entity: Entity,
    source: Draggable,
    /// Where on the card it was grabbed, so it doesn't jump to the cursor.
    grab_offset: Vec2,
    /// Has it moved far enough to count as a drag yet?
    moving: bool,
}

/// Can the local player act right now? Not while the end of the last turn
/// is still being replayed.
fn can_act(game: &Game, playback: &Playback, seat: Seat) -> bool {
    game.0.phase() == Phase::Playing && !game.0.player(seat.me).ended_turn && !playback.is_playing()
}

pub fn end_turn_button(
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Cursor,
    playback: Res<Playback>,
    popup: Res<CardPopup>,
    seat: Res<Seat>,
    client: Option<Res<ClientLink>>,
    mut game: ResMut<Game>,
    mut sounds: MessageWriter<PlaySound>,
) {
    // While the popup is open, a click only closes it.
    if !mouse.just_pressed(MouseButton::Left) || !can_act(&game, &playback, *seat) || popup.0.is_some() {
        return;
    }
    let Some(cursor) = cursor.position() else { return };
    if contains(END_TURN_POS, END_TURN_SIZE, cursor) {
        // Ending the turn can't fail here: `can_act` already checked.
        submit(&mut game, *seat, client.as_deref(), Action::EndTurn);
        sounds.write(PlaySound(Sound::EndTurn));
    }
}

/// Open or close the graveyard. Works at any time, even during the replay.
pub fn graveyard_button(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Cursor,
    popup: Res<CardPopup>,
    mut view: ResMut<GraveyardView>,
    mut sounds: MessageWriter<PlaySound>,
) {
    if !mouse.just_pressed(MouseButton::Left) || popup.0.is_some() {
        return;
    }
    let Some(cursor) = cursor.position() else { return };
    if contains(GRAVEYARD_BUTTON_POS, GRAVEYARD_BUTTON_SIZE, cursor) {
        view.open = !view.open;
        view.opened_at = time.elapsed_secs();
        sounds.write(PlaySound(if view.open { Sound::Graveyard } else { Sound::ButtonClick }));
    }
}

/// Mouse press: close the popup if it's open; otherwise note which card was
/// pressed, and pick it up if it can be dragged.
pub fn start_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Cursor,
    game: Res<Game>,
    playback: Res<Playback>,
    graveyard: Res<GraveyardView>,
    seat: Res<Seat>,
    mut popup: ResMut<CardPopup>,
    mut drag: ResMut<Drag>,
    mut cards: Query<(Entity, &mut Transform, Option<&Draggable>), With<CardDetails>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    if popup.0.is_some() {
        popup.0 = None;
        return;
    }
    // During the replay, clicks skip ahead instead (see `playback.rs`).
    if playback.is_playing() {
        return;
    }
    let Some(cursor) = cursor.position() else { return };

    // Cards can overlap, so pick the one drawn on top (highest z). Board
    // cards are scaled down, so their clickable area is too. While the
    // graveyard is open, only its cards (drawn high above the board) count.
    let lowest_z = if graveyard.open { 250.0 } else { f32::MIN };
    let topmost = cards
        .iter_mut()
        .filter(|(_, t, _)| t.translation.z >= lowest_z)
        .filter(|(_, t, _)| contains(t.translation.truncate(), CARD_SIZE * t.scale.truncate(), cursor))
        .max_by(|a, b| a.1.translation.z.total_cmp(&b.1.translation.z));
    let Some((entity, mut transform, draggable)) = topmost else { return };

    drag.pressed = Some(Pressed { entity, at: cursor });
    if let Some(&source) = draggable {
        if can_act(&game, &playback, *seat) {
            let grab_offset = transform.translation.truncate() - cursor;
            drag.held = Some(Held { entity, source, grab_offset, moving: false });
            transform.translation.z = 100.0;
            transform.scale *= 1.1;
        }
    }
}

/// Keep the held card under the cursor.
pub fn move_drag(
    cursor: Cursor,
    mut drag: ResMut<Drag>,
    mut transforms: Query<&mut Transform, With<Draggable>>,
    mut sounds: MessageWriter<PlaySound>,
) {
    let Some(cursor) = cursor.position() else { return };
    let pressed_at = drag.pressed.as_ref().map(|pressed| pressed.at);
    let Some(held) = &mut drag.held else { return };

    // Only once the mouse has moved does the press count as a drag.
    if !held.moving && pressed_at.is_some_and(|at| at.distance(cursor) >= CLICK_DISTANCE) {
        held.moving = true;
        sounds.write(PlaySound(Sound::CardPickup));
    }
    if let Ok(mut transform) = transforms.get_mut(held.entity) {
        let position = cursor + held.grab_offset;
        transform.translation.x = position.x;
        transform.translation.y = position.y;
    }
}

/// Mouse release. A click opens the details popup for the pressed card. A
/// drag asks the rules to play, move or take back the card, depending on
/// what it is and where it was dropped.
pub fn drop_card(
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Cursor,
    details: Query<&CardDetails>,
    seat: Res<Seat>,
    client: Option<Res<ClientLink>>,
    mut game: ResMut<Game>,
    mut drag: ResMut<Drag>,
    mut popup: ResMut<CardPopup>,
    mut sounds: MessageWriter<PlaySound>,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    // `take()` empties each field and gives us what was in it.
    let pressed = drag.pressed.take();
    let held = drag.held.take();
    let point = cursor.position();

    let was_click = match (&pressed, point) {
        (Some(pressed), Some(point)) => pressed.at.distance(point) < CLICK_DISTANCE,
        _ => false,
    };
    if was_click {
        if let Some(Ok(details)) = pressed.map(|pressed| details.get(pressed.entity)) {
            // Opening the popup redraws the board, which also puts a lifted
            // card back in its place.
            popup.0 = Some(details.clone());
            sounds.write(PlaySound(Sound::ButtonClick));
        }
        return;
    }

    let Some(held) = held else { return };
    let target = point.and_then(target_at);
    let over_hand = point.is_some_and(in_hand_area);

    let action = match (held.source, target) {
        (Draggable::FromHand(card), Some(to)) => Some(Action::PlayCard { card, to }),
        (Draggable::OnBoard { uid, from }, Some(Target::Zone(to))) if to != from => Some(Action::MoveCard { uid, to }),
        (Draggable::Pending { uid }, _) if over_hand => Some(Action::ReturnCard { uid }),
        // Dropped somewhere that isn't a valid spot.
        _ => None,
    };

    let Some(action) = action else {
        // Nothing changed in the rules, but the card is still floating where
        // it was dropped. Marking the game as changed redraws it in place.
        game.set_changed();
        return;
    };
    let accepted = match submit(&mut game, *seat, client.as_deref(), action) {
        Submitted::Applied => true,
        // When playing in someone else's game, assume it's fine: the card
        // stays where it was dropped until the host's answer redraws it.
        Submitted::Sent => true,
        Submitted::Rejected(error) => {
            info!("Can't do that: {error}");
            false
        }
    };
    let sound = match action {
        _ if !accepted => Sound::Invalid,
        Action::PlayCard { .. } => Sound::CardPlace,
        Action::MoveCard { .. } => Sound::CardMove,
        Action::ReturnCard { .. } => Sound::CardReturn,
        Action::EndTurn => Sound::EndTurn,
    };
    sounds.write(PlaySound(sound));
    if !accepted {
        // Nothing changed in the rules, but the card is still floating where
        // it was dropped. Marking the game as changed redraws it in place.
        game.set_changed();
    }
}

/// Esc closes the card popup if it's open, and otherwise leaves the game.
/// Enter also leaves once the game is over (and the replay has finished),
/// or the other player has disconnected.
pub fn leave_game(
    keys: Res<ButtonInput<KeyCode>>,
    game: Res<Game>,
    playback: Res<Playback>,
    lost: Res<ConnectionLost>,
    mut popup: ResMut<CardPopup>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Escape) && popup.0.is_some() {
        popup.0 = None;
        return;
    }
    let game_over = (game.0.phase() == Phase::GameOver && !playback.is_playing()) || lost.0;
    if keys.just_pressed(KeyCode::Escape) || (game_over && keys.just_pressed(KeyCode::Enter)) {
        next_state.set(AppState::MainMenu);
    }
}
