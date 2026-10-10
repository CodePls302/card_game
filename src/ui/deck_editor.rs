//! The deck editor. Drag cards from the collection into the deck list to add
//! them, drag them out of the list to remove them, then save the deck as a
//! preset.
//!
//! Like the game board, it redraws everything whenever the `Editor`
//! resource changes.

use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use super::AppState;
use super::sound::{PlaySound, Sound};
use super::widgets::*;
use crate::game::{CardId, DECK_SIZE, Deck, all_cards, card};
use crate::presets::{self, Presets};

const PRESET_X: f32 = -555.0;
const PRESET_SIZE: Vec2 = Vec2::new(170.0, 38.0);

const COLLECTION_CENTER: Vec2 = Vec2::new(-200.0, 30.0);
const COLLECTION_COLUMNS: usize = 5;
const COLLECTION_SPACING: Vec2 = Vec2::new(84.0, 106.0);
const COLLECTION_SCALE: f32 = 0.7;

const PREVIEW_POSITION: Vec2 = Vec2::new(115.0, 40.0);
const PREVIEW_SCALE: f32 = 1.6;

const DECK_PANEL_CENTER: Vec2 = Vec2::new(455.0, -20.0);
const DECK_PANEL_SIZE: Vec2 = Vec2::new(330.0, 700.0);
const NAME_FIELD_POSITION: Vec2 = Vec2::new(455.0, 290.0);
const NAME_FIELD_SIZE: Vec2 = Vec2::new(300.0, 40.0);
const FIRST_ROW_Y: f32 = 205.0;
const ROW_HEIGHT: f32 = 28.0;
const ROW_SIZE: Vec2 = Vec2::new(300.0, 24.0);

const MAX_NAME_LENGTH: usize = 24;

/// What the editor is working on. Changing it triggers a redraw.
#[derive(Resource)]
struct Editor {
    /// Which preset is being edited, or `None` for a new, unsaved deck.
    preset: Option<usize>,
    deck: Deck,
    /// Is the name field taking keyboard input?
    editing_name: bool,
    /// The card under the mouse, shown large in the preview.
    hovered: Option<CardId>,
    /// Feedback such as "Saved".
    message: String,
}

impl Editor {
    fn open(presets: &[Deck], preset: Option<usize>) -> Self {
        let deck = match preset {
            Some(index) => presets[index].clone(),
            None => Deck { name: "New Deck".to_string(), cards: Vec::new() },
        };
        Editor { preset, deck, editing_name: false, hovered: None, message: String::new() }
    }
}

/// Marks everything the editor draws, so a redraw can remove it.
#[derive(Component)]
struct EditorItem;

#[derive(Component, Clone, Copy)]
enum EditorButton {
    Preset(usize),
    NewDeck,
    NameField,
    Save,
    Delete,
    Back,
}

/// Something that can be dragged: a card in the collection, or a row in the
/// deck list. The `Vec2` in `DragArea` is its unscaled clickable size.
#[derive(Component, Clone, Copy)]
enum DragSource {
    Collection(CardId),
    DeckRow(CardId),
}

impl DragSource {
    fn card(self) -> CardId {
        match self {
            DragSource::Collection(id) | DragSource::DeckRow(id) => id,
        }
    }
}

#[derive(Component)]
struct DragArea(Vec2);

#[derive(Resource, Default)]
struct EditorDrag(Option<Held>);

struct Held {
    entity: Entity,
    source: DragSource,
    grab_offset: Vec2,
}

pub struct DeckEditorPlugin;

impl Plugin for DeckEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorDrag>().add_systems(OnEnter(AppState::DeckEditor), open_editor).add_systems(
            Update,
            (click, move_drag, drop, hover, type_name, redraw.run_if(resource_exists_and_changed::<Editor>))
                .chain()
                .run_if(in_state(AppState::DeckEditor)),
        );
    }
}

fn open_editor(mut commands: Commands, presets: Res<Presets>) {
    let first = if presets.0.is_empty() { None } else { Some(0) };
    commands.insert_resource(Editor::open(&presets.0, first));
}

/// Components for everything the editor draws.
fn item() -> impl Bundle {
    (EditorItem, DespawnOnExit(AppState::DeckEditor))
}

