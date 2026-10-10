//! Deck-building rules.

use std::fmt;

use super::card::CardId;
use super::catalogue::card;

pub const DECK_SIZE: usize = 15;

/// A named list of cards, e.g. a saved preset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deck {
    pub name: String,
    pub cards: Vec<CardId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeckError {
    /// Holds how many cards the deck actually has.
    WrongSize(usize),
    Duplicate(CardId),
}

impl fmt::Display for DeckError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DeckError::WrongSize(count) => write!(f, "needs exactly {DECK_SIZE} cards (has {count})"),
            DeckError::Duplicate(id) => write!(f, "has more than one {}", card(*id).name),
        }
    }
}

impl Deck {
    /// Is this deck legal to play with?
    pub fn validate(&self) -> Result<(), DeckError> {
        for (index, id) in self.cards.iter().enumerate() {
            // Only look at cards *after* this one, so each pair is checked once.
            if self.cards[index + 1..].contains(id) {
                return Err(DeckError::Duplicate(*id));
            }
        }
        if self.cards.len() != DECK_SIZE {
            return Err(DeckError::WrongSize(self.cards.len()));
        }
        Ok(())
    }

    pub fn is_valid(&self) -> bool {
        self.validate().is_ok()
    }

    /// Could `id` be added without breaking the rules?
    pub fn can_add(&self, id: CardId) -> bool {
        self.cards.len() < DECK_SIZE && !self.cards.contains(&id)
    }
}

/// The presets everyone starts with.
pub fn default_decks() -> Vec<Deck> {
    vec![
        Deck { name: "Starter A".to_string(), cards: (0..15).map(CardId).collect() },
        Deck { name: "Starter B".to_string(), cards: (10..25).map(CardId).collect() },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_decks_are_valid() {
        for deck in default_decks() {
            assert_eq!(deck.validate(), Ok(()), "{}", deck.name);
        }
    }

    #[test]
    fn decks_need_exactly_fifteen_unique_cards() {
        let mut deck = Deck { name: "Test".to_string(), cards: (0..14).map(CardId).collect() };
        assert_eq!(deck.validate(), Err(DeckError::WrongSize(14)));
        assert!(deck.can_add(CardId(20)));
        assert!(!deck.can_add(CardId(3)), "already in the deck");

        deck.cards.push(CardId(3));
        assert_eq!(deck.validate(), Err(DeckError::Duplicate(CardId(3))));

        deck.cards.pop();
        deck.cards.push(CardId(20));
        assert_eq!(deck.validate(), Ok(()));
        assert!(!deck.can_add(CardId(21)), "deck is full");
    }
}
