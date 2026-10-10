//! Drawing and input helpers shared by every screen: colours, text, cards,
//! buttons, and the mouse position.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::text::{FontSize, FontWeight, Justify, TextBounds, TextLayout};
use bevy::window::PrimaryWindow;

use crate::assets::asset_exists;
use crate::game::{CardId, Effect, card};

/// The camera always shows at least this much of the world, scaling to fit
/// the window. (0, 0) is the centre; y grows upwards.
pub const VIEW_SIZE: Vec2 = Vec2::new(1300.0, 900.0);
pub const CARD_SIZE: Vec2 = Vec2::new(100.0, 140.0);

pub const BACKGROUND: Color = Color::srgb(0.07, 0.08, 0.13);
pub const PANEL: Color = Color::srgb(0.13, 0.15, 0.24);
pub const PANEL_LIGHT: Color = Color::srgb(0.21, 0.23, 0.38);
pub const MY_COLOR: Color = Color::srgb(0.25, 0.55, 0.95);
pub const THEIR_COLOR: Color = Color::srgb(0.95, 0.45, 0.25);
pub const NEUTRAL: Color = Color::srgb(0.35, 0.37, 0.45);
pub const CARD_BORDER: Color = Color::srgb(0.85, 0.85, 0.9);
pub const TEXT: Color = Color::WHITE;
pub const DIM_TEXT: Color = Color::srgb(0.75, 0.77, 0.85);
pub const ENERGY_COLOR: Color = Color::srgb(0.2, 0.75, 0.85);
const CARD_FACE: Color = Color::srgb(0.16, 0.16, 0.22);
const CARD_BACK: Color = Color::srgb(0.32, 0.18, 0.45);
const POWER_BACKGROUND: Color = Color::srgb(0.95, 0.6, 0.1);
/// Power that has been raised by another card's effect.
const BOOSTED_POWER: Color = Color::srgb(0.1, 0.4, 1.0);
const BOOSTED_BACKGROUND: Color = Color::WHITE;

/// The parts of a piece of text, bundled together. `impl Bundle` means
/// "some group of components"; Bevy works out the exact type.
pub fn text(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text2d::new(value), TextFont::from_font_size(size), TextColor(color), SharpText { size, wrap_width: None })
}

/// Bold text, e.g. the numbers on cards.
pub fn bold_text(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text2d::new(value),
        TextFont::from_font_size(size).with_font_weight(FontWeight::BOLD),
        TextColor(color),
        SharpText { size, wrap_width: None },
    )
}

/// Centred text that wraps onto several lines at `width`.
pub fn wrapped_text(value: impl Into<String>, size: f32, color: Color, width: f32) -> impl Bundle {
    (
        Text2d::new(value),
        TextFont::from_font_size(size),
        TextColor(color),
        TextBounds::new_horizontal(width),
        TextLayout::justify(Justify::Center),
        SharpText { size, wrap_width: Some(width) },
    )
}

/// Text that is re-rendered at the size it actually appears on screen.
///
/// Bevy draws text at its font size, and then the camera (which fits the
/// board to the window) and any parent card's scale stretch or shrink it,
/// which blurs it. `sharpen_text` instead raises or lowers the font size to
/// match the final on-screen size, and shrinks or grows the text entity by
/// the opposite amount, so it ends up the same size but drawn pixel-sharp.
#[derive(Component)]
pub struct SharpText {
    /// The font size as designed, in world units.
    size: f32,
    wrap_width: Option<f32>,
}

pub fn sharpen_text(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<&Projection, With<Camera2d>>,
    ancestors: Query<(&Transform, Option<&ChildOf>), Without<SharpText>>,
    mut texts: Query<(&SharpText, &mut TextFont, &mut Transform, Option<&mut TextBounds>, Option<&ChildOf>)>,
) {
    let Projection::Orthographic(projection) = *camera else { return };
    if projection.area.height() <= 0.0 {
        return;
    }
    // Physical screen pixels per world unit, for this camera and window.
    let pixels_per_unit = window.physical_height() as f32 / projection.area.height();

    for (sharp, mut font, mut transform, bounds, parent) in &mut texts {
        // Multiply up the scales of the cards/buttons this text sits inside.
        let mut parent_scale = 1.0;
        let mut next = parent.map(ChildOf::parent);
        while let Some(entity) = next {
            let Ok((ancestor, grandparent)) = ancestors.get(entity) else { break };
            parent_scale *= ancestor.scale.y;
            next = grandparent.map(ChildOf::parent);
        }

        // Bevy already renders text at the window's scale factor, so only the
        // rest of the zoom needs making up. Rounded to steps of 1/8 so that
        // animations don't re-render the text every single frame.
        let zoom = pixels_per_unit * parent_scale / window.scale_factor();
        let zoom = ((zoom * 8.0).round() / 8.0).max(0.25);

        let font_size = FontSize::Px(sharp.size * zoom);
        if font.font_size != font_size {
            font.font_size = font_size;
        }
        let scale = Vec3::splat(1.0 / zoom);
        if transform.scale != scale {
            transform.scale = scale;
        }
        if let (Some(mut bounds), Some(width)) = (bounds, sharp.wrap_width) {
            let width = Some(width * zoom);
            if bounds.width != width {
                bounds.width = width;
            }
        }
    }
}

