//! Everything the player sees and touches, built with Bevy.
//!
//! The app moves between screens (an `AppState`): the main menu, deck
//! select, the deck editor, the LAN lobby, and the game itself. Each screen is its own
//! plugin that only runs while its state is active. Entities tagged
//! `DespawnOnExit(state)` are cleaned up automatically when leaving a screen.

mod board;
mod deck_editor;
mod lobby;
mod menu;
mod session;
mod sound;
mod widgets;

use bevy::camera::ScalingMode;
use bevy::prelude::*;
use bevy::transform::TransformSystems;

use crate::presets::{self, Presets};

#[derive(States, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AppState {
    #[default]
    MainMenu,
    DeckSelect,
    DeckEditor,
    /// Waiting for, or looking for, a LAN game.
    Lobby,
    InGame,
}

pub struct GameUiPlugin;

impl Plugin for GameUiPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(widgets::BACKGROUND))
            .insert_resource(Presets(presets::load()))
            .init_state::<AppState>()
            .init_resource::<session::ChosenMode>()
            .add_message::<sound::PlaySound>()
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, sound::play_sounds)
            // After every screen has spawned its text for this frame, but
            // before positions are worked out for drawing.
            .add_systems(PostUpdate, widgets::sharpen_text.before(TransformSystems::Propagate))
            .add_plugins((menu::MenuPlugin, deck_editor::DeckEditorPlugin, lobby::LobbyPlugin, board::BoardPlugin));
    }
}

fn spawn_camera(mut commands: Commands) {
    // Show the whole view at any window size, keeping its proportions.
    let projection = OrthographicProjection {
        scaling_mode: ScalingMode::AutoMin { min_width: widgets::VIEW_SIZE.x, min_height: widgets::VIEW_SIZE.y },
        ..OrthographicProjection::default_2d()
    };
    commands.spawn((Camera2d, Projection::Orthographic(projection)));
}
