use std::collections::{BTreeMap, BTreeSet};

pub const DECK_SIZE: u8 = 20;
pub const MAX_COPIES_BY_NAME: u8 = 2;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CardId(pub String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CardType {
    Pokemon { basic: bool },
    Trainer,
}

#[derive(Clone, Debug)]
pub struct Card {
    pub id: CardId,
    pub name: String,
    pub card_type: CardType,
    pub legal: bool,
    pub mechanics_implemented: bool,
}

#[derive(Clone, Debug, Default)]
pub struct CardRegistry {
    cards: BTreeMap<CardId, Card>,
}

impl CardRegistry {
    pub fn insert(&mut self, card: Card) {
        self.cards.insert(card.id.clone(), card);
    }

    pub fn get(&self, id: &CardId) -> Option<&Card> {
        self.cards.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Card> {
        self.cards.values()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Deck {
    pub cards: BTreeMap<CardId, u8>,
    pub energy_types: BTreeSet<String>,
}

impl Deck {
    pub fn new(cards: BTreeMap<CardId, u8>, energy_types: BTreeSet<String>) -> Self {
        Self { cards, energy_types }
    }

    pub fn total_cards(&self) -> u8 {
        self.cards.values().copied().sum()
    }

    pub fn canonical_key(&self) -> String {
        let cards = self
            .cards
            .iter()
            .map(|(id, count)| format!("{}:{}", id.0, count))
            .collect::<Vec<_>>()
            .join("|");
        let energy = self
            .energy_types
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        format!("cards={cards};energy={energy}")
    }

    pub fn validate(&self, registry: &CardRegistry) -> Result<(), DeckValidationError> {
        if self.total_cards() != DECK_SIZE {
            return Err(DeckValidationError::WrongSize(self.total_cards()));
        }

        let mut name_counts: BTreeMap<&str, u8> = BTreeMap::new();
        let mut has_basic = false;

        for (id, count) in &self.cards {
            if *count == 0 {
                return Err(DeckValidationError::ZeroCount(id.clone()));
            }
            let card = registry
                .get(id)
                .ok_or_else(|| DeckValidationError::UnknownCard(id.clone()))?;
            if !card.legal {
                return Err(DeckValidationError::IllegalCard(id.clone()));
            }
            if !card.mechanics_implemented {
                return Err(DeckValidationError::UnsupportedCard(id.clone()));
            }
            *name_counts.entry(card.name.as_str()).or_default() += *count;
            if matches!(card.card_type, CardType::Pokemon { basic: true }) {
                has_basic = true;
            }
        }

        if let Some((name, count)) = name_counts
            .iter()
            .find(|(_, count)| **count > MAX_COPIES_BY_NAME)
        {
            return Err(DeckValidationError::TooManyCopies {
                name: (*name).to_string(),
                count: *count,
            });
        }
        if !has_basic {
            return Err(DeckValidationError::NoBasicPokemon);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeckValidationError {
    WrongSize(u8),
    UnknownCard(CardId),
    IllegalCard(CardId),
    UnsupportedCard(CardId),
    ZeroCount(CardId),
    TooManyCopies { name: String, count: u8 },
    NoBasicPokemon,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> CardRegistry {
        let mut r = CardRegistry::default();
        r.insert(Card {
            id: CardId("A-001".into()),
            name: "Alpha".into(),
            card_type: CardType::Pokemon { basic: true },
            legal: true,
            mechanics_implemented: true,
        });
        r.insert(Card {
            id: CardId("A-002".into()),
            name: "Beta".into(),
            card_type: CardType::Trainer,
            legal: true,
            mechanics_implemented: true,
        });
        r
    }

    #[test]
    fn canonical_key_is_order_independent() {
        let mut a = BTreeMap::new();
        a.insert(CardId("A-002".into()), 18);
        a.insert(CardId("A-001".into()), 2);
        let deck = Deck::new(a, BTreeSet::from(["fire".into()]));
        assert_eq!(deck.canonical_key(), "cards=A-001:2|A-002:18;energy=fire");
    }

    #[test]
    fn validates_name_copy_limit_across_printings() {
        let mut r = registry();
        r.insert(Card {
            id: CardId("B-001".into()),
            name: "Alpha".into(),
            card_type: CardType::Pokemon { basic: true },
            legal: true,
            mechanics_implemented: true,
        });
        let deck = Deck::new(
            BTreeMap::from([
                (CardId("A-001".into()), 2),
                (CardId("B-001".into()), 1),
                (CardId("A-002".into()), 17),
            ]),
            BTreeSet::new(),
        );
        assert!(matches!(
            deck.validate(&r),
            Err(DeckValidationError::TooManyCopies { .. })
        ));
    }
}