fn collection_slot(index: usize) -> Vec2 {
    let column = (index % COLLECTION_COLUMNS) as f32 - 2.0;
    let row = (index / COLLECTION_COLUMNS) as f32;
    COLLECTION_CENTER + Vec2::new(column * COLLECTION_SPACING.x, (2.0 - row) * COLLECTION_SPACING.y)
}

fn redraw(
    mut commands: Commands,
    editor: Res<Editor>,
    presets: Res<Presets>,
    old: Query<Entity, With<EditorItem>>,
    assets: Res<AssetServer>,
) {
    for entity in &old {
        commands.entity(entity).despawn();
    }

    // Title and Back button.
    commands.spawn((
        item(),
        text("Deck Editor", 36.0, TEXT),
        Anchor::CENTER_LEFT,
        Transform::from_xyz(-VIEW_SIZE.x / 2.0 + 20.0, 400.0, 1.0),
    ));
    spawn_button(&mut commands, "Back", Vec2::new(560.0, 400.0), Vec2::new(140.0, 44.0), NEUTRAL)
        .insert((item(), EditorButton::Back));

    // Preset list on the left.
    commands.spawn((item(), text("Presets", 24.0, TEXT), Transform::from_xyz(PRESET_X, 340.0, 1.0)));
    for (index, deck) in presets.0.iter().enumerate() {
        let position = Vec2::new(PRESET_X, 290.0 - index as f32 * 46.0);
        let color = if editor.preset == Some(index) { MY_COLOR } else { PANEL_LIGHT };
        spawn_button(&mut commands, &deck.name, position, PRESET_SIZE, color)
            .insert((item(), EditorButton::Preset(index)));
    }
    let new_deck_y = 290.0 - presets.0.len() as f32 * 46.0;
    let new_deck_color = if editor.preset.is_none() { MY_COLOR } else { NEUTRAL };
    spawn_button(&mut commands, "+ New deck", Vec2::new(PRESET_X, new_deck_y), PRESET_SIZE, new_deck_color)
        .insert((item(), EditorButton::NewDeck));

    // The collection: every card, greyed out once it's in the deck.
    commands.spawn((
        item(),
        text("Collection: drag cards into your deck", 20.0, DIM_TEXT),
        Transform::from_xyz(COLLECTION_CENTER.x, 330.0, 1.0),
    ));
    for (index, id) in all_cards().into_iter().enumerate() {
        let position = collection_slot(index).extend(10.0);
        let in_deck = editor.deck.cards.contains(&id);
        let border = if in_deck { NEUTRAL } else { CARD_BORDER };
        let entity = spawn_card(&mut commands, &assets, id, card(id).power, border, position, COLLECTION_SCALE);
        commands.entity(entity).insert(item());
        if in_deck {
            dim_card(&mut commands, entity);
        } else {
            commands.entity(entity).insert((DragSource::Collection(id), DragArea(CARD_SIZE)));
        }
    }

    // Large preview of the hovered card.
    match editor.hovered {
        Some(id) => {
            let entity = spawn_card(
                &mut commands,
                &assets,
                id,
                card(id).power,
                CARD_BORDER,
                PREVIEW_POSITION.extend(20.0),
                PREVIEW_SCALE,
            );
            commands.entity(entity).insert(item());
        }
        None => {
            commands.spawn((
                item(),
                text("Hover a card\nto see it up close", 18.0, NEUTRAL),
                Transform::from_translation(PREVIEW_POSITION.extend(1.0)),
            ));
        }
    }

    // The deck panel: name, card count, and one row per card.
    commands.spawn((
        item(),
        Sprite::from_color(PANEL, DECK_PANEL_SIZE),
        Transform::from_translation(DECK_PANEL_CENTER.extend(0.0)),
    ));
    let (name_label, name_color) = if editor.editing_name {
        (format!("{}|", editor.deck.name), MY_COLOR)
    } else {
        (editor.deck.name.clone(), PANEL_LIGHT)
    };
    spawn_button(&mut commands, name_label, NAME_FIELD_POSITION, NAME_FIELD_SIZE, name_color)
        .insert((item(), EditorButton::NameField));

    let count = editor.deck.cards.len();
    let count_color = if editor.deck.is_valid() { Color::srgb(0.35, 0.85, 0.45) } else { DIM_TEXT };
    commands.spawn((
        item(),
        text(format!("{count} / {DECK_SIZE} cards"), 20.0, count_color),
        Transform::from_xyz(DECK_PANEL_CENTER.x, 245.0, 1.0),
    ));

    let mut rows = editor.deck.cards.clone();
    // Sort by cost, then name, like most deck lists.
    rows.sort_by_key(|&id| (card(id).cost, card(id).name));
    for (index, id) in rows.into_iter().enumerate() {
        let def = card(id);
        let position = Vec2::new(DECK_PANEL_CENTER.x, FIRST_ROW_Y - index as f32 * ROW_HEIGHT);
        commands
            .spawn((
                item(),
                Sprite::from_color(PANEL_LIGHT, ROW_SIZE),
                Transform::from_translation(position.extend(10.0)),
                DragSource::DeckRow(id),
                DragArea(ROW_SIZE),
            ))
            .with_children(|row| {
                row.spawn((Sprite::from_color(ENERGY_COLOR, Vec2::splat(22.0)), Transform::from_xyz(-138.0, 0.0, 0.1)));
                row.spawn((text(def.cost.to_string(), 16.0, TEXT), Transform::from_xyz(-138.0, 0.0, 0.2)));
                row.spawn((text(def.name, 16.0, TEXT), Anchor::CENTER_LEFT, Transform::from_xyz(-118.0, 0.0, 0.2)));
                row.spawn((
                    text(format!("{} power", def.power), 14.0, DIM_TEXT),
                    Anchor::CENTER_RIGHT,
                    Transform::from_xyz(140.0, 0.0, 0.2),
                ));
            });
    }
    commands.spawn((
        item(),
        text("Drag a card out of the list to remove it", 14.0, NEUTRAL),
        Transform::from_xyz(DECK_PANEL_CENTER.x, -240.0, 1.0),
    ));

    spawn_button(&mut commands, "Save", Vec2::new(375.0, -320.0), Vec2::new(140.0, 44.0), MY_COLOR)
        .insert((item(), EditorButton::Save));
    if editor.preset.is_some() {
        spawn_button(&mut commands, "Delete", Vec2::new(535.0, -320.0), Vec2::new(140.0, 44.0), THEIR_COLOR)
            .insert((item(), EditorButton::Delete));
    }
    commands.spawn((
        item(),
        text(&editor.message, 18.0, DIM_TEXT),
        Transform::from_xyz(DECK_PANEL_CENTER.x, -405.0, 1.0),
    ));
}

