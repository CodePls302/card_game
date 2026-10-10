//! Sound effects.
//!
//! Every sound the game makes is listed in `Sound`, and `Sound::file` says
//! which file (inside `assets/`) it plays. To change a sound, either replace
//! that file with your own (same name), or edit the file name below. A
//! missing file is simply skipped, so you can also delete a sound to mute it.
//!
//! To play a sound from any system, add a `MessageWriter<PlaySound>`
//! parameter and call `sounds.write(PlaySound(Sound::CardPlace))`.

use bevy::audio::Volume;
use bevy::prelude::*;

use crate::assets::asset_exists;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    /// Any menu or board button.
    ButtonClick,
    /// Picking a card up to drag it.
    CardPickup,
    /// Playing a card, or adding it to a deck.
    CardPlace,
    /// Taking a card back to the hand, or out of a deck.
    CardReturn,
    /// A card moving to another zone.
    CardMove,
    /// A card turning face-up at the end of the turn.
    CardReveal,
    CardDestroyed,
    /// Clockwork Ant copying itself.
    CardCopied,
    SecretFlipped,
    MissionComplete,
    EndTurn,
    /// A new turn starting, after the replay.
    TurnStart,
    /// Something that isn't allowed (e.g. not enough energy).
    Invalid,
    /// Each of the last seconds of the turn timer.
    TimerTick,
    Victory,
    Defeat,
    /// Opening the graveyard.
    Graveyard,
}

impl Sound {
    /// The file this sound plays, relative to the `assets/` folder.
    pub fn file(self) -> &'static str {
        match self {
            Sound::ButtonClick => "sounds/button_click.ogg",
            Sound::CardPickup => "sounds/card_pickup.ogg",
            Sound::CardPlace => "sounds/card_place.ogg",
            Sound::CardReturn => "sounds/card_return.ogg",
            Sound::CardMove => "sounds/card_move.ogg",
            Sound::CardReveal => "sounds/card_reveal.ogg",
            Sound::CardDestroyed => "sounds/card_destroyed.ogg",
            Sound::CardCopied => "sounds/card_copied.ogg",
            Sound::SecretFlipped => "sounds/secret_flipped.ogg",
            Sound::MissionComplete => "sounds/mission_complete.ogg",
            Sound::EndTurn => "sounds/end_turn.ogg",
            Sound::TurnStart => "sounds/turn_start.ogg",
            Sound::Invalid => "sounds/invalid.ogg",
            Sound::TimerTick => "sounds/timer_tick.ogg",
            Sound::Victory => "sounds/victory.ogg",
            Sound::Defeat => "sounds/defeat.ogg",
            Sound::Graveyard => "sounds/graveyard.ogg",
        }
    }

    /// How loud this sound plays: 1.0 is the file's own volume, 0.5 half.
    pub fn volume(self) -> f32 {
        match self {
            Sound::TimerTick | Sound::ButtonClick => 0.5,
            _ => 0.8,
        }
    }
}

/// A request to play a sound. Any system can send one; `play_sounds` plays
/// them. (A "message" in Bevy is a value one system sends and others read.)
#[derive(Message, Clone, Copy)]
pub struct PlaySound(pub Sound);

pub fn play_sounds(mut commands: Commands, mut requests: MessageReader<PlaySound>, assets: Res<AssetServer>) {
    for PlaySound(sound) in requests.read() {
        let path = sound.file();
        if !asset_exists(path) {
            continue;
        }
        // An entity with an `AudioPlayer` plays once; `DESPAWN` removes the
        // entity again when the sound has finished.
        commands.spawn((
            AudioPlayer::new(assets.load(path)),
            PlaybackSettings { volume: Volume::Linear(sound.volume()), ..PlaybackSettings::DESPAWN },
        ));
    }
}
