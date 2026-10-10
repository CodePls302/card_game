//! The main menu, and the deck select screen shown before a game starts.

use bevy::prelude::*;
use rand::seq::IndexedRandom;

use super::AppState;
use super::board::Game;
use super::session::{ChosenDeck, ChosenMode, PlayMode, Seat};
use super::sound::{PlaySound, Sound};
use super::widgets::*;
use crate::game::{DECK_SIZE, Deck, PlayerId, default_decks};
use crate::presets::Presets;

/// What each menu button does. `ChooseDeck` holds the preset's position in
/// the `Presets` list.
#[derive(Component, Clone, Copy)]
enum MenuButton {
    /// Start a game in this mode (after choosing a deck).
    Play(PlayMode),
    EditDecks,
    Quit,
    ChooseDeck(usize),
    Back,
}

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::MainMenu), spawn_main_menu)
            .add_systems(OnEnter(AppState::DeckSelect), spawn_deck_select)
            // Buttons only exist on the menu screens, so this system has
            // nothing to do anywhere else.
            .add_systems(Update, click_menu_buttons);
    }
}

fn spawn_main_menu(mut commands: Commands) {
    let scope = DespawnOnExit(AppState::MainMenu);
    commands.spawn((text("CARD GAME", 72.0, TEXT), Transform::from_xyz(0.0, 180.0, 1.0), scope.clone()));

    let size = Vec2::new(280.0, 60.0);
    let buttons = [
        ("Play vs AI", MenuButton::Play(PlayMode::VsAi)),
        ("Host LAN Game", MenuButton::Play(PlayMode::Host)),
        ("Join LAN Game", MenuButton::Play(PlayMode::Join)),
        ("Deck Editor", MenuButton::EditDecks),
        ("Quit", MenuButton::Quit),
    ];
    for (index, (label, action)) in buttons.into_iter().enumerate() {
        let position = Vec2::new(0.0, 40.0 - index as f32 * 75.0);
        spawn_button(&mut commands, label, position, size, MY_COLOR).insert((action, scope.clone()));
    }
}

fn spawn_deck_select(mut commands: Commands, presets: Res<Presets>, mode: Res<ChosenMode>) {
    let scope = DespawnOnExit(AppState::DeckSelect);
    commands.spawn((text("Choose your deck", 44.0, TEXT), Transform::from_xyz(0.0, 330.0, 1.0), scope.clone()));
    let purpose = match mode.0 {
        PlayMode::VsAi => "to play against the AI",
        PlayMode::Host => "to host a LAN game",
        PlayMode::Join => "to join a LAN game",
    };
    commands.spawn((text(purpose, 20.0, DIM_TEXT), Transform::from_xyz(0.0, 290.0, 1.0), scope.clone()));

    let size = Vec2::new(420.0, 50.0);
    for (index, deck) in presets.0.iter().enumerate() {
        let position = Vec2::new(0.0, 240.0 - index as f32 * 62.0);
        if deck.is_valid() {
            spawn_button(&mut commands, &deck.name, position, size, MY_COLOR)
                .insert((MenuButton::ChooseDeck(index), scope.clone()));
        } else {
            // Shown but not clickable, so you can see why it's missing.
            let label = format!("{} ({} / {DECK_SIZE}, incomplete)", deck.name, deck.cards.len());
            spawn_button(&mut commands, label, position, size, NEUTRAL).insert(scope.clone());
        }
    }

    spawn_button(&mut commands, "Back", Vec2::new(0.0, -380.0), Vec2::new(200.0, 50.0), NEUTRAL)
        .insert((MenuButton::Back, scope));
}

fn click_menu_buttons(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Cursor,
    buttons: Query<(&MenuButton, &ButtonArea, &Transform)>,
    presets: Res<Presets>,
    mode: Res<ChosenMode>,
    mut next_state: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
    mut sounds: MessageWriter<PlaySound>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(cursor) = cursor.position() else { return };
    let Some(action) = button_under(cursor, &buttons) else { return };
    sounds.write(PlaySound(Sound::ButtonClick));

    match action {
        MenuButton::Play(mode) => {
            commands.insert_resource(ChosenMode(mode));
            next_state.set(AppState::DeckSelect);
        }
        MenuButton::EditDecks => next_state.set(AppState::DeckEditor),
        MenuButton::Quit => {
            exit.write(AppExit::Success);
        }
        MenuButton::Back => next_state.set(AppState::MainMenu),
        MenuButton::ChooseDeck(index) => {
            let my_deck = presets.0[index].cards.clone();
            match mode.0 {
                PlayMode::VsAi => {
                    let their_deck = random_opponent_deck(&presets.0);
                    commands.insert_resource(Game::new(my_deck, their_deck));
                    commands.insert_resource(Seat { me: PlayerId::One });
                    next_state.set(AppState::InGame);
                }
                // LAN games start from the lobby, once both players are in.
                PlayMode::Host | PlayMode::Join => {
                    commands.insert_resource(ChosenDeck(my_deck));
                    next_state.set(AppState::Lobby);
                }
            }
        }
    }
}

/// The AI plays a random valid preset (falling back to the defaults if none
/// of the saved ones are valid).
fn random_opponent_deck(presets: &[Deck]) -> Vec<crate::game::CardId> {
    let mut valid: Vec<&Deck> = presets.iter().filter(|deck| deck.is_valid()).collect();
    let defaults = default_decks();
    if valid.is_empty() {
        valid = defaults.iter().collect();
    }
    let deck = valid.choose(&mut rand::rng()).expect("the default decks are never empty");
    deck.cards.clone()
}