/// Mouse presses: buttons first, otherwise pick up a card or row.
fn click(
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Cursor,
    buttons: Query<(&EditorButton, &ButtonArea, &Transform)>,
    mut draggables: Query<(Entity, &DragSource, &DragArea, &mut Transform), Without<ButtonArea>>,
    mut editor: ResMut<Editor>,
    mut drag: ResMut<EditorDrag>,
    mut presets: ResMut<Presets>,
    mut next_state: ResMut<NextState<AppState>>,
    mut sounds: MessageWriter<PlaySound>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(cursor) = cursor.position() else { return };
    let button = button_under(cursor, &buttons);
    if button.is_some() {
        sounds.write(PlaySound(Sound::ButtonClick));
    }

    // Clicking anywhere except the name field stops editing it. (Only write
    // when it changes: any write to `editor` causes a redraw.)
    if editor.editing_name && !matches!(button, Some(EditorButton::NameField)) {
        editor.editing_name = false;
    }

    match button {
        Some(EditorButton::Preset(index)) => *editor = Editor::open(&presets.0, Some(index)),
        Some(EditorButton::NewDeck) => *editor = Editor::open(&presets.0, None),
        Some(EditorButton::NameField) => editor.editing_name = true,
        Some(EditorButton::Save) => save(&mut editor, &mut presets),
        Some(EditorButton::Delete) => {
            if let Some(index) = editor.preset {
                let removed = presets.0.remove(index);
                *editor = Editor::open(&presets.0, None);
                editor.message = match presets::save(&presets.0) {
                    Ok(()) => format!("Deleted \"{}\"", removed.name),
                    Err(error) => format!("Couldn't save: {error}"),
                };
            }
        }
        Some(EditorButton::Back) => next_state.set(AppState::MainMenu),
        None => {
            // Not a button: pick up the topmost card or row under the cursor.
            let topmost = draggables
                .iter_mut()
                .filter(|(_, _, area, t)| contains(t.translation.truncate(), area.0 * t.scale.truncate(), cursor))
                .max_by(|a, b| a.3.translation.z.total_cmp(&b.3.translation.z));
            if let Some((entity, &source, _, mut transform)) = topmost {
                let grab_offset = transform.translation.truncate() - cursor;
                drag.0 = Some(Held { entity, source, grab_offset });
                transform.translation.z = 100.0;
                transform.scale *= 1.1;
                sounds.write(PlaySound(Sound::CardPickup));
            }
        }
    }
}

