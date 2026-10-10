mod assets;
mod game;
mod net;
mod presets;
mod ui;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Card Game".into(),
                        resolution: (1300, 900).into(),
                        ..default()
                    }),
                    ..default()
                })
                // Load art and sounds from the same folder our own checks use.
                .set(AssetPlugin { file_path: assets::assets_dir().to_string_lossy().into_owned(), ..default() }),
        )
        .add_plugins(ui::GameUiPlugin)
        .run();
}
