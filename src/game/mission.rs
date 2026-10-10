use serde::{Deserialize, Serialize};

/// A goal attached to a zone. Each player can complete it separately; once
/// completed it stays completed for the rest of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mission {
    /// Have the maximum number of cards at this zone.
    FillZone,
    /// Destroy one of the opponent's cards at this zone.
    DestroyEnemyHere,
    /// Have at least this much power at this zone.
    ReachPower(i32),
}

/// The missions zones are given at random at the start of a game.
pub const ALL_MISSIONS: [Mission; 3] = [Mission::FillZone, Mission::DestroyEnemyHere, Mission::ReachPower(20)];

impl Mission {
    pub fn description(self) -> String {
        match self {
            Mission::FillZone => "Fill this location".to_string(),
            Mission::DestroyEnemyHere => "Destroy an enemy card here".to_string(),
            Mission::ReachPower(target) => format!("Reach {target} power here"),
        }
    }
}
