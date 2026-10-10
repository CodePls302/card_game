//! Where things go on screen. All positions are in world units: (0, 0) is
//! the centre of the window, x grows to the right and y grows *upwards*.
//! The local player sits at the bottom, the opponent at the top.

use bevy::prelude::*;

use crate::game::{Target, Zone};
use crate::ui::widgets::CARD_SIZE;

/// Cards on the board are drawn smaller than cards in hand.
pub const BOARD_CARD_SCALE: f32 = 0.62;

pub const ZONE_WIDTH: f32 = 300.0;
const ZONE_SPACING: f32 = 340.0;
/// The three zones sit right of centre, leaving room for the secret zones.
const ZONES_CENTER_X: f32 = 90.0;
pub const ZONE_HEIGHT: f32 = 500.0;
pub const ZONE_LABEL_HEIGHT: f32 = 100.0;

/// Both secret zones share one column on the left: opponent's above the
/// centre line, yours below.
pub const SECRET_X: f32 = -555.0;
pub const SECRET_WIDTH: f32 = 170.0;

const HAND_Y: f32 = -355.0;
const HAND_CENTER_X: f32 = -40.0;
const HAND_MAX_WIDTH: f32 = 880.0;

pub const END_TURN_POS: Vec2 = Vec2::new(565.0, -365.0);
pub const END_TURN_SIZE: Vec2 = Vec2::new(130.0, 60.0);

pub const GRAVEYARD_BUTTON_POS: Vec2 = Vec2::new(-545.0, 360.0);
pub const GRAVEYARD_BUTTON_SIZE: Vec2 = Vec2::new(190.0, 44.0);

pub const TOP_BAR_Y: f32 = 415.0;
/// The status line, which also shows what happened during the replay.
pub const CAPTION_Y: f32 = 375.0;
/// Where the opponent's hand is shown when Night Owl reveals it.
const OPPONENT_HAND_Y: f32 = 315.0;
pub const OPPONENT_HAND_SCALE: f32 = 0.5;

pub fn zone_x(zone: Zone) -> f32 {
    ZONES_CENTER_X + (zone.index() as f32 - 1.0) * ZONE_SPACING
}

/// Centre of the `slot`-th card in a grid with `columns` columns, centred on
/// `center_x`, growing away from the centre line on the player's side.
fn grid_slot(center_x: f32, columns: usize, is_me: bool, slot: usize) -> Vec2 {
    let column = (slot % columns) as f32 - (columns as f32 - 1.0) / 2.0;
    let row = (slot / columns) as f32;
    let x = center_x + column * 72.0;
    let distance_from_centre = 100.0 + row * 95.0;
    let y = if is_me { -distance_from_centre } else { distance_from_centre };
    Vec2::new(x, y)
}

/// Where the `slot`-th card a player has at a zone goes: 3 on the first
/// row, 2 on the second.
pub fn zone_slot(is_me: bool, zone: Zone, slot: usize) -> Vec2 {
    grid_slot(zone_x(zone), 3, is_me, slot)
}

pub fn secret_slot(is_me: bool, slot: usize) -> Vec2 {
    grid_slot(SECRET_X, 2, is_me, slot)
}

/// Centre of the `index`-th of `count` cards in the local player's hand.
/// Cards squeeze closer together as the hand grows.
pub fn hand_slot(index: usize, count: usize) -> Vec2 {
    let spacing = (HAND_MAX_WIDTH / count as f32).min(CARD_SIZE.x + 8.0);
    let offset = index as f32 - (count as f32 - 1.0) / 2.0;
    Vec2::new(HAND_CENTER_X + offset * spacing, HAND_Y)
}

/// Centre of the `index`-th card of the opponent's hand, when it's visible.
pub fn opponent_hand_slot(index: usize, count: usize) -> Vec2 {
    let spacing = CARD_SIZE.x * OPPONENT_HAND_SCALE + 6.0;
    let offset = index as f32 - (count as f32 - 1.0) / 2.0;
    Vec2::new(ZONES_CENTER_X + offset * spacing, OPPONENT_HAND_Y)
}

/// Is `point` in the hand area, below the zones? Cards played this turn
/// can be dropped here to take them back.
pub fn in_hand_area(point: Vec2) -> bool {
    point.y < -ZONE_HEIGHT / 2.0 - 10.0
}

/// Where a card dropped at `point` lands, if anywhere. Only the local
/// player's half of each zone (plus the label strip) counts.
pub fn target_at(point: Vec2) -> Option<Target> {
    if point.y > ZONE_LABEL_HEIGHT / 2.0 || point.y < -ZONE_HEIGHT / 2.0 {
        return None;
    }
    if (point.x - SECRET_X).abs() < SECRET_WIDTH / 2.0 {
        return Some(Target::Secret);
    }
    Zone::ALL.into_iter().find(|&zone| (point.x - zone_x(zone)).abs() < ZONE_WIDTH / 2.0).map(Target::Zone)
}
