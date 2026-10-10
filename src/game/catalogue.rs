//! Every card in the game. The names, powers and effects here are
//! placeholders until the real card list is designed.

use super::card::{CardDef, CardId, Effect};

/// Shorthand so each card below fits on one line. A `const fn` can run at
/// compile time, which lets us build the `CARDS` table as a constant.
const fn def(id: u16, name: &'static str, cost: u32, power: i32, effect: Effect) -> CardDef {
    CardDef { id: CardId(id), name, cost, power, effect }
}

/// The card table. A card's `CardId` must equal its position in this list.
/// Columns: id, name, cost, power, effect.
static CARDS: [CardDef; 25] = [
    def(0, "Ember Sprite", 1, 1, Effect::None),
    def(1, "Stone Golem", 4, 5, Effect::None),
    def(2, "Tide Caller", 2, 3, Effect::None),
    def(3, "Night Owl", 2, 2, Effect::RevealHand),
    def(4, "Iron Knight", 3, 4, Effect::None),
    def(5, "Storm Hawk", 2, 3, Effect::None),
    def(6, "Moss Troll", 7, 6, Effect::DestroyWeaker),
    def(7, "Glass Mage", 1, 2, Effect::None),
    def(8, "Dune Runner", 2, 3, Effect::Wander),
    def(9, "Frost Wolf", 3, 4, Effect::None),
    def(10, "Sun Priest", 2, 1, Effect::EmpowerZone(1)),
    def(11, "Shadow Fox", 2, 3, Effect::None),
    def(12, "Coral Giant", 5, 6, Effect::None),
    def(13, "Thorn Archer", 3, 3, Effect::None),
    def(14, "Ash Drake", 4, 5, Effect::None),
    def(15, "Clockwork Ant", 3, 1, Effect::CopyEachTurn),
    def(16, "Lantern Ghost", 3, 2, Effect::AmbushStrong(5)),
    def(17, "River Otter", 1, 2, Effect::None),
    def(18, "Granite Bear", 8, 10, Effect::None),
    def(19, "Spark Imp", 1, 1, Effect::None),
    def(20, "Cloud Whale", 6, 6, Effect::Movable),
    def(21, "Rune Smith", 3, 2, Effect::EmpowerZone(1)),
    def(22, "Velvet Moth", 1, 1, Effect::None),
    def(23, "Hollow King", 5, 0, Effect::EmpowerZone(2)),
    def(24, "Star Seer", 9, 12, Effect::None),
];

/// Look up a card's printed details.
pub fn card(id: CardId) -> &'static CardDef {
    &CARDS[id.0 as usize]
}

/// Every card in the game, which is what decks are built from.
pub fn all_cards() -> Vec<CardId> {
    CARDS.iter().map(|c| c.id).collect()
}

/// Find a card by its printed name, e.g. when loading a saved deck.
pub fn card_by_name(name: &str) -> Option<CardId> {
    CARDS.iter().find(|c| c.name == name).map(|c| c.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_match_positions() {
        for (index, card) in CARDS.iter().enumerate() {
            assert_eq!(card.id.0 as usize, index, "{} has the wrong id", card.name);
            assert!((1..=9).contains(&card.cost), "{} costs {}", card.name, card.cost);
        }
    }
}
