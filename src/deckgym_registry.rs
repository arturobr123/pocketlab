//! Builds PocketLab's card registry from the exact DeckGym revision used by the simulator.
//! Keeping discovery and simulation on the same revision prevents us from generating
//! candidates containing cards the engine cannot actually execute.

use crate::domain::{Card, CardId, CardRegistry, CardType};
use deckgym::{
    card_ids::CardId as DeckGymCardId,
    card_validation::get_implementation_status,
    database::get_card_by_enum,
    models::Card as DeckGymCard,
};
use strum::IntoEnumIterator;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeckGymRegistryStats {
    pub total_cards: usize,
    pub supported_cards: usize,
    pub unsupported_cards: usize,
}

pub fn build_deckgym_registry() -> CardRegistry {
    let mut registry = CardRegistry::default();

    for deckgym_id in DeckGymCardId::iter() {
        let deckgym_card = get_card_by_enum(deckgym_id);
        let card_type = match &deckgym_card {
            DeckGymCard::Pokemon(pokemon) => CardType::Pokemon {
                basic: pokemon.stage == 0,
            },
            DeckGymCard::Trainer(_) => CardType::Trainer,
        };
        let mechanics_implemented = get_implementation_status(deckgym_id).is_complete();

        registry.insert(Card {
            id: CardId(normalize_deckgym_card_id(&deckgym_card.get_id())),
            name: deckgym_card.get_name(),
            card_type,
            legal: true,
            mechanics_implemented,
        });
    }

    registry
}

pub fn registry_stats(registry: &CardRegistry) -> DeckGymRegistryStats {
    let total_cards = registry.len();
    let supported_cards = registry
        .iter()
        .filter(|card| card.legal && card.mechanics_implemented)
        .count();

    DeckGymRegistryStats {
        total_cards,
        supported_cards,
        unsupported_cards: total_cards - supported_cards,
    }
}

fn normalize_deckgym_card_id(raw: &str) -> String {
    let mut parts = raw.split_whitespace();
    match (parts.next(), parts.next(), parts.next()) {
        (Some(set), Some(number), None) => format!("{set}-{number}"),
        _ => raw.trim().replace(' ', "-"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deckgym_registry_contains_real_cards_and_status() {
        let registry = build_deckgym_registry();
        let bulbasaur = registry
            .get(&CardId("A1-001".into()))
            .expect("DeckGym should contain Genetic Apex Bulbasaur");

        assert_eq!(bulbasaur.name, "Bulbasaur");
        assert!(matches!(
            bulbasaur.card_type,
            CardType::Pokemon { basic: true }
        ));

        let stats = registry_stats(&registry);
        assert!(stats.total_cards > 1_000);
        assert!(stats.supported_cards > 0);
        assert_eq!(
            stats.total_cards,
            stats.supported_cards + stats.unsupported_cards
        );
    }

    #[test]
    fn normalizes_promo_and_regular_ids() {
        assert_eq!(normalize_deckgym_card_id("A3b 041"), "A3b-041");
        assert_eq!(normalize_deckgym_card_id("P-A 005"), "P-A-005");
    }
}
