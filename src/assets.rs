//! Where the game's files (card art, sounds) are found.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The `assets` folder: in the folder the game was started from (which is
/// the case with `cargo run`), or otherwise next to the game's program file.
/// Bevy is told to load from the same place (see `main.rs`), so checks like
/// `asset_exists` always agree with what Bevy can load.
pub fn assets_dir() -> &'static Path {
    // `OnceLock` works the answer out the first time, then remembers it.
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        if let Ok(here) = std::env::current_dir() {
            let dir = here.join("assets");
            if dir.is_dir() {
                return dir;
            }
        }
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|folder| folder.join("assets")))
            .unwrap_or_else(|| PathBuf::from("assets"))
    })
}

/// Does `path` (relative to the assets folder, e.g. "sounds/end_turn.ogg")
/// exist?
pub fn asset_exists(path: &str) -> bool {
    assets_dir().join(path).exists()
}