/// Draw a face-up card and return its entity. `power` is its current power,
/// shown in blue when another card's effect has raised it.
pub fn spawn_card(
    commands: &mut Commands,
    assets: &AssetServer,
    id: CardId,
    power: i32,
    border: Color,
    position: Vec3,
    scale: f32,
) -> Entity {
    let def = card(id);
    let art = art_sprite(assets, id);
    let (power_color, power_background) =
        if power > def.power { (BOOSTED_POWER, BOOSTED_BACKGROUND) } else { (TEXT, POWER_BACKGROUND) };

    commands
        .spawn((
            Sprite::from_color(border, CARD_SIZE),
            Transform::from_translation(position).with_scale(Vec3::splat(scale)),
        ))
        // Children are positioned relative to the card, and move and scale
        // with it. The small z values keep each layer above the last.
        .with_children(|card| {
            card.spawn((Sprite::from_color(CARD_FACE, CARD_SIZE - 6.0), Transform::from_xyz(0.0, 0.0, 0.1)));
            card.spawn((art, Transform::from_xyz(0.0, 28.0, 0.2)));
            // Cost in the top-left corner, power in the top-right.
            card.spawn((Sprite::from_color(ENERGY_COLOR, Vec2::splat(30.0)), Transform::from_xyz(-36.0, 56.0, 0.3)));
            card.spawn((bold_text(def.cost.to_string(), 20.0, TEXT), Transform::from_xyz(-36.0, 56.0, 0.4)));
            card.spawn((Sprite::from_color(power_background, Vec2::splat(30.0)), Transform::from_xyz(36.0, 56.0, 0.3)));
            card.spawn((bold_text(power.to_string(), 20.0, power_color), Transform::from_xyz(36.0, 56.0, 0.4)));
            card.spawn((text(def.name, 12.0, TEXT), Transform::from_xyz(0.0, -13.0, 0.3)));
            if def.effect != Effect::None {
                card.spawn((
                    wrapped_text(def.effect.description(), 9.0, DIM_TEXT, 90.0),
                    Transform::from_xyz(0.0, -44.0, 0.3),
                ));
            }
        })
        .id()
}

/// Darken a card to show it can't be used right now.
pub fn dim_card(commands: &mut Commands, card: Entity) {
    commands.entity(card).with_children(|card| {
        card.spawn((
            Sprite::from_color(Color::srgba(0.0, 0.0, 0.0, 0.6), CARD_SIZE),
            Transform::from_xyz(0.0, 0.0, 1.0),
        ));
    });
}

/// The card's art if `assets/cards/<name>.png` exists, otherwise a coloured
/// placeholder so every card still looks different.
fn art_sprite(assets: &AssetServer, id: CardId) -> Sprite {
    let size = Vec2::new(88.0, 60.0);
    let path = card(id).art_path();
    if asset_exists(&path) {
        Sprite { image: assets.load(path), custom_size: Some(size), ..default() }
    } else {
        let hue = id.0 as f32 * 360.0 / 25.0;
        Sprite::from_color(Color::hsl(hue, 0.45, 0.4), size)
    }
}

/// A face-down card the local player isn't allowed to see.
pub fn spawn_card_back(commands: &mut Commands, position: Vec3, scale: f32) -> Entity {
    commands
        .spawn((
            Sprite::from_color(CARD_BORDER, CARD_SIZE),
            Transform::from_translation(position).with_scale(Vec3::splat(scale)),
        ))
        .with_children(|card| {
            card.spawn((Sprite::from_color(CARD_BACK, CARD_SIZE - 6.0), Transform::from_xyz(0.0, 0.0, 0.1)));
            card.spawn((text("?", 48.0, TEXT), Transform::from_xyz(0.0, 0.0, 0.2)));
        })
        .id()
}

/// The clickable size of a button.
#[derive(Component)]
pub struct ButtonArea(pub Vec2);

/// Draw a button. Callers add their own component saying what it does, e.g.
/// `spawn_button(...).insert(MenuButton::Play)`.
pub fn spawn_button<'a>(
    commands: &'a mut Commands,
    label: impl Into<String>,
    center: Vec2,
    size: Vec2,
    color: Color,
) -> EntityCommands<'a> {
    let mut button = commands.spawn((
        Sprite::from_color(color, size),
        Transform::from_translation(center.extend(5.0)),
        ButtonArea(size),
    ));
    button.with_children(|button| {
        button.spawn((text(label, 22.0, TEXT), Transform::from_xyz(0.0, 0.0, 0.1)));
    });
    button
}

/// Which button, if any, is under `cursor`. `A` is the screen's own button
/// component, so each screen only sees its own buttons.
pub fn button_under<A: Component + Copy>(cursor: Vec2, buttons: &Query<(&A, &ButtonArea, &Transform)>) -> Option<A> {
    buttons
        .iter()
        .find(|(_, area, transform)| contains(transform.translation.truncate(), area.0, cursor))
        .map(|(action, _, _)| *action)
}

/// Is `point` inside the rectangle with this centre and size?
pub fn contains(center: Vec2, size: Vec2, point: Vec2) -> bool {
    let distance = (point - center).abs();
    distance.x <= size.x / 2.0 && distance.y <= size.y / 2.0
}

/// A system parameter that finds the mouse in world coordinates. Deriving
/// `SystemParam` lets systems ask for `Cursor` instead of listing the window
/// and camera queries every time.
#[derive(SystemParam)]
pub struct Cursor<'w, 's> {
    window: Single<'w, 's, &'static Window, With<PrimaryWindow>>,
    camera: Single<'w, 's, (&'static Camera, &'static GlobalTransform)>,
}

impl Cursor<'_, '_> {
    /// The mouse position in world coordinates, or `None` if it's outside
    /// the window.
    pub fn position(&self) -> Option<Vec2> {
        let cursor = self.window.cursor_position()?;
        let (camera, camera_transform) = *self.camera;
        camera.viewport_to_world_2d(camera_transform, cursor).ok()
    }
}
