//! Saving and loading deck presets to a file on disk.

use std::fs;
use std::path::Path;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::game::{Deck, card, card_by_name, default_decks};

const SAVE_FILE: &str = "saves/decks.json";

/// Every saved deck, shared by the deck editor and the deck select screen.
#[derive(Resource)]
pub struct Presets(pub Vec<Deck>);

/// How a deck looks on disk. Cards are stored by name rather than by id, so
/// saved decks still load if the catalogue gets reordered.
///
/// `derive(Serialize, Deserialize)` asks the serde library to write the code
/// that converts this struct to and from JSON.
#[derive(Serialize, Deserialize)]
struct SavedDeck {
    name: String,
    cards: Vec<String>,
}

/// Read the saved presets, or fall back to the default decks if there is no
/// save file yet (or it can't be read).
pub fn load() -> Vec<Deck> {
    let Ok(text) = fs::read_to_string(SAVE_FILE) else { return default_decks() };
    let saved: Vec<SavedDeck> = match serde_json::from_str(&text) {
        Ok(saved) => saved,
        Err(error) => {
            warn!("Couldn't read {SAVE_FILE}: {error}. Using the default decks.");
            return default_decks();
        }
    };

    saved
        .into_iter()
        .map(|deck| {
            let mut cards = Vec::new();
            for name in &deck.cards {
                match card_by_name(name) {
                    Some(id) => cards.push(id),
                    None => warn!("Deck \"{}\" has an unknown card \"{name}\"; skipping it.", deck.name),
                }
            }
            Deck { name: deck.name, cards }
        })
        .collect()
}

pub fn save(decks: &[Deck]) -> Result<(), String> {
    let saved: Vec<SavedDeck> = decks
        .iter()
        .map(|deck| SavedDeck {
            name: deck.name.clone(),
            cards: deck.cards.iter().map(|&id| card(id).name.to_string()).collect(),
        })
        .collect();

    // `map_err` turns each kind of error into a String, so `?` can return it.
    let text = serde_json::to_string_pretty(&saved).map_err(|e| e.to_string())?;
    if let Some(folder) = Path::new(SAVE_FILE).parent() {
        fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    }
    fs::write(SAVE_FILE, text).map_err(|e| e.to_string())
}