fn save(editor: &mut Editor, presets: &mut Presets) {
    if editor.deck.name.trim().is_empty() {
        editor.message = "Give the deck a name first".to_string();
        return;
    }
    match editor.preset {
        Some(index) => presets.0[index] = editor.deck.clone(),
        None => {
            presets.0.push(editor.deck.clone());
            editor.preset = Some(presets.0.len() - 1);
        }
    }
    editor.message = match (presets::save(&presets.0), editor.deck.validate()) {
        (Err(error), _) => format!("Couldn't save: {error}"),
        (Ok(()), Ok(())) => "Saved".to_string(),
        (Ok(()), Err(problem)) => format!("Saved, but it can't be played yet: {problem}"),
    };
}

fn move_drag(cursor: Cursor, drag: Res<EditorDrag>, mut transforms: Query<&mut Transform, With<DragSource>>) {
    let Some(held) = &drag.0 else { return };
    let Some(cursor) = cursor.position() else { return };
    if let Ok(mut transform) = transforms.get_mut(held.entity) {
        let position = cursor + held.grab_offset;
        transform.translation.x = position.x;
        transform.translation.y = position.y;
    }
}

/// On release: a collection card dropped on the deck panel is added; a deck
/// row dropped outside the panel is removed.
fn drop(
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Cursor,
    mut drag: ResMut<EditorDrag>,
    mut editor: ResMut<Editor>,
    mut sounds: MessageWriter<PlaySound>,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(held) = drag.0.take() else { return };
    let on_deck = cursor.position().is_some_and(|point| contains(DECK_PANEL_CENTER, DECK_PANEL_SIZE, point));

    match held.source {
        DragSource::Collection(id) if on_deck => {
            if editor.deck.can_add(id) {
                editor.deck.cards.push(id);
                sounds.write(PlaySound(Sound::CardPlace));
            } else {
                editor.message = format!("The deck is full ({DECK_SIZE} cards)");
                sounds.write(PlaySound(Sound::Invalid));
            }
        }
        DragSource::DeckRow(id) if !on_deck => {
            editor.deck.cards.retain(|&c| c != id);
            sounds.write(PlaySound(Sound::CardReturn));
        }
        _ => {}
    }
    // Redraw even if nothing changed, so the dragged card snaps back.
    editor.set_changed();
}

/// Show the card under the mouse in the preview.
fn hover(
    cursor: Cursor,
    drag: Res<EditorDrag>,
    draggables: Query<(&DragSource, &DragArea, &Transform)>,
    mut editor: ResMut<Editor>,
) {
    if drag.0.is_some() {
        return;
    }
    let Some(cursor) = cursor.position() else { return };
    let hovered = draggables
        .iter()
        .find(|(_, area, t)| contains(t.translation.truncate(), area.0 * t.scale.truncate(), cursor))
        .map(|(source, _, _)| source.card());
    // Only write when it changes; writing every frame would redraw every frame.
    if editor.hovered != hovered {
        editor.hovered = hovered;
    }
}

/// Typing into the name field.
fn type_name(mut keys: MessageReader<KeyboardInput>, mut editor: ResMut<Editor>) {
    for key in keys.read() {
        if !editor.editing_name || key.state != ButtonState::Pressed {
            continue;
        }
        match key.key_code {
            KeyCode::Backspace => {
                editor.deck.name.pop();
            }
            KeyCode::Enter | KeyCode::Escape => editor.editing_name = false,
            _ => {
                if let Some(typed) = &key.text {
                    for character in typed.chars().filter(|c| !c.is_control()) {
                        if editor.deck.name.chars().count() < MAX_NAME_LENGTH {
                            editor.deck.name.push(character);
                        }
                    }
                }
            }
        }
    }
}
