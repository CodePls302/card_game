use serde::{Deserialize, Serialize};

/// Identifies a card design, e.g. "Stone Golem". It is an index into the
/// card catalogue (see `catalogue.rs`).
///
/// This is a "newtype": a struct wrapping a single number. It stops us from
/// accidentally passing some other `u16` (like a turn number) where a card is
/// expected; the compiler will reject it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardId(pub u16);

/// The four families of effect. Each one decides *when* an effect happens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectKind {
    /// Flips face-up the moment it is played and does something right away.
    Flip,
    /// Waits for a condition, then does something.
    Trigger,
    /// Always active while the card is face-up.
    Passive,
    /// Stays face-down in the secret zone until its condition is met.
    Secret,
}

impl EffectKind {
    pub fn name(self) -> &'static str {
        match self {
            EffectKind::Flip => "Flip",
            EffectKind::Trigger => "Trigger",
            EffectKind::Passive => "Passive",
            EffectKind::Secret => "Secret",
        }
    }
}

/// What a card does beyond adding its power to a zone.
///
/// New effects get added here as the rules grow. Because Rust `match`es must
/// handle every variant, adding one makes the compiler point out every place
/// that needs to learn about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    /// Flip: destroy every face-up card at this zone (both sides) with less
    /// power than this one.
    DestroyWeaker,
    /// Trigger: at the end of each turn, add a copy of this card to its zone.
    CopyEachTurn,
    /// Passive: your other cards here get +N power.
    EmpowerZone(i32),
    /// Passive: can move to another zone once per turn.
    Movable,
    /// Secret: when an enemy card with at least N power is revealed, flip and
    /// destroy that card.
    AmbushStrong(i32),
    /// Trigger: at the end of each turn, move to a random other zone.
    Wander,
    /// Secret: flips when your next turn starts, letting you see the
    /// opponent's hand for that turn.
    RevealHand,
}

impl Effect {
    /// Which family this effect belongs to, or `None` for plain cards.
    pub fn kind(&self) -> Option<EffectKind> {
        match self {
            Effect::None => None,
            Effect::DestroyWeaker => Some(EffectKind::Flip),
            Effect::CopyEachTurn | Effect::Wander => Some(EffectKind::Trigger),
            Effect::EmpowerZone(_) | Effect::Movable => Some(EffectKind::Passive),
            Effect::AmbushStrong(_) | Effect::RevealHand => Some(EffectKind::Secret),
        }
    }

    /// Rules text printed on the card.
    pub fn description(&self) -> String {
        let text = match self {
            Effect::None => return String::new(),
            Effect::DestroyWeaker => "Destroy all weaker cards here.".to_string(),
            Effect::CopyEachTurn => "Each turn, add a copy of this card here.".to_string(),
            Effect::EmpowerZone(n) => format!("Your other cards here get +{n} Power."),
            Effect::Movable => "Can move once per turn.".to_string(),
            Effect::AmbushStrong(n) => format!("When an enemy {n}+ Power card is revealed, destroy it."),
            Effect::Wander => "Each turn, move to a random other zone.".to_string(),
            Effect::RevealHand => "When your next turn starts, see the enemy's hand.".to_string(),
        };
        // `kind()` is always `Some` here because `Effect::None` returned above.
        let kind = self.kind().map(EffectKind::name).unwrap_or_default();
        format!("{kind}: {text}")
    }
}

/// The fixed, printed information about a card design.
#[derive(Debug)]
pub struct CardDef {
    pub id: CardId,
    pub name: &'static str,
    /// Energy needed to play the card (1-9).
    pub cost: u32,
    pub power: i32,
    pub effect: Effect,
}

impl CardDef {
    /// Where this card's art lives inside the `assets/` folder, e.g.
    /// "Stone Golem" -> "cards/stone_golem.png".
    pub fn art_path(&self) -> String {
        format!("cards/{}.png", self.name.to_lowercase().replace(' ', "_"))
    }

    pub fn is_secret(&self) -> bool {
        self.effect.kind() == Some(EffectKind::Secret)
    }
}
